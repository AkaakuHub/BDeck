use zephium_core::extensions::ExtensionBrowserRequestId;
mod support;
use super::*;
use support::Fixture;
use zephium_core::ids::DeckAccountId;

#[test]
fn closing_a_cached_composer_after_the_last_column_is_removed_leaves_no_main_pane() {
    let mut fixture = Fixture::new(1);
    let shell = &mut fixture.shell;
    let _ = shell.operation_open_composer(Some(DeckAccountId::from(1)));
    let cached = shell.composer.active_item().unwrap();
    let mut state = shell.deck.state().unwrap().clone();
    state.active_mut().unwrap().columns.clear();
    assert!(shell.apply_deck_state(state).is_some());
    let _ = shell.operation_close_composer();
    assert_eq!(shell.composer.active_item(), None);
    assert!(shell.items.tab(cached).is_some());
    assert_eq!(shell.windows.focused().unwrap().active, None);
    assert_eq!(shell.pane_tree(), None);
}

#[test]
fn closing_login_without_columns_leaves_no_removed_view_as_the_active_page() {
    let mut fixture = Fixture::new(1);
    let shell = &mut fixture.shell;
    let mut state = shell.deck.state().unwrap().clone();
    state.active_mut().unwrap().columns.clear();
    assert!(shell.apply_deck_state(state).is_some());
    shell.browser_page = Some(shell.windows.focused().unwrap().id);
    let _ = shell.operation_open_account(DeckAccountId::from(1));
    let login = shell.accounts.active_item().unwrap();
    let _ = shell.operation_close_account();
    assert!(shell.items.tab(login).is_none());
    assert_eq!(shell.windows.focused().unwrap().active, None);
    assert_eq!(shell.pane_tree(), None);
}

#[test]
fn account_deletion_waits_for_native_proof_and_preserves_other_profiles() {
    use zephium_core::ports::store::Store;
    for count in [1, 2] {
        let mut fixture = Fixture::new(count);
        let target = ProfileId::from(1);
        let _ = fixture
            .shell
            .operation_open_composer(Some(DeckAccountId::from(count)));
        let composer = fixture.shell.composer.active_item().unwrap();
        let begun = fixture
            .shell
            .begin_profile_deletion(target, Some("delete-account".into()));
        assert_eq!(begun.outcome, OperationOutcome::Deferred);
        assert!(fixture.shell.profile_deletion_quarantines(target));
        for _ in 0..8 {
            fixture.shell.drive_profile_deletion(target);
        }
        assert!(!fixture.shell.profile_deletion.failed_closed);
        assert_eq!(fixture.engine.erasures.lock().unwrap().len(), 1);
        assert!(fixture.shell.profiles.get(target).is_none());
        assert!(!fixture
            .engine
            .closed
            .lock()
            .unwrap()
            .contains(&ItemId::from(101)));
        assert!(!fixture.events.lock().unwrap().iter().any(|event| matches!(event, Projection::OperationProcessed(result) if result.operation_id == "delete-account")));
        let stored: zephium_core::deck::DeckState =
            serde_json::from_str(&fixture.store.app_setting("deck.state").unwrap()).unwrap();
        assert_eq!(stored.accounts.len(), (count - 1) as usize);
        assert!(fixture
            .shell
            .spaces
            .get(fixture.shell.windows.focused().unwrap().space)
            .is_some());
        if count == 2 {
            assert!(fixture.shell.items.tab(ItemId::from(102)).is_some());
            assert_eq!(
                fixture.shell.composer.profile_for_item(composer),
                Some(ProfileId::from(2))
            );
        } else {
            assert!(fixture.shell.items.tab(composer).is_none());
            assert_ne!(fixture.shell.windows.focused().unwrap().profile, target);
            assert_eq!(fixture.shell.windows.focused().unwrap().active, None);
        }
        fixture.engine.verify_erasure(target);
        fixture.shell.drain_profile_deletion_inbox();
        assert!(fixture.shell.profile_deletion.states.is_empty());
        assert!(fixture.events.lock().unwrap().iter().any(|event| matches!(event, Projection::OperationProcessed(result) if result.operation_id == "delete-account" && result.outcome == OperationOutcome::Applied && result.reason == OperationReason::ProfileDeletionCompleted)));
        assert!(fixture
            .engine
            .views
            .lock()
            .unwrap()
            .iter()
            .all(|(_, owner)| *owner != target));
        assert!(fixture.store.flush());
    }
}

