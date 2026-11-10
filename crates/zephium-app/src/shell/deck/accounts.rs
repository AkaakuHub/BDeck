mod avatars;
use super::*;
use std::collections::HashMap;
use zephium_core::deck::account::{AccountIdentity, AccountLoginState};
use zephium_core::ids::DeckAccountId;

struct AccountView {
    account: DeckAccountId,
    profile: ProfileId,
    item: ItemId,
    return_to: Option<ItemId>,
    identity_verified: bool,
    refresh_requested: bool,
}

#[derive(Default)]
pub(in crate::shell) struct AccountRuntime {
    active: Option<AccountView>,
    sessions: HashMap<ProfileId, AccountLoginState>,
    authenticated_items: HashMap<ItemId, ProfileId>,
    avatars: zephium_core::deck::avatars::AccountAvatars,
}

impl AccountRuntime {
    pub(in crate::shell) fn retire_profile(&mut self, profile: ProfileId) {
        self.sessions.remove(&profile);
        self.authenticated_items.retain(|_, owner| *owner != profile);
        self.avatars.remove(&profile);
        if self
            .active
            .as_ref()
            .is_some_and(|view| view.profile == profile)
        {
            self.active = None;
        }
    }

    pub(in crate::shell) fn account_changed(&self, profile: ProfileId) -> bool {
        self.sessions.get(&profile) == Some(&AccountLoginState::AccountChanged)
    }
    pub(in crate::shell) fn active_profile(&self) -> Option<ProfileId> {
        self.active.as_ref().map(|view| view.profile)
    }
    pub(in crate::shell) fn active_item(&self) -> Option<ItemId> {
        self.active.as_ref().map(|view| view.item)
    }

    pub(in crate::shell) fn profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.active
            .as_ref()
            .filter(|view| view.item == item)
            .map(|view| view.profile)
    }

    pub(in crate::shell) fn return_to(&self) -> Option<ItemId> {
        self.active.as_ref().and_then(|view| view.return_to)
    }
}

impl Shell {
    pub(in crate::shell) fn account_available(&self, account: &zephium_core::deck::DeckAccount) -> bool {
        account.identity.is_some()
            && self.accounts.sessions.get(&account.profile) == Some(&AccountLoginState::LoggedIn)
            && self.accounts.active.as_ref().is_none_or(|view| view.profile != account.profile || view.identity_verified)
            && self.profiles.get(account.profile).is_some()
            && !self.profile_deletion_quarantines(account.profile)
    }

    fn complete_account_login(&mut self, profile: ProfileId) {
        let connected = self.accounts.active.as_ref()
            .filter(|view| view.profile == profile)
            .and_then(|view| self.deck.state()?.account(view.account))
            .filter(|account| self.account_available(account))
            .is_some();
        if connected {
            let cached = self.composer.retained_items()
                .filter(|item| self.composer.profile_for_item(*item) == Some(profile))
                .collect::<Vec<_>>();
            let _ = self.operation_close_account();
            for item in cached {
                self.accounts.authenticated_items.remove(&item);
                let _ = self.engine.reload(item);
            }
            self.prepare_composers();
        }
    }

