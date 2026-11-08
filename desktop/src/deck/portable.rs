use std::io::Read;
use tauri::{Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;
use zephium_app::{Command, Handle};
use zephium_core::deck::{
    portable::{LayoutArchive, MAX_LAYOUT_BYTES},
    DeckState,
};
use zephium_ipc::OperationAdmission;

fn admitted(caller: &WebviewWindow, command: &str) -> bool {
    crate::authorize(caller, crate::CallerPolicy::Main, command)
        && !crate::shutdown_started(caller.app_handle())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn deck_layout_export(caller: WebviewWindow) -> Result<bool, String> {
    if !admitted(&caller, "deck_layout_export") {
        return Err("操作できません。".into());
    }
    let dialog = caller
        .app_handle()
        .dialog()
        .file()
        .set_parent(&caller)
        .add_filter("BDeck layout", &["json"])
        .set_file_name("bdeck-layout.json");
    tauri::async_runtime::spawn_blocking(move || {
        let state: DeckState = crate::app_setting("deck.state")
            .and_then(|value| serde_json::from_str(&value).ok())
            .ok_or("配置を取得できませんでした。")?;
        let archive = LayoutArchive::export(&state).ok_or("配置を取得できませんでした。")?;
        let bytes =
            serde_json::to_vec(&archive).map_err(|_| "配置を保存できませんでした。")?;
        if bytes.len() > MAX_LAYOUT_BYTES {
            return Err("配置ファイルの上限を超えています。".into());
        }
        let Some(path) = dialog.blocking_save_file() else {
            return Ok(false);
        };
        let path = path
            .into_path()
            .map_err(|_| "この保存先は使用できません。")?;
        std::fs::write(path, bytes).map_err(|_| "配置を保存できませんでした。")?;
        Ok(true)
    })
    .await
    .map_err(|_| "配置を保存できませんでした。".to_string())?
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn deck_layout_import(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
) -> Result<Option<OperationAdmission>, String> {
    if !admitted(&caller, "deck_layout_import") {
        return Err("操作できません。".into());
    }
    let dialog = caller
        .app_handle()
        .dialog()
        .file()
        .set_parent(&caller)
        .add_filter("BDeck layout", &["json"]);
    let archive =
        tauri::async_runtime::spawn_blocking(move || -> Result<Option<LayoutArchive>, String> {
            let Some(path) = dialog.blocking_pick_file() else {
                return Ok(None);
            };
            let path = path
                .into_path()
                .map_err(|_| "このファイルは読み込めません。")?;
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .and_then(|file| {
                    file.take(MAX_LAYOUT_BYTES as u64 + 1)
                        .read_to_end(&mut bytes)
                })
                .map_err(|_| "配置ファイルを読み込めませんでした。")?;
            LayoutArchive::decode(&bytes)
                .map(Some)
                .map_err(|_| "有効なBDeckの配置ファイルではありません。".into())
        })
        .await
        .map_err(|_| "配置ファイルを読み込めませんでした。".to_string())??;
    let Some(archive) = archive else {
        return Ok(None);
    };
    if !admitted(&caller, "deck_layout_import") {
        return Err("操作できません。".into());
    }
    Ok(Some(crate::dispatch_operation(
        caller.app_handle(),
        &shell,
        Command::ImportDeck(archive),
    )))
}
