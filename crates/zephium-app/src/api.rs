//! Stable application-shell protocol exposed to the desktop composition root.

use std::fmt;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use zephium_core::extensions::{
    ExtensionActionRevision, ExtensionPopupAnchor, ExtensionRuntimeInstance,
};
use zephium_core::geometry::Size;
use zephium_core::ids::{DeckAccountId, ItemId, ProfileId};
use zephium_core::ports::chrome::Chrome as GeometryChrome;
use zephium_core::ports::engine::{DiscardProbeId, Engine, EngineEvent, NavigationPresentationId};
use zephium_core::ports::store::Store;
use zephium_core::ports::store::{
    PagePermissionCatalogLoadOutcome, PagePermissionCatalogMutationOutcome,
};
use zephium_core::split::Axis;
use zephium_ipc::{Projection, TabView};

use crate::store_reads::StoreReadResult;

pub type SharedEngine = Arc<dyn Engine + Send + Sync>;
pub type SharedStore = Arc<dyn Store + Send + Sync>;
pub type SharedChrome = Arc<dyn PresentationChrome + Send + Sync>;
/// Redacted terminal reason delivered to the desktop composition root when
/// the shell can no longer continue safely in the current process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellTerminalFailure {
    ProfileDeletionInvariant,
    ActorExitedUnexpectedly,
    /// The saved session or its deletion journal could not be opened. The
    /// store kept it untouched; the person is told so instead of facing an
    /// empty window that cannot do anything.
    SessionUnavailable,
    /// The saved session was written by a newer Zephium and is kept for it.
    SessionFromNewerVersion,
}

impl std::fmt::Display for ShellTerminalFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ProfileDeletionInvariant => {
                "profile deletion violated a post-retirement invariant"
            }
            Self::ActorExitedUnexpectedly => "application shell actor exited unexpectedly",
            Self::SessionUnavailable => "the saved session could not be opened",
            Self::SessionFromNewerVersion => "the saved session belongs to a newer Zephium",
        })
    }
}

/// One-shot terminal shell handoff. Startup failures invoke it so the desktop
/// can enqueue Shell-owned orderly shutdown; unexpected actor exit invokes it
/// after bounded best-effort cleanup. Platform adapters must dispatch onto
/// their native event loop rather than performing teardown inline.
pub type ShellTerminalFailureCallback = Box<dyn FnOnce(ShellTerminalFailure) + Send>;
pub type EmitFn = Box<dyn Fn(Projection) + Send + Sync>;

/// Exact privileged-chrome work that must complete before one raw document
/// can become visible. The tab projection is carried in the same native eval
/// as the acknowledgement, avoiding an ordering assumption between generic
/// projection delivery and native content presentation.
#[derive(Clone, Debug, PartialEq)]
pub struct ChromePresentation {
    pub settings_visible: bool,
    pub id: ItemId,
    pub navigation: NavigationPresentationId,
    pub url: String,
    pub tab: TabView,
    pub active: Option<ItemId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChromePresentationDispatch {
    /// The adapter applied and verified the projection synchronously. Used by
    /// deterministic embedders/tests; production native adapters are async.
    Applied,
    /// Callback ownership was accepted. It must report verification success
    /// or failure without blocking its native UI thread.
    Scheduled,
    /// No callback ownership transfer occurred.
    Rejected,
}

pub type ChromePresentationCallback = Box<dyn FnOnce(bool) + Send>;

/// Geometry plus the privileged DOM acknowledgement required by the raw-view
/// anti-spoof boundary.
pub trait PresentationChrome: GeometryChrome {
    fn restore_browser_chrome(
        &self,
        _revision: u64,
        _items: zephium_ipc::ItemsState,
        _done: ChromePresentationCallback,
    ) -> ChromePresentationDispatch {
        ChromePresentationDispatch::Rejected
    }

    fn apply_tab_for_presentation(
        &self,
        presentation: ChromePresentation,
        done: ChromePresentationCallback,
    ) -> ChromePresentationDispatch;
}

/// Terminal result of the ordered application shutdown protocol.
///
/// A retryable failure happens before the native engine is torn down and
/// leaves the actor live. `Unclean` is terminal: either the actor exited
/// without completing the barrier or one of the Store or native cleanup steps
/// was not proven, so the process must exit unsuccessfully.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownOutcome {
    RetryableFailure,
    Clean,
    Unclean,
}

