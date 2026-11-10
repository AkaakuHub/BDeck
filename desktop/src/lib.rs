//! Composition root: the only crate that knows Tauri. Wires the dependency graph
//! (window -> chrome positioning, engine, shell) and the command surface.

mod updater;
mod deck;
mod diagnostics;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod external_links;
mod material;
mod memory_pressure;
mod platform;
#[cfg(target_os = "windows")]
mod privileged_runtime_windows;
mod profiles;
#[cfg(any(target_os = "macos", target_os = "windows", test))]
mod renderer_recovery;
mod startup_alert;
mod webext;
mod windows_privacy;

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use raw_window_handle::HasWindowHandle;
use serde::Serialize;
use tauri::{Manager, State, WebviewWindow};
use tauri_specta::collect_commands;

use zephium_app::{
    ChromePresentation, ChromePresentationCallback, ChromePresentationDispatch, Command, EmitFn,
    Handle, SharedChrome, ShellTerminalFailureCallback, ShutdownOutcome,
};
use zephium_core::geometry::Size;
use zephium_core::ids::ItemId;
use zephium_core::ports::engine::Engine as _;
use zephium_core::ports::store::{Store as _, StoreShutdownOutcome};
use zephium_engine::{MainThreadDispatch, WebviewEngine};
use zephium_ipc::Projection;
use zephium_store::SqliteStore;

macro_rules! diagnostic {
    ($($argument:tt)*) => {{
        write_diagnostic(format_args!($($argument)*));
    }};
}

static APP_STARTED: OnceLock<std::time::Instant> = OnceLock::new();
static APP_STORE: OnceLock<Arc<SqliteStore>> = OnceLock::new();
// Native browser work runs on these workers; the larger stack keeps nested
// WebView callbacks bounded.
const ASYNC_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
static ASYNC_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static AUTH_DENIAL_LOGS_REMAINING: AtomicUsize = AtomicUsize::new(16);
static NAVIGATION_DENIAL_LOGS_REMAINING: AtomicUsize = AtomicUsize::new(16);
static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);
static NATIVE_APPEARANCE: AtomicU8 = AtomicU8::new(APPEARANCE_SYSTEM);
const APPEARANCE_SYSTEM: u8 = 0;
const APPEARANCE_LIGHT: u8 = 1;
const APPEARANCE_DARK: u8 = 2;

const EVENT_ITEMS: &str = "zephium:items";
const EVENT_TAB: &str = "zephium:tab";
const EVENT_PRESENTATION_TAB: &str = "zephium:presentation-tab";
const PRE_SHELL_CLEANUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

const PRIVILEGED_PERMISSIONS_POLICY: &str = "accelerometer=(), attribution-reporting=(), autoplay=(), browsing-topics=(), camera=(), clipboard-read=(), clipboard-write=(self), compute-pressure=(), display-capture=(), document-domain=(), encrypted-media=(), fullscreen=(), gamepad=(), geolocation=(), gyroscope=(), hid=(), idle-detection=(), join-ad-interest-group=(), local-fonts=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), private-state-token-issuance=(), private-state-token-redemption=(), publickey-credentials-get=(), run-ad-auction=(), screen-wake-lock=(), serial=(), speaker-selection=(), storage-access=(), sync-xhr=(), unload=(), usb=(), web-share=(), window-management=(), xr-spatial-tracking=()";
const PRIVILEGED_BOOTSTRAP_URL: &str = "about:blank";
#[cfg(any(target_os = "windows", test))]
const PRIVILEGED_WEBVIEW2_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI";

#[derive(Clone)]
struct ShutdownCoordinator {
    started: Arc<AtomicBool>,
    terminal_started: Arc<AtomicBool>,
    startup_admission: Arc<Mutex<()>>,
    terminal_failure: Arc<AtomicBool>,
    authorized_exit_code: Arc<AtomicI32>,
    watchdog: Arc<HardExitWatchdog>,
}

struct StartupOwner<T> {
    inner: Arc<Mutex<Option<Arc<T>>>>,
}

impl<T> Clone for StartupOwner<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for StartupOwner<T> {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

impl<T> StartupOwner<T> {
    fn install(&self, value: Arc<T>) -> bool {
        let mut slot = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if slot.is_some() {
            return false;
        }
        *slot = Some(value);
        true
    }

    fn take(&self) -> Option<Arc<T>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    fn transfer_to(&self, owner: &Arc<T>) -> bool {
        let mut slot = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !slot
            .as_ref()
            .is_some_and(|candidate| Arc::ptr_eq(candidate, owner))
        {
            return false;
        }
        slot.take();
        true
    }
}

type StartupEngine = StartupOwner<WebviewEngine>;
/// Retains the storage actor from admission until the suspended Shell becomes
/// the sole ordered-cleanup owner.
type StartupStore = StartupOwner<SqliteStore>;

/// Complete move/temporary-owner set for a terminal failure before Shell has
/// become the sole lifecycle coordinator. Direct owners are retained when a
/// one-shot startup slot refuses installation, so no native worker is shut
/// down synchronously from Tauri's setup callback.
#[derive(Default)]
struct TerminalStartupResources {
    shell: Option<Handle>,
    engine_owner: Option<StartupEngine>,
    direct_store: Option<Arc<SqliteStore>>,
    store_owner: Option<StartupStore>,
}

#[derive(Default)]
struct ClaimedTerminalStartupResources {
    engine: Option<Arc<WebviewEngine>>,
    direct_store: Option<Arc<SqliteStore>>,
    retained_store: Option<Arc<SqliteStore>>,
}

impl TerminalStartupResources {
    fn claim(self) -> (Option<Handle>, ClaimedTerminalStartupResources) {
        let Self {
            shell,
            engine_owner,
            direct_store,
            store_owner,
        } = self;
        (
            shell,
            ClaimedTerminalStartupResources {
                engine: engine_owner.and_then(|owner| owner.take()),
                direct_store,
                retained_store: store_owner.and_then(|owner| owner.take()),
            },
        )
    }
}

impl ClaimedTerminalStartupResources {
    fn is_empty(&self) -> bool {
        self.engine.is_none() && self.direct_store.is_none() && self.retained_store.is_none()
    }
}

#[derive(Default)]
struct HardExitWatchdog {
    prepared: AtomicBool,
    deadline: Mutex<Option<std::time::Instant>>,
    changed: std::sync::Condvar,
}

#[derive(Clone)]
struct UiStartupGate {
    expected_url: Arc<Mutex<tauri::Url>>,
    document_loaded: Arc<AtomicBool>,
    frontend_ready: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
}

// WebView2 cold starts compete with runtime servicing and antivirus scanning.
// Keep initialization bounded without terminating an otherwise healthy launch.
#[cfg(target_os = "windows")]
const UI_INITIALIZATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
#[cfg(not(target_os = "windows"))]
const UI_INITIALIZATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl UiStartupGate {
    fn new(expected_url: tauri::Url) -> Self {
        Self {
            expected_url: Arc::new(Mutex::new(expected_url)),
            document_loaded: Arc::new(AtomicBool::new(false)),
            frontend_ready: Arc::new(AtomicBool::new(false)),
            visible: Arc::new(AtomicBool::new(false)),
        }
    }

    fn expected(&self) -> tauri::Url {
        self.expected_url
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn mark_document_loaded(&self, window: &WebviewWindow, loaded_url: &tauri::Url) {
        if loaded_url != &self.expected() {
            return;
        }
        self.document_loaded.store(true, Ordering::Release);
        self.show_if_ready(window);
    }

    fn mark_frontend_ready(&self, window: &WebviewWindow) -> bool {
        let Ok(current_url) = window.url() else {
            return false;
        };
        if current_url != self.expected() {
            return false;
        }
        self.frontend_ready.store(true, Ordering::Release);
        self.show_if_ready(window);
        true
    }

    fn show_if_ready(&self, window: &WebviewWindow) {
        if !self.document_loaded.load(Ordering::Acquire)
            || !self.frontend_ready.load(Ordering::Acquire)
        {
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if renderer_recovery::ready(window) && !platform::imp::set_chrome_hidden(window, false) {
            request_startup_failure(window.app_handle(), "could not reveal recovered chrome");
            return;
        }
        if self
            .visible
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        if let Err(error) = show_initialized_main_window(window) {
            request_startup_failure(
                window.app_handle(),
                format_args!("could not show initialized main window: {error}"),
            );
        } else {
            if let Some(started) = APP_STARTED.get() {
                diagnostic!(
                    "startup: main document initialized and window shown after {} ms",
                    started.elapsed().as_millis()
                );
            }
            on_main_window_mapped(window);
        }
    }

    /// Whether the main window may navigate to `url` during startup. Once the
    /// first document has been shown, the bootstrap blank page is gone for
    /// good. It remains in the back-forward list, so history navigation of
    /// any kind ends here.
    fn permits_navigation(&self, url: &tauri::Url) -> bool {
        if !ui_navigation_allowed(url) {
            return false;
        }
        if url.as_str() == PRIVILEGED_BOOTSTRAP_URL {
            return !self.visible.load(Ordering::Acquire);
        }
        true
    }

    fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn prepare_recovery(&self, window: &WebviewWindow) -> Option<tauri::Url> {
        self.document_loaded.store(false, Ordering::Release);
        self.frontend_ready.store(false, Ordering::Release);
        platform::imp::set_chrome_hidden(window, true).then(|| self.expected())
    }
}

/// DWM can discard the backdrop installed while the window was hidden.
/// Restore it at the first reveal, not at some later focus event, and use
/// the person's saved appearance rather than the operating-system default.
#[cfg(target_os = "windows")]
pub(crate) fn restore_window_material(window: &WebviewWindow) -> bool {
    let appearance = APP_STORE
        .get()
        .and_then(|store| store.app_setting("appearance"))
        .unwrap_or_else(|| "system".to_owned());
    let dark = matches!(
        resolved_native_theme(window, &appearance),
        tauri::Theme::Dark
    );
    platform::imp::apply_material(window, dark)
}

#[cfg(target_os = "windows")]
fn on_main_window_mapped(window: &WebviewWindow) {
    if !restore_window_material(window) {
        diagnostic!("material: main-window backdrop could not be restored after initial reveal");
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn on_main_window_mapped(_window: &WebviewWindow) {}

fn show_initialized_main_window(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Tao implements Window::show as gtk_window.show_all(), which also
        // remaps raw content children that the native stage deliberately hid
        // while the trusted frontend initialized. The Linux composition root
        // has already marked chrome and its container visible; reveal only
        // the top-level widget and preserve every stage-owned child state.
        platform::imp::show_initialized_top_level(window).map_err(|error| error.to_string())
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    {
        window.show().map_err(|error| error.to_string())
    }
}

const NO_AUTHORIZED_EXIT_CODE: i32 = -1;

pub(crate) fn write_diagnostic(arguments: std::fmt::Arguments<'_>) {
    // `eprintln!` panics when stderr writes fail. Several callers are native
    // Objective-C/COM/GTK callbacks where unwinding is forbidden, so keep
    // diagnostics best-effort and make failure unobservable to control flow.
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    write_diagnostic_to(&mut stderr, arguments);
}

fn write_diagnostic_to(writer: &mut dyn std::io::Write, arguments: std::fmt::Arguments<'_>) {
    static SESSION_STARTED: std::sync::OnceLock<u128> = std::sync::OnceLock::new();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_millis());
    let session = SESSION_STARTED.get_or_init(|| millis);
    let _ = write!(
        writer,
        "ts_ms={millis} session={session}-{} ",
        std::process::id()
    );
    let _ = writer.write_fmt(arguments);
    let _ = writer.write_all(b"\n");
}

fn cleanup_pre_shell_resources_until(
    resources: ClaimedTerminalStartupResources,
    deadline: std::time::Instant,
) {
    let ClaimedTerminalStartupResources {
        engine,
        direct_store,
        retained_store,
    } = resources;

    for store in [direct_store, retained_store].into_iter().flatten() {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.shutdown_until(deadline)
        })) {
            Ok(StoreShutdownOutcome::Clean) => {}
            Ok(StoreShutdownOutcome::RetryableFailure) => {
                write_diagnostic(format_args!(
                    "startup: storage rejected terminal cleanup before the deadline"
                ));
            }
            Ok(StoreShutdownOutcome::Unclean) => write_diagnostic(format_args!(
                "startup: storage termination could not be proven before process exit"
            )),
            Err(_) => write_diagnostic(format_args!("startup: storage terminal cleanup panicked")),
        }
    }

    // The engine Arc remains alive until its exact completion callback has
    // been observed.
    let native_cleanup = engine.map(|engine| {
        let (native_done, native_wait) = std::sync::mpsc::sync_channel(1);
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.shutdown(Box::new(move |clean| {
                let _ = native_done.send(clean);
            }));
        }))
        .is_err()
        {
            write_diagnostic(format_args!(
                "startup: pre-shell native engine cleanup admission panicked"
            ));
        }
        (engine, native_wait)
    });
    if let Some((_engine, native_wait)) = native_cleanup {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if native_wait.recv_timeout(remaining) != Ok(true) {
            write_diagnostic(format_args!(
                "startup: pre-shell native engine cleanup was not proven before the deadline"
            ));
        }
    }
}

