#[cfg(any(target_os = "macos", target_os = "windows"))]
mod automatic_pip;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod certificates;
mod construction;
pub(crate) mod deck;
mod discard;
mod dispatch;
#[cfg(target_os = "macos")]
mod download_files;
#[cfg(target_os = "windows")]
#[path = "download_files_windows.rs"]
mod download_files;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) mod downloads;
#[cfg(target_os = "macos")]
mod extension_action;
mod extension_browser_surface;
#[cfg(target_os = "macos")]
mod extension_storage;
mod favicon;
#[cfg(target_os = "macos")]
mod file_uploads;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod fullscreen;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod http_auth;
mod lifecycle;
mod media;
mod navigation;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod page_open;
mod page_ops;
#[cfg(target_os = "macos")]
mod page_permissions;
mod permits;
mod popup_sites;
mod profiles;
mod resources;
mod site_permissions;
mod stages;
#[cfg(target_os = "macos")]
mod webext;
#[cfg(target_os = "windows")]
mod webext_windows;

#[cfg(test)]
pub(crate) use dispatch::make_unavailable_for_test;
pub(crate) use dispatch::try_with_discard_state_erasure;
#[cfg(target_os = "macos")]
pub(crate) use dispatch::with_page_permission_terminal;
pub(crate) use dispatch::{
    best_effort_with, install, shutdown, try_with, try_with_close, try_with_profile_erasure,
};
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) use profiles::release_linux_erasure_obligations;
#[cfg(target_os = "macos")]
pub(crate) use profiles::release_macos_erasure_obligation;

#[cfg(target_os = "windows")]
use dispatch::queue_windows_cleanup_debt;
#[cfg(target_os = "windows")]
pub(crate) use dispatch::with_discard_terminal;
use permits::{EventPermit, Sink};
use profiles::ProfilePersistenceClass;
pub(crate) use resources::NativeResourceLease;
use resources::NativeResourceLedger;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
#[cfg(not(target_os = "macos"))]
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle};
use wry::WebView;

use crate::navigation_epoch::{NavigationEpoch, NavigationEpochTracker};
use zephium_core::ids::{ItemId, ProfileId, WindowId};
use zephium_core::ports::engine::Partition;
#[cfg(target_os = "macos")]
use {crate::platform::imp::ContentStage, objc2::rc::Retained};

#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment;

#[cfg(target_os = "windows")]
const MAX_WINDOWS_CLEANUP_DEBTS: usize = resources::MAX_NATIVE_TEARDOWN_DEBTS;
#[cfg(any(target_os = "macos", target_os = "windows"))]
struct ParentHandle(RawWindowHandle);

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl HasWindowHandle for ParentHandle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the parent is the app window, which outlives every child
        // content webview created from it.
        Ok(unsafe { WindowHandle::borrow_raw(self.0) })
    }
}

// A prebuilt hidden webview: renderer spawn costs hundreds of ms on weak
// machines, so the next navigation adopts this one and rebinds its id.
struct Spare {
    partition: Partition,
    view: ObservedView,
    id: Rc<Cell<ItemId>>,
}