/// Browser-owned response vocabulary for one exact page capability request.
/// `Always*` is authority to mutate the durable per-profile catalog; `*Once`
/// never writes policy and holds for the asking document until its tab shows
/// another one. `Dismiss` refuses this request only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePermissionPromptDecision {
    AllowOnce,
    AlwaysAllow,
    DenyOnce,
    AlwaysDeny,
    Dismiss,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabMetadata {
    pub id: ItemId,
    pub title: String,
    pub url: Option<String>,
}

/// What the tab menu can do to a tab, beyond the commands it shares with the
/// rest of the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    /// Opens the same page in a new tab right after it.
    Duplicate,
    /// Closes every open tab of the space but this one.
    CloseOthers,
    /// Closes the open tabs listed after this one.
    CloseBelow,
}

/// Space mutations from chrome, menus and keys. The shell checks each
/// against the focused profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpaceCommand {
    Create {
        name: String,
        look: zephium_core::spaces::SpaceLook,
    },
    Update {
        id: zephium_core::ids::SpaceId,
        name: String,
        look: zephium_core::spaces::SpaceLook,
    },
    Delete(zephium_core::ids::SpaceId),
    Reorder {
        id: zephium_core::ids::SpaceId,
        before: Option<zephium_core::ids::SpaceId>,
    },
    Switch(zephium_core::ids::SpaceId),
    /// The next (1) or previous (-1) space, wrapping.
    Step(i8),
    MoveTab {
        tab: ItemId,
        space: zephium_core::ids::SpaceId,
    },
    /// Moves the tab in front, for the menu bar.
    MoveActive(zephium_core::ids::SpaceId),
}

/// What the person chose for a page's request to open another application
/// or a blocked new tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageRequestDecision {
    Allow,
    /// Allow, and let this site open the same kind of link without asking
    /// again until Zephium quits.
    AlwaysAllow,
    Dismiss,
}