    pub(in crate::shell) fn register_deck_account(&mut self, profile: ProfileId) -> Option<DeckAccountId> {
        let mut state = self.deck.state().cloned()?;
        let id = DeckAccountId::generate();
        state.accounts.push(zephium_core::deck::DeckAccount::new(
            id,
            profile,
        ));
        let encoded = serde_json::to_string(&state).ok()?;
        let result = self.operation_set_app_setting("deck.state".into(), encoded);
        (result.outcome != OperationOutcome::Rejected).then_some(id)
    }
    pub(in crate::shell) fn account_identity_observed(
        &mut self,
        item: ItemId,
        identity: AccountIdentity,
    ) {
        let Some(profile) = self.deck_profile_for_item(item) else {
            return;
        };
        if self.profile_deletion_quarantines(profile)
            || self.accounts.active.as_ref().is_some_and(|view| view.profile == profile && view.item != item)
        {
            return;
        }
        let Some(mut state) = self.deck.state().cloned() else {
            return;
        };
        let Some(account) = state
            .accounts
            .iter_mut()
            .find(|account| account.profile == profile)
        else {
            return;
        };
        if account
            .identity
            .as_ref()
            .is_some_and(|previous| previous.user_id != identity.user_id)
        {
            self.accounts
                .sessions
                .insert(profile, AccountLoginState::AccountChanged);
            self.project_account_sessions();
            return;
        }
        if self.accounts.account_changed(profile) {
            self.accounts.sessions.remove(&profile);
            self.project_account_sessions();
        }
        if let Some(view) = self.accounts.active.as_mut().filter(|view| view.item == item) {
            view.identity_verified = true;
        }
        if account.identity.as_ref() == Some(&identity) {
            self.complete_account_login(profile);
            return;
        }
        account.identity = Some(identity);
        let Ok(encoded) = serde_json::to_string(&state) else {
            return;
        };
        let _ = self.operation_set_app_setting("deck.state".into(), encoded);
        self.complete_account_login(profile);
    }
    pub(in crate::shell) fn account_frame(&self) -> Option<zephium_ipc::ComposerFrame> {
        let view = self.accounts.active.as_ref()?;
        let window = self.windows.focused()?;
        let width = 620.0_f64.min((window.size.width - 80.0).max(0.0));
        let height = 680.0_f64.min((window.size.height - 96.0).max(0.0));
        Some(zephium_ipc::ComposerFrame {
            tab: view.item.to_string(),
            account: view.account.to_string(),
            managing: true,
            x: (window.size.width - width) / 2.0,
            y: 48.0 + (window.size.height - 48.0 - height) / 2.0,
            width,
            height,
        })
    }

