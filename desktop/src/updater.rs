use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Manager, WebviewWindow};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tauri_plugin_updater::{Update, UpdaterExt};
mod schedule;

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UpdatePhase {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Verifying,
    Installing,
    Failed,
}

#[derive(Clone, serde::Serialize, specta::Type)]
pub(crate) struct UpdateStatus {
    revision: u32,
    phase: UpdatePhase,
    current_version: String,
    version: Option<String>,
    downloaded: f64,
    total: Option<f64>,
    error: Option<String>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: UpdatePhase::Idle,
            current_version: env!("CARGO_PKG_VERSION").into(),
            version: None,
            downloaded: 0.0,
            total: None,
            error: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct UpdateManager {
    status: Mutex<UpdateStatus>,
    pending: Mutex<Option<Update>>,
    busy: AtomicBool,
    started: AtomicBool,
}

fn publish(app: &tauri::AppHandle, update: impl FnOnce(&mut UpdateStatus)) {
    let state = app.state::<UpdateManager>();
    if let Ok(mut status) = state.status.lock() {
        update(&mut status);
        status.revision = status.revision.saturating_add(1);
        crate::emit_to_privileged(app, crate::MAIN_LABEL, "bdeck:updater", &*status);
    };
}

fn failed(app: &tauri::AppHandle, error: impl std::fmt::Display) {
    eprintln!("updater: {error}");
    publish(app, |status| {
        status.phase = UpdatePhase::Failed;
        status.error = Some("更新を完了できませんでした。もう一度お試しください。".into());
    });
}

pub(crate) fn startup(app: &tauri::AppHandle) {
    let updater = app.state::<UpdateManager>();
    if updater.started.swap(true, Ordering::AcqRel) || tauri::is_dev() {
        return;
    }
    if crate::app_setting("updates.auto-check").as_deref() != Some("false") {
        check(app.clone(), true);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut schedule = schedule::daily();
        loop {
            schedule.tick().await;
            if crate::shutdown_started(&app) {
                return;
            }
            let enabled = tauri::async_runtime::spawn_blocking(|| {
                crate::app_setting("updates.auto-check").as_deref() != Some("false")
            })
            .await
            .unwrap_or(false);
            if enabled {
                check(app.clone(), false);
            }
        }
    });
}

fn check(app: tauri::AppHandle, notify: bool) {
    if crate::shutdown_started(&app) {
        return;
    }
    if app
        .state::<UpdateManager>()
        .busy
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    publish(&app, |status| {
        status.phase = UpdatePhase::Checking;
        status.error = None;
    });
    tauri::async_runtime::spawn(async move {
        let result = async {
            let update = app
                .updater_builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|error| error.to_string())?
                .check()
                .await
                .map_err(|error| error.to_string())?;
            let version = update.as_ref().map(|update| update.version.clone());
            let available = version.is_some();
            *app.state::<UpdateManager>()
                .pending
                .lock()
                .map_err(|_| "update state unavailable".to_owned())? = update;
            publish(&app, |status| {
                status.phase = if available {
                    UpdatePhase::Available
                } else {
                    UpdatePhase::Current
                };
                status.version = version;
            });
            Ok::<bool, String>(available)
        }
        .await;
        app.state::<UpdateManager>()
            .busy
            .store(false, Ordering::Release);
        match result {
            Err(error) => failed(&app, error),
            Ok(true) if notify => {
                let next = app.clone();
                app.dialog()
                    .message("新しいバージョンのBDeckがあります。更新して再起動しますか？")
                    .title("BDeckの更新")
                    .buttons(MessageDialogButtons::OkCancelCustom(
                        "更新して再起動".into(),
                        "後で".into(),
                    ))
                    .show(move |accepted| {
                        if accepted {
                            install(next);
                        }
                    });
            }
            _ => {}
        }
    });
}

fn install(app: tauri::AppHandle) {
    if crate::shutdown_started(&app) {
        return;
    }
    if app
        .state::<UpdateManager>()
        .busy
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let result = install_update(&app).await;
        app.state::<UpdateManager>()
            .busy
            .store(false, Ordering::Release);
        if let Err(error) = result {
            failed(&app, error);
        }
    });
}

async fn install_update(app: &tauri::AppHandle) -> Result<(), String> {
    let update = app
        .state::<UpdateManager>()
        .pending
        .lock()
        .map_err(|_| "update state unavailable".to_owned())?
        .take()
        .ok_or("更新を確認してください。")?;
    publish(app, |status| {
        status.phase = UpdatePhase::Downloading;
        status.downloaded = 0.0;
        status.total = None;
        status.error = None;
    });
    let mut downloaded = 0_u64;
    let mut reported = std::time::Instant::now();
    let bytes = update
        .download(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                if reported.elapsed() >= std::time::Duration::from_millis(150)
                    || total == Some(downloaded)
                {
                    reported = std::time::Instant::now();
                    publish(app, |status| {
                        status.downloaded = downloaded as f64;
                        status.total = total.map(|value| value as f64);
                    });
                }
            },
            || publish(app, |status| status.phase = UpdatePhase::Verifying),
        )
        .await
        .map_err(|error| error.to_string())?;
    publish(app, |status| status.phase = UpdatePhase::Installing);
    let coordinator = app.state::<crate::ShutdownCoordinator>();
    if coordinator.started.swap(true, Ordering::AcqRel) {
        return Err("終了処理中です。".into());
    }
    coordinator.mark_terminal_start();
    let request = app.state::<zephium_app::Handle>().shutdown();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        crate::shutdown_receive_outcome(request.recv_until_deadline())
    })
    .await
    .map_err(|error| error.to_string())?;
    if !matches!(outcome, zephium_app::ShutdownOutcome::Clean) {
        coordinator.schedule_authorized_exit(app.clone(), 1);
        return Err("更新前の保存を完了できませんでした。".into());
    }
    if let Err(error) = update.install(bytes) {
        eprintln!("updater: installation failed after shutdown: {error}");
        let exit = app.clone();
        app.dialog()
            .message("更新を適用できませんでした。BDeckを起動し直してください。")
            .title("BDeckの更新")
            .show(move |_| {
                exit.state::<crate::ShutdownCoordinator>()
                    .schedule_authorized_exit(exit.clone(), 1);
            });
        return Err(error.to_string());
    }
    app.restart();
}

#[tauri::command]
#[specta::specta]
pub(crate) fn updater_status(caller: WebviewWindow) -> Result<UpdateStatus, String> {
    if !crate::authorize(&caller, crate::CallerPolicy::Main, "updater_status") {
        return Err("unavailable".into());
    }
    let state = caller.app_handle().state::<UpdateManager>();
    let result = state
        .status
        .lock()
        .map_err(|_| "unavailable".to_owned())?
        .clone();
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn updater_check(caller: WebviewWindow) -> Result<(), String> {
    if !crate::authorize(&caller, crate::CallerPolicy::Main, "updater_check")
        || crate::shutdown_started(caller.app_handle())
    {
        return Err("unavailable".into());
    }
    check(caller.app_handle().clone(), false);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn updater_install(caller: WebviewWindow) -> Result<(), String> {
    if !crate::authorize(&caller, crate::CallerPolicy::Main, "updater_install")
        || crate::shutdown_started(caller.app_handle())
    {
        return Err("unavailable".into());
    }
    install(caller.app_handle().clone());
    Ok(())
}
