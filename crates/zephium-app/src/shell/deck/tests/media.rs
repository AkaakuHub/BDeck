use super::*;

#[test]
fn media_uses_a_separate_reusable_view_without_navigating_the_timeline() {
    let mut fixture = Fixture::new(1);
    let source = ItemId::from(101);
    let url = fixture.shell.items.tab(source).unwrap().url.clone();
    let panes = fixture.shell.pane_tree();
    fixture.engine.navigations.lock().unwrap().clear();
    fixture.shell.open_media(source, url::Url::parse("https://x.com/user/status/1/photo/1").unwrap());
    let viewer = fixture.shell.deck.media.active_item().unwrap();
    assert_ne!(viewer, source);
    assert_eq!(fixture.shell.items.tab(source).unwrap().url, url);
    assert_eq!(fixture.shell.pane_tree(), panes);
    fixture.shell.close_media(viewer);
    let navigations = fixture.engine.navigations.lock().unwrap().len();
    fixture.shell.open_media(source, url::Url::parse("https://x.com/user/status/1/photo/1").unwrap());
    assert_eq!(fixture.engine.navigations.lock().unwrap().len(), navigations);
    assert_eq!(fixture.shell.deck.media.active_item(), Some(viewer));
    fixture.shell.close_media(viewer);
    assert!(!fixture.shell.discard_protected_leaves().contains(&viewer));
    assert_eq!(fixture.shell.deck.media.active_item(), None);
    assert_eq!(fixture.shell.pane_tree(), panes);
    fixture.shell.open_media(source, url::Url::parse("https://x.com/user/status/2/video/1").unwrap());
    assert_eq!(fixture.shell.deck.media.active_item(), Some(viewer));
    assert_eq!(fixture.shell.items.tab(source).unwrap().url, url);
    assert!(fixture.engine.navigations.lock().unwrap().iter().all(|(item, _)| *item == viewer));
    assert!(!fixture.shell.session_snapshot().items.iter().any(|item| item.id == viewer));
}

#[test]
fn switching_accounts_replaces_the_single_media_view_and_closes_its_native_resource() {
    let mut fixture = Fixture::new(2);
    let media = url::Url::parse("https://x.com/user/status/1/photo/1").unwrap();
    fixture.shell.open_media(ItemId::from(101), media.clone());
    let previous = fixture.shell.deck.media.active_item().unwrap();
    fixture.shell.close_media(previous);
    fixture.shell.open_media(ItemId::from(102), media);
    let current = fixture.shell.deck.media.active_item().unwrap();
    assert_ne!(previous, current);
    assert!(fixture.shell.items.tab(previous).is_none());
    assert!(fixture.engine.closed.lock().unwrap().contains(&previous));
    assert_eq!(fixture.shell.profile_of_item(current), Some(ProfileId::from(2)));
    assert_eq!(fixture.shell.deck.media.item(), Some(current));
}
