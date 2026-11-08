use tauri::{State, WebviewWindow};
use zephium_app::{Command, Handle};

#[tauri::command]
#[specta::specta]
pub(crate) fn composer_open(
    caller: WebviewWindow,
    app: tauri::AppHandle,
    shell: State<'_, Handle>,
    account: String,
) -> zephium_ipc::OperationAdmission {
    if !crate::authorize(&caller, crate::CallerPolicy::Main, "composer_open")
        || crate::shutdown_started(&app)
    {
        return crate::rejected_operation();
    }
    let Some(account) = zephium_core::ids::DeckAccountId::parse(&account) else {
        return crate::rejected_operation();
    };
    crate::dispatch_operation(&app, &shell, Command::OpenComposer(account))
}