    pub(in crate::shell) fn operation_open_account(
        &mut self,
        id: DeckAccountId,
    ) -> OperationDisposition {
        if !self.settings_visible() {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        let Some(account) = self
            .deck
            .state()
            .and_then(|state| state.account(id))
            .cloned()
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        if self.profile_deletion_quarantines(account.profile)
            || self.profiles.get(account.profile).is_none()
        {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if let Some(active) = self.accounts.active.as_ref() {
            if active.account == id {
                return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
            }
            let _ = self.operation_close_account();
        }
        let Some(window) = self.windows.focused() else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let return_to = window.active;
        let item = ItemId::generate();
        if !self.items.insert_unloaded_tab(
            item,
            zephium_core::item::Placement::Space {
                space: window.space,
                section: zephium_core::item::SpaceSection::Today,
            },
            url::Url::parse("https://x.com/home").expect("static account URL"),
            "アカウント",
        ) {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::ItemLimitReached,
            );
        }
        self.accounts.active = Some(AccountView {
            account: id,
            profile: account.profile,
            item,
            return_to,
            identity_verified: false,
            refresh_requested: false,
        });
        self.accounts.sessions.insert(account.profile, AccountLoginState::Unknown);
        self.project_account_sessions();
        let effects = self.focus_tab(item);
        let native = self.commit(effects);
        if !native.rejected {
            let _ = self.engine.focus(item);
        }
        mutation_result(native)
    }

    pub(in crate::shell) fn operation_close_account(&mut self) -> OperationDisposition {
        let Some(view) = self.accounts.active.take() else {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        };
        if !view.identity_verified && self.accounts.sessions.get(&view.profile) == Some(&AccountLoginState::LoggedIn) {
            self.accounts.sessions.insert(view.profile, AccountLoginState::Unknown);
        }
        self.project_account_sessions();
        let mut effects = self.items.remove(view.item);
        let target = view
            .return_to
            .filter(|id| self.item_in_focused_scope(*id))
            .or_else(|| self.deck.active_item());
        if let Some(item) = target {
            effects.extend(self.focus_tab(item));
        } else if let Some(window) = self.windows.focused_mut() {
            window.active = None;
        }
        mutation_result(self.commit(effects))
    }

    pub(in crate::shell) fn account_login_observed(
        &mut self,
        item: ItemId,
        state: AccountLoginState,
    ) {
        let Some(profile) = self.deck_profile_for_item(item) else {
            return;
        };
        if self.profile_deletion_quarantines(profile)
            || self.accounts.active.as_ref().is_some_and(|view| view.profile == profile && view.item != item)
        {
            return;
        }
        if self.accounts.sessions.get(&profile) == Some(&AccountLoginState::AccountChanged) {
            return;
        }
        if state == AccountLoginState::LoggedOut
            && self.accounts.sessions.get(&profile) == Some(&AccountLoginState::LoggedIn)
            && !self.accounts.authenticated_items.contains_key(&item)
            && self.accounts.active_item() != Some(item)
            && self.windows.focused().and_then(|window| window.active) != Some(item)
        {
            return;
        }
        self.accounts.authenticated_items.retain(|item, _| self.items.get(*item).is_some());
        if state == AccountLoginState::LoggedIn {
            self.accounts.authenticated_items.insert(item, profile);
        } else {
            self.accounts.authenticated_items.remove(&item);
        }
        if self.accounts.sessions.insert(profile, state) != Some(state) {
            self.project_account_sessions();
        }
        if state != AccountLoginState::LoggedIn
            && self.composer.active_item().is_some_and(|item| self.composer.profile_for_item(item) == Some(profile))
        {
            let _ = self.operation_close_composer();
        }
        let refresh = self.accounts.active.as_mut().filter(|view| view.item == item)
            .is_some_and(|view| {
                if state != AccountLoginState::LoggedIn || view.identity_verified || view.refresh_requested {
                    return false;
                }
                view.refresh_requested = true;
                true
            });
        if refresh {
            let _ = self.operation_reload(item);
        }
        self.complete_account_login(profile);
    }

    pub(in crate::shell) fn project_account_sessions(&self) {
        (self.emit)(Projection::UiCommand(format!(
            "account.deleting={}",
            !self.profile_deletion.states.is_empty()
        )));
        let Some(state) = self.deck.state() else {
            return;
        };
        for account in &state.accounts {
            let mut login = self
                .accounts
                .sessions
                .get(&account.profile)
                .copied()
                .unwrap_or_default();
            if login == AccountLoginState::LoggedIn && self.accounts.active.as_ref().is_some_and(|view| view.profile == account.profile && !view.identity_verified) {
                login = AccountLoginState::Unknown;
            }
            (self.emit)(Projection::UiCommand(format!(
                "account.status={}:{}",
                account.id,
                login.as_str()
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retiring_a_profile_clears_its_login_and_avatar_without_touching_another_login() {
        let first = ProfileId::from(1);
        let second = ProfileId::from(2);
        let mut runtime = AccountRuntime::default();
        runtime.sessions.insert(first, AccountLoginState::LoggedIn);
        runtime
            .sessions
            .insert(second, AccountLoginState::LoggedOut);
        runtime
            .avatars
            .insert(first, "https://pbs.twimg.com/profile_images/1/a.jpg".into());
        runtime.avatars.insert(
            second,
            "https://pbs.twimg.com/profile_images/2/b.jpg".into(),
        );
        runtime.active = Some(AccountView {
            account: DeckAccountId::from(2),
            profile: second,
            item: ItemId::from(2),
            return_to: None,
            identity_verified: false,
            refresh_requested: false,
        });
        runtime.retire_profile(first);
        assert!(!runtime.sessions.contains_key(&first));
        assert!(!runtime.avatars.contains_key(&first));
        assert_eq!(
            runtime.sessions.get(&second),
            Some(&AccountLoginState::LoggedOut)
        );
        assert_eq!(
            runtime.avatars.get(&second).map(String::as_str),
            Some("https://pbs.twimg.com/profile_images/2/b.jpg")
        );
        assert_eq!(runtime.active_item(), Some(ItemId::from(2)));
        runtime.retire_profile(second);
        assert!(runtime.sessions.is_empty());
        assert!(runtime.avatars.is_empty());
        assert_eq!(runtime.active_item(), None);
    }

}