fn cleanup_supplemental_startup_resources(direct_store: Option<Arc<SqliteStore>>) {
    if direct_store.is_none() {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let now = std::time::Instant::now();
        let deadline = now.checked_add(PRE_SHELL_CLEANUP_TIMEOUT).unwrap_or(now);
        cleanup_pre_shell_resources_until(
            ClaimedTerminalStartupResources {
                direct_store,
                ..ClaimedTerminalStartupResources::default()
            },
            deadline,
        );
    });
}

impl HardExitWatchdog {
    fn prepare(self: &Arc<Self>) -> std::io::Result<()> {
        if self
            .prepared
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }

        let watchdog = self.clone();
        let spawned = std::thread::Builder::new()
            .name("zephium-hard-exit".into())
            .spawn(move || {
                let mut deadline = watchdog
                    .deadline
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                while deadline.is_none() {
                    deadline = watchdog
                        .changed
                        .wait(deadline)
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                }
                let armed_deadline = (*deadline).unwrap_or_else(std::time::Instant::now);
                drop(deadline);

                let remaining =
                    armed_deadline.saturating_duration_since(std::time::Instant::now());
                if !remaining.is_zero() {
                    std::thread::sleep(remaining);
                }
                write_diagnostic(format_args!(
                    "shutdown: process exit did not complete within the hard deadline; forcing unsuccessful termination"
                ));
                std::process::exit(1);
            });
        if let Err(error) = spawned {
            self.prepared.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }

    fn arm(&self) -> bool {
        if !self.prepared.load(Ordering::Acquire) {
            return false;
        }
        let mut deadline = self
            .deadline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if deadline.is_none() {
            *deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(12));
            self.changed.notify_one();
        }
        true
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self {
            started: Arc::new(AtomicBool::new(false)),
            terminal_started: Arc::new(AtomicBool::new(false)),
            startup_admission: Arc::new(Mutex::new(())),
            terminal_failure: Arc::new(AtomicBool::new(false)),
            authorized_exit_code: Arc::new(AtomicI32::new(NO_AUTHORIZED_EXIT_CODE)),
            watchdog: Arc::new(HardExitWatchdog::default()),
        }
    }
}

impl ShutdownCoordinator {
    /// Linearizes every terminal-start publication against the Shell's sole
    /// startup-admission transition. The atomic mirror keeps later setup
    /// gates read-only and lock-free.
    fn mark_terminal_start(&self) {
        let admission = self
            .startup_admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.terminal_started.store(true, Ordering::Release);
        drop(admission);
    }

    fn try_startup_admission(&self, admit: impl FnOnce() -> bool) -> bool {
        let admission = self
            .startup_admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let admitted = !self.terminal_started.load(Ordering::Acquire) && admit();
        drop(admission);
        admitted
    }

    fn try_admit_shell(&self, shell: &Handle) -> bool {
        self.try_startup_admission(|| shell.admit_startup())
    }

    fn prepare_hard_exit_watchdog(&self) -> std::io::Result<()> {
        self.watchdog.prepare()
    }

    /// Publishes terminal intent and arms the independent process deadline
    /// before actor-owned cleanup begins. This does not claim the teardown
    /// single-flight gate, so the correlated Shell request remains authoritative.
    fn signal_terminal_start(&self, app: &tauri::AppHandle) {
        self.mark_terminal_start();
        self.terminal_failure.store(true, Ordering::Release);
        if !self.arm_hard_exit_watchdog() {
            write_diagnostic(format_args!(
                "shutdown: terminal signal could not arm the prepared hard-exit watchdog"
            ));
            self.schedule_authorized_exit(app.clone(), 1);
        }
    }

    fn terminal_started(&self) -> bool {
        self.terminal_started.load(Ordering::Acquire)
    }

    fn request(&self, app: tauri::AppHandle, shell: Handle) {
        self.mark_terminal_start();
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        if !self.arm_hard_exit_watchdog() {
            self.terminal_failure.store(true, Ordering::Release);
            write_diagnostic(format_args!(
                "shutdown: hard-exit watchdog was not prepared; requesting immediate unsuccessful event-loop exit"
            ));
            self.schedule_authorized_exit(app, 1);
            return;
        }
        self.request_after_work(app, shell);
    }

    fn request_after_work(&self, app: tauri::AppHandle, shell: Handle) {
        let completion = shell.shutdown();
        let coordinator = self.clone();
        // SQLite and the shell actor are blocking by design. Wait away from
        // the event loop, then schedule a permitted exit back onto it.
        tauri::async_runtime::spawn_blocking(move || {
            let outcome = shutdown_receive_outcome(completion.recv_until_deadline());
            coordinator.finish_requested_shutdown(app, outcome);
        });
    }

    fn finish_requested_shutdown(&self, app: tauri::AppHandle, outcome: ShutdownOutcome) {
        match outcome {
            ShutdownOutcome::RetryableFailure => {
                // The shell has not started native teardown, so an embedding
                // host could safely retry. A desktop close is terminal: exit
                // non-zero instead of leaving an unclosable process.
                diagnostic!(
                    "shutdown: final session durability was not proven before the deadline; exiting unsuccessfully"
                );
            }
            ShutdownOutcome::Clean => {}
            ShutdownOutcome::Unclean => {
                // The actor may be dead or native teardown may already be
                // partial, so resuming is unsafe.
                diagnostic!("shutdown: clean completion was not proven; exiting unsuccessfully");
            }
        }
        // Startup admission failure is sticky. Even a subsequently clean
        // shell/store teardown cannot turn failed initialization successful.
        let exit_code =
            coordinated_exit_code(outcome, self.terminal_failure.load(Ordering::Acquire));
        self.schedule_authorized_exit(app, exit_code);
    }

    fn request_terminal_startup_failure(
        &self,
        app: tauri::AppHandle,
        resources: TerminalStartupResources,
    ) {
        let TerminalStartupResources {
            shell,
            engine_owner,
            mut direct_store,
            store_owner,
        } = resources;
        self.mark_terminal_start();
        self.terminal_failure.store(true, Ordering::Release);
        if let Some(shell) = shell {
            cleanup_supplemental_startup_resources(direct_store.take());
            // Once the shell exists it is the sole authority for the ordered
            // Store -> native teardown protocol.
            // `request` observes the sticky failure bit and exits non-zero
            // even when cleanup is otherwise clean.
            self.request(app, shell);
            return;
        }
        if self.started.swap(true, Ordering::AcqRel) {
            // The winning callback owns every managed temporary slot. A
            // losing callback can still carry an uninstalled direct owner;
            // reap only that supplemental owner instead of dropping it or
            // racing the winner for shared slots.
            cleanup_supplemental_startup_resources(direct_store.take());
            return;
        }
        // Claim the single-flight gate before taking any temporary owner.
        // Otherwise concurrent failure callbacks could each remove one
        // resource while only one callback remains authorized to reap it.
        let (_, resources) = TerminalStartupResources {
            shell: None,
            engine_owner,
            direct_store,
            store_owner,
        }
        .claim();
        if !self.arm_hard_exit_watchdog() {
            write_diagnostic(format_args!(
                "startup: hard-exit watchdog was not prepared; continuing bounded cleanup before unsuccessful exit"
            ));
        }
        if !resources.is_empty() {
            let deadline = std::time::Instant::now() + PRE_SHELL_CLEANUP_TIMEOUT;
            let coordinator = self.clone();
            tauri::async_runtime::spawn_blocking(move || {
                cleanup_pre_shell_resources_until(resources, deadline);
                coordinator.schedule_authorized_exit(app, 1);
            });
        } else {
            // Storage admission itself failed, so there is no background
            // resource to reap. Request event-loop exit directly; do not
            // return an error through Tauri's native Ready callback.
            self.authorized_exit_code.store(1, Ordering::Release);
            app.exit(1);
        }
    }

