use super::{Panel, PanelView};
use std::sync::Mutex;
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

pub(crate) const LABEL: &str = "column-panel";
const WIDTH: f64 = 336.0;
const HEIGHT: f64 = 220.0;

pub(super) struct Context {
    url: tauri::Url,
    #[cfg(target_os = "windows")]
    directory: std::path::PathBuf,
    #[cfg(target_os = "windows")]
    attempted: std::sync::Arc<std::sync::atomic::AtomicU8>,
    #[cfg(target_os = "windows")]
    runtime_update: crate::platform::imp::RuntimeUpdateCallback,
}

pub(crate) fn install(
    main: &WebviewWindow,
    url: tauri::Url,
    #[cfg(target_os = "windows")] directory: std::path::PathBuf,
    #[cfg(target_os = "windows")] attempted: std::sync::Arc<std::sync::atomic::AtomicU8>,
    #[cfg(target_os = "windows")] runtime_update: crate::platform::imp::RuntimeUpdateCallback,
) {
    main.app_handle().manage(Panel {
        view: Mutex::new(None),
        anchor: Mutex::new(None),
        placement: Mutex::new(None),
        context: Context {
            url,
            #[cfg(target_os = "windows")]
            directory,
            #[cfg(target_os = "windows")]
            attempted,
            #[cfg(target_os = "windows")]
            runtime_update,
        },
    });
    let app = main.app_handle().clone();
    main.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
                | tauri::WindowEvent::Focused(true)
        ) {
            if let Some(panel) = app.get_webview_window(LABEL) {
                let composer = app
                    .state::<Panel>()
                    .view
                    .lock()
                    .ok()
                    .and_then(|view| view.as_ref().map(|view| view.composer))
                    == Some(true);
                if composer {
                    let _ = resize(&app, crate::inner_logical(&panel).height);
                } else {
                    let _ = panel.hide();
                }
            }
        }
    });
}

pub(super) fn open(
    app: &tauri::AppHandle,
    view: PanelView,
    x: f64,
    y: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    if crate::shutdown_started(app) {
        return Err("unavailable".into());
    }
    let main = app
        .get_webview_window(crate::MAIN_LABEL)
        .ok_or("unavailable")?;
    let panel = app.state::<Panel>();
    let existed = app.get_webview_window(LABEL);
    let mut updating_composer = false;
    if let Some(window) = &existed {
        let same = panel
            .view
            .lock()
            .map_err(|_| "unavailable")?
            .as_ref()
            .is_some_and(|current| current.id == view.id);
        updating_composer = same && view.composer && window.is_visible()?;
        if same && window.is_visible()? && !view.composer {
            window.hide()?;
            return Ok(());
        }
    }
    if !view.composer {
        if let Some(window) = &existed {
            window.hide()?;
        }
    }
    *panel.view.lock().map_err(|_| "unavailable")? = Some(view.clone());
    if existed.is_none() {
        build(app, &main, &panel.context)?;
    }
    *panel.anchor.lock().map_err(|_| "unavailable")? = Some((x, y));
    *panel.placement.lock().map_err(|_| "unavailable")? = None;
    let event = if updating_composer {
        "zephium:column-panel-view"
    } else {
        "zephium:column-panel-open"
    };
    crate::emit_to_privileged(app, LABEL, event, &Some(view));
    Ok(())
}

pub(super) fn close(app: &tauri::AppHandle) -> Result<(), String> {
    let composer = app
        .state::<Panel>()
        .view
        .lock()
        .ok()
        .and_then(|view| view.as_ref().map(|view| view.composer))
        == Some(true);
    if composer {
        let shell = app.state::<zephium_app::Handle>();
        let _ = crate::dispatch_operation(
            app,
            &shell,
            zephium_app::Command::Run("composer.close".into()),
        );
    }
    app.get_webview_window(LABEL)
        .ok_or("unavailable")?
        .hide()
        .map_err(|error| error.to_string())
}

