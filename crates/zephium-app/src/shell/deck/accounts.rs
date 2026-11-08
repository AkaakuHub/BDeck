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
}

impl AccountView {
    fn completes_login(&self, item: ItemId, state: AccountLoginState) -> bool {
        self.item == item && state == AccountLoginState::LoggedIn
    }
}

#[derive(Default)]
pub(in crate::shell) struct AccountRuntime {
    active: Option<AccountView>,
    sessions: HashMap<ProfileId, AccountLoginState>,
    avatars: zephium_core::deck::avatars::AccountAvatars,
}

impl AccountRuntime {
    pub(in crate::shell) fn retire_profile(&mut self, profile: ProfileId) {
        self.sessions.remove(&profile);
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
    pub(in crate::shell) fn register_deck_account(&mut self, profile: ProfileId) {
        let Some(mut state) = self.deck.state().cloned() else {
            return;
        };
        state.accounts.push(zephium_core::deck::DeckAccount::new(
            zephium_core::ids::DeckAccountId::generate(),
            profile,
        ));
        if let Ok(encoded) = serde_json::to_string(&state) {
            let _ = self.operation_set_app_setting("deck.state".into(), encoded);
        }
    }
    pub(in crate::shell) fn account_identity_observed(
        &mut self,
        item: ItemId,
        identity: AccountIdentity,
    ) {
        let Some(profile) = self.deck_profile_for_item(item) else {
            return;
        };
        if self.profile_deletion_quarantines(profile) {
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
        if account.identity.as_ref() == Some(&identity) {
            return;
        }
        account.identity = Some(identity);
        let Ok(encoded) = serde_json::to_string(&state) else {
            return;
        };
        let _ = self.operation_set_app_setting("deck.state".into(), encoded);
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
        });
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
        if self.profile_deletion_quarantines(profile) {
            return;
        }
        if self.accounts.sessions.get(&profile) == Some(&AccountLoginState::AccountChanged) {
            return;
        }
        if self.accounts.sessions.insert(profile, state) != Some(state) {
            self.project_account_sessions();
        }
        let connected = self
            .accounts
            .active
            .as_ref()
            .filter(|view| view.completes_login(item, state))
            .map(|view| view.account);
        if let Some(account) = connected {
            let _ = self.operation_close_account();
            (self.emit)(Projection::UiCommand(format!(
                "account.connected={account}"
            )));
        }
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
            let login = self
                .accounts
                .sessions
                .get(&account.profile)
                .copied()
                .unwrap_or_default();
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

    #[test]
    fn only_the_login_view_completes_its_own_sign_in() {
        let view = AccountView {
            account: DeckAccountId::from(1),
            profile: ProfileId::from(1),
            item: ItemId::from(1),
            return_to: None,
        };
        assert!(!view.completes_login(ItemId::from(1), AccountLoginState::Unknown));
        assert!(!view.completes_login(ItemId::from(1), AccountLoginState::LoggedOut));
        assert!(!view.completes_login(ItemId::from(2), AccountLoginState::LoggedIn));
        assert!(view.completes_login(ItemId::from(1), AccountLoginState::LoggedIn));
    }
}