// Keep native observer registrations adjacent to their WebView and drop them
// first. Platform observers never strongly capture this wrapper or WebView.
struct ObservedView {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    automatic_pip_sequence: Rc<Cell<u64>>,
    #[cfg(target_os = "macos")]
    _capture_observer: objc2::rc::Retained<crate::platform::macos::capture::CaptureObserver>,
    #[cfg(target_os = "macos")]
    _audio_observer: Option<objc2::rc::Retained<crate::platform::macos::audio::AudioObserver>>,
    #[cfg(target_os = "windows")]
    audio_observer: Option<crate::platform::imp::AudioObserver>,
    #[cfg(target_os = "macos")]
    _fullscreen_observer:
        objc2::rc::Retained<crate::platform::macos::fullscreen::FullscreenObserver>,
    #[cfg(target_os = "windows")]
    _fullscreen_observer: crate::platform::imp::FullscreenObserver,
    #[cfg(target_os = "windows")]
    _favicon_observer: Option<crate::platform::imp::FaviconObserver>,
    // Which icon watch of this view's document is current; an older watch's
    // reports are ignored.
    favicon_watch: Cell<u64>,
    // The media watch's generation, and whether it is running.
    media_watch: Cell<(u64, bool)>,
    replay_safety: Rc<discard::ReplaySafety>,
    discard_probe_lease: std::cell::RefCell<Option<discard::ProbeLease>>,
    #[cfg(target_os = "macos")]
    final_discard: Option<Rc<discard::FinalDiscard>>,
    #[cfg(target_os = "windows")]
    discard_settle_timer: std::cell::RefCell<Option<crate::platform::imp::ContentPolicyTimeout>>,
    #[cfg(target_os = "windows")]
    request_witness: Option<crate::platform::imp::RequestWitness>,
    // The DevTools window id of this exact controller, which never changes:
    // asked once instead of on every extension toolbar refresh.
    #[cfg(target_os = "windows")]
    extension_window: Cell<Option<i64>>,
    #[cfg(target_os = "windows")]
    windows_final_discard: Option<Arc<discard::WindowsFinalDiscard>>,
    #[cfg(target_os = "windows")]
    discard_deadline: Option<crate::platform::imp::ContentPolicyTimeout>,
    #[cfg(target_os = "windows")]
    suspend_deadline: Option<crate::platform::imp::ContentPolicyTimeout>,
    #[cfg(target_os = "windows")]
    suspend_attempt: Option<Arc<discard::SuspendAttempt>>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    paint_cover: Option<crate::platform::imp::PaintCover>,
    #[cfg(target_os = "macos")]
    popup_policy: Rc<popup_sites::ViewPopupPolicy>,
    #[cfg(target_os = "macos")]
    file_uploads: Rc<file_uploads::FileUploadBroker>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    http_auth: Rc<http_auth::HttpAuthBroker>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    certificates: Rc<certificates::CertificateWatch>,
    // A current layout may request a view before its first document commits
    // (for example a download URL entered in a new tab). This permits only a
    // native download decision, never document presentation or file access.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    download_surface_intent: Arc<AtomicBool>,
    event_permit: EventPermit,
    navigation: NavigationEpochTracker,
    // Shared with every stage that can reveal this exact physical view.
    // Wry's commit guard flips it false synchronously before any native hide
    // can re-enter layout; verified exact privileged-chrome application is
    // the sole true transition for this generation.
    presentation_permit: Arc<AtomicBool>,
    // Native page zoom is per-view state. This is advanced only after Wry's
    // platform call succeeds and is returned with every settlement, making a
    // newest-per-item result authoritative even if intermediate results are
    // coalesced before the shell processes them.
    //
    // Shared with the Windows zoom listener, which tells a zoom made in the
    // page from one this engine applied by comparing against it.
    applied_zoom: Rc<Cell<f64>>,
    // False until privileged chrome has applied and verified this exact
    // committed main-frame URL/revision and the shell returns the same opaque
    // epoch. It is reset
    // when a warm spare is adopted, so the spare's old about:blank surface
    // can never be revealed.
    presentable: bool,
    // Emitted at most once for the initially hidden navigation. Native finish
    // may idempotently re-drive the same fact if queue coalescing replaced the
    // original commit notification.
    presentation_announced: Option<NavigationEpoch>,
    // Page titles have no portable native navigation identifier. They become
    // admissible only after this exact identity-bearing navigation finished;
    // transitional callbacks are discarded and the finished document's
    // current native title is queried under URL/epoch revalidation instead.
    title_ready: Option<NavigationEpoch>,
    // A warm spare's private about:blank commit predates logical ownership
    // and can never be used as a recovery surface for its adopted tab.
    nonpresentable_bootstrap: Option<NavigationEpoch>,
    #[cfg(target_os = "windows")]
    _crash_observer: crate::platform::imp::CrashObserver,
    #[cfg(target_os = "windows")]
    _accelerator_registration: Option<crate::platform::imp::AcceleratorRegistration>,
    #[cfg(target_os = "windows")]
    _zoom_registration: crate::platform::imp::ZoomRegistration,
    #[cfg(target_os = "windows")]
    _security_policy: crate::platform::imp::SecurityPolicy,
    _observer: crate::platform::imp::InstalledNavigationObserver,
    #[cfg(target_os = "windows")]
    cleanup_profile: ProfileId,
    #[cfg(target_os = "windows")]
    native_close_attempted: bool,
    #[cfg(target_os = "windows")]
    native_terminal_failure: Arc<dyn Fn(&'static str) + Send + Sync>,
    // Before `view`, so a running search is stopped while its view lives.
    find: Option<crate::platform::imp::FindSession>,
    view: WebView,
    // Keep this after `view`: fields drop in declaration order, so the native
    // WebView wrapper completes its teardown path before capacity can be
    // reissued. On Windows a failed explicit close takes and transfers the
    // lease to the retained cleanup debt instead.
    native_resource: Option<NativeResourceLease>,
}

/// A failed WebView2 close keeps the exact resource lease until every native
/// teardown step succeeds. A refused class transfer also retains the original
/// lease and is marked unaccounted so the host can quarantine construction;
/// it never releases capacity for a native object that may still exist.
#[cfg(target_os = "windows")]
struct OwnedWindowsCleanupDebt {
    debt: wry::WebView2CleanupDebt,
    _native_resource: Option<NativeResourceLease>,
    accounted_as_debt: bool,
    native_parent: Option<Box<dyn std::any::Any>>,
}

#[cfg(target_os = "windows")]
impl OwnedWindowsCleanupDebt {
    fn new(
        debt: wry::WebView2CleanupDebt,
        mut native_resource: Option<NativeResourceLease>,
    ) -> Self {
        let accounted_as_debt = native_resource
            .as_mut()
            .is_some_and(|resource| resource.mark_as_teardown_debt().is_ok());
        Self {
            debt,
            _native_resource: native_resource,
            accounted_as_debt,
            native_parent: None,
        }
    }

