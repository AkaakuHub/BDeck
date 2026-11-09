use super::*;
use std::collections::HashSet;
use zephium_core::deck::portable::SettingsArchive;
use zephium_core::ports::store::DeckImportOutcome;
use zephium_core::session::{PersistedProfile, PersistedSpace};

pub(in crate::shell) struct PendingImport {
    operation_id: String,
    state: zephium_core::deck::DeckState,
    preferences: zephium_core::deck::portable::Preferences,
    profiles: Vec<PersistedProfile>,
    spaces: Vec<PersistedSpace>,
}

impl Shell {
    pub(in crate::shell) fn begin_deck_import(
        &mut self,
        archive: &SettingsArchive,
        operation_id: String,
    ) -> Option<OperationDisposition> {
        let rejected = || {
            Some(operation_result(
                OperationOutcome::Rejected,
                OperationReason::InvalidInput,
            ))
        };
        if !self.bootstrapped
            || !self.settings_visible()
            || !self.profile_deletion.states.is_empty()
        {
            return rejected();
        }
        let (Some(current), Some(queue)) = (self.deck.state(), self.self_queue.as_ref()) else {
            return rejected();
        };
        let callback = CallbackHandle {
            queue: Arc::downgrade(&queue.inner),
        };
        let Ok(plan) = archive.plan(
            current,
            self.profiles.iter().map(|profile| profile.id),
            ProfileId::generate,
            zephium_core::ids::DeckAccountId::generate,
            || zephium_core::ids::DeckColumnId::generate(),
        ) else {
            return rejected();
        };
        if plan
            .state
            .decks
            .iter()
            .flat_map(|deck| &deck.columns)
            .any(|column| {
                ItemId::parse(&column.id.to_string()).is_none_or(|id| self.items.get(id).is_some())
            })
        {
            return rejected();
        }
        let mut session = self.session_snapshot();
        let retired: HashSet<_> = current
            .decks
            .iter()
            .flat_map(|deck| &deck.columns)
            .filter_map(|column| ItemId::parse(&column.id.to_string()))
            .collect();
        session.items.retain(|item| !retired.contains(&item.id));
        session.active_item = session.active_item.filter(|item| !retired.contains(item));
        for item in retired {
            session.splits = session.splits.take().and_then(|pane| pane.remove(item));
        }
        let mut spaces: HashSet<_> = session.spaces.iter().map(|space| space.id).collect();
        let mut added_profiles = Vec::new();
        let mut added_spaces = Vec::new();
        for account in &plan.new_accounts {
            let Some(space) = (0..8)
                .map(|_| SpaceId::generate())
                .find(|id| spaces.insert(*id))
            else {
                return rejected();
            };
            added_profiles.push(PersistedProfile {
                id: account.profile,
                name: "未ログイン".into(),
                kind: ProfileKind::Named,
            });
            added_spaces.push(PersistedSpace {
                id: space,
                profile: account.profile,
                name: "main".into(),
                look: Default::default(),
            });
        }
        session.profiles.extend(added_profiles.clone());
        session.spaces.extend(added_spaces.clone());
        if session::canonicalize(session.clone()) != session {
            return rejected();
        }
        self.deck.pending_import = Some(PendingImport {
            operation_id,
            state: plan.state.clone(),
            preferences: archive.preferences().clone(),
            profiles: added_profiles,
            spaces: added_spaces,
        });
        self.store.import_deck(
            session,
            plan.state,
            archive.preferences().clone(),
            Box::new(move |outcome| {
                let _ = callback.dispatch(Command::DeckImported(outcome));
            }),
        );
        None
    }

    pub(in crate::shell) fn finish_deck_import(&mut self, outcome: DeckImportOutcome) {
        let Some(pending) = self.deck.pending_import.take() else {
            return;
        };
        let operation_id = pending.operation_id.clone();
        let mut completion = match outcome {
            DeckImportOutcome::Rejected => operation_result(
                OperationOutcome::Rejected,
                OperationReason::StoreAdmissionRejected,
            ),
            DeckImportOutcome::Unavailable => {
                self.deck.pending_import = Some(pending);
                operation_result(
                    OperationOutcome::Rejected,
                    OperationReason::StoreOutcomeUnknown,
                )
            }
            DeckImportOutcome::Applied => {
                let profiles_inserted = pending.profiles.iter().all(|profile| {
                    self.profiles.insert(Profile {
                        id: profile.id,
                        name: profile.name.clone(),
                        kind: profile.kind,
                    })
                });
                let spaces_inserted = pending.spaces.iter().all(|space| {
                    self.spaces.insert(Space {
                        id: space.id,
                        profile: space.profile,
                        name: space.name.clone(),
                        look: space.look.clone(),
                    })
                });
                if !profiles_inserted || !spaces_inserted {
                    self.deck.pending_import = Some(pending);
                    operation_result(
                        OperationOutcome::Rejected,
                        OperationReason::StoreOutcomeUnknown,
                    )
                } else {
                    for profile in &pending.profiles {
                        self.apply_shared_web_extensions_for(profile.id, None);
                    }
                    self.engine.set_deck_appearance(
                        pending.preferences.appearance,
                        pending.preferences.hide_advertisements,
                    );
                    for (key, value) in pending.preferences.values() {
                        let command = if key == "appearance" {
                            format!("theme.{value}")
                        } else {
                            format!("preference.{key}={value}")
                        };
                        (self.emit)(Projection::UiCommand(command));
                    }
                    let result = self.apply_deck_state(pending.state).map_or_else(
                        || {
                            operation_result(
                                OperationOutcome::Rejected,
                                OperationReason::InvalidScope,
                            )
                        },
                        mutation_result,
                    );
                    self.project_profiles();
                    self.project_deck_state();
                    self.project_account_sessions();
                    result
                }
            }
        };
        if self.deck.pending_import.is_none() {
            self.schedule_current_session_persist();
        }
        completion.operation_id = operation_id;
        (self.emit)(Projection::OperationProcessed(completion));
    }
}