    fn request_unrecoverable_native_failure(&self, app: tauri::AppHandle) {
        // Native authority has already become inconsistent, so asking the
        // shell to drive that same engine through ordered teardown is unsafe.
        // Still return through Tauri's event loop: App drop and Windows
        // `run_return` can then release/prove privileged environments before
        // the process exits unsuccessfully.
        if !self.begin_unrecoverable_native_failure() {
            return;
        }
        if !self.arm_hard_exit_watchdog() {
            write_diagnostic(format_args!(
                "security: hard-exit watchdog was not prepared; requesting immediate unsuccessful event-loop exit"
            ));
        }
        self.schedule_authorized_exit(app, 1);
    }

    fn begin_unrecoverable_native_failure(&self) -> bool {
        self.mark_terminal_start();
        self.terminal_failure.store(true, Ordering::Release);
        // Share the exact single-flight gate with ordinary shutdown. If a
        // durability barrier already owns teardown, make its result nonzero
        // without overtaking its store/native completion.
        !self.started.swap(true, Ordering::AcqRel)
    }

    fn schedule_authorized_exit(&self, app: tauri::AppHandle, exit_code: i32) {
        let exit_coordinator = self.clone();
        let exit_on_main = app.clone();
        if let Err(error) = app.run_on_main_thread(move || {
            // Authorize only this exact main-thread exit call. Publishing the
            // code on a worker before dispatch leaves a window where an
            // unrelated OS request can overtake the correlated result.
            let exit_code = if exit_coordinator.terminal_failure.load(Ordering::Acquire) {
                1
            } else {
                exit_code
            };
            exit_coordinator
                .authorized_exit_code
                .store(exit_code, Ordering::Release);
            exit_on_main.exit(exit_code);
        }) {
            write_diagnostic(format_args!(
                "shutdown: main-thread exit dispatch failed: {error}"
            ));
            // Keep the same invariant for the direct fallback: publish
            // immediately before the correlated request.
            let exit_code = if self.terminal_failure.load(Ordering::Acquire) {
                1
            } else {
                exit_code
            };
            self.authorized_exit_code
                .store(exit_code, Ordering::Release);
            app.exit(exit_code);
        }
    }

    fn arm_hard_exit_watchdog(&self) -> bool {
        self.watchdog.arm()
    }
}

fn shutdown_exit_code(outcome: ShutdownOutcome) -> i32 {
    if outcome == ShutdownOutcome::Clean {
        0
    } else {
        1
    }
}

fn coordinated_exit_code(outcome: ShutdownOutcome, terminal_failure: bool) -> i32 {
    if terminal_failure {
        1
    } else {
        shutdown_exit_code(outcome)
    }
}

fn exit_request_is_authorized(requested: Option<i32>, authorized: i32) -> bool {
    requested.is_some_and(|code| code == authorized && code != NO_AUTHORIZED_EXIT_CODE)
}

fn shutdown_receive_outcome(
    received: Result<ShutdownOutcome, std::sync::mpsc::RecvTimeoutError>,
) -> ShutdownOutcome {
    match received {
        Ok(outcome) => outcome,
        Err(error) => {
            // Timeout and disconnect are both terminal here. The actor either
            // exceeded the same end-to-end deadline it received at admission
            // or exited without proving native/private-data cleanup.
            diagnostic!("shutdown: shell actor did not acknowledge the barrier: {error}");
            ShutdownOutcome::Unclean
        }
    }
}

type SetupResult = Result<(), Box<dyn std::error::Error>>;

/// Tauri 2.11 turns a setup-hook `Err` into a panic from its runtime Ready
/// callback. On macOS that callback is entered from Objective-C, so release
/// `panic = "abort"` terminates the process before Tauri/native cleanup can
/// run. Keep environmental failures inside our own fallible transaction and
/// make the framework-facing hook unconditionally successful.
fn contain_tauri_setup_failure<E>(
    result: Result<(), E>,
    on_failure: impl FnOnce(E),
) -> SetupResult {
    if let Err(error) = result {
        on_failure(error);
    }
    Ok(())
}

fn request_startup_failure(app: &tauri::AppHandle, error: impl std::fmt::Display) {
    let error = error.to_string();
    let problem = startup_alert::StartupProblem::classify(&error);
    request_classified_startup_failure(app, problem, &error, &error);
}

/// A failure of the setup transaction: one that knows what the person can do
/// about it says so, any other is classified from its text.
fn request_setup_failure(app: &tauri::AppHandle, error: Box<dyn std::error::Error>) {
    match error.downcast::<startup_alert::StartupFailure>() {
        Ok(failure) => request_classified_startup_failure(
            app,
            failure.problem(),
            &failure.to_string(),
            failure.detail(),
        ),
        Err(error) => request_startup_failure(app, error),
    }
}

fn request_classified_startup_failure(
    app: &tauri::AppHandle,
    problem: startup_alert::StartupProblem,
    cause: &str,
    detail: &str,
) {
    write_diagnostic(format_args!(
        "startup: failed to initialize BDeck: {cause}"
    ));
    startup_alert::show_then(app, problem, detail, request_orderly_terminal_failure);
}

fn request_shell_terminal_failure(
    app: &tauri::AppHandle,
    coordinator: &ShutdownCoordinator,
    error: impl std::fmt::Display,
) {
    // This is deliberately first: even diagnostics, Tauri state lookup, or
    // async-runtime admission may fail after the actor has begun unwinding.
    coordinator.signal_terminal_start(app);
    write_diagnostic(format_args!(
        "runtime: terminal application shell failure: {error}"
    ));
    request_orderly_terminal_failure(app);
}

fn request_orderly_terminal_failure(app: &tauri::AppHandle) {
    let shell = app.try_state::<Handle>().map(|shell| shell.inner().clone());
    let engine_owner = if shell.is_none() {
        app.try_state::<StartupEngine>()
            .map(|engine| engine.inner().clone())
    } else {
        None
    };
    let store_owner = if shell.is_none() {
        app.try_state::<StartupStore>()
            .map(|store| store.inner().clone())
    } else {
        None
    };
    let Some(coordinator) = app.try_state::<ShutdownCoordinator>() else {
        // Builder installs this state before the event loop begins. Even if
        // that invariant is broken, retain the dependency-complete cleanup
        // path instead of abandoning managed workers on the event loop.
        write_diagnostic(format_args!("startup: shutdown coordinator is unavailable"));
        if let Some(shell) = shell {
            let completion = shell.shutdown();
            let exit_app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = completion.recv_until_deadline();
                exit_app.exit(1);
            });
        } else {
            request_pre_shell_cleanup_without_coordinator(
                app,
                TerminalStartupResources {
                    engine_owner,
                    store_owner,
                    ..TerminalStartupResources::default()
                },
            );
        }
        return;
    };
    coordinator.request_terminal_startup_failure(
        app.clone(),
        TerminalStartupResources {
            shell,
            engine_owner,
            store_owner,
            ..TerminalStartupResources::default()
        },
    );
}

/// Routes temporary startup owners into the same asynchronous,
/// dependency-ordered cleanup when the coordinator state is unavailable. This
/// helper is valid only before Shell construction; callers retain the normal
/// setup error so the outer containment callback can observe the already-owned
/// single-flight failure without starting a second teardown.
fn request_pre_shell_cleanup_without_coordinator(
    app: &tauri::AppHandle,
    resources: TerminalStartupResources,
) {
    let (shell, resources) = resources.claim();
    debug_assert!(shell.is_none());
    let exit_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let now = std::time::Instant::now();
        let deadline = now.checked_add(PRE_SHELL_CLEANUP_TIMEOUT).unwrap_or(now);
        cleanup_pre_shell_resources_until(resources, deadline);
        exit_app.exit(1);
    });
}

/// Losslessly routes a Store actor refused by the one-shot temporary owner
/// into the same asynchronous dependency cleanup. No Store join may run on
/// Tauri's native setup callback.
fn request_pre_shell_startup_failure_with_store(
    app: &tauri::AppHandle,
    error: impl std::fmt::Display,
    store: Arc<SqliteStore>,
) {
    write_diagnostic(format_args!(
        "startup: failed to initialize BDeck: {error}"
    ));
    let store_owner = app
        .try_state::<StartupStore>()
        .map(|owner| owner.inner().clone());
    let resources = TerminalStartupResources {
        direct_store: Some(store),
        store_owner,
        ..TerminalStartupResources::default()
    };
    let Some(coordinator) = app.try_state::<ShutdownCoordinator>() else {
        write_diagnostic(format_args!("startup: shutdown coordinator is unavailable"));
        request_pre_shell_cleanup_without_coordinator(app, resources);
        return;
    };
    coordinator.request_terminal_startup_failure(app.clone(), resources);
}