#[derive(Clone, Debug)]
pub enum Command {
    /// Native pressure notifications only; pages cannot choose resource policy.
    SetMemoryPressure(zephium_core::ports::engine::MemoryPressure),
    /// A privileged user mutation with an externally visible admission and
    /// actor-order disposition identity. Engine callbacks and replaceable UI
    /// facts never use this wrapper.
    Operation {
        operation_id: String,
        command: Box<Command>,
    },
    Bootstrap,
    Open,
    Activate(ItemId),
    Close(ItemId),
    /// A named profile with its own space, engine store and content policy.
    CreateProfile {
        name: String,
        avatar: zephium_core::profiles::ProfileAvatar,
        photo: Option<Arc<Vec<u8>>>,
        /// Shows the new profile in the window once it exists.
        switch: bool,
    },
    /// Shows a persistent profile in the window.
    SwitchProfile(ProfileId),
    RenameProfile {
        profile: ProfileId,
        name: String,
    },
    /// A photo style carries its already downscaled bytes.
    SetProfileAvatar {
        profile: ProfileId,
        avatar: zephium_core::profiles::ProfileAvatar,
        photo: Option<Arc<Vec<u8>>>,
    },
    ProfileAvatarPhoto {
        profile: ProfileId,
        reply: SyncSender<Option<Vec<u8>>>,
    },
    Profiles {
        reply: SyncSender<Option<zephium_ipc::ProfilesView>>,
    },
    Navigate {
        id: ItemId,
        input: String,
    },
    Reload(ItemId),
    /// The person's answer to what the tab's page asked for.
    AnswerPageRequest {
        id: ItemId,
        decision: PageRequestDecision,
    },
    /// The person's answer to the tab's sign-in prompt: credentials, or
    /// None to cancel. Credentials are wiped when the command drops.
    AnswerHttpAuth {
        id: ItemId,
        request: u32,
        credentials: Option<zephium_core::http_auth::HttpCredentials>,
    },
    /// The person chose to continue past the certificate the tab refused,
    /// on a host on their own network, and to load it again.
    ContinuePastCertificate(ItemId),
    GoBack(ItemId),
    GoForward(ItemId),
    SplitWith {
        other: ItemId,
        axis: Axis,
    },
    Unsplit,
    /// Takes one tab out of the focused split, leaving the rest paired.
    LeaveSplit(ItemId),
    /// Something the tab menu does to the one tab it was opened on.
    TabAction {
        id: ItemId,
        action: TabAction,
    },
    /// Something done to the focused profile's spaces.
    Space(Box<SpaceCommand>),
    SetWindowSize(Size),
    /// Whether the OS can currently present the main window. Minimized
    /// windows hide native content views so the engine can lower their memory
    /// priority and, after the normal idle grace, suspend them.
    SetWindowVisible(bool),
    /// OS focus is separate from visibility: background pages may keep playing
    /// audio, but they may not initiate or retain browser-owned device consent.
    SetWindowFocused(bool),
    /// The sidebar's width, and whether it changed by a deliberate change of
    /// shape — a toggle, a snap, a tool opening — that the content should
    /// travel with, rather than by a drag that it should simply follow.
    SetSidebarWidth(f64, bool),
    OpenComposer(DeckAccountId),
    OpenAccount(DeckAccountId),
    /// The sidebar is put away, the page taking the window, or brought back.
    SetSidebarHidden(bool),
    /// A hidden sidebar comes over the page, or goes back beneath it, sliding
    /// as the card does or at once.
    PeekSidebar {
        peek: bool,
        slide: bool,
    },
    /// Transient Windows resize feedback only; no width or layout mutation.
    SidebarResizeGuide(Option<f64>),
    DeckDragPreview(Option<zephium_core::geometry::Rect>),
    /// Whether the expanded sidebar is on screen to show a page-permission
    /// prompt under its address field.
    SetPagePermissionCardHost(bool),
    DragOver {
        point: Option<(f64, f64)>,
    },
    DropTab {
        id: ItemId,
        x: f64,
        y: f64,
    },
    DividerGrab {
        x: f64,
        y: f64,
    },
    DividerDrag {
        x: f64,
        y: f64,
    },
    DividerRelease {
        /// Final pointer position, folded into the same ordered mutation as
        /// release so separate IPC deliveries cannot persist a stale ratio.
        x: Option<f64>,
        y: Option<f64>,
    },
    Run(String),
    InvokeExtensionAction {
        runtime: ExtensionRuntimeInstance,
        revision: ExtensionActionRevision,
        anchor: ExtensionPopupAnchor,
    },
    SetWebExtensions {
        extensions: Vec<zephium_core::ports::engine::WebExtensionLoad>,
        failures: Vec<(zephium_core::ids::ExtensionInstallId, String)>,
        retry: Option<zephium_core::ids::ExtensionInstallId>,
    },
    OpenWebExtensionOptions {
        profile: ProfileId,
        extension_id: String,
    },
    AnswerWebExtensionAccess {
        profile: ProfileId,
        request: u64,
        allowed: bool,
    },
    RemoveWebExtension {
        extension: Box<zephium_core::ports::engine::WebExtensionLoad>,
    },
    ResolveWebExtensionTarget {
        tab: Option<ItemId>,
        reply: SyncSender<Option<crate::shell::WebExtensionTarget>>,
    },
    WebExtensionStatus {
        reply: SyncSender<
            Vec<(
                zephium_core::ids::ExtensionInstallId,
                crate::shell::WebExtensionStatus,
            )>,
        >,
    },
    /// Browser-owned response to the exact currently projected foreground
    /// page request. Public composition wraps this in `Operation`.
    RespondToPagePermissionPrompt {
        profile: ProfileId,
        item: ItemId,
        request: zephium_core::permissions::PagePermissionRequestId,
        decision: PagePermissionPromptDecision,
    },
    /// Internal callback from one admitted on-demand durable catalog read.
    PagePermissionCatalogLoaded {
        profile: ProfileId,
        item: ItemId,
        request: zephium_core::permissions::PagePermissionRequestId,
        outcome: Box<PagePermissionCatalogLoadOutcome>,
    },
    /// Internal callback from one admitted remembered-decision mutation.
    PagePermissionCatalogMutated {
        profile: ProfileId,
        item: ItemId,
        request: zephium_core::permissions::PagePermissionRequestId,
        outcome: Box<PagePermissionCatalogMutationOutcome>,
    },
    /// Hard Shell-side bound shorter than the native completion watchdog.
    PagePermissionTimeout {
        profile: ProfileId,
        item: ItemId,
        request: zephium_core::permissions::PagePermissionRequestId,
    },
    OpenUrl {
        input: String,
        new_tab: bool,
    },
    /// Finds text in the page in front; `None` ends the search.
    Find(Option<zephium_core::ports::engine::FindRequest>),
    /// Addresses handed over by another application, already admitted by
    /// `navigation::external_target`. Each opens in a new tab; any that
    /// arrive before the session is restored wait for it.
    OpenExternal(Vec<String>),
    SetAppSetting {
        key: String,
        value: String,
    },
    /// Permanently removes one inactive named profile through the durable
    /// cross-store deletion coordinator. The profile id comes only from
    /// privileged chrome and is revalidated against authoritative state.
    DeleteProfile(ProfileId),
    /// The focused profile's saved site decisions, of one origin when given.
    SitePermissions {
        origin: Option<zephium_core::permissions::PageOrigin>,
        reply: SyncSender<Option<zephium_ipc::SitePermissionsView>>,
    },
    /// Changes or, with None, forgets one decision of the focused profile.
    /// Accepted only as an operation.
    SetSitePermission {
        origin: zephium_core::permissions::PageOrigin,
        kind: zephium_core::permissions::SitePermissionKind,
        decision: Option<zephium_core::permissions::RememberedPagePermission>,
    },
    SitePermissionCatalogLoaded {
        operation_id: String,
        profile: ProfileId,
        origin: zephium_core::permissions::PageOrigin,
        kind: zephium_core::permissions::PagePermissionKind,
        decision: Option<zephium_core::permissions::RememberedPagePermission>,
        outcome: Box<zephium_core::ports::store::PagePermissionCatalogLoadOutcome>,
    },
    SitePermissionSettled {
        operation_id: String,
        applied: bool,
    },
    /// Title and URL of Space tabs owned by `profile`, for context admission.
    /// Never page content; a page read is a separately authorized capability.
    TabMetadata {
        profile: ProfileId,
        ids: Vec<ItemId>,
        reply: SyncSender<Vec<TabMetadata>>,
    },
    /// Title and URL of the open tabs in the focused window of `profile`,
    /// for context the person consented to. Never page content.
    WindowTabs {
        profile: ProfileId,
        reply: SyncSender<Vec<TabMetadata>>,
    },
    /// A page opened behind the person's work in `space`: right after
    /// `after` when that tab is in Today, never focused. Answers the new tab,
    /// or `None` when the profile, space, address or tab limit refuses.
    OpenBackgroundTab {
        profile: ProfileId,
        space: zephium_core::ids::SpaceId,
        after: Option<ItemId>,
        url: String,
        /// Show it at once instead of behind the person's work.
        focus: bool,
        done: TabCompletion,
    },
    /// Hands the shell the anonymous origin prober, once, after startup.
    AttachFaviconProber(FaviconProberAttachment),
    /// Icons for origins shown outside a tab: cached or stored rasters are
    /// delivered to chrome, the rest are queued for the origin probe.
    ProbeFavicons {
        profile: ProfileId,
        origins: Vec<String>,
    },
    /// The prober's single answer for one requested origin: an exact 32x32
    /// RGBA raster, or `None` when the origin gave no usable icon.
    FaviconProbed {
        profile: ProfileId,
        origin: String,
        rgba: Option<Vec<u8>>,
    },
    /// The prober's single answer for one page's icon: the raster, the
    /// address it came from, and the addresses that gave nothing.
    FaviconFetched {
        ticket: u64,
        rgba: Option<Vec<u8>>,
        source: Option<String>,
        failed: Vec<String>,
    },
    /// One-shot deadline for a tab's icon work: attempt 0 is a lost store
    /// read, the maximum is a deferred page check, and the one below it ends
    /// the wait that keeps the previous icon on show.
    FaviconPoll {
        id: ItemId,
        attempt: u8,
    },
    /// Bounded admission retry for one exact committed navigation. Normal
    /// presentation is requested immediately after its URL reaches chrome;
    /// stale identities can never reveal overlapping content.
    PresentationFallback {
        id: ItemId,
        navigation: NavigationPresentationId,
        /// Absolute dispatch-admission bound. Retries and overlapping
        /// navigations cannot move it later.
        hard_deadline: std::time::Instant,
    },
    /// Result of one privileged eval-with-callback presentation barrier. The
    /// callback is untrusted lifecycle timing: the actor revalidates every
    /// field against its current exact pending obligation.
    BrowserChromeRestored {
        revision: u64,
        applied: bool,
    },
    ChromePresentationApplied {
        id: ItemId,
        navigation: NavigationPresentationId,
        url: String,
        active: Option<ItemId>,
        projection_revision: String,
        applied: bool,
    },
    /// Fail-closed deadline for one exact renderer discard-safety probe.
    DiscardProbeTimeout {
        id: ItemId,
        probe: DiscardProbeId,
    },
    /// Deadline wake for one bounded foreground residency request.
    ViewCapacityRetry(ItemId),
    /// Exact-generation wakeup for a native profile-erasure callback. The
    /// outcome itself stays in a bounded inbox so queue overload cannot lose
    /// the security-critical proof.
    ProfileDeletionReady(ProfileId),
    /// One bounded-backoff retry for journal reconciliation, native erasure,
    /// or local SQLite finalization.
    ProfileDeletionRetry {
        profile: ProfileId,
        generation: u64,
    },
    /// Completion from the bounded storage-read worker. Every result carries
    /// the exact request generation and is revalidated against current shell
    /// state before it can affect privileged projections.
    StoreRead(StoreReadResult),
    /// Internal one-shot debounce fired by the queue's single timer thread.
    Persist,
    /// Periodic maintenance heartbeat; idle tabs suspend or hibernate even
    /// when no user command arrives.
    Tick,
    Engine(EngineEvent),
    /// Ordered process-boundary barrier. The actor snapshots after every
    /// command already queued ahead of this one, then flushes the store.
    Shutdown {
        deadline: std::time::Instant,
        ack: SyncSender<ShutdownOutcome>,
    },
}

