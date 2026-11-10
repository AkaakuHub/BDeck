use super::{Panel, PanelSnapshot, LABEL};
use tauri::Manager;
use zephium_core::ids::ItemId;

pub(crate) fn observe_composer(
    app: &tauri::AppHandle,
    composer: Option<&zephium_ipc::ComposerFrame>,
) {
    let Some(panel) = app.try_state::<Panel>() else {
        return;
    };
    let item = composer
        .filter(|frame| !frame.managing)
        .and_then(|frame| ItemId::parse(&frame.tab));
    let invalidated = panel
        .presentation
        .lock()
        .ok()
        .is_some_and(|mut state| state.observe_composer(item));
    if invalidated {
        let target = app.clone();
        let _ = app.run_on_main_thread(move || {
            let _ = super::window::hide_if_closed(&target);
        });
    }
}

pub(super) fn close(app: &tauri::AppHandle, revision: u32) -> Result<(), String> {
    let view = app
        .state::<Panel>()
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .dismiss(revision);
    let Some(view) = view else {
        return Ok(());
    };
    if view.composer {
        let shell = app.state::<zephium_app::Handle>();
        let _ = crate::dispatch_operation(
            app,
            &shell,
            zephium_app::Command::Run("composer.close".into()),
        );
    }
    hide_if_closed(app)
}

pub(super) fn hide_if_closed(app: &tauri::AppHandle) -> Result<(), String> {
    if app
        .state::<Panel>()
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .view
        .is_some()
    {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        let focused = window.is_focused().unwrap_or(false);
        window.hide().map_err(|error| error.to_string())?;
        crate::emit_to_privileged(
            app,
            LABEL,
            "zephium:column-panel-view",
            &Option::<PanelSnapshot>::None,
        );
        if focused && super::window::application_active() {
            if let Some(main) = app.get_webview_window(crate::MAIN_LABEL) {
                if main.is_visible().unwrap_or(false) && !main.is_minimized().unwrap_or(true) {
                    let _ = main.set_focus();
                }
            }
        }
    }
    Ok(())
}

pub(super) fn show(app: &tauri::AppHandle, revision: u32) -> Result<(), String> {
    let snapshot = app
        .state::<Panel>()
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .snapshot_at(revision);
    let Some(snapshot) = snapshot else {
        return Ok(());
    };
    let main = app
        .get_webview_window(crate::MAIN_LABEL)
        .ok_or("unavailable")?;
    let window = app.get_webview_window(LABEL).ok_or("unavailable")?;
    if !main.is_visible().map_err(|error| error.to_string())?
        || main.is_minimized().map_err(|error| error.to_string())?
    {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    super::macos::attach(&main, &window)?;
    window
        .show()
        .and_then(|_| window.set_focus())
        .map_err(|error| error.to_string())?;
    if app
        .state::<Panel>()
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .snapshot_at(revision)
        .is_none()
    {
        return Ok(());
    }
    if snapshot.view.composer {
        let item = snapshot
            .view
            .id
            .as_deref()
            .and_then(zephium_core::ids::ItemId::parse)
            .ok_or("invalid composer")?;
        if !zephium_engine::focus_floating_composer(item) {
            return Err("composer focus failed".into());
        }
    }
    Ok(())
}