fn request_unrecoverable_native_failure(app: &tauri::AppHandle, reason: &str) {
    write_diagnostic(format_args!(
        "security: terminal native lifecycle failure: {reason}"
    ));
    let Some(coordinator) = app.try_state::<ShutdownCoordinator>() else {
        write_diagnostic(format_args!(
            "security: shutdown coordinator is unavailable"
        ));
        app.exit(1);
        return;
    };
    coordinator.request_unrecoverable_native_failure(app.clone());
}

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .error_handling(tauri_specta::ErrorHandlingMode::Throw)
        .commands(collect_commands![
            updater::updater_status,
            updater::updater_check,
            updater::updater_install,
            diagnostics::diagnostics_show_logs,
            tabs_bootstrap,
            tabs_activate,
            tabs_reload,
            tabs_back,
            tabs_forward,
            profiles::profiles_get,
            deck::accounts::profiles_create,
            deck::accounts::account_open,
            deck::accounts::account_delete,
            deck::portable::deck_settings_export,
            deck::portable::deck_settings_import,
            setting_get,
            setting_set,
            run_command,
            deck::composer::composer_open,
            deck::drag::deck_drag_preview,
            deck::column_panel::column_panel_open,
            deck::column_panel::column_panel_get,
            deck::column_panel::column_panel_ready,
            deck::column_panel::column_panel_resize,
            deck::column_panel::column_panel_close,
            deck::column_panel::column_panel_composer_sync,
            deck::column_panel::column_panel_action,
            deck::column_panel::column_panel_sync,
            webext::actions::extension_action_invoke,
            webext::web_extension_prepare,
            webext::web_extension_confirm,
            webext::web_extension_cancel,
            webext::web_extension_list,
            webext::web_extension_set_enabled,
            webext::web_extension_retry,
            webext::web_extension_answer_access,
            webext::web_extension_uninstall,
            webext::web_extension_set_access,
            webext::web_extension_open_options,
            webext::web_extension_choose_file,
            webext::web_extension_prepare_file,
            webext::web_extension_review_update,
            ui_ready,
        ])
        .typ::<zephium_ipc::WebExtensionAccessRequestView>()
        .typ::<zephium_ipc::ExtensionActionsView>()
        .typ::<zephium_ipc::ExtensionActionFailedView>()
        .typ::<zephium_ipc::ExtensionActionShortcutView>()
        .typ::<zephium_ipc::ItemsState>()
        .typ::<zephium_ipc::TabView>()
        .typ::<zephium_ipc::ProfilesView>()
        .typ::<zephium_ipc::LayoutState>()
        .typ::<zephium_ipc::OperationDisposition>()
        .typ::<zephium_core::deck::DeckAppearance>()
        .typ::<zephium_core::deck::account::AccountLoginState>()
        .typ::<zephium_core::deck::account::AccountIdentity>()
}

fn ui_navigation_allowed(url: &tauri::Url) -> bool {
    let clean_authority = url.username().is_empty() && url.password().is_none();
    // Privileged WebViews are constructed at the browser-generated empty
    // document, hardened natively, and only then navigated to the app origin.
    // Keep this exact: other about: URLs and even fragments/queries are not a
    // bootstrap document.
    let bootstrap = url.as_str() == PRIVILEGED_BOOTSTRAP_URL;
    #[cfg(target_os = "windows")]
    let bundled =
        url.scheme() == "http" && url.host_str() == Some("tauri.localhost") && url.port().is_none();
    #[cfg(not(target_os = "windows"))]
    let bundled =
        url.scheme() == "tauri" && url.host_str() == Some("localhost") && url.port().is_none();
    let development = cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("localhost")
        && url.port() == Some(1420);
    clean_authority && (bootstrap || bundled || development)
}

fn privileged_app_url(
    app: &tauri::App,
    configured: &tauri::WebviewUrl,
) -> tauri::Result<tauri::Url> {
    use tauri::utils::config::FrontendDist;

    #[cfg(target_os = "windows")]
    let fallback = "http://tauri.localhost";
    #[cfg(not(target_os = "windows"))]
    let fallback = "tauri://localhost";

    let base = if tauri::is_dev() {
        match app.config().build.dev_url.clone() {
            Some(url) => url,
            None => tauri::Url::parse(fallback).map_err(tauri::Error::InvalidUrl)?,
        }
    } else if let Some(FrontendDist::Url(url)) = app.config().build.frontend_dist.as_ref() {
        url.clone()
    } else {
        tauri::Url::parse(fallback).map_err(tauri::Error::InvalidUrl)?
    };

    resolve_privileged_target(&base, configured).map_err(Into::into)
}

fn resolve_privileged_target(
    base: &tauri::Url,
    configured: &tauri::WebviewUrl,
) -> std::io::Result<tauri::Url> {
    let target = match configured {
        tauri::WebviewUrl::App(path) if path.to_str() == Some("index.html") => base
            .join("browser.html")
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        tauri::WebviewUrl::App(path) => base
            .join(&path.to_string_lossy())
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        tauri::WebviewUrl::External(url) | tauri::WebviewUrl::CustomProtocol(url) => url.clone(),
        _ => {
            return Err(std::io::Error::other(
                "unsupported privileged application URL configuration",
            ));
        }
    };
    if !ui_navigation_allowed(&target) || target.as_str() == PRIVILEGED_BOOTSTRAP_URL {
        return Err(std::io::Error::other(format!(
            "privileged application URL is outside the navigation policy: {target}"
        )));
    }
    Ok(target)
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct PrivilegedRuntimeDirectories {
    main: std::path::PathBuf,
    column: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
fn prepare_privileged_runtime_directories(
    data_dir: &std::path::Path,
) -> std::io::Result<PrivilegedRuntimeDirectories> {
    let prepared =
        privileged_runtime_windows::prepare(data_dir, MAIN_LABEL, deck::column_panel::LABEL)?;
    Ok(PrivilegedRuntimeDirectories {
        main: prepared.main,
        column: prepared.column,
    })
}

#[cfg(target_os = "windows")]
fn cleanup_privileged_runtime_after_exit() -> bool {
    privileged_runtime_windows::cleanup_current_after_proven_exit()
}

fn navigation_lock() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("ui-navigation-lock")
        .on_navigation(|webview, url| {
            let allowed = if webview.label() == MAIN_LABEL {
                webview.try_state::<UiStartupGate>().map_or_else(
                    || ui_navigation_allowed(url),
                    |gate| gate.permits_navigation(url),
                )
            } else {
                ui_navigation_allowed(url)
            };
            if !allowed
                && NAVIGATION_DENIAL_LOGS_REMAINING
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                        value.checked_sub(1)
                    })
                    .is_ok()
            {
                diagnostic!(
                    "security: blocked privileged webview {} navigation to scheme={} host={}",
                    webview.label(),
                    url.scheme(),
                    url.host_str().unwrap_or("<none>")
                );
            }
            allowed
        })
        .build()
}

fn harden_privileged_headers(headers: &mut tauri::http::HeaderMap) {
    use tauri::http::HeaderValue;

    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static(PRIVILEGED_PERMISSIONS_POLICY),
    );
    headers.insert("Referrer-Policy", HeaderValue::from_static("no-referrer"));
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-DNS-Prefetch-Control", HeaderValue::from_static("off"));
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
}

/// Delivers a projection directly to one privileged WebView. Generic Tauri
/// event listening accepts a caller-selected target, so capabilities cannot
/// stop a compromised panel from subscribing to the main window's events.
/// Native eval is label-scoped and the payload is serialized as JSON.
fn try_emit_to_privileged<T: Serialize>(
    app: &tauri::AppHandle,
    label: &str,
    event: &str,
    payload: &T,
) -> bool {
    let (Ok(event), Ok(payload)) = (serde_json::to_string(event), serde_json::to_string(payload))
    else {
        diagnostic!("projection: failed to serialize privileged event");
        return false;
    };
    let Some(window) = app.get_webview_window(label) else {
        return false;
    };
    let script = format!("window.dispatchEvent(new CustomEvent({event},{{detail:{payload}}}));");
    if let Err(error) = window.eval(&script) {
        diagnostic!("projection: delivery to {label} failed: {error}");
        return false;
    }
    true
}

fn emit_to_privileged<T: Serialize>(app: &tauri::AppHandle, label: &str, event: &str, payload: &T) {
    let _ = try_emit_to_privileged(app, label, event, payload);
}

/// Applies one exact revision-bearing tab projection and verifies the
/// privileged DOM state in the same JavaScript evaluation. The callback is a
/// lifecycle fact only; the shell independently revalidates id, URL and native
/// navigation identity before revealing raw content.
pub(crate) fn apply_chrome_presentation(
    window: &WebviewWindow,
    presentation: ChromePresentation,
    done: ChromePresentationCallback,
) -> ChromePresentationDispatch {
    if presentation.tab.id != presentation.id.to_string()
        || presentation.tab.url.as_deref() != Some(presentation.url.as_str())
    {
        return ChromePresentationDispatch::Rejected;
    }
    let Ok(event) = serde_json::to_string(EVENT_PRESENTATION_TAB) else {
        return ChromePresentationDispatch::Rejected;
    };
    let Ok(payload) = serde_json::to_string(&presentation.tab) else {
        return ChromePresentationDispatch::Rejected;
    };
    let active = presentation.active.map(|id| id.to_string());
    let Ok(active) = serde_json::to_string(&active) else {
        return ChromePresentationDispatch::Rejected;
    };
    let nonce = format!(
        "zephium-presentation-v1:{}:{:032x}",
        presentation.id,
        presentation.navigation.into_raw()
    );
    let Ok(serialized_nonce) = serde_json::to_string(&nonce) else {
        return ChromePresentationDispatch::Rejected;
    };
    let script = format!(
        r#"(() => {{
  "use strict";
  const rejected = "zephium-presentation-rejected";
  try {{
    const tab = {payload};
    const active = {active};
    window.dispatchEvent(new CustomEvent({event}, {{ detail: {{ tab, active }} }}));
    const shell = document.querySelector("[data-zephium-active-tab]");
    if (!shell || shell.dataset.zephiumSurface !== "deck") return rejected;
    if (shell.dataset.zephiumActiveTab !== (active ?? "")) return rejected;
    void document.documentElement.getBoundingClientRect();
    return {serialized_nonce};
  }} catch (_) {{
    return rejected;
  }}
}})()"#
    );
    let expected = nonce;
    let done = Arc::new(Mutex::new(Some(done)));
    match window.eval_with_callback(script, move |result| {
        let applied =
            serde_json::from_str::<String>(&result).is_ok_and(|returned| returned == expected);
        if let Some(done) = done
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            done(applied);
        }
    }) {
        Ok(()) => ChromePresentationDispatch::Scheduled,
        Err(_) => {
            diagnostic!("projection: privileged presentation evaluation was not admitted");
            ChromePresentationDispatch::Rejected
        }
    }
}

