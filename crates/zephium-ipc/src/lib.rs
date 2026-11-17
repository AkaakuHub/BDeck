//! Typed contract between the Rust core and the frame. Pure data, no tauri;
//! the desktop crate maps `Projection` onto typed events and exports the TS
//! bindings. ULIDs cross the boundary as strings.

use serde::{Deserialize, Serialize};
use specta::Type;

mod zoom;

pub use zoom::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct TabView {
    pub id: String,
    /// Process-local monotonically increasing projection revision, encoded as
    /// fixed-width hexadecimal so JavaScript can compare it without losing
    /// integer precision. Privileged chrome rejects an older per-tab delta
    /// after a newer presentation barrier has applied.
    pub projection_revision: String,
    pub title: String,
    pub url: Option<String>,
    /// Explicit content owner. Internal pages never carry a navigable URL.
    #[serde(default)]
    pub content: TabContentView,
    pub loading: bool,
    /// What the page asked for that waits on the person.
    #[serde(default)]
    #[specta(optional)]
    pub page_request: Option<PageRequestView>,
    /// Transient native residency state. It never replaces the committed URL
    /// or title, and an explicit retry remains a fresh navigation intent.
    #[serde(default)]
    #[specta(optional)]
    pub availability: Option<TabAvailability>,
    /// The last navigation the person asked for that did not load, until the
    /// next attempt or commit. Never carries native error text.
    #[serde(default)]
    #[specta(optional)]
    pub failure: Option<TabFailure>,
    /// The page is from an origin whose certificate the person chose to
    /// continue past; chrome must never show it as secure.
    #[serde(default)]
    pub certificate_exception: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub icon: Option<IconRef>,
    #[serde(default)]
    #[specta(optional)]
    pub capture: Option<MediaCaptureView>,
    /// The page's zoom in whole percent, while the tab has a page to zoom. It
    /// is the level just asked for until the view reports it applied.
    #[serde(default)]
    #[specta(optional)]
    pub zoom: Option<u16>,
    /// Present while the page plays sound, and for as long as it is muted.
    #[serde(default)]
    #[specta(optional)]
    pub audio: Option<TabAudioView>,
}

