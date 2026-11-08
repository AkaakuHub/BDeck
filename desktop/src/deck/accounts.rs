use tauri::{Manager, State, WebviewWindow};
use zephium_app::{Command, Handle};
use zephium_core::profiles::{ProfileAvatar, ProfileAvatarStyle};
use zephium_ipc::OperationAdmission;

fn admitted(caller: &WebviewWindow, command: &str) -> bool {
    crate::authorize(caller, crate::CallerPolicy::Main, command)
        && !crate::shutdown_started(caller.app_handle())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn account_open(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    account: String,
) -> OperationAdmission {
    let Some(account) = zephium_core::ids::DeckAccountId::parse(&account)
        .filter(|_| admitted(&caller, "account_open"))
    else {
        return crate::rejected_operation();
    };
    crate::dispatch_operation(caller.app_handle(), &shell, Command::OpenAccount(account))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn profiles_create(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
) -> OperationAdmission {
    if !admitted(&caller, "profiles_create") {
        return crate::rejected_operation();
    }
    crate::dispatch_operation(
        caller.app_handle(),
        &shell,
        Command::CreateProfile {
            name: "未ログイン".into(),
            avatar: ProfileAvatar {
                style: ProfileAvatarStyle::Initials,
                color: 0,
                symbol: 0,
                revision: 0,
            },
            photo: None,
            switch: false,
        },
    )
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn account_delete(
    caller: WebviewWindow,
    app: tauri::AppHandle,
    shell: State<'_, Handle>,
    account: String,
) -> Result<Option<OperationAdmission>, String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    if !admitted(&caller, "account_delete") {
        return Err("unavailable".into());
    }
    let id = zephium_core::ids::DeckAccountId::parse(&account).ok_or("invalid account")?;
    let candidate = tauri::async_runtime::spawn_blocking(move || {
        let state: zephium_core::deck::DeckState =
            serde_json::from_str(&crate::app_setting("deck.state")?).ok()?;
        if !state.valid() {
            return None;
        }
        state.account(id).cloned()
    })
    .await
    .map_err(|_| "unavailable")?
    .ok_or("unavailable")?;
    let name = candidate
        .identity
        .as_ref()
        .map(|identity| format!("{} (@{})", identity.display_name, identity.screen_name))
        .unwrap_or_else(|| "このアカウント".into());
    let (send, receive) = tokio::sync::oneshot::channel();
    app.dialog().message(format!("{name}をBDeckから削除します。ログイン情報と関連するカラムを削除します。Xのアカウント自体は削除しません。"))
        .parent(&caller).title("アカウントを削除").kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom("削除".into(), "キャンセル".into()))
        .show(move |confirmed| { let _ = send.send(confirmed); });
    if !receive.await.map_err(|_| "unavailable")? {
        return Ok(None);
    }
    if !admitted(&caller, "account_delete") {
        return Err("unavailable".into());
    }
    Ok(Some(crate::dispatch_operation(
        &app,
        &shell,
        Command::DeleteProfile(candidate.profile),
    )))
}