/// Restores the complete snapshot and checks visible browser identity before
/// the actor is allowed to reattach raw native content.
pub(crate) fn restore_browser_chrome(
    window: &WebviewWindow,
    revision: u64,
    items: zephium_ipc::ItemsState,
    done: ChromePresentationCallback,
) -> ChromePresentationDispatch {
    let Ok(payload) = serde_json::to_string(&items) else {
        return ChromePresentationDispatch::Rejected;
    };
    let nonce = format!("zephium-browser-return:{revision}");
    let Ok(expected) = serde_json::to_string(&nonce) else {
        return ChromePresentationDispatch::Rejected;
    };
    let script = format!(
        r#"(() => {{
      const items = {payload};
      window.dispatchEvent(new CustomEvent('zephium:browser-return', {{ detail: items }}));
      const shell = document.querySelector('[data-zephium-active-tab]');
      if (!shell || shell.dataset.zephiumSurface !== 'deck' || shell.dataset.zephiumActiveTab !== (items.active ?? '')) return '';
      void document.documentElement.getBoundingClientRect();
      return {expected};
    }})()"#
    );
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    match window.eval_with_callback(script, move |result| {
        if let Some(sender) = sender.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = sender
                .send(serde_json::from_str::<String>(&result).is_ok_and(|value| value == nonce));
        }
    }) {
        Ok(()) => {
            tauri::async_runtime::spawn(async move {
                let applied = tokio::time::timeout(std::time::Duration::from_secs(5), receiver)
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(false);
                done(applied);
            });
            ChromePresentationDispatch::Scheduled
        }
        Err(_) => ChromePresentationDispatch::Rejected,
    }
}

fn shutdown_started(app: &tauri::AppHandle) -> bool {
    app.try_state::<ShutdownCoordinator>()
        .is_some_and(|state| state.terminal_started())
}

const MAIN_LABEL: &str = "main";
#[cfg(any(target_os = "windows", test))]
const PRIVILEGED_MAIN_ENVIRONMENT: u8 = 1 << 0;
#[cfg(any(target_os = "windows", test))]
const PRIVILEGED_COLUMN_ENVIRONMENT: u8 = 1 << 1;
const MAX_ITEM_ID_BYTES: usize = 64;

#[cfg(any(target_os = "windows", test))]
fn expected_privileged_environment_labels(bits: u8) -> Vec<&'static str> {
    let mut labels = Vec::with_capacity(2);
    if bits & PRIVILEGED_MAIN_ENVIRONMENT != 0 {
        labels.push(MAIN_LABEL);
    }
    if bits & PRIVILEGED_COLUMN_ENVIRONMENT != 0 {
        labels.push(deck::column_panel::LABEL);
    }
    labels
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallerPolicy {
    Main,
    ColumnPanel,
}

// `WebviewWindow` is injected from Tauri's invoke message (and omitted by
// Specta), so the label cannot be forged as a serialized command argument.
fn caller_allowed(policy: CallerPolicy, label: &str) -> bool {
    match policy {
        CallerPolicy::Main => label == MAIN_LABEL,
        CallerPolicy::ColumnPanel => label == deck::column_panel::LABEL,
    }
}

fn authorize(caller: &WebviewWindow, policy: CallerPolicy, command: &str) -> bool {
    let allowed = caller_allowed(policy, caller.label());
    let report = !allowed
        && AUTH_DENIAL_LOGS_REMAINING
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok();
    if report {
        diagnostic!(
            "security: blocked {command} from privileged webview {}",
            caller.label()
        );
    }
    allowed
}

fn bounded(value: &str, max_bytes: usize) -> bool {
    value.len() <= max_bytes
}

fn setting_value_allowed(key: &str, value: &str) -> bool {
    zephium_core::preferences::value_allowed(key, value)
}

// Ids arrive as ULID strings from a semi-trusted webview; anything that does
// not parse is dropped here, before it reaches the shell.
fn rejected_operation() -> zephium_ipc::OperationAdmission {
    zephium_ipc::OperationAdmission {
        operation_id: None,
        accepted: false,
    }
}

fn dispatch_operation(
    _app: &tauri::AppHandle,
    shell: &Handle,
    command: Command,
) -> zephium_ipc::OperationAdmission {
    let Ok(sequence) = NEXT_OPERATION_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
    else {
        return rejected_operation();
    };
    let operation_id = format!("{sequence:016x}");
    if shell.dispatch_operation(operation_id.clone(), command) {
        zephium_ipc::OperationAdmission {
            operation_id: Some(operation_id),
            accepted: true,
        }
    } else {
        rejected_operation()
    }
}

fn dispatch_with_id(
    app: &tauri::AppHandle,
    shell: &Handle,
    id: &str,
    cmd: impl FnOnce(ItemId) -> Command,
) -> zephium_ipc::OperationAdmission {
    if !bounded(id, MAX_ITEM_ID_BYTES) {
        return rejected_operation();
    }
    if let Some(id) = ItemId::parse(id) {
        return dispatch_operation(app, shell, cmd(id));
    }
    rejected_operation()
}

#[tauri::command]
#[specta::specta]
fn tabs_bootstrap(caller: WebviewWindow, shell: State<'_, Handle>) {
    if !authorize(&caller, CallerPolicy::Main, "tabs_bootstrap") {
        return;
    }
    shell.dispatch(Command::Bootstrap);
}

#[tauri::command]
#[specta::specta]
fn tabs_activate(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    id: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "tabs_activate") {
        return rejected_operation();
    }
    dispatch_with_id(caller.app_handle(), &shell, &id, Command::Activate)
}

#[tauri::command]
#[specta::specta]
fn tabs_reload(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    id: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "tabs_reload") {
        return rejected_operation();
    }
    dispatch_with_id(caller.app_handle(), &shell, &id, Command::Reload)
}

#[tauri::command]
#[specta::specta]
fn tabs_back(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    id: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "tabs_back") {
        return rejected_operation();
    }
    dispatch_with_id(caller.app_handle(), &shell, &id, Command::GoBack)
}

#[tauri::command]
#[specta::specta]
fn tabs_forward(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    id: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "tabs_forward") {
        return rejected_operation();
    }
    dispatch_with_id(caller.app_handle(), &shell, &id, Command::GoForward)
}

const SETTING_KEYS: &[&str] = zephium_core::preferences::KEYS;

/// The person's overrides, for menus built on demand.
fn appearance_code(mode: &str) -> u8 {
    match mode {
        "light" => APPEARANCE_LIGHT,
        "dark" => APPEARANCE_DARK,
        _ => APPEARANCE_SYSTEM,
    }
}

#[cfg(target_os = "windows")]
fn resolved_native_theme(window: &WebviewWindow, mode: &str) -> tauri::Theme {
    match appearance_code(mode) {
        APPEARANCE_LIGHT => tauri::Theme::Light,
        APPEARANCE_DARK => tauri::Theme::Dark,
        _ => window.theme().unwrap_or(tauri::Theme::Dark),
    }
}

#[cfg(target_os = "windows")]
fn apply_native_materials(app: &tauri::AppHandle, mode: &str) {
    if let Some(window) = app.get_webview_window(MAIN_LABEL) {
        let dark = matches!(resolved_native_theme(&window, mode), tauri::Theme::Dark);
        platform::imp::apply_material(&window, dark);
    }
}

pub(crate) fn app_setting(key: &str) -> Option<String> {
    APP_STORE.get().and_then(|store| store.app_setting(key))
}

/// The appearance the app shows, which is the person's choice when they made
/// one. A window's own theme reports the system's, so a light app on a dark
/// Mac would otherwise paint its window and tint its glass for dark.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn appearance_is_light(window: &WebviewWindow) -> bool {
    match NATIVE_APPEARANCE.load(Ordering::Acquire) {
        APPEARANCE_LIGHT => true,
        APPEARANCE_DARK => false,
        _ => matches!(window.theme(), Ok(tauri::Theme::Light)),
    }
}

fn reinstall_native_materials(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "macos")]
        if let Some(window) = handle.get_webview_window(MAIN_LABEL) {
            material::install(&window);
        }
        #[cfg(target_os = "windows")]
        apply_native_materials(
            &handle,
            &app_setting("appearance").unwrap_or_else(|| "system".into()),
        );
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = handle;
    });
}

fn apply_native_theme(app: &tauri::AppHandle, mode: &str) {
    let appearance = appearance_code(mode);
    // Publish the mode before set_theme: a synchronous ThemeChanged callback
    // caused by a forced light/dark value must not be mistaken for a system
    // appearance transition.
    NATIVE_APPEARANCE.store(appearance, Ordering::Release);
    let theme = match appearance {
        APPEARANCE_LIGHT => Some(tauri::Theme::Light),
        APPEARANCE_DARK => Some(tauri::Theme::Dark),
        _ => None,
    };
    // Keeps vibrancy and native controls in step with a forced appearance.
    app.set_theme(theme);
    #[cfg(target_os = "windows")]
    apply_native_materials(app, mode);
    // The window's ground and the glass's tint follow the app's appearance,
    // which a forced choice changes without the system's changing.
    #[cfg(target_os = "macos")]
    reinstall_native_materials(app);
}

#[tauri::command]
#[specta::specta]
fn ui_ready(caller: WebviewWindow, gate: State<'_, UiStartupGate>) -> bool {
    if !authorize(&caller, CallerPolicy::Main, "ui_ready") {
        return false;
    }
    let ready = gate.mark_frontend_ready(&caller);
    if ready { updater::startup(caller.app_handle()); }
    ready
}

#[tauri::command]
#[specta::specta]
async fn setting_get(caller: WebviewWindow, key: String) -> Option<String> {
    if !authorize(&caller, CallerPolicy::Main, "setting_get") {
        return None;
    }
    if !SETTING_KEYS.contains(&key.as_str()) && key != zephium_core::deck::avatars::SETTING_KEY {
        return None;
    }
    // A synchronous command runs on the main thread, and the store may be
    // busy for seconds (an import, a deletion); the read waits off it.
    tauri::async_runtime::spawn_blocking(move || {
        APP_STORE.get().and_then(|store| store.app_setting(&key))
    })
    .await
    .ok()
    .flatten()
}

#[tauri::command]
#[specta::specta]
fn setting_set(
    caller: WebviewWindow,
    app: tauri::AppHandle,
    shell: State<'_, Handle>,
    key: String,
    value: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "setting_set") {
        return rejected_operation();
    }
    if shutdown_started(&app) {
        return rejected_operation();
    }
    if SETTING_KEYS.contains(&key.as_str()) && setting_value_allowed(&key, &value) {
        return dispatch_operation(&app, &shell, Command::SetAppSetting { key, value });
    }
    rejected_operation()
}

