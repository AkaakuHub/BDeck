use super::*;
use zephium_core::deck::account::{AccountIdentity, AccountLoginState};

#[test]
fn unverified_accounts_cannot_post_or_add_columns() {
    for status in [AccountLoginState::Unknown, AccountLoginState::LoggedOut, AccountLoginState::AccountChanged] {
        let mut fixture = Fixture::new(1);
        fixture.authenticate_accounts();
        fixture.shell.account_login_observed(ItemId::from(101), status);
        let result = fixture.shell.operation_open_composer(None);
        assert_eq!(result.outcome, OperationOutcome::Rejected);
        assert!(fixture.shell.composer.active_item().is_none());
        let mut state = fixture.shell.deck.state().unwrap().clone();
        let mut column = state.decks[0].columns[0].clone();
        column.id = zephium_core::ids::DeckColumnId::generate();
        state.decks[0].columns.push(column);
        let result = fixture.shell.operation_set_app_setting("deck.state".into(), serde_json::to_string(&state).unwrap());
        assert_eq!(result.outcome, OperationOutcome::Rejected);
        assert_eq!(fixture.shell.deck.state().unwrap().decks[0].columns.len(), 1);
    }
}

#[test]
fn login_waits_for_both_identity_and_session_in_either_arrival_order() {
    for identity_first in [true, false] {
        let mut fixture = Fixture::new(1);
        fixture.shell.browser_page = Some(fixture.shell.windows.focused().unwrap().id);
        fixture.shell.operation_open_account(DeckAccountId::from(1));
        let item = fixture.shell.accounts.active_item().unwrap();
        let identity = AccountIdentity { user_id: "123".into(), display_name: "Actual name".into(), screen_name: "actual_name".into() };
        if identity_first {
            fixture.shell.account_identity_observed(item, identity.clone());
        } else {
            fixture.shell.account_login_observed(item, AccountLoginState::LoggedIn);
        }
        assert_eq!(fixture.shell.accounts.active_item(), Some(item));
        assert_eq!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
        if identity_first {
            fixture.shell.account_login_observed(item, AccountLoginState::LoggedIn);
        } else {
            fixture.shell.account_identity_observed(item, identity);
        }
        assert!(fixture.shell.accounts.active_item().is_none());
        assert!(fixture.shell.deck.state().unwrap().accounts[0].identity.is_some());
        assert_ne!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
    }
}

#[test]
fn logout_closes_the_editor_and_prevents_reopening_it() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    assert_ne!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
    let item = fixture.shell.composer.active_item().unwrap();
    fixture.shell.account_login_observed(item, AccountLoginState::LoggedOut);
    assert!(fixture.shell.composer.active_item().is_none());
    assert_eq!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
}

#[test]
fn a_verified_session_can_add_columns_and_the_default_composer_skips_logged_out_accounts() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
    fixture.shell.account_login_observed(ItemId::from(101), AccountLoginState::LoggedOut);
    assert_ne!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
    let item = fixture.shell.composer.active_item().unwrap();
    assert_eq!(fixture.shell.composer.profile_for_item(item), Some(ProfileId::from(2)));
    fixture.shell.operation_close_composer();
    let mut state = fixture.shell.deck.state().unwrap().clone();
    let mut column = state.decks[0].columns[1].clone();
    column.id = zephium_core::ids::DeckColumnId::generate();
    state.decks[0].columns.push(column);
    let result = fixture.shell.operation_set_app_setting("deck.state".into(), serde_json::to_string(&state).unwrap());
    assert_ne!(result.outcome, OperationOutcome::Rejected);
    assert_eq!(fixture.shell.deck.state().unwrap().decks[0].columns.len(), 3);
}

#[test]
fn reauthentication_does_not_complete_using_a_cached_identity_or_a_login_signal_alone() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    fixture.shell.browser_page = Some(fixture.shell.windows.focused().unwrap().id);
    fixture.shell.operation_open_account(DeckAccountId::from(1));
    let item = fixture.shell.accounts.active_item().unwrap();
    fixture.shell.account_login_observed(item, AccountLoginState::LoggedIn);
    assert_eq!(fixture.shell.accounts.active_item(), Some(item));
    assert_eq!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
    fixture.shell.operation_close_account();
    assert_eq!(fixture.shell.operation_open_composer(None).outcome, OperationOutcome::Rejected);
    assert!(!fixture.events.lock().unwrap().iter().any(|event| matches!(event, Projection::UiCommand(value) if value.starts_with("account.connected="))));
}
