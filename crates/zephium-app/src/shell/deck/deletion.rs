use super::*;
use std::collections::{HashMap, HashSet};

pub(in crate::shell) fn filter_session(
    mut state: session::SessionState,
    profile: ProfileId,
    owners: &HashMap<ItemId, ProfileId>,
) -> Option<session::SessionState> {
    let removed_spaces: HashSet<_> = state
        .spaces
        .iter()
        .filter(|space| space.profile == profile)
        .map(|space| space.id)
        .collect();
    state.profiles.retain(|candidate| candidate.id != profile);
    state.spaces.retain(|space| space.profile != profile);
    state
        .recently_closed
        .retain(|entry| entry.profile != profile);
    let host = state
        .active_space
        .filter(|space| !removed_spaces.contains(space))
        .or_else(|| state.spaces.first().map(|space| space.id));
    let mut removed = Vec::new();
    state.items.retain_mut(|item| {
        let (Placement::Favorites { space } | Placement::Space { space, .. }) = item.placement;
        let owner = owners.get(&item.id).copied();
        if owner == Some(profile) || (removed_spaces.contains(&space) && owner.is_none()) {
            removed.push(item.id);
            return false;
        }
        if removed_spaces.contains(&space) {
            let Some(host) = host else {
                removed.push(item.id);
                return false;
            };
            item.placement = Placement::Space {
                space: host,
                section: zephium_core::item::SpaceSection::Today,
            };
            item.parent = None;
        }
        true
    });
    state.active_space = host;
    if state
        .active_item
        .is_some_and(|item| removed.contains(&item))
    {
        state.active_item = None;
    }
    for item in removed {
        state.splits = state.splits.take().and_then(|pane| pane.remove(item));
    }
    (session::canonicalize(state.clone()) == state).then_some(state)
}

impl Shell {
    pub(in crate::shell) fn deck_item_owners(&self) -> HashMap<ItemId, ProfileId> {
        let mut owners = self.deck.item_profiles.clone();
        if let Some(view) = &self.deck.media.view {
            owners.insert(view.item, view.profile);
        }
        owners.extend(self.composer.retained_items().filter_map(|item| {
            self.composer
                .profile_for_item(item)
                .map(|profile| (item, profile))
        }));
        if let Some(item) = self.accounts.active_item() {
            if let Some(profile) = self.accounts.profile_for_item(item) {
                owners.insert(item, profile);
            }
        }
        owners
    }

    pub(in crate::shell) fn retire_deck_profile(&mut self, profile: ProfileId) -> bool {
        let owners = self.deck_item_owners();
        if self.deck.media.view.as_ref().is_some_and(|view| view.profile == profile) {
            self.deck.media.active.set(false);
            self.deck.media.view = None;
        }
        self.composer.retire_profile(profile);
        self.accounts.retire_profile(profile);
        (self.emit)(Projection::UiCommand(format!(
            "account.avatar={profile}:null"
        )));
        for (item, owner) in &owners {
            if *owner == profile {
                let _ = self.items.remove(*item);
            }
        }
        self.deck.item_profiles.retain(|_, owner| *owner != profile);
        let missing_host = self
            .windows
            .focused()
            .is_some_and(|window| self.spaces.get(window.space).is_none());
        if missing_host {
            let existing = self
                .spaces
                .iter()
                .find(|space| {
                    self.profiles
                        .get(space.profile)
                        .is_some_and(|profile| profile.kind != ProfileKind::Incognito)
                })
                .map(|space| space.id);
            let host = existing.or_else(|| self.create_default_space());
            if let Some((space, owner)) =
                host.and_then(|space| self.spaces.get(space).map(|entry| (space, entry.profile)))
            {
                for (item, item_profile) in owners {
                    if item_profile != profile {
                        self.items.move_tab_to_root(
                            item,
                            Placement::Space {
                                space,
                                section: zephium_core::item::SpaceSection::Today,
                            },
                            None,
                        );
                    }
                }
                if let Some(window) = self.windows.focused_mut() {
                    window.profile = owner;
                    window.space = space;
                }
            } else {
                return false;
            }
        }
        if let Some(mut state) = self.deck.state().cloned() {
            if let Some(account) = state
                .accounts
                .iter()
                .find(|account| account.profile == profile)
            {
                (self.emit)(Projection::UiCommand(format!(
                    "account.removed={}",
                    account.id
                )));
            }
            state.remove_profile(profile);
            self.deck.set_state(state);
            self.project_deck_state();
        }
        true
    }
}

#[cfg(test)]
mod tests;
