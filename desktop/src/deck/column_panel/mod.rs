mod lifecycle;
#[cfg(target_os = "macos")]
mod macos;
mod model;
mod placement;
mod presentation;
mod window;
pub(crate) use lifecycle::observe_composer;
pub(crate) use model::{PanelAction, PanelSnapshot, PanelView};
use std::sync::Mutex;
use tauri::{Manager, State, WebviewWindow};
pub(crate) use window::{install, LABEL};

pub(crate) struct Panel {
    presentation: Mutex<presentation::Presentation>,
    context: window::Context,
    anchor: Mutex<Option<(f64, f64)>>,
    placement: Mutex<Option<placement::Placement>>,
}

fn admitted(caller: &WebviewWindow, policy: crate::CallerPolicy, command: &str) -> bool {
    crate::authorize(caller, policy, command) && !crate::shutdown_started(caller.app_handle())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn column_panel_open(
    caller: WebviewWindow,
    view: PanelView,
    x: f64,
    y: f64,
) -> Result<(), String> {
    if !admitted(&caller, crate::CallerPolicy::Main, "column_panel_open") {
        return Err("unavailable".into());
    }
    let size = crate::inner_logical(&caller);
    if view
        .id
        .as_ref()
        .is_some_and(|id| zephium_core::ids::DeckColumnId::parse(id).is_none())
        || !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x > size.width
        || y > size.height
        || view.accounts.len() > zephium_core::deck::MAX_ACCOUNTS
    {
        return Err("invalid column panel".into());
    }
    let app = caller.app_handle().clone();
    // WebView2 controller creation must leave the originating IPC callback.
    tauri::async_runtime::spawn_blocking(move || {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let task_app = app.clone();
        app.run_on_main_thread(move || {
            let result = window::open(&task_app, view, x, y).map_err(|error| error.to_string());
            let _ = send.send(result);
        })
        .map_err(|error| error.to_string())?;
        receive
            .recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|error| error.to_string())?
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_get(
    caller: WebviewWindow,
    panel: State<'_, Panel>,
) -> Option<PanelSnapshot> {
    admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_get",
    )
    .then(|| panel.presentation.lock().ok()?.snapshot())
    .flatten()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_ready(
    caller: WebviewWindow,
    revision: u32,
    height: f64,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_ready",
    ) {
        return Err("unavailable".into());
    }
    if !window::resize(caller.app_handle(), revision, height)? {
        return Ok(());
    }
    window::show(caller.app_handle(), revision)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_close(caller: WebviewWindow, revision: u32) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_close",
    ) {
        return Err("unavailable".into());
    }
    window::close(caller.app_handle(), revision)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_action(
    caller: WebviewWindow,
    panel: State<'_, Panel>,
    revision: u32,
    action: PanelAction,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_action",
    ) {
        return Err("unavailable".into());
    }
    let Some(snapshot) = panel
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .snapshot_at(revision)
    else {
        return Ok(());
    };
    let view = snapshot.view;
    let valid = match &action {
        PanelAction::Account(id) => view.accounts.iter().any(|account| &account.id == id),
        _ if view.composer => false,
        PanelAction::Create(draft) => draft.valid(&view),
        _ if view.id.is_none() => false,
        PanelAction::Width(width) => (zephium_core::deck::MIN_COLUMN_WIDTH
            ..=zephium_core::deck::MAX_COLUMN_WIDTH)
            .contains(width),
        PanelAction::Row(row) => *row <= 1,
        _ => true,
    };
    if !valid {
        return Err("invalid column action".into());
    }
    if view.composer {
        if let PanelAction::Account(id) = action {
            let account = zephium_core::ids::DeckAccountId::parse(&id).ok_or("invalid account")?;
            let shell = caller.app_handle().state::<zephium_app::Handle>();
            let _ = crate::dispatch_operation(
                caller.app_handle(),
                &shell,
                zephium_app::Command::OpenComposer(account),
            );
        }
        return Ok(());
    }
    crate::emit_to_privileged(
        caller.app_handle(),
        crate::MAIN_LABEL,
        "zephium:column-panel-action",
        &(view.id, action),
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_sync(
    caller: WebviewWindow,
    panel: State<'_, Panel>,
    view: Option<PanelView>,
) -> Result<(), String> {
    if !admitted(&caller, crate::CallerPolicy::Main, "column_panel_sync") {
        return Err("unavailable".into());
    }
    let snapshot = {
        let mut current = panel.presentation.lock().map_err(|_| "unavailable")?;
        if current.view.as_ref().is_some_and(|view| view.composer) {
            return Ok(());
        }
        let Some(view) = view else {
            if let Some(snapshot) = current.snapshot() {
                current.dismiss(snapshot.revision);
            }
            drop(current);
            return window::hide_if_closed(caller.app_handle());
        };
        if current
            .view
            .as_ref()
            .map(|current| (&current.id, current.composer))
            != Some((&view.id, view.composer))
        {
            return Ok(());
        }
        if !current.allows(&view) {
            return Ok(());
        }
        current.present(view)?
    };
    crate::emit_to_privileged(
        caller.app_handle(),
        LABEL,
        "zephium:column-panel-view",
        &Some(snapshot),
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn column_panel_composer_sync(
    caller: WebviewWindow,
    view: Option<PanelView>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::Main,
        "column_panel_composer_sync",
    ) {
        return Err("unavailable".into());
    }
    let Some(view) = view else {
        return Ok(());
    };
    if !view.composer
        || view
            .content_height
            .is_none_or(|height| !(160..=640).contains(&height))
    {
        return Err("invalid composer".into());
    }
    column_panel_open(caller, view, x, y).await
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_resize(
    caller: WebviewWindow,
    revision: u32,
    height: f64,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_resize",
    ) {
        return Err("unavailable".into());
    }
    window::resize(caller.app_handle(), revision, height).map(|_| ())
}
