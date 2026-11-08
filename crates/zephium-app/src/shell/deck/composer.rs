use super::*;
use zephium_core::ids::DeckAccountId;

const COMPOSE_URL: &str = "https://x.com/compose/post";

struct ComposerTab {
    account: DeckAccountId,
    profile: ProfileId,
    item: ItemId,
}

#[derive(Default)]
pub(in crate::shell) struct ComposerRuntime {
    tabs: Vec<ComposerTab>,
    active: Option<DeckAccountId>,
    last_account: Option<DeckAccountId>,
    pub(in crate::shell) return_to: Option<ItemId>,
}

impl ComposerRuntime {
    pub(in crate::shell) fn retire_profile(&mut self, profile: ProfileId) {
        let accounts = self
            .tabs
            .iter()
            .filter(|tab| tab.profile == profile)
            .map(|tab| tab.account)
            .collect::<Vec<_>>();
        self.tabs.retain(|tab| tab.profile != profile);
        if self
            .active
            .is_some_and(|account| accounts.contains(&account))
        {
            self.active = None;
            self.return_to = None;
        }
        if self
            .last_account
            .is_some_and(|account| accounts.contains(&account))
        {
            self.last_account = None;
        }
    }

    pub(in crate::shell) fn retained_items(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.tabs.iter().map(|tab| tab.item)
    }

    pub(in crate::shell) fn profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.tabs
            .iter()
            .find(|tab| tab.item == item)
            .map(|tab| tab.profile)
    }
    pub(in crate::shell) fn active_item(&self) -> Option<ItemId> {
        self.tabs
            .iter()
            .find(|tab| Some(tab.account) == self.active)
            .map(|tab| tab.item)
    }
}

impl Shell {
    pub(in crate::shell) fn composer_frame(&self) -> Option<zephium_ipc::ComposerFrame> {
        let tab = self.composer.active_item()?;
        let account = self.composer.active?;
        let window = self.windows.focused()?;
        let region = self.content_region();
        let width = 360.0_f64.min(region.width);
        Some(zephium_ipc::ComposerFrame {
            tab: tab.to_string(),
            account: account.to_string(),
            managing: false,
            x: region.x,
            y: region.y,
            width,
            height: (window.size.height - region.y).max(0.0),
        })
    }

    pub(in crate::shell) fn operation_open_composer(
        &mut self,
        account: Option<DeckAccountId>,
    ) -> OperationDisposition {
        let Some(account) = self
            .deck
            .state()
            .and_then(|state| match account {
                Some(id) => state.account(id),
                None => self
                    .composer
                    .last_account
                    .and_then(|id| state.account(id))
                    .or_else(|| state.accounts.first()),
            })
            .cloned()
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        let Some(space) = self.windows.focused().map(|window| window.space) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        if self.profile_deletion_quarantines(account.profile)
            || self.profiles.get(account.profile).is_none()
            || self.accounts.account_changed(account.profile)
        {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if self.composer.active.is_none() {
            self.composer.return_to = self.windows.focused().and_then(|window| window.active);
        }
        let item = if let Some(tab) = self
            .composer
            .tabs
            .iter()
            .find(|tab| tab.account == account.id)
        {
            tab.item
        } else {
            let item = ItemId::generate();
            if !self.items.insert_unloaded_tab(
                item,
                zephium_core::item::Placement::Space {
                    space,
                    section: zephium_core::item::SpaceSection::Today,
                },
                url::Url::parse(COMPOSE_URL).expect("static compose URL"),
                "新しいポスト",
            ) {
                return operation_result(
                    OperationOutcome::Rejected,
                    OperationReason::ItemLimitReached,
                );
            }
            self.composer.tabs.push(ComposerTab {
                account: account.id,
                profile: account.profile,
                item,
            });
            item
        };
        let needs_navigation = self
            .items
            .tab(item)
            .and_then(|tab| tab.url.as_ref())
            .is_some_and(|url| {
                url.host_str() == Some("x.com") && !url.path().starts_with("/compose/")
            });
        self.composer.active = Some(account.id);
        self.composer.last_account = Some(account.id);
        let effects = self.focus_tab(item);
        let native = self.commit(effects);
        if !native.rejected && needs_navigation {
            return self.operation_navigate(item, COMPOSE_URL.into());
        }
        if !native.rejected {
            let _ = self.engine.focus_composer(item);
        }
        mutation_result(native)
    }

    pub(in crate::shell) fn operation_close_composer(&mut self) -> OperationDisposition {
        if self.accounts.active_item().is_some() {
            return self.operation_close_account();
        }
        if self.composer.active.take().is_none() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let target = self
            .composer
            .return_to
            .take()
            .filter(|id| self.item_in_focused_scope(*id))
            .or_else(|| self.deck.active_item());
        let effects = target.map_or_else(Vec::new, |id| self.focus_tab(id));
        if target.is_none() {
            if let Some(window) = self.windows.focused_mut() {
                window.active = None;
            }
        }
        let native = self.commit(effects);
        if let Some(item) = target {
            let _ = self.engine.focus(item);
        }
        mutation_result(native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached() -> ComposerRuntime {
        ComposerRuntime {
            tabs: (1..=2)
                .map(|id| ComposerTab {
                    account: DeckAccountId::from(id),
                    profile: ProfileId::from(id),
                    item: ItemId::from(id),
                })
                .collect(),
            active: Some(DeckAccountId::from(2)),
            last_account: Some(DeckAccountId::from(2)),
            return_to: Some(ItemId::from(10)),
        }
    }

    #[test]
    fn deleting_an_inactive_account_keeps_the_other_composer_and_its_focus() {
        let mut runtime = cached();
        runtime.retire_profile(ProfileId::from(1));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(2)]
        );
        assert_eq!(runtime.active_item(), Some(ItemId::from(2)));
        assert_eq!(runtime.last_account, Some(DeckAccountId::from(2)));
        assert_eq!(runtime.return_to, Some(ItemId::from(10)));
        assert_eq!(runtime.profile_for_item(ItemId::from(1)), None);
    }

    #[test]
    fn deleting_the_active_account_retires_its_cached_composer_without_losing_others() {
        let mut runtime = cached();
        runtime.retire_profile(ProfileId::from(2));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(1)]
        );
        assert_eq!(runtime.active_item(), None);
        assert_eq!(runtime.last_account, None);
        assert_eq!(runtime.return_to, None);
        runtime.retire_profile(ProfileId::from(2));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(1)]
        );
    }
}
