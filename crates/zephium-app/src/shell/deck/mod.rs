pub(super) mod accounts;
pub(super) mod composer;
pub(super) mod deletion;
mod portable;
mod profile;
mod shortcuts;
mod viewport;

use super::*;

pub(super) const SETTING_KEY: &str = "deck.state";

pub(super) fn view_preferences(
    store: &dyn zephium_core::ports::store::Store,
) -> (zephium_core::deck::DeckAppearance, bool) {
    let appearance = store
        .app_setting("deck.appearance")
        .and_then(|value| serde_json::from_str::<zephium_core::deck::DeckAppearance>(&value).ok())
        .filter(zephium_core::deck::DeckAppearance::valid)
        .unwrap_or_default();
    let hidden = store
        .app_setting(zephium_core::deck::appearance::HIDE_ADVERTISEMENTS_SETTING)
        .as_deref()
        == Some("true");
    (appearance, hidden)
}

#[derive(Default)]
pub(super) struct DeckRuntime {
    pub(super) pending_import: Option<portable::PendingImport>,
    state: Option<zephium_core::deck::DeckState>,
    pub(super) scroll_offset: std::cell::Cell<f64>,
    item_profiles: std::collections::HashMap<ItemId, ProfileId>,
}

impl DeckRuntime {
    pub(super) fn load(store: &dyn zephium_core::ports::store::Store) -> Self {
        let state = store
            .app_setting(SETTING_KEY)
            .and_then(|value| serde_json::from_str(&value).ok())
            .filter(|state: &zephium_core::deck::DeckState| state.valid());
        Self {
            pending_import: None,
            state,
            item_profiles: std::collections::HashMap::new(),
            scroll_offset: std::cell::Cell::new(0.0),
        }
    }

    pub(super) fn profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.item_profiles.get(&item).copied()
    }

    pub(super) fn state(&self) -> Option<&zephium_core::deck::DeckState> {
        self.state.as_ref()
    }

    fn columns_for(state: &zephium_core::deck::DeckState) -> Vec<(ItemId, ProfileId, String)> {
        let Some(deck) = state.decks.iter().find(|deck| deck.id == state.active) else {
            return Vec::new();
        };
        deck.columns
            .iter()
            .filter(|column| !column.suspended)
            .filter_map(|column| {
                let item = ItemId::parse(&column.id.to_string())?;
                let account = state.account(column.account)?;
                let url = column.target.url()?;
                Some((item, account.profile, url))
            })
            .collect()
    }

    fn active_columns(&self) -> Vec<(ItemId, ProfileId, String)> {
        self.state.as_ref().map_or_else(Vec::new, Self::columns_for)
    }

    fn can_apply(&self, shell: &Shell, state: &zephium_core::deck::DeckState) -> bool {
        if !state.valid()
            || self
                .state
                .as_ref()
                .is_some_and(|previous| !previous.preserves_column_profiles(state))
            || state.accounts.iter().any(|account| {
                shell.profiles.get(account.profile).is_none()
                    || shell.profile_deletion_quarantines(account.profile)
            })
        {
            return false;
        }
        state.decks.iter().all(|deck| {
            deck.columns.iter().all(|column| {
                let Some(item) = ItemId::parse(&column.id.to_string()) else {
                    return false;
                };
                shell.items.get(item).is_none()
                    || self.item_profiles.contains_key(&item)
                    || self.state.as_ref().is_some_and(|current| {
                        current.decks.iter().any(|candidate| {
                            candidate
                                .columns
                                .iter()
                                .any(|existing| existing.id == column.id)
                        })
                    })
            })
        })
    }

    fn set_state(&mut self, state: zephium_core::deck::DeckState) {
        self.state = Some(state);
    }

    pub(super) fn active_item(&self) -> Option<ItemId> {
        self.active_columns().first().map(|(item, _, _)| *item)
    }
}

impl Shell {
    pub(in crate::shell) fn project_deck_state(&self) {
        if let Some(encoded) = self
            .deck
            .state()
            .and_then(|state| serde_json::to_string(state).ok())
        {
            (self.emit)(Projection::UiCommand(format!("deck.state={encoded}")));
        }
    }

    pub(super) fn deck_profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.web_extensions
            .pages
            .get(&item)
            .map(|page| page.profile)
            .or_else(|| self.composer.profile_for_item(item))
            .or_else(|| self.accounts.profile_for_item(item))
            .or_else(|| self.deck.profile_for_item(item))
    }

    pub(super) fn can_apply_deck(&self, state: &zephium_core::deck::DeckState) -> bool {
        self.deck.can_apply(self, state)
    }

    pub(super) fn apply_deck_state(
        &mut self,
        state: zephium_core::deck::DeckState,
    ) -> Option<NativeWork> {
        if !self.can_apply_deck(&state) {
            return None;
        }
        let Some(window) = self
            .windows
            .focused()
            .map(|window| (window.profile, window.space))
        else {
            self.deck.set_state(state);
            return Some(NativeWork::default());
        };
        if self
            .deck
            .state()
            .is_some_and(|previous| previous.active != state.active)
        {
            self.deck.scroll_offset.set(0.0);
        }
        let active = DeckRuntime::columns_for(&state);
        let active_ids = active
            .iter()
            .map(|(item, _, _)| *item)
            .collect::<std::collections::HashSet<_>>();
        let old_ids = self.deck.item_profiles.keys().copied().collect::<Vec<_>>();
        let mut effects = Vec::new();
        for item in old_ids {
            if !active_ids.contains(&item) {
                effects.extend(self.items.remove(item));
                self.deck.item_profiles.remove(&item);
            }
        }
        for (item, profile, url) in &active {
            let restore_target = !self.deck.item_profiles.contains_key(item);
            if self.items.get(*item).is_none() {
                if !self.items.insert_unloaded_tab(
                    *item,
                    zephium_core::item::Placement::Space {
                        space: window.1,
                        section: zephium_core::item::SpaceSection::Today,
                    },
                    url::Url::parse(url).ok()?,
                    "X",
                ) {
                    return None;
                }
            }
            self.deck.item_profiles.insert(*item, *profile);
            if restore_target
                && self
                    .items
                    .tab(*item)
                    .and_then(|tab| tab.url.as_ref())
                    .is_some_and(|current| current.as_str() != url)
            {
                effects.extend(self.items.navigate(*item, url));
            }
            self.touch(*item);
        }
        if let Some(window) = self.windows.focused_mut() {
            window.splits = None;
        }
        self.deck.set_state(state);
        if let Some(item) = self
            .windows
            .focused()
            .and_then(|window| window.active)
            .filter(|item| active_ids.contains(item))
            .or_else(|| self.deck.active_item())
        {
            effects.extend(self.focus_tab(item));
        } else if let Some(window) = self.windows.focused_mut() {
            window.active = None;
        }
        let native = self.commit(effects);
        self.prepare_composers();
        Some(native)
    }
}

#[cfg(test)]
mod tests;