pub(super) fn resize(app: &tauri::AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() || !(100.0..=700.0).contains(&height) {
        return Err("invalid panel height".into());
    }
    let main = app
        .get_webview_window(crate::MAIN_LABEL)
        .ok_or("unavailable")?;
    let window = app.get_webview_window(LABEL).ok_or("unavailable")?;
    let panel = app.state::<Panel>();
    let width = panel
        .view
        .lock()
        .map_err(|_| "unavailable")?
        .as_ref()
        .filter(|view| view.composer)
        .map(|view| f64::from(view.width))
        .unwrap_or(WIDTH);
    let (x, y) = panel
        .anchor
        .lock()
        .map_err(|_| "unavailable")?
        .ok_or("unavailable")?;
    let place = || -> Result<(), tauri::Error> {
        let scale = main.scale_factor()?;
        let origin = main.inner_position()?.to_logical::<f64>(scale);
        let size = crate::inner_logical(&main);
        let placement = {
            let mut placement = panel
                .placement
                .lock()
                .map_err(|_| tauri::Error::WindowNotFound)?;
            *placement.get_or_insert_with(|| {
                super::placement::Placement::new((x, y), (size.width, size.height), width, height)
            })
        };
        let height = placement.height(height);
        let left = placement.left;
        let top = placement.top;
        let current_size = window.inner_size()?.to_logical::<f64>(scale);
        if (current_size.width - width).abs() > 0.5 || (current_size.height - height).abs() > 0.5 {
            window.set_size(tauri::LogicalSize::new(width, height))?;
        }
        let current = window.outer_position()?.to_logical::<f64>(scale);
        let position = tauri::LogicalPosition::new(origin.x + left, origin.y + top);
        if (current.x - position.x).abs() > 0.5 || (current.y - position.y).abs() > 0.5 {
            window.set_position(position)?;
        }
        Ok(())
    };
    place().map_err(|error| error.to_string())
}

fn build(
    app: &tauri::AppHandle,
    main: &WebviewWindow,
    context: &Context,
) -> Result<WebviewWindow, Box<dyn std::error::Error>> {
    let builder = WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::External(tauri::Url::parse(crate::PRIVILEGED_BOOTSTRAP_URL)?),
    )
    .title("カラムの設定")
    .inner_size(WIDTH, HEIGHT)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(true)
    .visible(false)
    .skip_taskbar(true)
    .incognito(true)
    .general_autofill_enabled(false)
    .devtools(cfg!(debug_assertions))
    .initialization_script_for_all_frames(zephium_engine::PAGE_PRINT_DENY_SCRIPT)
    .on_navigation({
        let url = context.url.clone();
        move |target| target.as_str() == crate::PRIVILEGED_BOOTSTRAP_URL || target == &url
    })
    .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
    .on_web_resource_request(|_, response| {
        crate::harden_privileged_headers(response.headers_mut())
    });
    #[cfg(target_os = "macos")]
    let builder = builder.parent(main)?;
    #[cfg(target_os = "windows")]
    let builder = builder
        .owner(main)?
        .data_directory(context.directory.clone())
        .additional_browser_args(crate::PRIVILEGED_WEBVIEW2_BROWSER_ARGS);
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    let builder = builder.on_download(|_, _| false);
    #[cfg(target_os = "windows")]
    context.attempted.fetch_or(
        crate::PRIVILEGED_COLUMN_ENVIRONMENT,
        std::sync::atomic::Ordering::Release,
    );
    let window = builder.build()?;
    #[cfg(target_os = "macos")]
    if !crate::platform::imp::harden_privileged(&window) {
        window.destroy()?;
        return Err("column panel hardening failed".into());
    }
    #[cfg(target_os = "windows")]
    if let Err(error) = crate::platform::imp::harden_privileged(
        &window,
        &context.directory,
        context.runtime_update.clone(),
    ) {
        crate::request_unrecoverable_native_failure(app, "column panel hardening failed");
        return Err(error.into());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    crate::platform::imp::harden_privileged(&window)?;
    let observed = window.clone();
    let parent = main.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = close(observed.app_handle());
        }
        tauri::WindowEvent::Focused(false)
            if parent.is_focused().unwrap_or(false) || !application_active() =>
        {
            let composer = observed
                .app_handle()
                .state::<Panel>()
                .view
                .lock()
                .ok()
                .and_then(|view| view.as_ref().map(|view| view.composer))
                == Some(true);
            if composer {
                return;
            }
            let _ = observed.hide();
        }
        #[cfg(target_os = "windows")]
        tauri::WindowEvent::Destroyed => {
            let _ = crate::platform::imp::remove_privileged_version_observer(LABEL);
        }
        _ => {}
    });
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        use raw_window_handle::HasWindowHandle;
        if !zephium_engine::attach_composer_parent(window.window_handle()?.as_raw()) {
            window.destroy()?;
            return Err("composer surface attachment failed".into());
        }
    }
    window.navigate(context.url.clone())?;
    Ok(window)
}

fn application_active() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2::MainThreadMarker::new().is_some_and(|marker| {
            objc2_app_kit::NSApplication::sharedApplication(marker).isActive()
        })
    }
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId,
        };
        let mut process = 0;
        // The OS owns the queried window; only its process ID is copied.
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut process));
        }
        process == std::process::id()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}