type Completion<T> = Arc<Mutex<Option<Box<dyn FnOnce(T) + Send>>>>;

/// Where an icon comes from, for a request the prober answers with
/// [`Command::FaviconFetched`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaviconPageSource {
    /// Icon addresses the page declared, best first. Each is fetched without
    /// cookies, credentials or referrer; the first that decodes wins.
    Addresses(Vec<String>),
    /// A PNG the web engine already chose for the page.
    Encoded(Vec<u8>),
}

/// One request to the prober.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FaviconProbe {
    /// An origin's icon, answered exactly once with [`Command::FaviconProbed`]
    /// for the same profile and origin.
    Origin { profile: ProfileId, origin: String },
    /// A page's icon, answered exactly once with [`Command::FaviconFetched`]
    /// for the same ticket.
    Page {
        ticket: u64,
        profile: ProfileId,
        origin: String,
        source: FaviconPageSource,
    },
}

/// Fetches icons anonymously and decodes them into the fixed raster.
pub type FaviconProber = Arc<dyn Fn(FaviconProbe) + Send + Sync>;

#[derive(Clone)]
pub struct FaviconProberAttachment(pub FaviconProber);
impl fmt::Debug for FaviconProberAttachment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FaviconProberAttachment")
    }
}

#[derive(Clone)]
pub struct TabCompletion(Completion<Option<ItemId>>);
impl TabCompletion {
    pub fn new(done: impl FnOnce(Option<ItemId>) + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Some(Box::new(done)))))
    }
    pub fn finish(self, tab: Option<ItemId>) {
        let done = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(done) = done {
            done(tab);
        }
    }
}
impl fmt::Debug for TabCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TabCompletion")
    }
}
