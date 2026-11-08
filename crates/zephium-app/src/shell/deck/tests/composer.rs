use super::*;
use zephium_core::deck::account::AccountIdentity;

#[test]
fn preparing_account_composers_keeps_focus_and_reuses_each_isolated_view() {
    let mut fixture = Fixture::new(2);
    let mut state = fixture.shell.deck.state().unwrap().clone();
    for (index, account) in state.accounts.iter_mut().enumerate() {
        account.identity = Some(AccountIdentity {
            user_id: (index + 1).to_string(),
            display_name: "Account".into(),
            screen_name: format!("account{index}"),
        });
    }
    let previous = fixture.shell.windows.focused().unwrap().active;
    assert!(fixture.shell.apply_deck_state(state).is_some());
    assert_eq!(fixture.shell.windows.focused().unwrap().active, previous);
    let cached = fixture.shell.composer.retained_items().collect::<Vec<_>>();
    assert_eq!(cached.len(), 2);
    for (index, item) in cached.iter().enumerate() {
        assert_eq!(
            fixture.shell.composer.profile_for_item(*item),
            Some(ProfileId::from(index as u128 + 1))
        );
        assert!(fixture
            .engine
            .views
            .lock()
            .unwrap()
            .contains(&(*item, ProfileId::from(index as u128 + 1))));
    }
    fixture.shell.prepare_composers();
    assert_eq!(
        fixture.shell.composer.retained_items().collect::<Vec<_>>(),
        cached
    );
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(2)));
    assert_eq!(fixture.shell.composer.active_item(), Some(cached[1]));
    fixture.shell.operation_close_composer();
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(2)));
    assert_eq!(fixture.shell.composer.active_item(), Some(cached[1]));
}

#[test]
fn only_a_prepared_editor_returning_home_is_reset_for_the_next_post() {
    let mut fixture = Fixture::new(1);
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(1)));
    let item = fixture.shell.composer.active_item().unwrap();
    let home = url::Url::parse("https://x.com/home").unwrap();
    fixture.shell.items.set_committed_url(item, home.clone());
    let initial = fixture.shell.items.pending_navigation_request(item);
    fixture.shell.reset_composer_after_navigation(item, &home);
    assert_eq!(
        fixture.shell.items.pending_navigation_request(item),
        initial
    );
    fixture.shell.composer_ready(item);
    fixture.shell.reset_composer_after_navigation(item, &home);
    let request = fixture.shell.items.pending_navigation_request(item);
    assert!(request.is_some());
    assert_ne!(request, initial);
    fixture.shell.reset_composer_after_navigation(item, &home);
    assert_eq!(
        fixture.shell.items.pending_navigation_request(item),
        request
    );
}