#[tauri::command]
#[specta::specta]
fn run_command(
    caller: WebviewWindow,
    app: tauri::AppHandle,
    shell: State<'_, Handle>,
    id: String,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "run_command")
        || !(deck::commands::admits_scroll(&id)
            || matches!(
                id.as_str(),
                "browser.settings"
                    | "browser.return"
                    | "composer.open"
                    | "composer.close"
                    | "tab.close"
            ))
        || shutdown_started(&app)
    {
        return rejected_operation();
    }
    dispatch_operation(&app, &shell, Command::Run(id))
}

#[tauri::command]
#[specta::specta]
fn inner_logical(window: &tauri::WebviewWindow) -> Size {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|s| Size::new(s.width as f64 / scale, s.height as f64 / scale))
        .unwrap_or_default()
}

fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Opened { urls } = &event {
        external_links::hand_off(app, urls.iter().map(|url| url.to_string()), true);
        return;
    }
    let tauri::RunEvent::ExitRequested { code, api, .. } = event else {
        return;
    };
    let Some(coordinator) = app.try_state::<ShutdownCoordinator>() else {
        write_diagnostic(format_args!(
            "shutdown: exit requested before coordinator setup"
        ));
        // Do not panic or terminate directly from the native event callback.
        // A second, explicitly unsuccessful request is allowed through so a
        // broken composition invariant still converges instead of looping.
        if code == Some(1) {
            return;
        }
        api.prevent_exit();
        app.exit(1);
        return;
    };
    // Tauri restart cannot normally be prevented. A startup failure is
    // sticky, however: never allow a failed initialization to masquerade as
    // a successful restart into a potentially rollback-vulnerable state.
    if code == Some(tauri::RESTART_EXIT_CODE)
        && !coordinator.terminal_failure.load(Ordering::Acquire)
    {
        return;
    };
    if exit_request_is_authorized(
        code,
        coordinator.authorized_exit_code.load(Ordering::Acquire),
    ) {
        if coordinator.terminal_failure.load(Ordering::Acquire) && code != Some(1) {
            // Cleanup may have completed just before a startup watchdog made
            // failure sticky. Replace the already-authorized successful exit
            // with a correlated failure; do not let the stale status through.
            api.prevent_exit();
            coordinator.schedule_authorized_exit(app.clone(), 1);
            return;
        }
        return;
    }
    api.prevent_exit();
    if let Some(shell) = app.try_state::<Handle>() {
        let owner = coordinator.inner().clone();
        let exit_app = app.clone();
        let handle = shell.inner().clone();
        owner.request(exit_app, handle);
    } else {
        write_diagnostic(format_args!("shutdown: exit requested before shell setup"));
        let engine_owner = app
            .try_state::<StartupEngine>()
            .map(|engine| engine.inner().clone());
        let store_owner = app
            .try_state::<StartupStore>()
            .map(|store| store.inner().clone());
        coordinator.request_terminal_startup_failure(
            app.clone(),
            TerminalStartupResources {
                engine_owner,
                store_owner,
                ..TerminalStartupResources::default()
            },
        );
    }
}

