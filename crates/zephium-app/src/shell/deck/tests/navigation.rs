use super::*;
use zephium_core::deck::ColumnTarget;

#[test]
fn horizontal_scroll_never_reserves_space_below_the_columns() {
    for width in [360.0, 1800.0] {
        let mut fixture = Fixture::new(4);
        fixture.shell.windows.focused_mut().unwrap().size = Size::new(width, 480.0);
        let expected = fixture.shell.content_region();
        let _ = fixture.shell.relayout();
        let contents = fixture.engine.contents.lock().unwrap();
        let (tree, region) = contents.last().unwrap();
        assert_eq!(*region, Some(expected));
        let panes = zephium_core::split::layout(
            tree.as_ref().unwrap(),
            Rect::new(0.0, 0.0, expected.width, expected.height),
            1.0,
        );
        assert!(
            panes
                .iter()
                .all(|(_, rect)| rect.y + rect.height == expected.height)
        );
        let events = fixture.events.lock().unwrap();
        let layout = events
            .iter()
            .rev()
            .find_map(|event| match event {
                Projection::Layout(layout) => Some(layout),
                _ => None,
            })
            .unwrap();
        if let Some(scroll) = &layout.scroll {
            let header = layout.columns.first().unwrap();
            assert!(scroll.y >= header.y);
            assert!(scroll.y + scroll.height <= header.y + header.height);
        }
    }
}

#[test]
fn reloading_a_column_returns_to_its_configured_target_after_navigation() {
    for (target, expected) in [
        (ColumnTarget::Home, "https://x.com/home"),
        (ColumnTarget::Notifications, "https://x.com/notifications"),
        (ColumnTarget::Profile, "https://x.com/i/user/1"),
        (
            ColumnTarget::Search {
                query: "rust lang".into(),
            },
            "https://x.com/search?q=rust+lang",
        ),
        (
            ColumnTarget::List {
                url: "https://x.com/i/lists/123".into(),
            },
            "https://x.com/i/lists/123",
        ),
    ] {
        let mut fixture = Fixture::new(1);
        fixture.authenticate_accounts();
        let id = ItemId::from(101);
        let mut state = fixture.shell.deck.state().unwrap().clone();
        state.active_mut().unwrap().columns[0].target = target;
        assert!(fixture.shell.apply_deck_state(state).is_some());
        let _ = fixture
            .shell
            .operation_navigate(id, "https://x.com/explore".into());
        fixture.engine.navigations.lock().unwrap().clear();
        let result = fixture.shell.operation_reload(id);
        assert!(matches!(
            result.outcome,
            OperationOutcome::Applied | OperationOutcome::Deferred
        ));
        assert_eq!(
            *fixture.engine.navigations.lock().unwrap(),
            vec![(id, expected.into())]
        );
    }
}