#[test]
fn a_suspended_column_can_resume_with_the_same_account_and_native_identity() {
    let mut fixture = Fixture::new(1);
    let id = ItemId::from(101);
    let mut state = fixture.shell.deck.state().unwrap().clone();
    state.active_mut().unwrap().columns[0].suspended = true;
    assert!(fixture.shell.apply_deck_state(state.clone()).is_some());
    assert!(fixture.shell.items.tab(id).is_none());
    assert_eq!(fixture.shell.windows.focused().unwrap().active, None);
    state.active_mut().unwrap().columns[0].suspended = false;
    assert!(fixture.shell.apply_deck_state(state).is_some());
    assert!(fixture.shell.items.tab(id).is_some());
    assert_eq!(
        fixture.shell.deck.profile_for_item(id),
        Some(ProfileId::from(1))
    );
    assert_eq!(fixture.shell.windows.focused().unwrap().active, Some(id));
}

#[test]
fn extension_surfaces_keep_columns_isolated_when_the_window_hosts_two_accounts() {
    let mut fixture = Fixture::new(2);
    let shell = &mut fixture.shell;
    for account in [1, 2] {
        let profile = ProfileId::from(account);
        let tab = ItemId::from(100 + account);
        let window = shell.windows.focused().unwrap().id;
        shell.windows.get_mut(window).unwrap().active = Some(tab);
        assert_eq!(shell.web_extension_target(None).unwrap().profile, profile);
        let surface = shell
            .build_extension_browser_surface(profile, ExtensionBrowserSurfaceGeneration::INITIAL)
            .unwrap();
        let tabs = surface.windows()[0].tabs();
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].id(), tab);
        assert_eq!(surface.windows()[0].active(), Some(tab));
    }
}

#[test]
fn one_extension_installation_applies_to_existing_and_new_profiles_and_keeps_statuses_separate() {
    use crate::shell::WebExtensionStatus;
    use zephium_core::ids::ExtensionInstallId;
    use zephium_core::ports::engine::WebExtensionLoad;
    let mut fixture = Fixture::new(2);
    let install = ExtensionInstallId::from(7);
    let extension = WebExtensionLoad {
        install,
        extension_id: "a".repeat(32),
        root: "/package".into(),
        permissions: vec![],
        external_native_messaging: false,
        match_patterns: vec![],
        start_background: true,
    };
    fixture
        .shell
        .set_shared_web_extensions(vec![extension.clone()], vec![], None);
    assert_eq!(fixture.engine.extension_loads.lock().unwrap().len(), 2);
    fixture
        .shell
        .on_web_extension_settled(ProfileId::from(1), install, Err("failed".into()));
    assert_eq!(
        fixture.shell.web_extension_status(ProfileId::from(1))[0].1,
        WebExtensionStatus::Failed("failed".into())
    );
    assert_eq!(
        fixture.shell.web_extension_status(ProfileId::from(2))[0].1,
        WebExtensionStatus::Loading
    );
    assert_eq!(
        fixture.shell.shared_web_extension_status(),
        vec![(install, WebExtensionStatus::Failed("failed".into()))]
    );
    let result = fixture.shell.operation_create_profile(
        "new",
        zephium_core::profiles::ProfileAvatar::initial(ProfileId::from(1)),
        None,
        false,
    );
    assert_eq!(result.outcome, OperationOutcome::Applied);
    assert_eq!(fixture.engine.extension_loads.lock().unwrap().len(), 3);
    fixture.shell.retire_web_extensions(ProfileId::from(1));
    assert_eq!(
        fixture.shell.web_extension_status(ProfileId::from(2))[0].1,
        WebExtensionStatus::Loading
    );
    assert_eq!(
        fixture.shell.shared_web_extension_status(),
        vec![(install, WebExtensionStatus::Loading)]
    );
    fixture.shell.remove_shared_web_extension(extension);
    assert_eq!(fixture.engine.extension_removals.lock().unwrap().len(), 2);
    assert!(fixture
        .shell
        .web_extension_status(ProfileId::from(2))
        .is_empty());
}

