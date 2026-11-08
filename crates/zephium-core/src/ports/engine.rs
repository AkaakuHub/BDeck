use std::sync::Arc;

use crate::extensions::{
    ExtensionBrowserRequest, ExtensionBrowserRequestId, ExtensionBrowserRequestSettlement,
    ExtensionBrowserSurface,
};
use crate::geometry::Rect;
use crate::ids::{ExtensionInstallId, ItemId, ProfileId, WindowId};
pub use crate::permissions::PagePermissionKind as PermissionKind;
use crate::permissions::{
    PagePermissionRequest, PagePermissionRequestId, PagePermissionRequestSettlement,
};
use crate::runtime_security::RuntimeSecurityAdvisories;
use crate::split::Pane;

/// OS memory pressure, independent of tab count or process RSS estimates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemoryPressure {
    #[default]
    Normal,
    Warning,
    Critical,
}

/// Native capture observations; page JavaScript cannot establish these facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaptureDeviceState {
    #[default]
    None,
    Active,
    Muted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MediaCaptureState {
    pub camera: CaptureDeviceState,
    pub microphone: CaptureDeviceState,
}

impl MediaCaptureState {
    pub fn is_capturing(self) -> bool {
        self.camera != CaptureDeviceState::None || self.microphone != CaptureDeviceState::None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebExtensionLoad {
    pub install: ExtensionInstallId,
    pub extension_id: String,
    pub root: std::path::PathBuf,
    pub permissions: Vec<String>,
    pub external_native_messaging: bool,
    pub match_patterns: Vec<String>,
    pub start_background: bool,
}

impl WebExtensionLoad {
    pub fn extends_access_of(&self, previous: &Self) -> bool {
        self.install == previous.install
            && self.extension_id == previous.extension_id
            && self.root == previous.root
            && self.external_native_messaging == previous.external_native_messaging
            && previous
                .permissions
                .iter()
                .all(|value| self.permissions.contains(value))
            && previous
                .match_patterns
                .iter()
                .all(|value| self.match_patterns.contains(value))
            && (self
                .permissions
                .iter()
                .any(|value| !previous.permissions.contains(value))
                || self
                    .match_patterns
                    .iter()
                    .any(|value| !previous.match_patterns.contains(value)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebExtensionLoaded {
    pub name: String,
    pub version: String,
}

/// Which engine data partition a view lives in. Every persistent profile gets
/// its own engine store; incognito is ephemeral and never intentionally
/// persists browsing data. Within one engine process a `ProfileId` is
/// permanently bound to either the durable class (`Default`/`Persistent`) or
/// `Ephemeral`; callers must mint a new id instead of changing that class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Partition {
    Default(ProfileId),
    Persistent(ProfileId),
    Ephemeral(ProfileId),
}

impl Partition {
    pub fn profile(self) -> ProfileId {
        match self {
            Partition::Default(p) | Partition::Persistent(p) | Partition::Ephemeral(p) => p,
        }
    }
}

/// What the engine can truthfully establish at its synchronous call boundary.
///
/// `Scheduled` is deliberately not a claim that WebKit/WebView2 applied the
/// action, navigated, or produced a renderer result. It proves that the exact,
/// lifecycle-checked task was admitted to the owning native UI event loop. A
/// task can still be invalidated by an ordered close before that queue runs.
///
/// Some platforms have a second bounded host queue because a native engine
/// can pump the event loop re-entrantly. Refusal at that later boundary is not
/// silently treated as success: the backend terminally revokes content
/// authority and invokes its mandatory fatal callback.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeDispatch {
    Scheduled,
    Rejected,
    Unsupported,
}

/// Opaque identity for one native main-frame navigation presentation.
///
/// The browser shell may return this token only to [`Engine::present_navigation`].
/// URLs are deliberately not identities: redirects and overlapping loads can
/// otherwise reveal content before chrome has attributed the exact commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NavigationPresentationId(u64);

impl NavigationPresentationId {
    /// Native adapters mint non-wrapping identities from their own exact
    /// navigation epochs. Callers must treat the value as opaque.
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub const fn into_raw(self) -> u64 {
        self.0
    }
}

impl NativeDispatch {
    pub fn from_scheduled(scheduled: bool) -> Self {
        if scheduled {
            Self::Scheduled
        } else {
            Self::Rejected
        }
    }
}

/// Result of permanently retiring an engine profile and erasing the native
/// web engine data it owned.
///
/// `Verified` is deliberately narrow: the engine has released every native
/// view/context it knows for the profile and proved its engine-owned data
/// store absent (or empty for an ephemeral native store). It does not include
/// Zephium's SQLite/session data; callers must coordinate that separate store
/// transaction before removing a profile from the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDataErasureOutcome {
    /// Native work settled and the scoped engine data was proven absent.
    Verified,
    /// The native attempt reached a terminal failure, including refusal to
    /// dispatch it. A retry may be admitted, but the synchronous retirement
    /// gate remains permanent and this outcome is never proof that existing
    /// native references closed or that their data was deleted. A dispatcher
    /// refusal is terminal for continued browsing and requires process exit
    /// unless a later attempt reaches `Verified`.
    Failed,
    /// The caller's bounded wait elapsed. This is not native cancellation or
    /// settlement; the in-process attempt remains active until a later native
    /// terminal callback, so an immediate retry must be rejected. Existing
    /// native references and data are unproven; continued browsing is unsafe
    /// and the coordinator must drive verified teardown or process exit.
    TimedOut,
}

/// Longest text one find may search for.
pub const MAX_FIND_QUERY_BYTES: usize = 1024;
/// Matches an engine counts before it stops.
pub const MAX_FIND_MATCHES: u32 = 1000;

/// A find in the page. Repeating the same `query` steps to the next match
/// (or the previous one when `forward` is false); a different query starts a
/// new search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindRequest {
    pub query: String,
    pub forward: bool,
}

/// A resolved keyboard shortcut for platforms where the engine must
/// intercept keys natively (WebView2 AcceleratorKeyPressed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub id: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: u32,
}

/// How a content layout that follows a deliberate change of the window's
/// shape is carried out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageMotion {
    /// The content keeps its size for the journey and slides to its new
    /// place, so a page is laid out at most once.
    Slide,
    /// The content was hidden behind a browser page and returns to view.
    Arrive,
}

