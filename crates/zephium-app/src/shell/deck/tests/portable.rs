use super::*;
use zephium_core::deck::account::AccountIdentity;
use zephium_core::deck::portable::LayoutArchive;
use zephium_core::ports::store::Store;

#[test]
fn importing_waits_for_durable_profiles_before_adopting_the_layout() {
    let mut fixture = Fixture::new(1);
    let queue = CommandQueue::new();
    fixture.shell.self_queue = Some(queue.clone());
    fixture.shell.browser_page = Some(fixture.shell.windows.focused().unwrap().id);
    let original = fixture.shell.deck.state().unwrap().clone();
    let mut incoming = original.clone();
    incoming.accounts[0].identity = Some(AccountIdentity {
        user_id: "9007199254740993".into(),
        display_name: "Imported".into(),
        screen_name: "imported".into(),
    });
    incoming.decks[0].columns[0].width = 440;
    let archive = LayoutArchive::export(&incoming).unwrap();
    assert!(fixture
        .shell
        .begin_deck_import(&archive, "import-layout".into())
        .is_none());
    assert_eq!(fixture.shell.deck.state(), Some(&original));
    assert_eq!(fixture.shell.profiles.iter().count(), 1);
    let mut changed = original.clone();
    changed.decks[0].columns[0].width = 500;
    assert_eq!(
        fixture
            .shell
            .operation_set_app_setting(
                "deck.state".into(),
                serde_json::to_string(&changed).unwrap()
            )
            .reason,
        OperationReason::StorePending
    );
    fixture.shell.persist();
    assert!(fixture.store.flush());
    let zephium_core::ports::store::SessionLoad::Loaded { state: durable } =
        fixture.store.load_session()
    else {
        panic!("expected committed session")
    };
    assert_eq!(durable.profiles.len(), 2);
    assert!(durable.items.is_empty());
    let Command::DeckImported(outcome) = queue.recv().unwrap() else {
        panic!("expected import settlement")
    };
    fixture.shell.finish_deck_import(outcome);
    assert!(fixture.shell.deck.pending_import.is_none());
    assert_eq!(fixture.shell.profiles.iter().count(), 2);
    let applied = fixture.shell.deck.state().unwrap();
    assert_eq!(applied.decks[0].columns[0].width, 440);
    assert_eq!(applied.accounts[0], original.accounts[0]);
    assert_ne!(applied.accounts[0].profile, applied.accounts[1].profile);
    assert!(fixture.events.lock().unwrap().iter().any(|event| matches!(event, Projection::OperationProcessed(result) if result.operation_id == "import-layout" && matches!(result.outcome, OperationOutcome::Applied | OperationOutcome::Deferred))));
    fixture.shell.persist();
    assert!(fixture.store.flush());
    assert_eq!(
        fixture.store.load_session(),
        zephium_core::ports::store::SessionLoad::Loaded {
            state: fixture.shell.session_snapshot()
        }
    );
}
