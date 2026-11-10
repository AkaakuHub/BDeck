use super::*;
use zephium_core::deck::account::AccountIdentity;
use zephium_core::ports::engine::ComposerFocus;

#[test]
fn expanding_a_composer_menu_does_not_shrink_or_recreate_the_editor_after_closing_it() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    fixture.shell.operation_open_composer(None, None);
    let item = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_height(item, 560.0);
    let expanded = fixture.shell.composer_frame().unwrap().height;
    fixture.shell.composer_height(item, 260.0);
    assert_eq!(fixture.shell.composer_frame().unwrap().height, expanded);
    fixture.shell.composer_height(item, 700.0);
    assert_eq!(fixture.shell.composer_frame().unwrap().height, expanded);
    fixture.shell.operation_close_composer();
    fixture.shell.operation_open_composer(None, None);
    assert_eq!(fixture.shell.composer.active_item(), Some(item));
    assert_eq!(fixture.shell.composer_frame().unwrap().height, expanded);
}

#[test]
fn preparing_account_composers_keeps_focus_and_reuses_each_isolated_view() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
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
        .operation_open_composer(Some(DeckAccountId::from(2)), None);
    assert_eq!(fixture.shell.composer.active_item(), Some(cached[1]));
    fixture.shell.operation_close_composer();
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(2)), None);
    assert_eq!(fixture.shell.composer.active_item(), Some(cached[1]));
}

#[test]
fn only_a_prepared_editor_returning_home_is_reset_for_the_next_post() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(1)), None);
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

#[test]
fn completing_a_post_closes_the_panel_but_retains_the_editor_for_reuse() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    let previous = fixture.shell.windows.focused().unwrap().active;
    fixture.shell.operation_open_composer(None, None);
    let item = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_ready(item);
    let home = url::Url::parse("https://x.com/home").unwrap();
    fixture.shell.items.set_committed_url(item, home.clone());
    fixture.shell.reset_composer_after_navigation(item, &home);
    assert!(fixture.shell.composer.active_item().is_none());
    assert!(fixture.shell.composer_frame().is_none());
    assert_eq!(fixture.shell.windows.focused().unwrap().active, previous);
    assert!(fixture.shell.items.tab(item).is_some());
    assert!(!fixture.engine.closed.lock().unwrap().contains(&item));
    fixture.shell.operation_open_composer(None, None);
    assert_eq!(fixture.shell.composer.active_item(), Some(item));
}

#[test]
fn a_background_editors_home_navigation_does_not_close_another_accounts_panel() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(1)), None);
    let background = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_ready(background);
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(2)), None);
    let active = fixture.shell.composer.active_item().unwrap();
    fixture.shell.reset_composer_after_navigation(
        background,
        &url::Url::parse("https://x.com/home").unwrap(),
    );
    assert_eq!(fixture.shell.composer.active_item(), Some(active));
    assert!(!fixture.engine.closed.lock().unwrap().contains(&background));
}

#[test]
fn profile_shortcut_uses_the_source_account_and_defers_the_mention_until_ready() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
    fixture
        .shell
        .operation_open_composer(Some(DeckAccountId::from(2)), None);
    fixture.shell.operation_close_composer();
    fixture.engine.composer_focus.lock().unwrap().clear();
    let source = ItemId::from(101);
    fixture.shell.items.set_committed_url(
        source,
        url::Url::parse("https://x.com/Viewed_User").unwrap(),
    );
    fixture.shell.handle_deck_shortcut(
        source,
        r#"composer.open="https://x.com/Viewed_User""#.into(),
    );
    let editor = fixture.shell.composer.active_item().unwrap();
    assert_eq!(
        fixture.shell.composer.profile_for_item(editor),
        Some(ProfileId::from(1))
    );
    assert!(fixture
        .engine
        .composer_focus
        .lock()
        .unwrap()
        .iter()
        .all(|(_, mention)| *mention == ComposerFocus::Preserve));
    fixture.shell.composer_ready(editor);
    assert_eq!(
        fixture.engine.composer_focus.lock().unwrap().last(),
        Some(&(editor, ComposerFocus::Recipient(Some("Viewed_User".into()))))
    );
    fixture.shell.composer_ready(editor);
    assert_eq!(
        fixture
            .engine
            .composer_focus
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, mention)| matches!(mention, ComposerFocus::Recipient(Some(_))))
            .count(),
        1
    );
}