/// A tab dragged over a pane's edge, shown before it is dropped: the pane's
/// page slides aside by `shift` without being resized and the tint marks where
/// the new pane will stand. Rects are relative to the content region; nothing
/// is laid out until the drop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropPreview {
    pub tab: ItemId,
    pub zone: Rect,
    pub shift: (f64, f64),
}

/// Where page content goes when it enters element fullscreen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FullscreenPresentation {
    /// The engine moves the page into a fullscreen window of its own (WebKit
    /// on macOS). Browser layout stays as it is.
    OwnWindow,
    /// The page fills its own view only (WebView2). The browser makes its
    /// window fullscreen and lays that one view over the whole of it.
    FillHostWindow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebExtensionAccessRequest {
    pub profile: ProfileId,
    pub request: u64,
    pub extension_id: String,
    pub warnings: Vec<String>,
    pub permissions: Vec<String>,
    pub patterns: Vec<String>,
}

pub trait Engine {
    /// Replaces the sites in `profile` whose pages may open a new tab
    /// without a click.
    fn set_popup_sites(&self, _profile: ProfileId, _origins: Vec<crate::permissions::PageOrigin>) {}
    /// The decisions the engine itself saved in `profile`. None when it keeps
    /// none of its own, as on macOS, or cannot read them now.
    fn site_permissions(&self, _profile: ProfileId, done: crate::permissions::SitePermissionsDone) {
        done(None);
    }
    /// Saves one decision in the engine's own store for `profile`; None
    /// forgets it so the site is asked again.
    fn set_site_permission(
        &self,
        _profile: ProfileId,
        _origin: crate::permissions::PageOrigin,
        _kind: crate::permissions::SitePermissionKind,
        _decision: Option<crate::permissions::RememberedPagePermission>,
        done: crate::permissions::SitePermissionSetDone,
    ) {
        done(false);
    }
    /// Browser-owned file actions. Only trusted Shell admission supplies the
    /// profile partition; the caller supplies IDs, never filesystem paths.
    fn download_call(
        &self,
        _partition: Partition,
        _call: crate::downloads::DownloadCall,
        done: crate::downloads::DownloadCompletion,
    ) -> bool {
        done.finish(crate::downloads::DownloadResponse::Error {
            error: crate::downloads::DownloadError::Unsupported,
        });
        true
    }