fn install_async_runtime() {
    match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(ASYNC_WORKER_STACK_BYTES)
        .build()
    {
        Ok(runtime) => {
            tauri::async_runtime::set(ASYNC_RUNTIME.get_or_init(|| runtime).handle().clone())
        }
        Err(_) => diagnostic!("startup: async runtime stack configuration unavailable"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if cfg!(all(unix, not(target_os = "macos"))) {
        diagnostic!(
            "BDeck supports macOS and Windows. Linux is not supported."
        );
        std::process::exit(1);
    }
    install_async_runtime();
    APP_STARTED.get_or_init(std::time::Instant::now);
    let context = tauri::generate_context!();
    // A launch that stops before setup has nowhere else to say why.
    #[cfg(target_os = "windows")]
    platform::imp::open_startup_log(&context.config().identifier);
    #[cfg(target_os = "macos")]
    let runtime_security_advisories = match platform::imp::enforce_runtime_security_floor() {
        Ok(advisory) => advisory,
        Err(error) => {
            // WebKit is dynamically supplied by macOS. Only an unsupported
            // or unparseable runtime stops here, before Builder creates even
            // the privileged blank bootstrap WKWebView; an outdated one
            // starts with an update advisory.
            diagnostic!("security: {error}");
            startup_alert::show_blocking(startup_alert::StartupProblem::UnsupportedSystem, &error);
            std::process::exit(78);
        }
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let runtime_security_advisories = match zephium_engine::enforce_runtime_security_floor() {
        Ok(advisory) => advisory,
        Err(error) => {
            // WebKitGTK is dynamically supplied by the OS. Reject known
            // obsolete/development runtimes and security overrides before
            // Builder constructs even the privileged blank bootstrap WebView.
            diagnostic!("security: {error}");
            std::process::exit(78);
        }
    };

    #[cfg(target_os = "windows")]
    let runtime_security_advisories = match platform::imp::enforce_runtime_security_floor() {
        Ok(advisory) => advisory,
        Err(failure) => {
            // This runs before Builder creates either privileged chrome
            // or raw content. Preview, overridden, or unparseable runtimes
            // remain hard failures; an outdated one is an advisory.
            diagnostic!("security: {failure}");
            startup_alert::show_blocking(failure.problem(), failure.detail());
            std::process::exit(78);
        }
    };

    let specta = specta_builder();
    #[cfg(target_os = "windows")]
    let attempted_privileged_environments = Arc::new(AtomicU8::new(0));
    #[cfg(target_os = "windows")]
    let setup_privileged_environments = attempted_privileged_environments.clone();
    let builder = tauri::Builder::default()
        // Every Tauri-managed webview is zone 2. It may load only the bundled
        // application origin (or the exact Vite origin in debug builds).
        .plugin(navigation_lock())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdateManager::default())
        .plugin(tauri_plugin_dialog::init())
        // Must register first: a second launch (file association, dock, a
        // stale instance holding the global hotkey and the profile dbs)
        // focuses the running window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            external_links::bring_forward(app);
            external_links::hand_off(app, argv.into_iter().skip(1), true);
        }));
    // Windows hears the same from WebView2 ProcessFailed. Any other WebView
    // keeps Tauri's own plain reload, which this hook replaces for all.
    #[cfg(target_os = "macos")]
    let builder = builder.on_web_content_process_terminate(|webview| {
        let label = webview.label().to_owned();
        if !renderer_recovery::owns(&label) {
            let _ = webview.reload();
            return;
        }
        let app = webview.app_handle().clone();
        let target = app.clone();
        let _ = app.run_on_main_thread(move || renderer_recovery::failed(&target, &label, false));
    });
    let app = builder
        .invoke_handler(specta.invoke_handler())
        // This state must predate the setup hook: every environmental failure
        // is contained inside that native Ready callback and converted into a
        // correlated event-loop exit instead of escaping as Tauri's panic.
        .manage(ShutdownCoordinator::default())
        // Storage is admitted first inside setup, but its temporary cleanup
        // owner must already exist so installation and later Shell transfer are
        // one exact, failure-observable transaction.
        .manage(StartupStore::default())
        // Own a successfully installed native host until shell ownership is
        // published. Setup failures in that narrow interval can therefore
        // execute the same explicit bounded engine teardown.
        .manage(StartupEngine::default())
        .setup(move |app| {
            let setup_result: SetupResult = (|| {
                let shutdown = app
                    .try_state::<ShutdownCoordinator>()
                    .ok_or_else(|| {
                        std::io::Error::other("shutdown coordinator state is unavailable")
                    })?;
                // Allocate the independent deadline thread before any native
                // callback or persistent actor exists. Later callbacks only
                // signal it and can never fail while spawning a watchdog.
                shutdown.prepare_hard_exit_watchdog()?;
                specta.mount_events(app);
                let data_dir = app.path().app_data_dir()?;
                std::fs::create_dir_all(&data_dir)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700))?;
                }
                #[cfg(target_os = "windows")]
                {
                    // Release builds have no console. Establish the bounded,
                    // private diagnostic sink before storage admission so a
                    // schema/corruption failure is still actionable without
                    // constructing WebView2 state merely to initialize logs.
                    platform::imp::redirect_stderr(&data_dir);
                    diagnostics::install(data_dir.clone());
                    diagnostic!("zephium {} starting", env!("CARGO_PKG_VERSION"));
                }
                #[cfg(target_os = "macos")]
                {
                    // Finder-launched apps write stderr nowhere; keep it in
                    // a private, bounded log the person can choose to share.
                    let logs = app.path().app_log_dir()?;
                    platform::imp::redirect_stderr(&logs);
                    diagnostics::install(logs);
                    diagnostic!("zephium {} starting", env!("CARGO_PKG_VERSION"));
                }

                // Storage is the first fallible subsystem admitted after the
                // private data root is secured. A corrupt/newer database must
                // fail before either privileged chrome or raw content creates
                // native renderer state.
                let store = Arc::new(SqliteStore::open(&data_dir)?);
                let startup_store = app.try_state::<StartupStore>().ok_or_else(|| {
                    std::io::Error::other("startup storage cleanup owner is unavailable")
                })?;
                if !startup_store.install(store.clone()) {
                    let error =
                        std::io::Error::other("startup storage cleanup owner is already armed");
                    request_pre_shell_startup_failure_with_store(
                        app.handle(),
                        &error,
                        store.clone(),
                    );
                    return Err(error.into());
                }
                APP_STORE.set(store.clone()).map_err(|_| {
                    std::io::Error::other("process-global application store is already installed")
                })?;
                for (key, value) in
                    zephium_core::preferences::migrations(|key| store.app_setting(key))
                {
                    let _ = store.set_app_setting(key.into(), value);
                }
                #[cfg(target_os = "windows")]
                windows_privacy::initialize(app.handle(), store.clone());

                #[cfg(target_os = "windows")]
                let privileged_runtime = prepare_privileged_runtime_directories(&data_dir)?;

                let mut main_config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or_else(|| std::io::Error::other("main window configuration is missing"))?;
                let browser_url = privileged_app_url(app, &main_config.url)?;
                let app_url = browser_url.clone();
                main_config.url = tauri::WebviewUrl::External(
                tauri::Url::parse(PRIVILEGED_BOOTSTRAP_URL)
                    .map_err(|error| std::io::Error::other(error.to_string()))?,
            );
                #[cfg(target_os = "macos")]
                if let Some(position) = main_config.traffic_light_position.as_mut() {
                    position.y += platform::imp::window_controls_lift();
                }
                let main_builder = tauri::WebviewWindowBuilder::from_config(app, &main_config)?
                // Do not expose an interactive privileged renderer until the
                // platform deny handlers below have replaced WebKit/WebView2
                // defaults. This also closes the small post-build attachment
                // window while we remain on Tauri's public construction API.
                .visible(false)
                // Privileged chrome is a projection of Rust-owned state. It
                // must not persist cookies, cache, service workers, or web
                // storage across launches.
                .incognito(true)
                .devtools(cfg!(debug_assertions))
                .general_autofill_enabled(false)
                .initialization_script_for_all_frames(zephium_engine::PAGE_PRINT_DENY_SCRIPT);
            #[cfg(target_os = "windows")]
            let main_builder = main_builder
                .decorations(true)
                // Seed the native parent's first erase, not just WebView2's
                // transparent renderer. Otherwise a hidden decorated window
                // can reveal an unpainted white client surface until redraw.
                .background_color(tauri::utils::config::Color(0, 0, 0, 0))
                .data_directory(privileged_runtime.main.clone())
                // Supplying any explicit value replaces Wry's default, which
                // also disables msSmartScreenProtection. Keep only the two
                // browser-UI suppressions for privileged chrome.
                .additional_browser_args(PRIVILEGED_WEBVIEW2_BROWSER_ARGS);
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            let main_builder = main_builder.on_download(|_, _| false);
            let ui_startup_gate = UiStartupGate::new(app_url.clone());
            app.manage(ui_startup_gate.clone());
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            app.manage(renderer_recovery::Renderers::default());
            let page_gate = ui_startup_gate.clone();
            #[cfg(target_os = "windows")]
            setup_privileged_environments
                .fetch_or(PRIVILEGED_MAIN_ENVIRONMENT, Ordering::Release);
            let window = main_builder
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .on_web_resource_request(|_request, response| {
                    harden_privileged_headers(response.headers_mut());
                })
                .on_page_load(move |window, payload| {
                    // The native window is transparent and its privileged
                    // WebView is deliberately constructed at about:blank so
                    // deny handlers can be installed before app code runs.
                    // Never expose that engine-default backing surface. A
                    // native Finished event proves only document loading; the
                    // trusted frontend separately acknowledges deterministic
                    // theme/DOM initialization. The opaque bootstrap surface
                    // remains in place until then; the gate requires both
                    // facts and is idempotent in either order.
                    if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                        page_gate.mark_document_loaded(&window, payload.url());
                    }
                })
                .build()?;
            let startup_watchdog_gate = ui_startup_gate.clone();
            let startup_watchdog_app = app.handle().clone();
            std::thread::Builder::new()
                .name("zephium-ui-startup-watchdog".into())
                .spawn(move || {
                    std::thread::sleep(UI_INITIALIZATION_TIMEOUT);
                    if startup_watchdog_gate.is_visible() {
                        return;
                    }
                    let exit_app = startup_watchdog_app.clone();
                    let exit_gate = startup_watchdog_gate.clone();
                    let _ = startup_watchdog_app.run_on_main_thread(move || {
                        if !exit_gate.is_visible() {
                            request_startup_failure(
                                &exit_app,
                                "trusted application document did not finish and initialize before the deadline",
                            );
                        }
                    });
                })
                .map_err(|error| {
                    std::io::Error::other(format!(
                        "could not start UI startup watchdog: {error}"
                    ))
                })?;
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            let parent = window.window_handle()?.as_raw();
            let handle = app.handle().clone();

            // Privileged WebView2 environments exist before the shell and raw
            // engine do. Keep an early callback target plus a sticky bit; once
            // the engine exists, every privileged notification enters its
            // process-global dedupe gate and the shell can reconcile the bit
            // even if its command queue was not installed yet.
            let slot: Arc<OnceLock<zephium_app::CallbackHandle>> = Arc::new(OnceLock::new());
            #[cfg(target_os = "windows")]
            let runtime_engine_slot: Arc<OnceLock<Arc<zephium_engine::WebviewEngine>>> =
                Arc::new(OnceLock::new());
            #[cfg(target_os = "windows")]
            let pending_runtime_update = Arc::new(AtomicBool::new(false));
            #[cfg(target_os = "windows")]
            let runtime_update_notifier: platform::imp::RuntimeUpdateCallback = {
                let engine_slot = runtime_engine_slot.clone();
                let pending = pending_runtime_update.clone();
                Arc::new(move || {
                    if let Some(engine) = engine_slot.get() {
                        engine.notify_runtime_restart_required();
                    } else {
                        pending.store(true, Ordering::Release);
                    }
                })
            };

            let dispatch_handle = handle.clone();
            let dispatch: MainThreadDispatch = Arc::new(move |task: Box<dyn FnOnce() + Send>| {
                dispatch_handle.run_on_main_thread(task).is_ok()
            });

            #[cfg(target_os = "windows")]
            platform::imp::init(
                &window,
                &privileged_runtime.main,
                runtime_update_notifier.clone(),
            )?;
            #[cfg(target_os = "macos")]
            if !platform::imp::init(&window) {
                return Err(std::io::Error::other(
                    "the browser window couldn't be set up with its required protections",
                )
                .into());
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            platform::imp::init(&window).map_err(|error| {
                std::io::Error::other(format!(
                    "required privileged WebKitGTK hardening or GTK composition failed: {error}"
                ))
            })?;
            deck::column_panel::install(
                &window,
                browser_url.join("column.html")?,
                #[cfg(target_os = "windows")] privileged_runtime.column.clone(),
                #[cfg(target_os = "windows")] setup_privileged_environments.clone(),
                #[cfg(target_os = "windows")] runtime_update_notifier.clone(),
            );
            // The engine is owned by the shell. Retaining a strong Handle in
            // its callback would form Shell -> Engine -> Handle -> queue and
            // keep the actor/ticker alive after every application owner drops.
            let sink_slot = slot.clone();
            let terminal_failure_app = handle.clone();
            let engine = Arc::new(zephium_engine::install(
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                parent,
                dispatch.clone(),
                data_dir.join("web-content"),
                runtime_security_advisories,
                move |event| {
                    if let Some(shell) = sink_slot.get() {
                        shell.dispatch(Command::Engine(event));
                    }
                },
                move |reason| {
                    // A refused native close/erasure transition can leave a
                    // renderer executing after Rust has retired authority.
                    // Skip engine-driven teardown, but preserve Tauri/App and
                    // Windows privileged-environment finalization.
                    request_unrecoverable_native_failure(&terminal_failure_app, reason);
                },
            )?);
            let startup_engine = app.try_state::<StartupEngine>().ok_or_else(|| {
                std::io::Error::other("startup engine cleanup owner is unavailable")
            })?;
            if !startup_engine.install(engine.clone()) {
                return Err(
                    std::io::Error::other("startup engine cleanup owner is already armed").into(),
                );
            }
            #[cfg(target_os = "windows")]
            runtime_engine_slot
                .set(engine.clone())
                .map_err(|_| std::io::Error::other("runtime engine callback slot already set"))?;
            #[cfg(target_os = "macos")]
            let _ = std::thread::Builder::new()
                .name("webkit-private-api".into())
                .spawn(|| {
                    let missing = zephium_engine::missing_private_webkit_api();
                    if missing.is_empty() {
                        write_diagnostic(format_args!("webkit: private api present"));
                    } else {
                        write_diagnostic(format_args!(
                            "webkit: missing private api {}",
                            missing.join(", ")
                        ));
                    }
                });
            let emit_handle = handle.clone();
            let emit: EmitFn = Box::new(move |projection| match projection {
                Projection::Items(state) => {
                    emit_to_privileged(&emit_handle, MAIN_LABEL, EVENT_ITEMS, &state)
                }
                Projection::Tab(tab) => {
                    emit_to_privileged(&emit_handle, MAIN_LABEL, EVENT_TAB, &tab)
                }
                Projection::Profiles(view) => {
                    emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:profiles", &view)
                }
                Projection::OperationProcessed(disposition) => {
                    emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:operation-processed", &disposition);
                }
                Projection::UiCommand(id) => {
                    if let Some(mode) = id.strip_prefix("theme.") {
                        if matches!(mode, "system" | "light" | "dark") {
                            apply_native_theme(&emit_handle, mode);
                        }
                    }
                    emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:ui-command", &id);
                }
                Projection::Layout(layout) => emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:layout", &layout),
                Projection::ExtensionActions(view) => emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:extension-actions", &view),
                Projection::ExtensionActionFailed(view) => emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:extension-action-failed", &view),
                Projection::ExtensionActionShortcut(view) => emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:extension-action-shortcut", &view),
                Projection::WebExtensionAccessRequest(request) => emit_to_privileged(&emit_handle, MAIN_LABEL, "zephium:web-extension-access", &request),
                Projection::PanelOwner(_)
                | Projection::Favicons(_)
                | Projection::PagePermissionPrompt(_)
                | Projection::FindResult(_)
                | Projection::HostFullscreen(_)
                | Projection::RuntimeStatus(_)
                | Projection::RecentlyClosed(_)
                | Projection::MediaPlayer(_) => {}

            });

            let chrome: SharedChrome = platform::imp::make_chrome(&window, dispatch.clone());
            let terminal_failure_app = app.handle().clone();
            let terminal_failure_shutdown = shutdown.inner().clone();
            let shell_terminal_failure: ShellTerminalFailureCallback = Box::new(move |failure| {
                // An unopenable session would otherwise leave an empty window
                // that does nothing. Say so, then quit in order; the store
                // has kept the saved data as it was.
                if matches!(
                    failure,
                    zephium_app::ShellTerminalFailure::SessionUnavailable
                        | zephium_app::ShellTerminalFailure::SessionFromNewerVersion
                ) {
                    let app = terminal_failure_app.clone();
                    let shutdown = terminal_failure_shutdown.clone();
                    let explained = terminal_failure_app.run_on_main_thread(move || {
                        #[cfg(not(target_os = "linux"))]
                        startup_alert::show_blocking(
                            if matches!(
                                failure,
                                zephium_app::ShellTerminalFailure::SessionFromNewerVersion
                            ) {
                                startup_alert::StartupProblem::NewerProfile
                            } else {
                                startup_alert::StartupProblem::DamagedProfile
                            },
                            "the saved tabs and settings could not be opened",
                        );
                        request_shell_terminal_failure(&app, &shutdown, failure);
                    });
                    if explained.is_ok() {
                        return;
                    }
                }
                request_shell_terminal_failure(
                    &terminal_failure_app,
                    &terminal_failure_shutdown,
                    failure,
                )
            });
            let shell = match zephium_app::spawn_suspended(
                engine.clone(),
                store.clone(),
                shell_terminal_failure,
                chrome,
                emit,
            ) {
                Ok(shell) => shell,
                Err(failure) => {
                    if !failure.worker_cleanup_proven() {
                        write_diagnostic(format_args!(
                            "startup: application helper-worker cleanup was not proven after Shell construction failed"
                        ));
                    }
                    return Err(failure.into());
                }
            };
            if !app.manage(shell.clone()) {
                let error = std::io::Error::other("shell cleanup state is already installed");
                // The local actor owns the Store and engine even
                // though Tauri refused to publish its Handle. Route that
                // exact owner through the coordinator before the outer setup
                // error callback can observe unrelated managed state.
                shutdown.request_terminal_startup_failure(
                    app.handle().clone(),
                    TerminalStartupResources {
                        shell: Some(shell),
                        ..TerminalStartupResources::default()
                    },
                );
                return Err(error.into());
            }
            let web_extensions = webext::WebExtensions::new(&data_dir);
            web_extensions.restore(&shell);
            app.manage(web_extensions);
            external_links::adopt(app.handle());
            if !startup_engine.transfer_to(&engine) {
                return Err(std::io::Error::other(
                    "startup engine ownership did not transfer to the shell",
                )
                .into());
            }
            if !startup_store.transfer_to(&store) {
                return Err(std::io::Error::other(
                    "startup storage ownership did not transfer to the shell",
                )
                .into());
            }
            // Publish the weak callback ingress before actor admission, then
            // make the single exact transition only after every temporary
            // cleanup owner has been cleared. A concurrent terminal request may
            // cancel admission and is never allowed to be overtaken here.
            slot.set(shell.callback_handle()).map_err(|_| {
                std::io::Error::other("native engine callback ingress is already published")
            })?;
            if !shutdown.try_admit_shell(&shell) {
                return Err(std::io::Error::other(
                    "terminal shutdown started before Shell actor admission, or admission was already consumed",
                )
                .into());
            }
            if shutdown.terminal_started() {
                return Err(std::io::Error::other(
                    "terminal shutdown overtook Shell actor admission",
                )
                .into());
            }
            #[cfg(target_os = "windows")]
            if pending_runtime_update.swap(false, Ordering::AcqRel) {
                engine.notify_runtime_restart_required();
            }

            let initial =
                platform::imp::content_size(&window).unwrap_or_else(|| inner_logical(&window));
            shell.dispatch(Command::SetWindowSize(initial));
            shell.dispatch(Command::SetWindowFocused(window.is_focused().unwrap_or(false)));

            let resize_shell = shell.clone();
            let resize_window = window.clone();
            let exit_handle = handle.clone();
            let window_shutdown = shutdown.inner().clone();
            window.on_window_event(move |event| {
                match event {
                    tauri::WindowEvent::Resized(size)
                        if !window_shutdown.terminal_started() =>
                    {
                        let minimized = resize_window
                            .is_minimized()
                            .unwrap_or(size.width == 0 || size.height == 0);
                        resize_shell.dispatch(Command::SetWindowVisible(!minimized));
                        if minimized {
                            return;
                        }
                        let size = platform::imp::content_size(&resize_window)
                            .unwrap_or_else(|| inner_logical(&resize_window));
                        resize_shell.dispatch(Command::SetWindowSize(size));
                    }
                    #[cfg(target_os = "windows")]
                    tauri::WindowEvent::Moved(_)
                        if !window_shutdown.terminal_started()
                            && !resize_window.is_minimized().unwrap_or(false) =>
                    {
                        // WebView2 requires an explicit parent-position
                        // notification for IME, accessibility and popup
                        // coordinates. Re-submit the coalescible layout even
                        // when logical size is unchanged; the Windows stage
                        // emits only the position-dependent native delta.
                        let size = platform::imp::content_size(&resize_window)
                            .unwrap_or_else(|| inner_logical(&resize_window));
                        resize_shell.dispatch(Command::SetWindowSize(size));
                    }
                    tauri::WindowEvent::ScaleFactorChanged { .. }
                        if !window_shutdown.terminal_started() =>
                    {
                        let minimized = resize_window.is_minimized().unwrap_or(false);
                        resize_shell.dispatch(Command::SetWindowVisible(!minimized));
                        if !minimized {
                            // Re-read logical content size at the new scale.
                            // The layout refresh also rebuilds Windows
                            // physical bounds/corner radii and parent-position
                            // notifications using the new per-monitor DPI.
                            let size = platform::imp::content_size(&resize_window)
                                .unwrap_or_else(|| inner_logical(&resize_window));
                            resize_shell.dispatch(Command::SetWindowSize(size));
                        }
                    }
                    tauri::WindowEvent::ThemeChanged(_theme)
                        if NATIVE_APPEARANCE.load(Ordering::Acquire) == APPEARANCE_SYSTEM =>
                    {
                        #[cfg(target_os = "windows")]
                        apply_native_materials(&exit_handle, "system");
                    }
                    // Keep the main window alive and responsive until every
                    // shell command queued before close has been snapshotted.
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        window_shutdown.request(exit_handle.clone(), resize_shell.clone());
                    }
                    // Fallback for platform/programmatic destruction paths
                    // that do not emit a preventable close request first.
                    tauri::WindowEvent::Destroyed => {
                        #[cfg(target_os = "windows")]
                        if !platform::imp::remove_privileged_version_observer(MAIN_LABEL) {
                            diagnostic!(
                                "runtime: main WebView2 update observer removal was reentrant"
                            );
                        }
                        window_shutdown.request(exit_handle.clone(), resize_shell.clone())
                    }
                    tauri::WindowEvent::Focused(true) => {
                        resize_shell.dispatch(Command::SetWindowFocused(true));
                        // Some window managers restore without a distinct
                        // resize notification. Wake content before it can be
                        // interacted with.
                        resize_shell.dispatch(Command::SetWindowVisible(true));
                    }
                    // Losing focus alone does not make a browser tab
                    // background work: audio and timers must continue. Only
                    // an OS-minimized window hides all content views.
                    tauri::WindowEvent::Focused(false) => {
                        resize_shell.dispatch(Command::SetWindowFocused(false));
                        if resize_window.is_minimized().unwrap_or(false) {
                            resize_shell.dispatch(Command::SetWindowVisible(false));
                        }
                    }
                    _ => {}
                }
            });

            memory_pressure::install(&handle);

            let appearance = APP_STORE
                .get()
                .and_then(|store| store.app_setting("appearance"))
                .unwrap_or_else(|| "system".to_owned());
            apply_native_theme(&handle, &appearance);

            // No bundled application asset or page script runs before every
            // mandatory native deny handler has installed. Tauri's trusted
            // IPC/document-start plumbing and app protocols were registered
            // when the blank WebViews were built, so this navigation activates
            // the normal application without rebuilding the native views.
            if shutdown.terminal_started() {
                return Err(std::io::Error::other(
                    "terminal shutdown started before trusted application navigation",
                )
                .into());
            }
            if shutdown.terminal_started() {
                return Err(std::io::Error::other(
                    "terminal shutdown overtook privileged panel navigation",
                )
                .into());
            }
            window.navigate(app_url)?;
            #[cfg(all(unix, not(target_os = "macos")))]
            if std::env::var("ZEPHIUM_NATIVE_STARTUP_PROBE").as_deref() == Ok("1") {
                // CI accepts this marker only after both privileged WebViews
                // exist, their deny-by-default GTK policy/composition has
                // completed, and both trusted navigations were admitted.
                diagnostic!("startup-probe: Linux privileged WebViews are hardened and composed");
            }
            Ok(())
            })();

            contain_tauri_setup_failure(setup_result, |error| {
                request_setup_failure(app.handle(), error)
            })
        })
        .build(context)
        .unwrap_or_else(|error| {
            diagnostic!("startup: failed to build BDeck: {error}");
            #[cfg(not(target_os = "linux"))]
            {
                let error = error.to_string();
                startup_alert::show_blocking(
                    startup_alert::StartupProblem::classify(&error),
                    &error,
                );
            }
            std::process::exit(1);
        });

    #[cfg(target_os = "windows")]
    {
        // The reviewed local Tauri runtime seals and drains its native window
        // registry before `run_return` completes, including dispatcher cycles
        // retained by window listeners. Only then can Environment5 plus the
        // exact process HANDLE authorize deletion of these private UDFs. A
        // failure remains visible in the process status and quarantines the
        // generation for a later verified cleanup pass.
        let exit_code = app.run_return(handle_run_event);
        // Setup can fail after zero or one privileged environment exists.
        // Every attempted build may have created native state before returning
        // an error. Require an exact observer/PID/HANDLE proof for that
        // conservative set; a partial registration therefore mismatches and
        // keeps the runtime generation quarantined.
        let expected_environments = expected_privileged_environment_labels(
            attempted_privileged_environments.load(Ordering::Acquire),
        );
        let process_exit_proven =
            platform::imp::finalize_privileged_environment_observers(&expected_environments);
        let cleanup_succeeded = process_exit_proven && cleanup_privileged_runtime_after_exit();
        if !process_exit_proven {
            diagnostic!(
                "privacy: privileged WebView2 Environment5/PID/HANDLE exit was not proven; leaving this run's UDF generation quarantined"
            );
        }
        std::process::exit(if cleanup_succeeded || exit_code != 0 {
            exit_code
        } else {
            1
        });
    }

    #[cfg(not(target_os = "windows"))]
    app.run(handle_run_event);
}

#[cfg(test)]
mod frame_sources;

#[cfg(test)]
mod binding_export {
    #[test]
    fn export_typescript_bindings() {
        let path = "../frame/src/shared/ipc/bindings.ts";
        super::specta_builder()
            .export(specta_typescript::Typescript::default(), path)
            .expect("export IPC bindings");
        let source = std::fs::read_to_string(path).expect("read generated bindings");
        let lines: Vec<_> = source.trim_end().lines().map(str::trim_end).collect();
        std::fs::write(path, format!("{}\n", lines.join("\n"))).expect("format generated bindings");
    }
}