#[test]
fn extension_options_from_each_account_return_to_the_original_column() {
    for account in [1, 2] {
        let mut fixture = Fixture::new(2);
        let shell = &mut fixture.shell;
        let profile = ProfileId::from(account);
        let column = ItemId::from(100 + account);
        let _ = shell.operation_activate(column);
        let mut profiles = zephium_core::extensions::ExtensionActiveProfiles::EMPTY;
        assert!(profiles.try_insert(profile));
        assert!(shell.extension_browser_surfaces.replace_active_profiles(profiles));
        shell.browser_page = Some(shell.windows.focused().unwrap().id);
        shell.on_extension_browser_request(ExtensionBrowserRequest::new(
            profile,
            ExtensionBrowserRequestId::new(1).unwrap(),
            ExtensionBrowserRequestAction::OpenExtensionPage,
        ).unwrap());
        let page = shell.windows.focused().unwrap().active.unwrap();
        assert_ne!(page, column);
        assert_eq!(shell.deck_profile_for_item(page), Some(profile));
        assert_eq!(shell.pane_tree(), Some(Pane::Leaf(page)));
        assert_eq!(shell.browser_page, None);
        assert_eq!(shell.items.tab(page).unwrap().content, zephium_core::item::TabContent::ExtensionOwned);
        let closed = shell.operation_run_command("tab.close");
        assert_ne!(closed.outcome, OperationOutcome::Rejected);
        assert_eq!(shell.windows.focused().unwrap().active, Some(column));
        assert!(!shell.web_extensions.pages.contains_key(&page));
        assert!(shell.items.tab(page).is_none());
        assert!(fixture.engine.closed.lock().unwrap().contains(&page));
    }
}

#[test]
fn extension_options_without_columns_close_without_creating_a_browser_tab() {
    let mut fixture = Fixture::new(1);
    let shell = &mut fixture.shell;
    let mut state = shell.deck.state().unwrap().clone();
    state.active_mut().unwrap().columns.clear();
    assert!(shell.apply_deck_state(state).is_some());
    let profile = ProfileId::from(1);
    let mut profiles = zephium_core::extensions::ExtensionActiveProfiles::EMPTY;
    assert!(profiles.try_insert(profile));
    assert!(shell.extension_browser_surfaces.replace_active_profiles(profiles));
    shell.on_extension_browser_request(ExtensionBrowserRequest::new(
        profile,
        ExtensionBrowserRequestId::new(1).unwrap(),
        ExtensionBrowserRequestAction::OpenExtensionPage,
    ).unwrap());
    let page = shell.windows.focused().unwrap().active.unwrap();
    let _ = shell.close(page);
    assert_eq!(shell.windows.focused().unwrap().active, None);
    assert_eq!(shell.pane_tree(), None);
    assert!(shell.web_extensions.pages.is_empty());
}

#[test]
fn extension_toolbar_events_follow_the_selected_account_and_reject_another_account() {
    use zephium_core::extensions::{ExtensionActionRejection, ExtensionActionRevision, ExtensionRuntimeGeneration, ExtensionRuntimeInstance};
    use zephium_core::ids::ExtensionInstallId;
    for account in [1, 2] {
        let mut fixture = Fixture::new(2);
        let column = ItemId::from(100 + account);
        let _ = fixture.shell.operation_activate(column);
        fixture.events.lock().unwrap().clear();
        for profile in [ProfileId::from(1), ProfileId::from(2)] {
            fixture.shell.project_extension_actions(profile);
            fixture.shell.project_extension_action_failure(profile, Some(column), ExtensionActionRejection::NativeAdmissionFailed);
            fixture.shell.project_extension_action_shortcut(
                ExtensionRuntimeInstance::new(profile, ExtensionInstallId::from(1), ExtensionRuntimeGeneration::INITIAL),
                column,
                ExtensionActionRevision::INITIAL,
            );
        }
        let expected = ProfileId::from(account).to_string();
        let events = fixture.events.lock().unwrap();
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|event| match event {
            Projection::ExtensionActions(view) => view.profile_id == expected,
            Projection::ExtensionActionFailed(view) => view.profile_id == expected && view.tab_id == column.to_string(),
            Projection::ExtensionActionShortcut(view) => view.profile_id == expected && view.tab_id == column.to_string(),
            _ => false,
        }));
    }
}

#[test]
fn reloading_chrome_replays_extension_actions_without_recreating_views() {
    let mut fixture = Fixture::new(2);
    let _ = fixture.shell.operation_activate(ItemId::from(102));
    fixture.events.lock().unwrap().clear();
    let views = fixture.engine.views.lock().unwrap().len();
    fixture.shell.bootstrap();
    assert_eq!(fixture.engine.views.lock().unwrap().len(), views);
    assert!(fixture.events.lock().unwrap().iter().any(|event| matches!(event,
        Projection::ExtensionActions(view) if view.profile_id == ProfileId::from(2).to_string()
    )));
}