    /// Schedules creation on the native UI thread. `false` means the request
    /// was not admitted at all, so the shell must roll back its live-view bit.
    fn create_view(&self, id: ItemId, partition: Partition, url: &str, bounds: Rect) -> bool;
    /// Schedules a navigation on the native UI thread. `false` means the
    /// request was not admitted. An admitted request can still fail before
    /// its identity-bearing native commit, synchronously or asynchronously;
    /// that arrives as `EngineEvent::NavigationFailed` with the same request
    /// id.
    fn navigate(&self, id: ItemId, url: &str, request: NavigationRequestId) -> bool;
    /// Reveals an initially hidden native view only if `navigation` is still
    /// the exact committed main-frame epoch whose URL was delivered to the
    /// shell. The shell calls this only after privileged chrome has applied
    /// and verified the matching revision-bearing URL projection; native
    /// completion may re-drive the same idempotent acknowledgement.
    fn present_navigation(
        &self,
        _id: ItemId,
        _navigation: NavigationPresentationId,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn reload(&self, id: ItemId) -> NativeDispatch;
    /// Reloads without using the cache, where the engine offers it; a plain
    /// reload where it does not.
    fn hard_reload(&self, id: ItemId) -> NativeDispatch {
        self.reload(id)
    }
    fn stop(&self, id: ItemId) -> NativeDispatch;
    fn go_back(&self, id: ItemId) -> NativeDispatch;
    fn go_forward(&self, id: ItemId) -> NativeDispatch;
    /// Asks for the entries the view can go back and forward to. The answer
    /// arrives as `EngineEvent::HistoryEntries` carrying `request`.
    fn history_entries(&self, _id: ItemId, _request: HistoryRequestId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Goes to the entry `offset` steps from the current one, negative for
    /// back. The engine goes only if that entry is still at `url`, so a page
    /// that navigated since the list was read is never sent somewhere else.
    fn go_to_history_entry(&self, _id: ItemId, _offset: i32, _url: &str) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn close(&self, id: ItemId) -> NativeDispatch;
    /// Lay the split `tree` into `region` of `window`, or hide that window's
    /// content when `region` is `None`. The engine owns pane geometry so
    /// resize stays in the native pass.
    fn set_content(
        &self,
        window: WindowId,
        tree: Option<Pane>,
        region: Option<Rect>,
    ) -> NativeDispatch;
    fn set_composer(
        &self,
        _item: Option<ItemId>,
        _region: Option<Rect>,
        _floating: bool,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn focus_composer(&self, _item: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn focus(&self, _item: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn set_drop_indicator(&self, window: WindowId, preview: Option<DropPreview>) -> NativeDispatch;
    /// A transient guide in window coordinates; never changes pane geometry.
    fn set_resize_guide(&self, _window: WindowId, _zone: Option<Rect>) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Stacks the chrome over the content while a hidden sidebar is shown, or
    /// back beneath it. Only the stacking changes; nothing is laid out.
    /// `card` is where the column stands over the page, in window
    /// coordinates, for a platform whose chrome must be cut to it.
    /// `slide` follows the card as it slides in or out, rather than cutting
    /// to it at once.
    fn raise_chrome(
        &self,
        _window: WindowId,
        _raised: bool,
        _card: Rect,
        _slide: bool,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Asks the next content layout applied to `window` to move rather than
    /// jump, because it follows a deliberate change of the window's shape
    /// and not a resize. Consumed by that layout whether or not it moved
    /// anything; an engine without native motion ignores it.
    fn hint_stage_motion(&self, _window: WindowId, _motion: StageMotion) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Requests an exact page zoom for the current native-view generation.
    /// Queue admission is not application: the authoritative native scale
    /// arrives as [`EngineEvent::ZoomSettled`] carrying the same `request`.
    fn zoom(&self, id: ItemId, scale: f64, request: ZoomRequestId) -> NativeDispatch;
    /// Mutes or unmutes the current native view. The view keeps it across
    /// navigations; a new view starts unmuted, so the shell sends it again.
    fn set_muted(&self, id: ItemId, muted: bool) -> NativeDispatch;
    /// Whether [`Engine::set_muted`] works here, so chrome can offer it.
    fn can_mute(&self) -> bool {
        false
    }
    /// Watches the current document's media until it has none, reporting
    /// each change as [`EngineEvent::MediaChanged`]. A tab already watched is
    /// left as it is.
    fn watch_media(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Acts on the media the watch reports. The new state arrives through
    /// the watch.
    fn media_action(&self, _id: ItemId, _action: MediaAction) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// One step of finding text in the page; `None` ends the session and
    /// clears its highlights. Results arrive as `EngineEvent::FindResult`.
    fn find(&self, id: ItemId, request: Option<FindRequest>) -> NativeDispatch;
    /// Result arrives as `EngineEvent::Captured`.
    fn capture(&self, id: ItemId) -> NativeDispatch;
    /// Saves the page in front through the engine's own Save As: the web
    /// engine's dialog on Windows, a save panel offering a web archive or the
    /// page's source on macOS. Only a page a person reads is ever saved, and
    /// nothing is written until they choose where.
    fn save_page(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Opens the system's saved-passwords panel at the text field focused in
    /// the page, as the page's right-click AutoFill menu does. Nothing opens
    /// unless the page has the keyboard and a field in it has focus. WebView2
    /// offers saved passwords itself as a field takes focus.
    fn autofill_password(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Starts watching the committed document's icon links: what they name
    /// arrives as `EngineEvent::FaviconCandidates` when the head first has
    /// icons and whenever they change until the page has loaded. One call per
    /// committed document; a later call replaces the earlier watch. Nothing is
    /// polled.
    fn discover_favicon(&self, id: ItemId) -> NativeDispatch;
    /// Asks the sandboxed page renderer to fetch and decode one icon address
    /// the page declared. An exact 32x32 RGBA result arrives as
    /// `EngineEvent::FaviconPixels`, a failure as `FaviconRasterFailed`.
    fn rasterize_favicon(&self, id: ItemId, url: &str) -> NativeDispatch;
    /// Whether the engine reports each page's icon itself as
    /// `EngineEvent::FaviconImage`, the exact one the web engine chose. The
    /// declared candidates then only back it up.
    fn native_favicons(&self) -> bool {
        false
    }
    /// Asynchronously asks the exact live native-view generation and its
    /// current committed navigation whether it is safe to discard. A missing
    /// or malformed response is deliberately not a positive result. Callers
    /// must correlate `probe` with `EngineEvent::DiscardSafety` and recheck
    /// visibility/loading state before closing the view.
    fn probe_discard_safety(&self, id: ItemId, probe: DiscardProbeId) -> bool;
    /// Refreshes safety and prepares restoration before retiring a probed view.
    /// Refusal keeps the view live; successful completion arrives only after
    /// native destruction. False means admission failed without retirement.
    fn discard_view(&self, id: ItemId, probe: DiscardProbeId) -> bool;
    /// Synchronously cancels this exact pending discard before retirement.
    /// Its eventual terminal event still settles the caller's closing state.
    fn cancel_discard(&self, _id: ItemId, _probe: DiscardProbeId) {}
    /// Erases volatile restoration data on logical close or browsing-data deletion.
    /// None clears the profile; this never closes an active page.
    fn forget_discarded_state(&self, _profile: ProfileId, _item: Option<ItemId>) {}
    fn print(&self, id: ItemId) -> NativeDispatch;
    /// Opens the page's own inspector. Browser chrome is never inspectable.
    fn open_devtools(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Rejected
    }
    /// Hands an application link the person allowed to the system.
    fn open_external_app(&self, _url: &str) -> NativeDispatch {
        NativeDispatch::Rejected
    }
    fn set_extension_browser_surface(&self, _surface: ExtensionBrowserSurface) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn load_web_extension(&self, _profile: ProfileId, _load: WebExtensionLoad) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn unload_web_extension(
        &self,
        _profile: ProfileId,
        _install: ExtensionInstallId,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn open_web_extension_options(
        &self,
        _profile: ProfileId,
        _extension_id: String,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn answer_web_extension_access(
        &self,
        _profile: ProfileId,
        _request: u64,
        _allowed: bool,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn remove_web_extension(&self, _profile: ProfileId, _load: WebExtensionLoad) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn request_extension_actions(
        &self,
        _profile: ProfileId,
        _tab: ItemId,
        _surface_generation: crate::extensions::ExtensionBrowserSurfaceGeneration,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn invoke_extension_action(
        &self,
        _request: crate::extensions::ExtensionActionRequest,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn settle_extension_browser_request(
        &self,
        _profile: ProfileId,
        _request: ExtensionBrowserRequestId,
        _settlement: ExtensionBrowserRequestSettlement,
        _first_url_after_reply: Option<(Arc<str>, NavigationRequestId)>,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// The proxy views created from now on go through, or none for the
    /// system's. Views already open keep what they were made with.
    fn set_proxy(&self, _proxy: Option<crate::proxy::ProxyServer>) {}
    /// Applies the selected deck stylesheet to X documents only.
    fn set_deck_appearance(&self, _theme: crate::deck::DeckAppearance, _hide_advertisements: bool) {
    }
    /// Prebuild a hidden webview for `partition` so the next open adopts it
    /// instead of paying the renderer spawn. Safe moment: after a page load.
    fn warm_spare(&self, _partition: Partition) {}
    /// Hidden views the shell's idle policy wants suspended. The engine
    /// suspends where it has a primitive (WebView2, WebKit scheduling policy)
    /// and resumes implicitly when a view becomes visible again.
    fn set_dormant(&self, _ids: Vec<ItemId>) {}
    /// Whether `set_dormant` actually suspends hidden renderers here.
    fn suspends_hidden_views(&self) -> bool {
        false
    }
    /// Drops speculative resources during OS pressure; never evicts user work.
    fn set_memory_pressure(&self, _pressure: MemoryPressure) {}
    /// Requests `EngineEvent::PageMemory` for these live pages where the
    /// platform can attribute a renderer process to a page.
    fn sample_page_memory(&self, _ids: Vec<ItemId>) {}
    /// Settles one exact, origin-labelled native page-permission completion.
    ///
    /// The adapter must bind all four coordinates to the same live view and
    /// retained native request. Unknown, stale, navigated, retired, or already
    /// settled identities are rejected and can never target a replacement
    /// view reusing the same logical item id.
    fn settle_page_permission_request(
        &self,
        _profile: ProfileId,
        _item: ItemId,
        _request: PagePermissionRequestId,
        _settlement: PagePermissionRequestSettlement,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// The person chose to continue past the certificate the tab's engine
    /// refused at `url`. The engine accepts only an https address on a local
    /// host, with the exact leaf it refused there; anything else is ignored.
    fn allow_certificate_exception(&self, _item: ItemId, _url: String) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Answers the tab's sign-in prompt with credentials, or cancels it with
    /// None. A stale or unknown request is ignored and its credentials are
    /// dropped.
    fn answer_http_auth(
        &self,
        _item: ItemId,
        _request: u32,
        _credentials: Option<crate::http_auth::HttpCredentials>,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Ends a page that stopped responding: its view is closed as if its
    /// renderer had crashed, and `EngineEvent::Crashed` follows.
    fn end_unresponsive_page(&self, _item: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// How a page's element fullscreen is presented on this engine.
    fn fullscreen_presentation(&self) -> FullscreenPresentation {
        FullscreenPresentation::OwnWindow
    }
    /// Browser tab selection, never a page or Work command. Returning closes
    /// only the video PiP owned by this automatic presentation.
    fn automatic_pip(&self, _id: ItemId, _background: bool) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    fn cancel_automatic_pip(&self) {}

    /// Asks the page to leave element fullscreen. The result arrives as
    /// [`EngineEvent::FullscreenChanged`]; picture in picture is unaffected.
    fn exit_fullscreen(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Stop capture in the exact committed document. Unsupported engines
    /// expose no live indicator or pretend to stop through page JavaScript.
    fn stop_media_capture(
        &self,
        _item: ItemId,
        _navigation: NavigationPresentationId,
    ) -> NativeDispatch {
        NativeDispatch::Unsupported
    }
    /// Sticky process-local signal that the native browser runtime reported a
    /// newer version. `true` never means the running environments adopted the
    /// update: the composition root must use its ordinary ordered shutdown
    /// path and perform a whole-application restart, including privileged
    /// chrome, before clearing this state.
    fn runtime_restart_required(&self) -> bool {
        false
    }
    /// Non-fatal result of the process-start native runtime assessment.
    ///
    /// This is immutable for the current native process generation. It is
    /// computed locally before WebView construction and performs no network,
    /// filesystem, actor, or page-derived work.
    fn runtime_security_advisories(&self) -> RuntimeSecurityAdvisories {
        RuntimeSecurityAdvisories::new()
    }
    /// Permanently tombstones `profile` at the synchronous call boundary,
    /// rejects all future native access to it except cleanup, and schedules
    /// closure of every existing view/context plus asynchronous erasure of
    /// the profile's engine-owned data. The completion is invoked exactly
    /// once.
    ///
    /// Retirement does not depend on native-thread dispatch: even `Failed`
    /// leaves the profile inaccessible for the rest of this engine process.
    /// When dispatch itself fails, the backend must also reject continued
    /// content use globally because existing native pages cannot be proven
    /// closed; callers must terminate unless a retry verifies erasure.
    /// `Failed` may otherwise be retried because the native attempt is
    /// terminal, but it must never be interpreted as successful teardown.
    /// `TimedOut` is only a one-shot report to the caller: retry remains denied
    /// while the old native work might still be running, and a late terminal
    /// callback releases admission without invoking `done` again.
    fn erase_profile_data(
        &self,
        _profile: ProfileId,
        done: Box<dyn FnOnce(ProfileDataErasureOutcome) + Send>,
    );
    /// Close every native view/context on its owning thread. Completion runs
    /// only after native references have been released; composition roots use
    /// this as the final process-lifecycle barrier.
    fn shutdown(&self, done: Box<dyn FnOnce(bool) + Send>) {
        done(true);
    }
}

/// Correlates an explicit shell navigation with a native failure before its
/// identity-bearing main-frame commit.
///
/// Successful commits intentionally do not carry this token: page-initiated
/// navigations, redirects and same-document history changes have no shell
/// request and all use the same authoritative `UrlChanged` path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NavigationRequestId(pub u64);

/// Process-local correlation identity for one native page-zoom request.
///
/// Zoom is persisted only after the exact live native generation reports its
/// applied scale. Keeping this distinct from navigation identity prevents a
/// late result from a replaced view from mutating the replacement tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ZoomRequestId(pub u64);

/// Process-local correlation identity for one request of a view's history
/// list. A list read before the page navigated again is told apart by it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryRequestId(pub u64);

/// Entries kept on each side of the current one.
pub const MAX_HISTORY_SIDE: usize = 50;
pub const MAX_HISTORY_TITLE_CHARS: usize = 200;

/// One place a view can go back or forward to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Steps from the current entry: negative is back, positive forward.
    pub offset: i32,
    pub title: String,
    pub url: String,
}

/// The entries on each side of the current one, nearest first, so
/// `back[0]` is one step back and `forward[0]` one step ahead.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryEntries {
    pub back: Vec<HistoryEntry>,
    pub forward: Vec<HistoryEntry>,
}

impl HistoryEntries {
    /// Builds the list from every entry of a view in order and the index of the
    /// current one. Entries that are not an address the browser would load,
    /// such as the blank page a warm view starts on, are left out; the
    /// offsets keep counting through them, because they are still steps.
    pub fn from_session(entries: &[(String, String)], current: usize) -> Self {
        if current >= entries.len() {
            return Self::default();
        }
        let entry = |at: usize, offset: i32| {
            let (title, url) = entries.get(at)?;
            let web = url::Url::parse(url).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https") && crate::navigation::is_allowed(&url)
            });
            web.then(|| HistoryEntry {
                offset,
                title: crate::item::sanitize_page_title(title)
                    .chars()
                    .take(MAX_HISTORY_TITLE_CHARS)
                    .collect(),
                url: url.clone(),
            })
        };
        let step = |at: usize| i32::try_from(at).unwrap_or(i32::MAX);
        Self {
            back: (0..current)
                .rev()
                .take(MAX_HISTORY_SIDE)
                .filter_map(|at| entry(at, -step(current - at)))
                .collect(),
            forward: (current.saturating_add(1)..entries.len())
                .take(MAX_HISTORY_SIDE)
                .filter_map(|at| entry(at, step(at - current)))
                .collect(),
        }
    }
}

/// Process-local correlation identity for one renderer-state discard probe.
/// It is never persisted or accepted from page content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiscardProbeId(pub u64);

/// A native browser action whose synchronous platform invocation failed.
///
/// This does not describe page-load completion. Reload/history success still
/// settles through the ordinary navigation callbacks; this enum exists so an
/// HRESULT/native refusal is never silently discarded.
/// Why a navigation the person asked for did not load, as a category safe
/// to show them. Never carries native error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NavigationFailureReason {
    Offline,
    HostNotFound,
    Unreachable,
    TimedOut,
    Insecure,
    /// The server's certificate was refused, and what the engine could tell
    /// about why.
    Certificate(crate::certificates::CertificateProblem),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeAction {
    Reload,
    HardReload,
    GoBack,
    GoForward,
}

type NativeTabDecision = Box<dyn FnOnce(bool) + Send>;
struct NativeTabAdoptionInner(std::sync::Mutex<Option<NativeTabDecision>>);
#[derive(Clone)]
pub struct NativeTabAdoption(Arc<NativeTabAdoptionInner>);
impl NativeTabAdoption {
    pub fn new(done: impl FnOnce(bool) + Send + 'static) -> Self {
        Self(Arc::new(NativeTabAdoptionInner(std::sync::Mutex::new(
            Some(Box::new(done)),
        ))))
    }
    pub fn finish(&self, accepted: bool) {
        let done = self
            .0
             .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(done) = done {
            done(accepted);
        }
    }
}
impl Drop for NativeTabAdoptionInner {
    fn drop(&mut self) {
        if let Some(done) = self
            .0
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            done(false);
        }
    }
}
impl std::fmt::Debug for NativeTabAdoption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NativeTabAdoption")
    }
}
impl PartialEq for NativeTabAdoption {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// What the sidebar's player may ask of a tab's media.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaAction {
    Play,
    Pause,
    /// Seconds from the start.
    Seek(f64),
    /// Enters picture in picture, or leaves it.
    PictureInPicture,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    /// A native browser environment reported that a newer runtime is
    /// available. This is a process-global, sticky notification: recycling a
    /// content profile alone cannot update privileged chrome and must not be
    /// presented as successful adoption.
    RuntimeRestartRequired,
    ExtensionBrowserRequested {
        request: ExtensionBrowserRequest,
    },
    ExtensionCreatedTabReplied {
        profile: ProfileId,
        request: ExtensionBrowserRequestId,
        tab: ItemId,
        url: Arc<str>,
        intent: NavigationRequestId,
    },
    ExtensionPageClosed {
        profile: ProfileId,
        id: ItemId,
    },
    ExtensionPageChanged {
        profile: ProfileId,
        id: ItemId,
        title: String,
        loading: bool,
        can_go_back: bool,
        can_go_forward: bool,
    },
    ExtensionActionsSnapshotSettled {
        profile: ProfileId,
        tab: ItemId,
        surface_generation: crate::extensions::ExtensionBrowserSurfaceGeneration,
        settlement: crate::extensions::ExtensionActionSnapshotSettlement,
    },
    ExtensionActionSettled {
        profile: ProfileId,
        request: crate::extensions::ExtensionActionRequestId,
        settlement: crate::extensions::ExtensionActionSettlement,
    },
    ExtensionActionsInvalidated {
        profile: ProfileId,
    },
    WebExtensionSettled {
        profile: ProfileId,
        install: ExtensionInstallId,
        result: Result<WebExtensionLoaded, String>,
    },
    WebExtensionAccessRequested(Box<WebExtensionAccessRequest>),
    ExtensionActionShortcutRequested {
        runtime: crate::extensions::ExtensionRuntimeInstance,
        tab: ItemId,
        surface_generation: crate::extensions::ExtensionBrowserSurfaceGeneration,
    },
    TitleChanged {
        id: ItemId,
        title: String,
    },
    UrlChanged {
        id: ItemId,
        url: String,
    },
    /// The exact committed URL has already been delivered ahead of this
    /// event. After applying that URL to privileged chrome, the shell returns
    /// this opaque identity immediately to authorize the first presentation.
    PresentationPending {
        id: ItemId,
        navigation: NavigationPresentationId,
        /// Exact canonical URL emitted through `UrlChanged` immediately
        /// before this token. The shell must match both facts before it can
        /// acknowledge presentation; callback/queue order alone is not an
        /// authorization boundary.
        url: String,
    },
    /// Native completion re-drives the same exact presentation fact. This is
    /// an idempotent recovery path when a bounded queue coalesced `Pending`;
    /// it is not a prerequisite or an intentional first-paint delay.
    PresentationReady {
        id: ItemId,
        navigation: NavigationPresentationId,
        /// Same committed URL bound to `navigation`. This is repeated because
        /// a bounded lifecycle queue may coalesce `Pending` into `Ready`.
        url: String,
    },
    NavigationFailed {
        id: ItemId,
        request: NavigationRequestId,
    },
    /// Why the view's current main-frame navigation failed, reported before
    /// its failure settles.
    NavigationFailureReported {
        id: ItemId,
        reason: NavigationFailureReason,
        /// The main-frame address that failed, from the engine's own
        /// navigation record: for a refused certificate, and whenever
        /// `replaced_page` is set. It may differ from the one asked for after
        /// redirects.
        url: Option<String>,
        /// The engine put an empty error document in place of the page that
        /// was showing (WebView2). The view and its history stay; chrome
        /// shows the failure over it until another document commits.
        replaced_page: bool,
    },
    /// The exact native zoom invocation settled. `applied_scale` is the
    /// adapter's last successfully applied scale for this view generation, so
    /// the newest event remains authoritative even when intermediate results
    /// are coalesced under pressure.
    ZoomSettled {
        id: ItemId,
        request: ZoomRequestId,
        applied_scale: f64,
        succeeded: bool,
    },
    /// The person zoomed the page itself, such as with Ctrl and the mouse
    /// wheel, and the view already shows `scale`. It carries no request:
    /// nothing was asked of the engine.
    ZoomGesture {
        id: ItemId,
        scale: f64,
    },
    /// A reload/history platform call returned an error. An `Ok` call is only
    /// native invocation success, never a claim that navigation completed.
    NativeActionFailed {
        id: ItemId,
        action: NativeAction,
    },
    LoadingChanged {
        id: ItemId,
        loading: bool,
    },
    MediaCaptureChanged {
        id: ItemId,
        navigation: NavigationPresentationId,
        state: MediaCaptureState,
    },
    /// The page's renderer stopped responding (`responding: false`, repeated
    /// every few seconds while it stays hung) or ran again.
    PageResponsiveness {
        id: ItemId,
        responding: bool,
    },
    /// The page started or stopped making sound, muted or not. Native state
    /// that page script cannot fake.
    AudioChanged {
        id: ItemId,
        audible: bool,
    },
    /// What the page's media is doing, as page script reports it; `None`
    /// once the document has none it can control, or is gone.
    MediaChanged {
        id: ItemId,
        media: Option<Box<crate::media::MediaSnapshot>>,
    },
    /// The page entered or left element fullscreen. At most one view per
    /// engine is reported active at a time.
    FullscreenChanged {
        id: ItemId,
        active: bool,
    },
    FaviconPixels {
        id: ItemId,
        page_url: String,
        rgba: Vec<u8>,
    },
    /// The page renderer could not decode the address it was asked to.
    FaviconRasterFailed {
        id: ItemId,
        page_url: String,
        url: String,
    },
    /// The icon addresses a committed document's head names. Every report is
    /// a full snapshot; `complete` marks the last one the watch makes.
    FaviconCandidates {
        id: ItemId,
        page_url: String,
        complete: bool,
        candidates: Vec<crate::icon::IconCandidate>,
    },
    /// The icon the web engine itself chose for a page, as a bounded PNG. The
    /// privileged side decodes it with the same decoders as any fetched icon.
    FaviconImage {
        id: ItemId,
        page_url: String,
        png: Vec<u8>,
    },
    /// Positive results have already passed exact native-view generation,
    /// navigation-epoch, fixed-schema DOM-state, and (where available)
    /// native audio-state checks. The shell still owns the final visibility
    /// and idle/budget revalidation.
    DiscardSafety {
        id: ItemId,
        probe: DiscardProbeId,
        can_discard: bool,
    },
    ViewDiscarded {
        id: ItemId,
        profile: ProfileId,
        probe: DiscardProbeId,
    },
    /// Final native safety/restoration checks vetoed a requested discard.
    /// The exact resident view remains alive; this is not a close acknowledgement.
    ViewDiscardRefused {
        id: ItemId,
        profile: ProfileId,
        probe: DiscardProbeId,
    },
    /// Physical footprint of the renderer process serving one live page.
    PageMemory {
        id: ItemId,
        profile: ProfileId,
        bytes: u64,
    },
    NavState {
        id: ItemId,
        can_go_back: bool,
        can_go_forward: bool,
    },
    /// The answer to `Engine::history_entries`.
    HistoryEntries {
        id: ItemId,
        request: HistoryRequestId,
        entries: HistoryEntries,
    },
    /// The engine already owns this fully configured native child. Shell must
    /// adopt the exact id or reject the lease; it must never replay the URL.
    NativeTabOpened {
        id: ItemId,
        child: ItemId,
        foreground: bool,
        adoption: NativeTabAdoption,
    },
    NativeTabCloseRequested {
        id: ItemId,
    },
    PageOpenBlocked {
        id: ItemId,
        /// The refused page, only when it is an ordinary web address.
        url: Option<String>,
    },
    /// The page asked to open a link in another application. Nothing is
    /// opened until the person allows it.
    ExternalAppRequested {
        id: ItemId,
        url: String,
        app: Option<String>,
        /// The person clicked, rather than the page navigating by itself.
        user_gesture: bool,
    },
    LinkedDownloadStarted {
        id: ItemId,
    },
    NewWindowRequested {
        id: ItemId,
        url: String,
    },
    PermissionRequested {
        id: ItemId,
        profile: ProfileId,
        /// The committed document that asked. The engine settles an Allow
        /// only while this document is still the view's current one.
        navigation: NavigationPresentationId,
        request: PagePermissionRequest,
    },
    /// The tab's site, or its proxy, asks the person to sign in. The native
    /// request waits until [`Engine::answer_http_auth`] or until the
    /// engine cancels it, which [`EngineEvent::HttpAuthEnded`] reports.
    HttpAuthRequested {
        id: ItemId,
        prompt: crate::http_auth::HttpAuthPrompt,
    },
    /// The engine cancelled this prompt itself: the tab navigated, a newer
    /// challenge replaced it, or it waited too long.
    HttpAuthEnded {
        id: ItemId,
        request: u32,
    },
    DownloadRequested {
        id: ItemId,
        url: String,
    },
    ViewCreationFailed {
        id: ItemId,
    },
    /// The engine has already revoked and physically removed every listed
    /// native controller from the exited browser-process generation. The
    /// shell may recreate an id and must not issue a second id-only close.
    ProfileProcessExited {
        profile: ProfileId,
        ids: Vec<ItemId>,
    },
    /// The exact native-view generation was revoked and physically removed
    /// before this event was emitted. The shell may recreate `id` and must not
    /// send a cleanup close that could race the replacement generation.
    Crashed {
        id: ItemId,
    },
    Captured {
        id: ItemId,
        png: Vec<u8>,
    },
    /// Where a find in `id` landed. `query` names the search it answers, so a
    /// result for text since replaced can be told apart.
    FindResult {
        id: ItemId,
        query: String,
        matches: u32,
        /// One-based position of the current match, when the engine knows it.
        active: Option<u32>,
    },
    SplitChanged {
        window: WindowId,
        tree: Pane,
    },
    ShortcutPressed {
        item: ItemId,
        command: String,
    },
}

#[cfg(test)]
mod native_tab_adoption_tests {
    use super::*;
    #[test]
    fn abandoned_native_tabs_are_rejected_once_after_the_last_clone() {
        let (send, receive) = std::sync::mpsc::channel();
        let lease = NativeTabAdoption::new(move |accepted| send.send(accepted).unwrap());
        let retained = lease.clone();
        drop(lease);
        assert!(receive.try_recv().is_err());
        drop(retained);
        assert!(!receive.recv().unwrap());
        assert!(receive.try_recv().is_err());
    }
    #[test]
    fn a_settled_adoption_cannot_be_rejected_by_a_late_clone() {
        let (send, receive) = std::sync::mpsc::channel();
        let lease = NativeTabAdoption::new(move |accepted| send.send(accepted).unwrap());
        let retained = lease.clone();
        lease.finish(true);
        retained.finish(false);
        drop(lease);
        drop(retained);
        assert!(receive.recv().unwrap());
        assert!(receive.try_recv().is_err());
    }
}

#[cfg(test)]
mod history_entries_tests {
    use super::*;

    fn session(urls: &[&str]) -> Vec<(String, String)> {
        urls.iter()
            .map(|url| (format!("Title of {url}"), (*url).to_owned()))
            .collect()
    }

    #[test]
    fn entries_are_counted_in_steps_from_the_current_one_nearest_first() {
        let list = HistoryEntries::from_session(
            &session(&[
                "https://a.example/",
                "https://b.example/",
                "https://c.example/",
                "https://d.example/",
                "https://e.example/",
            ]),
            2,
        );
        fn steps(entries: &[HistoryEntry]) -> Vec<(i32, &str)> {
            entries
                .iter()
                .map(|entry| (entry.offset, entry.url.as_str()))
                .collect()
        }
        assert_eq!(
            steps(&list.back),
            [(-1, "https://b.example/"), (-2, "https://a.example/")]
        );
        assert_eq!(
            steps(&list.forward),
            [(1, "https://d.example/"), (2, "https://e.example/")]
        );
        assert_eq!(list.back[0].title, "Title of https://b.example/");
    }

    #[test]
    fn each_side_is_bounded_to_the_nearest_entries() {
        let urls: Vec<String> = (0..300)
            .map(|at| format!("https://e{at}.example/"))
            .collect();
        let urls: Vec<&str> = urls.iter().map(String::as_str).collect();
        let list = HistoryEntries::from_session(&session(&urls), 150);
        assert_eq!(list.back.len(), MAX_HISTORY_SIDE);
        assert_eq!(list.forward.len(), MAX_HISTORY_SIDE);
        assert_eq!(list.back[0].offset, -1);
        assert_eq!(list.back.last().unwrap().offset, -(MAX_HISTORY_SIDE as i32));
        assert_eq!(list.forward.last().unwrap().offset, MAX_HISTORY_SIDE as i32);
        assert_eq!(list.back[0].url, "https://e149.example/");
    }

    #[test]
    fn a_blank_start_page_and_unloadable_entries_are_hidden_but_still_count_as_steps() {
        let list = HistoryEntries::from_session(
            &session(&[
                "about:blank",
                "https://a.example/",
                "javascript:alert(1)",
                "https://current.example/",
                "file:///etc/hosts",
                "https://z.example/",
            ]),
            3,
        );
        assert_eq!(
            list.back
                .iter()
                .map(|entry| entry.offset)
                .collect::<Vec<_>>(),
            [-2]
        );
        assert_eq!(
            list.forward
                .iter()
                .map(|entry| entry.offset)
                .collect::<Vec<_>>(),
            [2]
        );
    }

    #[test]
    fn titles_are_cleaned_and_bounded_and_a_bad_index_yields_an_empty_list() {
        let list = HistoryEntries::from_session(
            &[
                (
                    "a\u{7}\nb".to_owned() + &"x".repeat(500),
                    "https://a.example/".into(),
                ),
                ("now".into(), "https://b.example/".into()),
            ],
            1,
        );
        assert!(list.back[0].title.starts_with("abxx"));
        assert!(list.back[0].title.chars().count() <= MAX_HISTORY_TITLE_CHARS);
        assert_eq!(
            HistoryEntries::from_session(&session(&["https://a.example/"]), 9),
            HistoryEntries::default()
        );
    }
}

#[cfg(test)]
#[path = "extension_access_tests.rs"]
mod extension_access_tests;
