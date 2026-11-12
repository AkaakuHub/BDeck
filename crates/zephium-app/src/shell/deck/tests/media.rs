use super::*;

#[test]
fn media_expands_the_existing_column_without_navigation_or_new_items() {
    let mut fixture = Fixture::new(1);
    let source = ItemId::from(101);
    let url = fixture.shell.items.tab(source).unwrap().url.clone();
    let panes = fixture.shell.pane_tree();
    let session = fixture.shell.session_snapshot().items.len();
    fixture.engine.navigations.lock().unwrap().clear();
    for _ in 0..3 {
        fixture.shell.open_media(source);
        assert_eq!(fixture.shell.deck.overlay.get(), Some(source));
        assert_eq!(fixture.shell.pane_tree(), panes);
        fixture.shell.close_media(source);
        assert_eq!(fixture.shell.deck.overlay.get(), None);
    }
    assert!(fixture.engine.navigations.lock().unwrap().is_empty());
    assert_eq!(fixture.shell.items.tab(source).unwrap().url, url);
    assert_eq!(fixture.shell.session_snapshot().items.len(), session);
}

#[test]
fn another_item_cannot_close_the_expanded_column() {
    let mut fixture = Fixture::new(1);
    let source = ItemId::from(101);
    fixture.shell.open_media(source);
    fixture.shell.close_media(ItemId::from(102));
    assert_eq!(fixture.shell.deck.overlay.get(), Some(source));
}
