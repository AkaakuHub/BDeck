use super::*;
use zephium_core::deck::ColumnTarget;

#[test]
fn reloading_a_column_returns_to_its_configured_target_after_navigation() {
    for (target, expected) in [
        (ColumnTarget::Home, "https://x.com/home"),
        (ColumnTarget::Notifications, "https://x.com/notifications"),
        (ColumnTarget::Profile, "https://x.com/i/user/1"),
        (ColumnTarget::Search { query: "rust lang".into() }, "https://x.com/search?q=rust+lang"),
        (ColumnTarget::List { url: "https://x.com/i/lists/123".into() }, "https://x.com/i/lists/123"),
    ] {
        let mut fixture = Fixture::new(1);
        fixture.authenticate_accounts();
        let id = ItemId::from(101);
        let mut state = fixture.shell.deck.state().unwrap().clone();
        state.active_mut().unwrap().columns[0].target = target;
        assert!(fixture.shell.apply_deck_state(state).is_some());
        let _ = fixture.shell.operation_navigate(id, "https://x.com/explore".into());
        fixture.engine.navigations.lock().unwrap().clear();
        let result = fixture.shell.operation_reload(id);
        assert!(matches!(result.outcome, OperationOutcome::Applied | OperationOutcome::Deferred));
        assert_eq!(*fixture.engine.navigations.lock().unwrap(), vec![(id, expected.into())]);
    }
}
