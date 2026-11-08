use std::time::Duration;
use tauri::{Manager, State, WebviewWindow};
use zephium_app::Handle;
use zephium_ipc::ProfilesView;

fn admitted(caller: &WebviewWindow, command: &str) -> bool {
    super::authorize(caller, super::CallerPolicy::Main, command)
        && !super::shutdown_started(caller.app_handle())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn profiles_get(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
) -> Result<Option<ProfilesView>, ()> {
    if !admitted(&caller, "profiles_get") {
        return Ok(None);
    }
    let receiver = shell.profiles();
    Ok(
        tokio::task::spawn_blocking(move || receiver.recv_timeout(Duration::from_secs(4)))
            .await
            .ok()
            .and_then(Result::ok)
            .flatten(),
    )
}
