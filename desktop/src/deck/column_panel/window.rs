pub(super) use super::lifecycle::{close, hide_if_closed, show};
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
        presentation: Mutex::new(super::presentation::Presentation::default()),
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
        if !matches!(
            event,
            tauri::WindowEvent::Moved(_)
                | tauri::WindowEvent::Resized(_)
                | tauri::WindowEvent::ScaleFactorChanged { .. }
                | tauri::WindowEvent::Focused(true)
        ) {
            return;
        }
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::WindowEvent::Moved(_)) {
            return;
        }
        let Some(panel) = app.get_webview_window(LABEL) else {
            return;
        };
        let snapshot = app
            .state::<Panel>()
            .presentation
            .lock()
            .ok()
            .and_then(|state| state.snapshot());
        let Some(snapshot) = snapshot else {
            return;
        };
        if matches!(event, tauri::WindowEvent::Focused(true)) && !snapshot.view.composer {
            let _ = close(&app, snapshot.revision);
            return;
        }
        let _ = resize(&app, snapshot.revision, crate::inner_logical(&panel).height);
        if snapshot.view.composer && matches!(event, tauri::WindowEvent::Focused(true)) {
            if !panel.is_visible().unwrap_or(false) {
                let _ = show(&app, snapshot.revision);
            } else {
                #[cfg(target_os = "macos")]
                if let Some(main) = app.get_webview_window(crate::MAIN_LABEL) {
                    let _ = super::macos::attach(&main, &panel);
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
    let previous = panel
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .snapshot();
    if !panel
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .allows(&view)
    {
        return if view.composer {
            Ok(())
        } else {
            Err("close composer before opening column settings".into())
        };
    }
    if !view.composer {
        if let Some(window) = &existed {
            if window.is_visible()?
                && previous
                    .as_ref()
                    .is_some_and(|current| current.view.id == view.id && !current.view.composer)
            {
                if let Some(previous) = previous {
                    close(app, previous.revision)?;
                }
                return Ok(());
            }
            window.hide()?;
        }
    }
    if existed.is_none() {
        build(app, &main, &panel.context)?;
    }
    let snapshot = panel
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .present(view)?;
    let updating_composer = previous
        .as_ref()
        .is_some_and(|previous| previous.revision == snapshot.revision);
    *panel.anchor.lock().map_err(|_| "unavailable")? = Some((x, y));
    if !updating_composer {
        *panel.placement.lock().map_err(|_| "unavailable")? = None;
    }
    let event = if updating_composer {
        "zephium:column-panel-view"
    } else {
        "zephium:column-panel-open"
    };
    crate::emit_to_privileged(app, LABEL, event, &Some(snapshot));
    Ok(())
}

pub(super) fn resize(app: &tauri::AppHandle, revision: u32, height: f64) -> Result<bool, String> {
    if !height.is_finite() || !(100.0..=700.0).contains(&height) {
        return Err("invalid panel height".into());
    }
    let main = app
        .get_webview_window(crate::MAIN_LABEL)
        .ok_or("unavailable")?;
    let window = app.get_webview_window(LABEL).ok_or("unavailable")?;
    let panel = app.state::<Panel>();
    let Some(snapshot) = panel
        .presentation
        .lock()
        .map_err(|_| "unavailable")?
        .snapshot_at(revision)
    else {
        return Ok(false);
    };
    if main.is_minimized().map_err(|error| error.to_string())? {
        return Ok(false);
    }
    let requested_width = if snapshot.view.composer {
        f64::from(snapshot.view.width)
    } else {
        WIDTH
    };
    let viewport = crate::inner_logical(&main);
    let width = requested_width.min((viewport.width - 16.0).max(1.0));
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
        let placement = placement.constrained((size.width, size.height), width);
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
    place().map_err(|error| error.to_string())?;
    Ok(true)
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
            let current = observed
                .app_handle()
                .state::<Panel>()
                .presentation
                .lock()
                .ok()
                .and_then(|state| state.snapshot());
            if let Some(current) = current {
                let _ = close(observed.app_handle(), current.revision);
            }
        }
        tauri::WindowEvent::Focused(false)
            if parent.is_focused().unwrap_or(false) || !application_active() =>
        {
            let current = observed
                .app_handle()
                .state::<Panel>()
                .presentation
                .lock()
                .ok()
                .and_then(|state| state.snapshot());
            if let Some(current) = current.filter(|current| !current.view.composer) {
                let _ = close(observed.app_handle(), current.revision);
            }
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

pub(super) fn application_active() -> bool {
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
