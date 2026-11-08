mod model;
mod placement;
mod window;
pub(crate) use model::{PanelAction, PanelView};
use std::sync::Mutex;
use tauri::{Manager, State, WebviewWindow};
pub(crate) use window::{install, LABEL};

pub(crate) struct Panel {
    view: Mutex<Option<PanelView>>,
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
) -> Option<PanelView> {
    admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_get",
    )
    .then(|| panel.view.lock().ok()?.clone())
    .flatten()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_ready(
    caller: WebviewWindow,
    panel: State<'_, Panel>,
    height: f64,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_ready",
    ) || panel.view.lock().map_err(|_| "unavailable")?.is_none()
    {
        return Err("unavailable".into());
    }
    window::resize(caller.app_handle(), height)?;
    caller
        .show()
        .and_then(|_| caller.set_focus())
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_close(caller: WebviewWindow) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_close",
    ) {
        return Err("unavailable".into());
    }
    caller.hide().map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_action(
    caller: WebviewWindow,
    panel: State<'_, Panel>,
    action: PanelAction,
) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_action",
    ) {
        return Err("unavailable".into());
    }
    let view = panel.view.lock().map_err(|_| "unavailable")?;
    let view = view.as_ref().ok_or("unavailable")?;
    let valid = match &action {
        PanelAction::Account(id) => view.accounts.iter().any(|account| &account.id == id),
        _ if view.composer => false,
        PanelAction::Create(draft) => draft.valid(view),
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
    crate::emit_to_privileged(
        caller.app_handle(),
        crate::MAIN_LABEL,
        "zephium:column-panel-action",
        &(view.id.clone(), action),
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
    let mut current = panel.view.lock().map_err(|_| "unavailable")?;
    if view.is_none() {
        *current = None;
        if let Some(window) = caller.app_handle().get_webview_window(LABEL) {
            let _ = window.hide();
            crate::emit_to_privileged(
                caller.app_handle(),
                LABEL,
                "zephium:column-panel-view",
                &Option::<PanelView>::None,
            );
        }
        return Ok(());
    }
    if current.as_ref().map(|view| (&view.id, view.composer))
        != view.as_ref().map(|view| (&view.id, view.composer))
    {
        return Ok(());
    }
    *current = view.clone();
    crate::emit_to_privileged(
        caller.app_handle(),
        LABEL,
        "zephium:column-panel-view",
        &view,
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn column_panel_resize(caller: WebviewWindow, height: f64) -> Result<(), String> {
    if !admitted(
        &caller,
        crate::CallerPolicy::ColumnPanel,
        "column_panel_resize",
    ) {
        return Err("unavailable".into());
    }
    window::resize(caller.app_handle(), height)
}