#[test]
fn home_shortcut_does_not_insert_an_account_name_as_a_recipient() {
    let mut fixture = Fixture::new(1);
    fixture.authenticate_accounts();
    fixture.shell.handle_deck_shortcut(
        ItemId::from(101),
        r#"composer.open="https://x.com/home""#.into(),
    );
    let editor = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_ready(editor);
    assert_eq!(
        fixture.engine.composer_focus.lock().unwrap().last(),
        Some(&(editor, ComposerFocus::Recipient(None)))
    );
}

#[test]
fn repeated_profile_shortcuts_return_to_the_originating_column_and_reuse_the_editor() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
    let source = ItemId::from(101);
    fixture.shell.items.set_committed_url(
        source,
        url::Url::parse("https://x.com/Viewed_User").unwrap(),
    );
    fixture.shell.operation_activate(ItemId::from(102));
    let mut cached = None;
    for _ in 0..3 {
        fixture.shell.handle_deck_shortcut(
            source,
            r#"composer.open="https://x.com/Viewed_User""#.into(),
        );
        let editor = fixture.shell.composer.active_item().unwrap();
        assert_eq!(fixture.shell.composer.return_to, Some(source));
        if let Some(previous) = cached {
            assert_eq!(editor, previous);
        }
        cached = Some(editor);
        fixture.shell.composer_ready(editor);
        fixture
            .shell
            .handle_deck_shortcut(editor, "composer.close".into());
        assert!(fixture.shell.composer.active_item().is_none());
        assert_eq!(
            fixture.shell.windows.focused().unwrap().active,
            Some(source)
        );
        assert!(!fixture.engine.closed.lock().unwrap().contains(&editor));
    }
}

#[test]
fn each_shortcut_uses_its_live_url_even_before_native_navigation_is_committed() {
    let mut fixture = Fixture::new(2);
    fixture.authenticate_accounts();
    let profile = ItemId::from(101);
    let notifications = ItemId::from(102);
    fixture.shell.items.set_committed_url(
        profile,
        url::Url::parse("https://x.com/Viewed_User").unwrap(),
    );
    fixture.shell.handle_deck_shortcut(
        profile,
        r#"composer.open="https://x.com/Viewed_User""#.into(),
    );
    let editor = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_ready(editor);
    fixture.shell.operation_close_composer();
    fixture.shell.items.set_committed_url(
        notifications,
        url::Url::parse("https://x.com/notifications").unwrap(),
    );
    fixture.shell.handle_deck_shortcut(
        notifications,
        r#"composer.open="https://x.com/notifications""#.into(),
    );
    let notification_editor = fixture.shell.composer.active_item().unwrap();
    fixture.shell.composer_ready(notification_editor);
    assert_eq!(
        fixture.engine.composer_focus.lock().unwrap().last(),
        Some(&(notification_editor, ComposerFocus::Recipient(None)))
    );
    fixture.shell.operation_close_composer();
    fixture
        .shell
        .handle_deck_shortcut(profile, r#"composer.open="https://x.com/home""#.into());
    assert_eq!(fixture.shell.composer.active_item(), Some(editor));
    assert_eq!(
        fixture.engine.composer_focus.lock().unwrap().last(),
        Some(&(editor, ComposerFocus::Recipient(None)))
    );
    fixture.shell.composer_ready(editor);
    assert_eq!(
        fixture.engine.composer_focus.lock().unwrap().last(),
        Some(&(editor, ComposerFocus::Preserve))
    );
}