    fn with_parent(mut self, parent: impl std::any::Any) -> Self {
        self.native_parent = Some(Box::new(parent));
        self
    }

    fn retry(&mut self) -> Result<(), wry::WebView2CleanupFailure> {
        self.debt.retry()
    }

    fn accounted_as_debt(&self) -> bool {
        self.accounted_as_debt
    }
}

#[cfg(target_os = "windows")]
impl Drop for OwnedWindowsCleanupDebt {
    fn drop(&mut self) {
        if !self.debt.is_complete() {
            // Wry's debt destructor retains the apartment-bound native
            // obligation in its fallback registry. There is no channel for
            // carrying our lease with that raw fallback entry, so leak only
            // the accounting lease as a permanent fail-closed reservation.
            // Releasing it here could authorize a replacement around an
            // unproven controller/HWND teardown.
            if let Some(parent) = self.native_parent.take() {
                std::mem::forget(parent);
            }
            if let Some(native_resource) = self._native_resource.take() {
                std::mem::forget(native_resource);
            }
        }
    }
}

impl Drop for ObservedView {
    fn drop(&mut self) {
        // Revoke before native observers and the WebView are dropped. Any
        // callback already queued elsewhere still carries this permit and is
        // rejected even if the shell reuses the same logical ItemId.
        self.event_permit.revoke();
        self.navigation.revoke();
        #[cfg(target_os = "macos")]
        self.file_uploads.cancel();
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.http_auth.cancel();
        #[cfg(target_os = "windows")]
        {
            use wry::WebViewExtWindows;
            if !self.native_close_attempted {
                self.native_close_attempted = true;
                if let Err(debt) = self.view.close() {
                    let debt = OwnedWindowsCleanupDebt::new(debt, self.native_resource.take());
                    if !debt.accounted_as_debt() {
                        (self.native_terminal_failure)(
                            "WebView2 cleanup debt exceeded native resource accounting",
                        );
                    }
                    queue_windows_cleanup_debt(self.cleanup_profile, debt);
                }
            }
        }
    }
}

impl ObservedView {
    #[cfg(target_os = "windows")]
    fn close_explicit(mut self) -> (Option<OwnedWindowsCleanupDebt>, bool) {
        use wry::WebViewExtWindows;
        self.event_permit.revoke();
        self.navigation.revoke();
        let policy_cleanup_failed = false;
        self.native_close_attempted = true;
        let debt = self
            .view
            .close()
            .err()
            .map(|debt| OwnedWindowsCleanupDebt::new(debt, self.native_resource.take()));
        (debt, policy_cleanup_failed)
    }
}

impl Deref for ObservedView {
    type Target = WebView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NavigationSnapshot {
    url: Option<String>,
    history: Option<(bool, bool)>,
}

pub(crate) struct EngineHost {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    parent: ParentHandle,
    // Content web data never shares a directory or WebContext with the
    // privileged Tauri chrome. A context is further partitioned per profile.
    #[cfg(not(target_os = "macos"))]
    profiles_root: PathBuf,
    #[cfg(target_os = "windows")]
    private_runtime: zephium_core::webview2::RuntimeGeneration,
    views: HashMap<ItemId, ObservedView>,
    native_resources: NativeResourceLedger,
    // Allocates only after an explicit Shell publication. Ordinary inert
    // startup retains the empty map and creates no native delegate graph.
    #[cfg(target_os = "macos")]
    page_permissions: page_permissions::PagePermissionBroker,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub(crate) downloads: Option<Rc<downloads::Downloads>>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    native_open_authority: Arc<crate::NativeOpenAuthority>,
    extension_browser_surfaces:
        HashMap<ProfileId, zephium_core::extensions::ExtensionBrowserSurface>,
    native_resource_accounting_failed: bool,
    navigation_snapshots: HashMap<ItemId, NavigationSnapshot>,
    partitions: HashMap<ItemId, Partition>,
    // A ProfileId can never change between disk-backed and ephemeral native
    // storage in one process. Closing, crashing, or erasing a profile does not
    // relax this binding and therefore cannot resurrect a UDF in private mode.
    profile_persistence_classes: HashMap<ProfileId, ProfilePersistenceClass>,
    deck_appearance: zephium_core::deck::DeckAppearance,
    hide_advertisements: bool,
    #[cfg(target_os = "windows")]
    main_dispatch: crate::MainThreadDispatch,
    popup_sites: HashMap<ProfileId, Rc<popup_sites::PopupSites>>,
    /// Certificates people chose to continue past, per profile, until quit.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    certificate_exceptions: HashMap<ProfileId, certificates::SharedExceptions>,
    spare: Option<Spare>,
    memory_pressure: zephium_core::ports::engine::MemoryPressure,
    #[cfg(target_os = "macos")]
    discarded_states: HashMap<ItemId, discard::DiscardedState>,
    #[cfg(target_os = "macos")]
    prepared_discard_states: HashMap<ItemId, discard::DiscardedState>,
    #[cfg(target_os = "macos")]
    discarded_state_revision: u64,
    // A restored tab's last frame and the URL it shows, awaiting that
    // document's first commit. A redirect elsewhere discards it unseen.
    #[cfg(target_os = "macos")]
    restore_snapshots: HashMap<
        ItemId,
        (
            zephium_core::ports::engine::Partition,
            String,
            crate::platform::imp::PageSnapshot,
        ),
    >,
    /// Shared with every live view's key handler, so a rebinding reaches
    /// open pages without rebuilding them.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fullscreen: crate::fullscreen::FullscreenLedger,
    #[cfg(target_os = "macos")]
    fullscreen_retiring: HashMap<u64, fullscreen::RetiringView>,
    #[cfg(target_os = "macos")]
    next_fullscreen_retirement: u64,
    #[cfg(target_os = "macos")]
    stages: HashMap<WindowId, Retained<ContentStage>>,
    #[cfg(not(target_os = "macos"))]
    stages: HashMap<WindowId, crate::platform::imp::Stage>,
    // Native callback admission failure, exhausted stage retries, or
    // unverifiable teardown must seal outer lifecycle/event authority before
    // the mandatory fatal path.
    native_terminal_failure: Arc<dyn Fn(&'static str) + Send + Sync>,
    // A private profile owns exactly one non-persistent WKWebsiteDataStore for
    // its entire host lifetime. Each tab gets a fresh configuration pointing
    // at this retained store; distinct profile ids can never share one.
    #[cfg(target_os = "macos")]
    macos_ephemeral_data_stores: HashMap<ProfileId, crate::platform::imp::WebsiteDataStore>,
    #[cfg(target_os = "macos")]
    webext: webext::WebextHost,
    #[cfg(target_os = "macos")]
    extension_storage: extension_storage::Storage,
    #[cfg(target_os = "windows")]
    windows_extensions: webext_windows::WindowsExtensions,
    // Off-screen views carrying the low-memory hint, and the subset the
    // shell's idle policy asked WebView2 to suspend.
    #[cfg(target_os = "windows")]
    hidden: std::collections::HashSet<ItemId>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    dormant: std::collections::HashSet<ItemId>,
    #[cfg(target_os = "windows")]
    desired_dormant: std::collections::HashSet<ItemId>,
    #[cfg(target_os = "windows")]
    suspending: std::collections::HashSet<ItemId>,
    #[cfg(target_os = "windows")]
    suspend_failed: std::collections::HashSet<ItemId>,
    #[cfg(target_os = "windows")]
    suspend_uncertain: std::collections::HashSet<ItemId>,
    // Dormant views that skipped a cosmetic refresh, owed one when they wake.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    styles_missed: std::collections::HashSet<ItemId>,
    #[cfg(not(target_os = "macos"))]
    web_contexts: HashMap<ProfileId, wry::WebContext>,
    // Native managers outlive every associated view/context until a profile
    // erasure has both cleared+fetched each manager and verified disk absence.
    // This closes the last-tab and failed-post-build proof gaps.
    #[cfg(all(unix, not(target_os = "macos")))]
    linux_data_managers: HashMap<ProfileId, Vec<webkit2gtk::WebsiteDataManager>>,
    #[cfg(all(unix, not(target_os = "macos")))]
    linux_unverifiable_data_managers: HashSet<ProfileId>,
    // Wry's WebContext is only a data-path holder on Windows. Reusing the
    // actual environment is what keeps one browser/network process group per
    // profile instead of creating one per tab.
    #[cfg(target_os = "windows")]
    browser_version_observers: HashMap<ProfileId, crate::platform::imp::BrowserVersionObserver>,
    #[cfg(target_os = "windows")]
    environments: HashMap<ProfileId, ICoreWebView2Environment>,
    #[cfg(target_os = "windows")]
    browser_processes: HashMap<ProfileId, crate::platform::imp::BrowserProcess>,
    #[cfg(target_os = "windows")]
    browser_process_exit_observers:
        HashMap<ProfileId, crate::platform::imp::BrowserProcessExitObserver>,
    // Exact process/proof pairs have moved to the one shutdown worker. Native
    // observers still record their proofs; late UI bookkeeping cannot retake
    // or poison those already-transferred obligations.
    #[cfg(target_os = "windows")]
    windows_process_shutdown_started: bool,
    // ProcessFailed retires controllers, but only the exact Environment5
    // BrowserProcessExited proof authorizes replacements. Retain every logical
    // id across that gap and emit it after the construction gate is reopened.
    #[cfg(target_os = "windows")]
    pending_profile_recovery: HashMap<ProfileId, Vec<ItemId>>,
    #[cfg(target_os = "windows")]
    exiting_browser_processes: HashSet<ProfileId>,
    #[cfg(target_os = "windows")]
    unverifiable_browser_processes: HashSet<ProfileId>,
    // Set before Wry begins a fallible controller build and cleared only once
    // the resulting environment, exact process HANDLE and Environment5 proof
    // are installed/revalidated. Empty process maps are not proof of absence
    // while this marker exists.
    #[cfg(target_os = "windows")]
    construction_unproven: HashSet<ProfileId>,
    // Unexpected or partially captured groups remain retained even though
    // their missing Environment5 proof makes erasure/shutdown fail closed.
    #[cfg(target_os = "windows")]
    unproven_browser_processes: HashMap<ProfileId, crate::platform::imp::BrowserProcess>,
    #[cfg(target_os = "windows")]
    unproven_environments: HashMap<ProfileId, ICoreWebView2Environment>,
    // A failed Controller::Close, parent-subclass removal, or Wry container
    // destruction remains an owned native obligation. It is never converted
    // into successful close/erasure merely because Rust released other COM
    // references.
    #[cfg(target_os = "windows")]
    windows_cleanup_debts: HashMap<ProfileId, Vec<OwnedWindowsCleanupDebt>>,
    #[cfg(target_os = "windows")]
    windows_cleanup_invariant_failed: bool,
    // A tombstone is process-lifetime state: no failed/partial deletion may
    // silently make the profile usable again. Attempts are separate so a
    // settled failure can be retried, while a caller-visible timeout remains
    // in flight until native work reaches a terminal state.
    erasure_tombstones: HashSet<ProfileId>,
    erasure_attempts: HashMap<ProfileId, Arc<std::sync::atomic::AtomicBool>>,
    pub(crate) sink: Sink,
}