/// The sidebar's player: the media of the tab that most recently started
/// playing off screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct MediaPlayerView {
    pub tab_id: String,
    /// The page's own name for what plays, else the tab's title.
    pub title: String,
    /// The page's artist, else the site.
    pub artist: String,
    pub icon: Option<IconRef>,
    /// Seconds; absent while unknown and for a live stream.
    pub duration: Option<f64>,
    /// Seconds, as of this projection; chrome advances it by `rate` while
    /// `playing` and not `waiting`.
    pub position: f64,
    pub rate: f64,
    pub playing: bool,
    pub waiting: bool,
    pub can_pip: bool,
    pub in_pip: bool,
    pub muted: bool,
    pub can_mute: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaPlayerAction {
    Play,
    Pause,
    Seek {
        seconds: f64,
    },
    PictureInPicture,
    /// Brings the tab to front, in its own space.
    Show,
    /// Closes the player until the tab plays something else.
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TabAudioView {
    pub muted: bool,
    /// The engine can mute this tab; without it the speaker only informs.
    pub can_mute: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageRequestView {
    /// Open a link in another application. `app` is its name when the system
    /// knows one; `scheme` names the kind of link otherwise.
    ExternalApp {
        site: Option<String>,
        scheme: String,
        app: Option<String>,
    },
    /// A new tab the page tried to open; `host` is set when it can be opened.
    /// `site` is the host of the page asking, when its profile can always
    /// allow that site's pop-ups.
    Popup {
        host: Option<String>,
        site: Option<String>,
    },
    /// The site, or its proxy, asks to sign in. Never carries the server's
    /// own text.
    HttpAuth {
        request: u32,
        /// Host, with its port when not the scheme's default.
        host: String,
        /// Plain http: the password would cross the network readable.
        insecure: bool,
        proxy: bool,
        /// The last username or password was wrong.
        retry: bool,
    },
    /// The page stopped responding: wait for it, or reload it.
    Unresponsive,
}

/// What the person typed into a sign-in prompt. Deliberately not Debug or
/// Clone: it moves straight into wiped storage.
#[derive(Deserialize, Type)]
pub struct SignInCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageRequestAnswer {
    Allow,
    AlwaysAllow,
    Dismiss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CaptureDeviceStateView {
    None,
    Active,
    Muted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MediaCaptureView {
    pub navigation_id: String,
    pub camera: CaptureDeviceStateView,
    pub microphone: CaptureDeviceStateView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TabFailure {
    pub url: String,
    pub reason: TabFailureReason,
    /// For a refused certificate, what the engine could tell about it.
    #[serde(default)]
    #[specta(optional)]
    pub certificate: Option<TabCertificateProblem>,
    /// The refused certificate is on a host on the person's own network, so
    /// they may continue past it. Never true for a public name.
    #[serde(default)]
    pub can_continue: bool,
    /// The page that was showing is gone; the failure stands in its place
    /// until another page opens, even while one loads.
    #[serde(default)]
    pub covers_page: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TabCertificateProblem {
    /// Expired, or not valid yet.
    Expired,
    /// Self-signed, or from an issuer the system does not trust.
    Untrusted,
    NameMismatch,
    Revoked,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TabFailureReason {
    Offline,
    HostNotFound,
    Unreachable,
    TimedOut,
    Insecure,
    /// The page's process ended and it could not be brought back on its own.
    Crashed,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TabAvailability {
    Sleeping,
    WaitingForCapacity { url: String },
    BlockedByCapacity { url: String },
}

/// Browser chrome's bounded tab renderer choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TabContentView {
    #[default]
    Web,
    ExtensionOwned,
    Settings,
    Source,
}

/// Names a cached site icon without carrying its pixels. Chrome keeps rasters
/// by key and repaints only when `revision` changes, so a projection costs
/// a short string per tab instead of a five-kilobyte raster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct IconRef {
    /// A site's origin, or one page of it (origin and path) that wears an icon
    /// of its own. Chrome treats it as an opaque key.
    pub origin: String,
    pub revision: String,
}

/// One site icon: canonical base64 of exactly 32x32 RGBA bytes. Chrome never
/// decodes a page-controlled image format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FaviconEntry {
    /// The key an `IconRef` names the raster by.
    pub origin: String,
    pub revision: String,
    pub rgba: String,
}

/// Which privileged webview a raster is destined for. Each keeps its own
/// cache, so delivery is tracked per surface rather than broadcast.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum IconSurface {
    Chrome,
    Panel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FaviconsView {
    pub surface: IconSurface,
    pub profile_id: String,
    pub entries: Vec<FaviconEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ExtensionActionRuntimeView {
    pub install_id: String,
    pub generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ExtensionActionView {
    pub runtime: ExtensionActionRuntimeView,
    pub revision: String,
    pub label: String,
    pub badge: String,
    pub icon_rgba_base64: Option<String>,
    pub enabled: bool,
    pub presents_popup: bool,
    pub unread_badge: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ExtensionActionsView {
    pub projection_revision: String,
    pub profile_id: String,
    pub tab_id: Option<String>,
    pub actions: Vec<ExtensionActionView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionActionFailure {
    InvalidRequest,
    RuntimeUnavailable,
    RuntimeSuperseded,
    TabUnavailable,
    TabDiscarded,
    ActionUnavailable,
    ActionDisabled,
    CapacityExceeded,
    PopupUnavailable,
    PopupCapacityExceeded,
    NativeAdmissionFailed,
    ShuttingDown,
    UnsupportedPlatform,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ExtensionActionFailedView {
    pub projection_revision: String,
    pub profile_id: String,
    pub tab_id: String,
    pub reason: ExtensionActionFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ExtensionActionShortcutView {
    pub projection_revision: String,
    pub profile_id: String,
    pub tab_id: String,
    pub runtime: ExtensionActionRuntimeView,
    pub action_revision: String,
}

/// Closed page capability names rendered by browser-owned chrome. Native
/// permission strings and page-controlled labels never cross this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PagePermissionKindView {
    Camera,
    Microphone,
    Geolocation,
}

/// One exact, foreground page-permission request. Every identity is an opaque
/// stale fence: privileged chrome may only echo it back to the Shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PagePermissionPromptEntryView {
    pub profile_id: String,
    pub item_id: String,
    pub request_id: String,
    pub origin: String,
    pub kinds: Vec<PagePermissionKindView>,
    /// False for ephemeral profiles; chrome must not offer durable policy.
    pub rememberable: bool,
    /// True while an exact durable remember-decision transaction is pending.
    pub processing: bool,
    /// Shown as a card under the address field of the expanded sidebar, with
    /// the page still on screen. Otherwise a window-modal sheet, with the
    /// page taken off screen until it is answered.
    pub in_sidebar: bool,
    /// The operating system refuses Zephium this capability; chrome says
    /// where to turn it on.
    pub blocked_by_system: bool,
}

/// Exact replacement for the one process-wide page permission surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PagePermissionPromptView {
    pub projection_revision: String,
    pub prompt: Option<PagePermissionPromptEntryView>,
}

/// A kind of decision Site settings lists. Which kinds a platform keeps is
/// in `SitePermissionsView::kinds`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SitePermissionKindView {
    Camera,
    Microphone,
    Geolocation,
    Notifications,
    ClipboardRead,
    Midi,
    Sensors,
    LocalFonts,
    AutomaticDownloads,
    Popups,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SitePermissionDecisionView {
    Allow,
    Block,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SitePermissionView {
    /// Canonical origin, such as `https://meet.example`.
    pub origin: String,
    pub kind: SitePermissionKindView,
    pub decision: SitePermissionDecisionView,
}

/// The focused profile's saved site decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SitePermissionsView {
    /// A private profile saves nothing; its list is always empty.
    pub private: bool,
    /// False when some decisions could not be read right now.
    pub complete: bool,
    /// The kinds this platform keeps, in display order.
    pub kinds: Vec<SitePermissionKindView>,
    pub entries: Vec<SitePermissionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WebExtensionAccessRequestView {
    pub profile_id: String,
    pub request: String,
    pub extension_id: String,
    pub warnings: Vec<String>,
    pub permissions: Vec<String>,
    pub patterns: Vec<String>,
}

/// The one retained split group owned by the focused window. Members are
/// normalized references into [`ItemsState::tabs`] in native pane traversal
/// order; geometry and mutable divider ratios remain native-only authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct SplitGroupView {
    pub members: Vec<String>,
}

/// Focused profile metadata. Profile isolation and lifecycle remain native
/// authority; this view is display-only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub kind: ProfileKindView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKindView {
    Default,
    Named,
    Incognito,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ProfileAvatarStyleView {
    Initials,
    Symbol,
    Photo,
}

/// How a profile is drawn. Colour and symbol index the frame's curated sets;
/// a photo is served by native under its revision, never sent as bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProfileAvatarView {
    pub style: ProfileAvatarStyleView,
    pub color: u8,
    pub symbol: u8,
    pub revision: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProfileEntryView {
    pub id: String,
    pub name: String,
    pub kind: ProfileKindView,
    pub avatar: ProfileAvatarView,
}

/// Every persistent profile, in order, and the one the window browses as.
/// `private` is set while the window shows its private scope instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ProfilesView {
    pub current: Option<String>,
    pub private: bool,
    /// The tabs of the private window while one exists, shown or set aside
    /// behind the regular tabs.
    #[serde(default)]
    pub private_tabs: Option<u32>,
    pub profiles: Vec<ProfileEntryView>,
}

/// One ordered space owned by the focused profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SpaceView {
    pub id: String,
    pub name: String,
    pub icon: Option<SpaceIconView>,
    /// An avatar hue, shared with profiles.
    pub color: u8,
    /// Overrides the app theme for the whole window while the space is active.
    pub theme: Option<SpaceThemeView>,
    /// Pinned and open tabs, which deleting the space closes.
    pub tabs: u32,
    /// The icons of its first tabs in sidebar order, at most three, for the
    /// switcher to stack beside the name. A tab with no icon yet is null.
    pub previews: Vec<Option<IconRef>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpaceIconView {
    /// An index into the frame's curated symbols.
    Symbol {
        index: u8,
    },
    Emoji {
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpaceThemeView {
    Preset {
        id: String,
    },
    /// Seeds in the shape of `ui.theme-custom`.
    Custom {
        seeds: String,
    },
}

/// Stable display section for a sidebar node. Children inherit their
/// authoritative placement from the native item aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SidebarSectionView {
    Favorites,
    Pinned,
    Today,
}

/// A folder carries bounded display metadata; a tab is a normalized reference
/// into [`ItemsState::tabs`]. The node id and tab id intentionally match, but
/// the explicit reference keeps consumers from inferring that invariant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidebarNodeKindView {
    Folder { name: String },
    Tab { tab_id: String },
}

/// One pre-order entry in the focused sidebar tree. Parents always precede
/// descendants, sibling order is native aggregate order, and `parent_id` is
/// `None` only for a section root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SidebarNodeView {
    pub id: String,
    pub parent_id: Option<String>,
    pub section: SidebarSectionView,
    pub kind: SidebarNodeKindView,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ItemsState {
    pub projection_revision: String,
    /// `None` is reserved for the frame's cold pre-bootstrap state. Native
    /// snapshots are emitted only with an exact focused profile and space.
    pub profile: Option<ProfileView>,
    pub spaces: Vec<SpaceView>,
    pub active_space_id: Option<String>,
    pub nodes: Vec<SidebarNodeView>,
    /// Every tab referenced by `nodes`, in the same pre-order traversal.
    pub tabs: Vec<TabView>,
    pub active: Option<String>,
    pub split_group: Option<SplitGroupView>,
}

/// Where a find in the page in front landed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FindResultView {
    /// The search this answers; chrome ignores results for older text.
    pub query: String,
    pub matches: u32,
    /// One-based position of the current match, when the engine knows it.
    pub active: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ClosedTabView {
    pub id: String,
    pub url: String,
    pub title: String,
    pub icon: Option<IconRef>,
}

/// Split divider hit-strip in window logical coordinates; the chrome renders
/// these as drag targets on platforms without native stage dividers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct DividerView {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub vertical: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct LayoutState {
    pub dividers: Vec<DividerView>,
    pub columns: Vec<DeckColumnFrame>,
    pub composer: Option<ComposerFrame>,
    pub scroll: Option<DeckScrollFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct DeckScrollFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub offset: f64,
    pub max_offset: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct DeckColumnFrame {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct ComposerFrame {
    pub tab: String,
    pub account: String,
    pub managing: bool,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Immediate result returned by a privileged IPC command. `accepted` with an
/// `operation_id` means the mutation was successfully and non-evictably
/// admitted to the shell's process-local ordered FIFO; it does not claim that
/// later native/store work succeeded or survive a process restart. `accepted`
/// without an id is reserved for a fully applied, privileged-UI-only action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OperationAdmission {
    pub operation_id: Option<String>,
    pub accepted: bool,
}

/// The bounded terminal classification the actor can establish while
/// processing an admitted operation. `Deferred` means native work was queued,
/// an exact discard acknowledgement is still required, or a durable write
/// became indeterminate and entered explicit reconciliation; it never means a
/// page load, renderer callback, or unknown store transaction succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OperationOutcome {
    Applied,
    NoOp,
    Rejected,
    NativeAdmissionFailed,
    Deferred,
}

/// Stable, non-page-derived detail for an operation outcome. Keeping this an
/// enum prevents native errors, URLs, or attacker-controlled strings from
/// becoming an unbounded privileged IPC/logging surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OperationReason {
    MutationApplied,
    StateUnchanged,
    InvalidScope,
    NoFocusedWindow,
    ItemLimitReached,
    InvalidInput,
    HistoryUnavailable,
    LayoutUnavailable,
    UnsupportedCommand,
    NativeDispatchRejected,
    NativePending,
    DiscardCompletionPending,
    StorePending,
    StoreAdmissionRejected,
    StoreConflict,
    StoreOutcomeUnknown,
    StoreReconciliationFailed,
    ProfileDeletionPolicyRejected,
    ProfileDeletionInProgress,
    ProfileDeletionCompleted,
}

/// The shell has processed an admitted operation in actor order. Consumers
/// reconcile logical effects from authoritative projections. A deferred
/// native navigation still resolves independently through engine events.
/// Long-running profile deletion retains its id internally and emits this
/// disposition exactly once, only after definitive rejection or both durable
/// deletion phases complete. Retry/reconciliation state is never mislabeled
/// as successfully applied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct OperationDisposition {
    pub operation_id: String,
    pub outcome: OperationOutcome,
    pub reason: OperationReason,
}

/// Process-local reconciliation state for an admitted mutation. Pending and
/// processed entries are retained in a bounded fail-closed desktop ledger;
/// processed entries remain queryable until privileged chrome acknowledges
/// them. `Unknown` means the id was never admitted in this process, was already
/// acknowledged, or belongs to a previous process lifetime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum OperationStatus {
    Unknown,
    Pending,
    Processed { disposition: OperationDisposition },
}

/// Process-local browser-runtime status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RuntimeStatus {
    pub restart_required: bool,
    /// The saved session could not be restored this launch: its bytes were
    /// kept in a file and the browser started with fresh tabs.
    pub session_set_aside: bool,
    /// Canonically ordered closed-vocabulary set. Rust emits at most five
    /// entries and privileged chrome must replace, never append, projections.
    pub security_advisories: Vec<RuntimeSecurityAdvisory>,
}

/// Non-fatal, process-local classification produced before native WebView
/// construction. Hard admission failures never reach privileged chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSecurityAdvisoryKind {
    ReviewOverdue,
    UpdateRecommended,
    UnreviewedRuntime,
}

/// Fixed destination of the recommended maintenance action. No page or
/// network response can select this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSecurityUpdateTarget {
    Zephium,
    OperatingSystem,
    BrowserRuntime,
}

/// Sanitized advisory delivered only to privileged main chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RuntimeSecurityAdvisory {
    pub kind: RuntimeSecurityAdvisoryKind,
    pub update_target: RuntimeSecurityUpdateTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PanelOwner {
    pub private: bool,
    pub window_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub space_id: String,
}

/// Snapshots for structural changes, single-row deltas for per-tab churn.
#[derive(Clone, Debug)]
pub enum Projection {
    PanelOwner(PanelOwner),
    Items(ItemsState),
    Tab(TabView),
    Favicons(FaviconsView),
    ExtensionActions(ExtensionActionsView),
    ExtensionActionFailed(ExtensionActionFailedView),
    ExtensionActionShortcut(ExtensionActionShortcutView),
    PagePermissionPrompt(PagePermissionPromptView),
    WebExtensionAccessRequest(WebExtensionAccessRequestView),
    UiCommand(String),
    FindResult(FindResultView),
    Layout(LayoutState),
    /// The page on screen is fullscreen and fills the browser window, which
    /// should itself be fullscreen until this turns false. Only engines that
    /// present fullscreen inside the browser window send it.
    HostFullscreen(bool),
    RuntimeStatus(RuntimeStatus),
    OperationProcessed(OperationDisposition),
    Profiles(ProfilesView),
    /// The newest tabs the focused profile can reopen by name, for the menus.
    /// Sent only when that list changes; icons are not part of it.
    RecentlyClosed(Vec<ClosedTabView>),
    /// The sidebar's player, sent when it changes; `None` hides it.
    MediaPlayer(Option<MediaPlayerView>),
}
