//! Storage actor over the per-profile SQLite hub. `rusqlite` is blocking, so
//! one dedicated thread owns every connection and serializes access. Session
//! saves are coalesced (latest wins) so navigation bursts cost one write, not
//! one per event; visits and loads are immediate. Loads and shutdown flush
//! pending state first.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use zephium_core::downloads::{DownloadStoreCall, DownloadStoreReply};
use zephium_core::ids::ProfileId;
use zephium_core::permissions::{PagePermissionCatalogRevision, PagePermissionPatch};
use zephium_core::ports::store::{
    PagePermissionCatalogLoadOutcome, PagePermissionCatalogMutationOutcome,
    ProfileDeletionAuthorizeOutcome, ProfileDeletionFinalizeOutcome, ProfileDeletionLoad,
    SessionLoad, Store, StoreShutdownOutcome, MAX_FAVICON_BATCH_ORIGINS,
};
use zephium_core::profiles::{ProfileAvatar, ProfileKind};
use zephium_core::session::{
    PersistedKind, SessionState, MAX_RECENTLY_CLOSED_TABS, MAX_SESSION_ITEMS,
    MAX_SESSION_NAME_CHARS, MAX_SESSION_PROFILES, MAX_SESSION_SPACES, MAX_SPLIT_DEPTH,
};
use zephium_core::split::Pane;

use crate::hub::{self, Hub, MAX_SETTING_KEY_BYTES, MAX_SETTING_VALUE_BYTES};

const DEBOUNCE: Duration = Duration::from_millis(400);
const MAX_PENDING_AGE: Duration = Duration::from_secs(2);
const SAVE_RETRY_INITIAL: Duration = Duration::from_secs(1);
const SAVE_RETRY_MAX: Duration = Duration::from_secs(30);
const MAX_PENDING_SETTINGS: usize = hub::MAX_APP_SETTINGS as usize;
const DEFAULT_FLUSH_TIMEOUT: Duration = Duration::from_secs(8);
const STORE_RPC_TIMEOUT: Duration = Duration::from_secs(2);
const IMPORT_RPC_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_PENDING_PAGE_PERMISSION_MUTATIONS: usize = 16;

type PagePermissionCatalogLoadDone = Box<dyn FnOnce(PagePermissionCatalogLoadOutcome) + Send>;
type PagePermissionCatalogMutationDone =
    Box<dyn FnOnce(PagePermissionCatalogMutationOutcome) + Send>;

#[derive(Default)]
struct PagePermissionMutationAdmission {
    count: usize,
}

struct PagePermissionMutationPermit {
    admission: Arc<Mutex<PagePermissionMutationAdmission>>,
}

impl PagePermissionMutationPermit {
    fn acquire(admission: &Arc<Mutex<PagePermissionMutationAdmission>>) -> Option<Self> {
        let mut state = admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = state.count.checked_add(1)?;
        if next > MAX_PENDING_PAGE_PERMISSION_MUTATIONS {
            return None;
        }
        state.count = next;
        Some(Self {
            admission: admission.clone(),
        })
    }
}

impl Drop for PagePermissionMutationPermit {
    fn drop(&mut self) {
        let mut state = self
            .admission
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(count) = state.count.checked_sub(1) else {
            // Close admission permanently on impossible accounting drift.
            state.count = usize::MAX;
            return;
        };
        state.count = count;
    }
}

#[derive(Default)]
struct PendingSettings {
    pending: HashMap<String, String>,
    in_flight: HashSet<String>,
    /// Exact union of durable keys and newly accepted keys. Keeping this next
    /// to the mailboxes makes cardinality admission atomic with enqueueing;
    /// actor queue pressure can never turn a definite acceptance into a later
    /// quota rejection.
    known_keys: HashSet<String>,
}

impl PendingSettings {
    fn with_known_keys(known_keys: HashSet<String>) -> Self {
        Self {
            known_keys,
            ..Self::default()
        }
    }

    fn contains_key(&self, key: &str) -> bool {
        self.known_keys.contains(key)
    }

    fn unique_keys(&self) -> usize {
        self.known_keys.len()
    }
}

struct PendingSession {
    state: SessionState,
    first: Instant,
    latest: Instant,
    failures: u32,
    retry_at: Option<Instant>,
}

impl PendingSession {
    fn new(state: SessionState, now: Instant) -> Self {
        Self {
            state,
            first: now,
            latest: now,
            failures: 0,
            retry_at: None,
        }
    }

    fn deadline(&self) -> Instant {
        self.retry_at
            .unwrap_or_else(|| (self.latest + DEBOUNCE).min(self.first + MAX_PENDING_AGE))
    }

    fn due(&self, now: Instant) -> bool {
        now >= self.deadline()
    }

    fn failed(&mut self, now: Instant) {
        let delay = retry_delay(self.failures);
        self.failures = self.failures.saturating_add(1);
        self.retry_at = Some(now + delay);
    }
}

#[derive(Default)]
struct WriteRetry {
    failures: u32,
    retry_at: Option<Instant>,
}

impl WriteRetry {
    fn failed(&mut self, now: Instant) {
        self.retry_at = Some(now + retry_delay(self.failures));
        self.failures = self.failures.saturating_add(1);
    }

    fn clear(&mut self) {
        self.failures = 0;
        self.retry_at = None;
    }
}

fn retry_delay(failures: u32) -> Duration {
    SAVE_RETRY_INITIAL
        .saturating_mul(1_u32 << failures.min(5))
        .min(SAVE_RETRY_MAX)
}

enum Cmd {
    SaveWake,
    SettingWake,
    Load(Sender<SessionLoad>),
    /// Sets an unrestorable session aside and loads the restarted one.
    SetAsideSession(Sender<Option<(SessionLoad, Option<std::path::PathBuf>)>>),
    LoadProfileAvatars(Sender<Vec<(ProfileId, ProfileAvatar)>>),
    SaveProfileAvatar(
        ProfileId,
        ProfileAvatar,
        Option<Arc<Vec<u8>>>,
        Box<dyn FnOnce(bool) + Send>,
    ),
    ProfileAvatarPhoto(ProfileId, Box<dyn FnOnce(Option<Vec<u8>>) + Send>),
    LoadPagePermissionCatalog(ProfileId, PagePermissionCatalogLoadDone),
    MutatePagePermissionCatalog(
        ProfileId,
        PagePermissionCatalogRevision,
        PagePermissionPatch,
        PagePermissionMutationPermit,
        PagePermissionCatalogMutationDone,
    ),
    DownloadRecoveryProfiles(Box<dyn FnOnce(DownloadStoreReply) + Send>),
    DownloadCall(
        ProfileId,
        DownloadStoreCall,
        Box<dyn FnOnce(DownloadStoreReply) + Send>,
    ),
    GetSetting(String, Sender<Option<String>>),
    ReadSetting(
        String,
        Sender<zephium_core::ports::store::AppSettingReadback>,
    ),
    ImportFavicons(ProfileId, Vec<(String, Vec<u8>)>, Sender<Option<u32>>),
    DiscardSessionBackup(ProfileId, Box<dyn FnOnce(bool) + Send>),
    ZoomLevels(ProfileId, Sender<Vec<zephium_core::zoom::SavedLevel>>),
    SaveZoomLevel(ProfileId, String, Option<u16>, i64),
    PopupSites(
        ProfileId,
        Sender<Option<Vec<zephium_core::permissions::PageOrigin>>>,
    ),
    SetPopupSite(
        ProfileId,
        zephium_core::permissions::PageOrigin,
        bool,
        Sender<bool>,
    ),
    FaviconAge(ProfileId, String, Sender<Option<i64>>),
    FaviconRasterWithAge(ProfileId, String, Sender<Option<(Vec<u8>, i64)>>),
    SaveFavicon(ProfileId, String, Option<String>, Vec<u8>),
    ForgetPageFavicon(ProfileId, String),
    FaviconBytes(ProfileId, String, Sender<Option<(Option<String>, Vec<u8>)>>),
    FaviconRasters(ProfileId, Vec<String>, Sender<Vec<(String, Vec<u8>)>>),
    PendingProfileDeletions(Sender<ProfileDeletionLoad>),
    AuthorizeProfileDeletion(
        ProfileId,
        SessionState,
        Sender<ProfileDeletionAuthorizeOutcome>,
    ),
    FinalizeProfileDeletion(ProfileId, Sender<ProfileDeletionFinalizeOutcome>),
    Flush(Sender<bool>),
    Shutdown(Sender<bool>),
}

struct ActorLifecycle {
    join: Option<JoinHandle<()>>,
    exited: Mutex<Receiver<()>>,
    terminal_admitted: bool,
}

pub struct SqliteStore {
    tx: SyncSender<Cmd>,
    latest_session: Arc<Mutex<Option<SessionState>>>,
    pending_settings: Arc<Mutex<PendingSettings>>,
    page_permission_mutation_admission: Arc<Mutex<PagePermissionMutationAdmission>>,
    lifecycle: RwLock<ActorLifecycle>,
    shutdown_clean: AtomicBool,
}

impl SqliteStore {
    fn admits_writes(&self) -> bool {
        let lifecycle = self.lifecycle.read().unwrap_or_else(|p| p.into_inner());
        !lifecycle.terminal_admitted && !self.shutdown_clean.load(Ordering::Acquire)
    }

    /// One request answered by the actor, or None when it was not admitted or
    /// did not answer in time.
    fn ask<T>(&self, command: impl FnOnce(Sender<T>) -> Cmd) -> Option<T> {
        let (tx, rx) = mpsc::channel();
        self.tx.try_send(command(tx)).ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok()
    }

    /// `dir` is the app data directory; the hub lays out `meta.sqlite` plus
    /// one `profile-<ulid>.sqlite` per profile inside it.
    pub fn open(dir: impl AsRef<Path>) -> rusqlite::Result<Self> {
        let mut hub = Hub::open(dir.as_ref().to_path_buf())?;
        // Fail before the shell/UI starts if even a compatibility snapshot
        // cannot be read. Startup must never turn storage failure into a new,
        // empty authoritative session.
        if let Err(error) = hub.load_authoritative() {
            if hub.recovery_reason().is_none() {
                return Err(error);
            }
        }
        Self::spawn(hub)
    }

    pub fn in_memory() -> rusqlite::Result<Self> {
        Self::spawn(Hub::in_memory()?)
    }

    /// Installs protected Work routing before startup tombstone reconciliation.
    /// Ordinary application/profile databases retain their existing location.
    fn spawn(hub: Hub) -> rusqlite::Result<Self> {
        let setting_keys = hub.app_setting_keys()?;
        // Session snapshots use a latest-value mailbox below. Bound every
        // remaining request too, so a compromised privileged UI cannot retain
        // unlimited settings/search/favicon commands in this actor.
        let (tx, rx) = mpsc::sync_channel::<Cmd>(256);
        let latest_session = Arc::new(Mutex::new(None));
        let pending_settings = Arc::new(Mutex::new(PendingSettings::with_known_keys(setting_keys)));
        let actor_latest_session = latest_session.clone();
        let actor_pending_settings = pending_settings.clone();
        let page_permission_mutation_admission =
            Arc::new(Mutex::new(PagePermissionMutationAdmission::default()));
        let (actor_exited, actor_exit) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("zephium-store".into())
            .spawn(move || {
                // Send the exit proof only after `actor` has returned and its
                // Hub/SQLite connections have been dropped. Preserve panic
                // visibility for JoinHandle while still unblocking the
                // bounded shutdown waiter.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    actor(hub, rx, actor_latest_session, actor_pending_settings)
                }));
                let _ = actor_exited.send(());
                if let Err(payload) = result {
                    std::panic::resume_unwind(payload);
                }
            })
            .map_err(|error| {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
                    Some(format!("cannot start the storage actor: {error}")),
                )
            })?;
        Ok(Self {
            tx,
            latest_session,
            pending_settings,
            page_permission_mutation_admission,
            lifecycle: RwLock::new(ActorLifecycle {
                join: Some(join),
                exited: Mutex::new(actor_exit),
                terminal_admitted: false,
            }),
            shutdown_clean: AtomicBool::new(false),
        })
    }

    /// Waits for the latest queued session snapshot to commit, but never past
    /// the store's bounded default shutdown budget.
    pub fn flush(&self) -> bool {
        self.flush_until(Instant::now() + DEFAULT_FLUSH_TIMEOUT)
    }

    /// Deadline-aware durability barrier. Admission to the bounded actor queue
    /// and waiting for SQLite completion share the same caller-owned budget.
    pub fn flush_until(&self, deadline: Instant) -> bool {
        let (tx, rx) = mpsc::channel();
        let mut command = Cmd::Flush(tx);
        loop {
            match self.tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return false;
                    }
                    thread::sleep(remaining.min(Duration::from_millis(1)));
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        !remaining.is_zero() && rx.recv_timeout(remaining).unwrap_or(false)
    }

    /// Executes the terminal actor protocol under the caller's original
    /// deadline. A negative actor reply means durability failed before the
    /// actor transferred terminal ownership and is therefore retryable. Once
    /// the command is admitted without such a reply, any uncertainty is
    /// terminal: the actor may already have released its database handles.
    pub fn shutdown_until(&self, deadline: Instant) -> StoreShutdownOutcome {
        if self.shutdown_clean.load(Ordering::Acquire) {
            return StoreShutdownOutcome::Clean;
        }

        let mut lifecycle = self
            .lifecycle
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.shutdown_clean.load(Ordering::Acquire) {
            return StoreShutdownOutcome::Clean;
        }

        if !lifecycle.terminal_admitted {
            let (reply, result) = mpsc::channel();
            let mut command = Cmd::Shutdown(reply);
            loop {
                match self.tx.try_send(command) {
                    Ok(()) => {
                        lifecycle.terminal_admitted = true;
                        break;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        return StoreShutdownOutcome::Unclean;
                    }
                    Err(mpsc::TrySendError::Full(returned)) => {
                        command = returned;
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return StoreShutdownOutcome::RetryableFailure;
                        }
                        thread::sleep(remaining.min(Duration::from_millis(1)));
                    }
                }
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return StoreShutdownOutcome::Unclean;
            }
            match result.recv_timeout(remaining) {
                Ok(true) => {}
                Ok(false) => {
                    // The actor stays live after a definite failed flush.
                    lifecycle.terminal_admitted = false;
                    return StoreShutdownOutcome::RetryableFailure;
                }
                Err(_) => return StoreShutdownOutcome::Unclean,
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero()
            || lifecycle
                .exited
                .get_mut()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .recv_timeout(remaining)
                .is_err()
        {
            return StoreShutdownOutcome::Unclean;
        }
        let Some(join) = lifecycle.join.take() else {
            return StoreShutdownOutcome::Unclean;
        };
        // The exit proof is sent after actor resources are dropped. Wait for
        // the OS thread itself to reach its terminal state before calling the
        // otherwise-unbounded JoinHandle::join.
        while !join.is_finished() && Instant::now() < deadline {
            thread::yield_now();
        }
        if !join.is_finished() || join.join().is_err() {
            return StoreShutdownOutcome::Unclean;
        }
        self.shutdown_clean.store(true, Ordering::Release);
        StoreShutdownOutcome::Clean
    }
}

impl Store for SqliteStore {
    fn save_session(&self, session: SessionState) {
        if !admissible_session(&session) {
            return;
        }
        let mut latest = self
            .latest_session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let needs_wake = latest.is_none();
        *latest = Some(session);
        drop(latest);
        // A full queue already guarantees the actor is awake. It checks this
        // mailbox before every queued command, so no blocking or lost wakeup.
        if needs_wake {
            let _ = self.tx.try_send(Cmd::SaveWake);
        }
    }

    fn flush(&self) -> bool {
        SqliteStore::flush(self)
    }

    fn flush_until(&self, deadline: Instant) -> bool {
        SqliteStore::flush_until(self, deadline)
    }

    fn shutdown_until(&self, deadline: Instant) -> StoreShutdownOutcome {
        SqliteStore::shutdown_until(self, deadline)
    }

    fn load_session(&self) -> SessionLoad {
        let (tx, rx) = mpsc::channel();
        if self.tx.try_send(Cmd::Load(tx)).is_err() {
            return SessionLoad::Failed;
        }
        rx.recv_timeout(STORE_RPC_TIMEOUT)
            .unwrap_or(SessionLoad::Failed)
    }

    fn set_aside_session(&self) -> Option<(SessionLoad, Option<std::path::PathBuf>)> {
        let (tx, rx) = mpsc::channel();
        self.tx.try_send(Cmd::SetAsideSession(tx)).ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok().flatten()
    }

    fn load_profile_avatars(&self) -> Vec<(ProfileId, ProfileAvatar)> {
        let (tx, rx) = mpsc::channel();
        if self.tx.try_send(Cmd::LoadProfileAvatars(tx)).is_err() {
            return Vec::new();
        }
        rx.recv_timeout(STORE_RPC_TIMEOUT).unwrap_or_default()
    }

    fn save_profile_avatar(
        &self,
        profile: ProfileId,
        avatar: ProfileAvatar,
        photo: Option<Arc<Vec<u8>>>,
        done: Box<dyn FnOnce(bool) + Send>,
    ) {
        if !self.admits_writes() {
            return done(false);
        }
        if let Err(
            mpsc::TrySendError::Full(Cmd::SaveProfileAvatar(_, _, _, done))
            | mpsc::TrySendError::Disconnected(Cmd::SaveProfileAvatar(_, _, _, done)),
        ) = self
            .tx
            .try_send(Cmd::SaveProfileAvatar(profile, avatar, photo, done))
        {
            done(false);
        }
    }

    fn profile_avatar_photo(
        &self,
        profile: ProfileId,
        done: Box<dyn FnOnce(Option<Vec<u8>>) + Send>,
    ) {
        if let Err(
            mpsc::TrySendError::Full(Cmd::ProfileAvatarPhoto(_, done))
            | mpsc::TrySendError::Disconnected(Cmd::ProfileAvatarPhoto(_, done)),
        ) = self.tx.try_send(Cmd::ProfileAvatarPhoto(profile, done))
        {
            done(None);
        }
    }

    fn load_page_permission_catalog(
        &self,
        profile: ProfileId,
        done: PagePermissionCatalogLoadDone,
    ) -> bool {
        let lifecycle = self
            .lifecycle
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if lifecycle.terminal_admitted || self.shutdown_clean.load(Ordering::Acquire) {
            return false;
        }
        self.tx
            .try_send(Cmd::LoadPagePermissionCatalog(profile, done))
            .is_ok()
    }

    fn mutate_page_permission_catalog(
        &self,
        profile: ProfileId,
        expected: PagePermissionCatalogRevision,
        patch: PagePermissionPatch,
        done: PagePermissionCatalogMutationDone,
    ) -> bool {
        let lifecycle = self
            .lifecycle
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if lifecycle.terminal_admitted || self.shutdown_clean.load(Ordering::Acquire) {
            return false;
        }
        let Some(permit) =
            PagePermissionMutationPermit::acquire(&self.page_permission_mutation_admission)
        else {
            return false;
        };
        self.tx
            .try_send(Cmd::MutatePagePermissionCatalog(
                profile, expected, patch, permit, done,
            ))
            .is_ok()
    }

    fn app_setting(&self, key: &str) -> Option<String> {
        if key.is_empty() || key.len() > MAX_SETTING_KEY_BYTES {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.tx.try_send(Cmd::GetSetting(key.into(), tx)).ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok().flatten()
    }

    fn app_setting_readback(&self, key: &str) -> zephium_core::ports::store::AppSettingReadback {
        use zephium_core::ports::store::AppSettingReadback as Readback;
        if key.is_empty() || key.len() > MAX_SETTING_KEY_BYTES {
            return Readback::Unavailable;
        }
        let (tx, rx) = mpsc::channel();
        if self.tx.try_send(Cmd::ReadSetting(key.into(), tx)).is_err() {
            return Readback::Unavailable;
        }
        rx.recv_timeout(STORE_RPC_TIMEOUT)
            .unwrap_or(Readback::Unavailable)
    }

    fn set_app_setting(&self, key: String, value: String) -> bool {
        if key.is_empty()
            || key.len() > MAX_SETTING_KEY_BYTES
            || value.len() > MAX_SETTING_VALUE_BYTES
        {
            return false;
        }
        let mut settings = self
            .pending_settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let was_known = settings.contains_key(&key);
        if !was_known && settings.unique_keys() >= MAX_PENDING_SETTINGS {
            return false;
        }
        settings.known_keys.insert(key.clone());
        let previous = settings.pending.insert(key.clone(), value);
        match self.tx.try_send(Cmd::SettingWake) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => true,
            Err(mpsc::TrySendError::Disconnected(_)) => {
                if !was_known {
                    settings.known_keys.remove(&key);
                }
                match previous {
                    Some(previous) => {
                        settings.pending.insert(key, previous);
                    }
                    None => {
                        settings.pending.remove(&key);
                    }
                }
                false
            }
        }
    }

    fn favicon_age(&self, profile: ProfileId, origin: &str) -> Option<i64> {
        if !hub::valid_favicon_key(origin) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .try_send(Cmd::FaviconAge(profile, origin.into(), tx))
            .ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok().flatten()
    }

    fn save_favicon(
        &self,
        profile: ProfileId,
        origin: String,
        _content_type: Option<String>,
        bytes: Vec<u8>,
    ) {
        let Some(content_type) = hub::validated_favicon(&origin, &bytes).map(str::to_owned) else {
            return;
        };
        let _ = self
            .tx
            .try_send(Cmd::SaveFavicon(profile, origin, Some(content_type), bytes));
    }

    fn forget_page_favicon(&self, profile: ProfileId, page: String) {
        if hub::valid_favicon_key(&page) {
            let _ = self.tx.try_send(Cmd::ForgetPageFavicon(profile, page));
        }
    }

    fn favicon_bytes(&self, profile: ProfileId, origin: &str) -> Option<(Option<String>, Vec<u8>)> {
        if !hub::valid_favicon_key(origin) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .try_send(Cmd::FaviconBytes(profile, origin.into(), tx))
            .ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok().flatten()
    }

    fn favicon_raster_with_age(&self, profile: ProfileId, origin: &str) -> Option<(Vec<u8>, i64)> {
        if !hub::valid_favicon_key(origin) {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.tx
            .try_send(Cmd::FaviconRasterWithAge(profile, origin.into(), tx))
            .ok()?;
        rx.recv_timeout(STORE_RPC_TIMEOUT).ok().flatten()
    }

    fn favicon_rasters(&self, profile: ProfileId, origins: &[String]) -> Vec<(String, Vec<u8>)> {
        if origins.len() > MAX_FAVICON_BATCH_ORIGINS
            || origins.iter().any(|origin| !hub::valid_favicon_key(origin))
        {
            return Vec::new();
        }
        let mut unique = HashSet::with_capacity(origins.len());
        if origins.iter().any(|origin| !unique.insert(origin.as_str())) {
            return Vec::new();
        }
        let (tx, rx) = mpsc::channel();
        if self
            .tx
            .try_send(Cmd::FaviconRasters(profile, origins.to_vec(), tx))
            .is_err()
        {
            return Vec::new();
        }
        rx.recv_timeout(STORE_RPC_TIMEOUT).unwrap_or_default()
    }

    fn download_recovery_profiles(&self, done: Box<dyn FnOnce(DownloadStoreReply) + Send>) -> bool {
        self.tx
            .try_send(Cmd::DownloadRecoveryProfiles(done))
            .is_ok()
    }

    fn download_call(
        &self,
        profile: ProfileId,
        call: DownloadStoreCall,
        done: Box<dyn FnOnce(DownloadStoreReply) + Send>,
    ) -> bool {
        if let DownloadStoreCall::Save(record) = &call {
            if !record.validate() {
                return false;
            }
        }
        self.tx
            .try_send(Cmd::DownloadCall(profile, call, done))
            .is_ok()
    }

    fn import_favicons(&self, profile: ProfileId, icons: Vec<(String, Vec<u8>)>) -> Option<u32> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .try_send(Cmd::ImportFavicons(profile, icons, tx))
            .ok()?;
        rx.recv_timeout(IMPORT_RPC_TIMEOUT).ok().flatten()
    }

    fn discard_session_backup(
        &self,
        profile: ProfileId,
        done: Box<dyn FnOnce(bool) + Send>,
    ) -> bool {
        self.tx
            .try_send(Cmd::DiscardSessionBackup(profile, done))
            .is_ok()
    }

    fn zoom_levels(&self, profile: ProfileId) -> Vec<zephium_core::zoom::SavedLevel> {
        self.ask(|reply| Cmd::ZoomLevels(profile, reply))
            .unwrap_or_default()
    }

    fn save_zoom_level(
        &self,
        profile: ProfileId,
        site: String,
        percent: Option<u16>,
        set_at: i64,
    ) -> bool {
        hub::valid_zoom_site(&site)
            && percent.is_none_or(zephium_core::zoom::valid_percent)
            && self
                .tx
                .try_send(Cmd::SaveZoomLevel(profile, site, percent, set_at))
                .is_ok()
    }

    fn popup_sites(
        &self,
        profile: ProfileId,
    ) -> Option<Vec<zephium_core::permissions::PageOrigin>> {
        self.ask(|reply| Cmd::PopupSites(profile, reply)).flatten()
    }

    fn set_popup_site(
        &self,
        profile: ProfileId,
        origin: zephium_core::permissions::PageOrigin,
        allowed: bool,
    ) -> bool {
        self.ask(|reply| Cmd::SetPopupSite(profile, origin, allowed, reply))
            .unwrap_or(false)
    }

    fn pending_profile_deletions(&self) -> ProfileDeletionLoad {
        let (tx, rx) = mpsc::channel();
        if self.tx.try_send(Cmd::PendingProfileDeletions(tx)).is_err() {
            return ProfileDeletionLoad::Failed;
        }
        rx.recv_timeout(STORE_RPC_TIMEOUT)
            .unwrap_or(ProfileDeletionLoad::Failed)
    }

    fn authorize_profile_deletion(
        &self,
        profile: ProfileId,
        filtered_session: SessionState,
        deadline: Instant,
    ) -> ProfileDeletionAuthorizeOutcome {
        if !admissible_session(&filtered_session)
            || filtered_session
                .profiles
                .iter()
                .any(|candidate| candidate.id == profile)
            || zephium_core::session::canonicalize(filtered_session.clone()) != filtered_session
        {
            return ProfileDeletionAuthorizeOutcome::InvalidSession;
        }
        if Instant::now() >= deadline {
            return ProfileDeletionAuthorizeOutcome::NotAdmitted;
        }

        let (tx, rx) = mpsc::channel();
        let mut command = Cmd::AuthorizeProfileDeletion(profile, filtered_session, tx);
        loop {
            match self.tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return ProfileDeletionAuthorizeOutcome::NotAdmitted;
                }
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return ProfileDeletionAuthorizeOutcome::NotAdmitted;
                    }
                    thread::sleep(remaining.min(Duration::from_millis(1)));
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return ProfileDeletionAuthorizeOutcome::OutcomeUnknown;
        }
        rx.recv_timeout(remaining)
            .unwrap_or(ProfileDeletionAuthorizeOutcome::OutcomeUnknown)
    }

    fn finalize_profile_deletion(
        &self,
        profile: ProfileId,
        deadline: Instant,
    ) -> ProfileDeletionFinalizeOutcome {
        if Instant::now() >= deadline {
            return ProfileDeletionFinalizeOutcome::NotAdmitted;
        }
        let (tx, rx) = mpsc::channel();
        let mut command = Cmd::FinalizeProfileDeletion(profile, tx);
        loop {
            match self.tx.try_send(command) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return ProfileDeletionFinalizeOutcome::NotAdmitted;
                }
                Err(mpsc::TrySendError::Full(returned)) => {
                    command = returned;
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return ProfileDeletionFinalizeOutcome::NotAdmitted;
                    }
                    thread::sleep(remaining.min(Duration::from_millis(1)));
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return ProfileDeletionFinalizeOutcome::OutcomeUnknown;
        }
        rx.recv_timeout(remaining)
            .unwrap_or(ProfileDeletionFinalizeOutcome::OutcomeUnknown)
    }
}

fn admissible_session(session: &SessionState) -> bool {
    const MAX_NAME_BYTES: usize = MAX_SESSION_NAME_CHARS * 4;
    const MAX_URL_BYTES: usize = 8 * 1024;
    const MAX_TITLE_BYTES: usize = zephium_core::item::MAX_PAGE_TITLE_CHARS * 4;

    session.profiles.len() <= MAX_SESSION_PROFILES
        && session.spaces.len() <= MAX_SESSION_SPACES
        && session.items.len() <= MAX_SESSION_ITEMS
        && session.profiles.iter().all(|profile| {
            profile.kind != ProfileKind::Incognito && profile.name.len() <= MAX_NAME_BYTES
        })
        && session
            .spaces
            .iter()
            .all(|space| space.name.len() <= MAX_NAME_BYTES)
        && session.items.iter().all(|item| match &item.kind {
            PersistedKind::Folder { name } => name.len() <= MAX_NAME_BYTES,
            PersistedKind::Tab {
                url, title, zoom, ..
            } => {
                url.len() <= MAX_URL_BYTES
                    && title.len() <= MAX_TITLE_BYTES
                    && zoom.is_finite()
                    && (0.3..=3.0).contains(zoom)
            }
        })
        && session.recently_closed.len() <= MAX_RECENTLY_CLOSED_TABS
        && session.recently_closed.iter().all(|entry| {
            entry.url.len() <= MAX_URL_BYTES
                && entry.title.len() <= MAX_TITLE_BYTES
                && entry.zoom.is_finite()
                && (0.3..=3.0).contains(&entry.zoom)
        })
        && session.splits.as_ref().is_none_or(admissible_split)
}

fn admissible_split(root: &Pane) -> bool {
    // This boundary accepts an in-process DTO, not only bounded JSON. Walk it
    // iteratively so a future privileged caller cannot feed an oversized
    // recursive tree into clone/canonicalization/serialization first.
    let mut stack = vec![(root, 0_usize)];
    let mut nodes = 0_usize;
    while let Some((pane, depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        if depth > MAX_SPLIT_DEPTH || nodes > MAX_SESSION_ITEMS.saturating_mul(2).saturating_sub(1)
        {
            return false;
        }
        match pane {
            Pane::Positioned(_) => return false,
            Pane::Leaf(_) => {}
            Pane::Branch { ratio, a, b, .. } => {
                if !ratio.is_finite() || !(0.05..=0.95).contains(ratio) {
                    return false;
                }
                stack.push((b, depth + 1));
                stack.push((a, depth + 1));
            }
        }
    }
    true
}

fn actor(
    mut hub: Hub,
    rx: Receiver<Cmd>,
    latest_session: Arc<Mutex<Option<SessionState>>>,
    pending_settings: Arc<Mutex<PendingSettings>>,
) {
    let mut pending: Option<PendingSession> = None;
    let mut setting_retry = WriteRetry::default();
    loop {
        if let Err(error) = hub.prune_expired_session_backup() {
            eprintln!("store: session backup expiration failed: {error}");
        }
        let deadline = pending
            .as_ref()
            .map(PendingSession::deadline)
            .into_iter()
            .chain(setting_retry.retry_at)
            .chain(hub.session_backup_deadline())
            .min();
        let cmd = if let Some(deadline) = deadline {
            let wait = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(wait) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => break,
            }
        };
        if cmd.is_some() {
            absorb_latest_session(&latest_session, &mut pending);
            if !matches!(
                &cmd,
                Some(Cmd::Flush(_) | Cmd::Shutdown(_) | Cmd::PendingProfileDeletions(_))
            ) {
                let _ = flush_settings(&mut hub, &pending_settings, &mut setting_retry, false);
            }
        }
        match cmd {
            None => {
                if pending
                    .as_ref()
                    .is_some_and(|pending| pending.due(Instant::now()))
                {
                    let _ = flush(&mut hub, &mut pending);
                }
                let _ = flush_settings(&mut hub, &pending_settings, &mut setting_retry, false);
            }
            Some(Cmd::SaveWake) => {}
            Some(Cmd::SettingWake) => {}
            Some(Cmd::Load(reply)) => {
                let _ = flush(&mut hub, &mut pending);
                let loaded = session_load(&mut hub);
                let _ = reply.send(loaded);
            }
            Some(Cmd::SetAsideSession(reply)) => {
                let restarted = match hub.set_aside_recovery() {
                    Ok(file) => Some((session_load(&mut hub), file)),
                    Err(error) => {
                        eprintln!("store: setting the unrestorable session aside failed: {error}");
                        None
                    }
                };
                let _ = reply.send(restarted);
            }
            Some(Cmd::LoadProfileAvatars(reply)) => {
                let _ = reply.send(hub.profile_avatars().unwrap_or_default());
            }
            Some(Cmd::SaveProfileAvatar(profile, avatar, photo, done)) => {
                if !hub.knows(profile) && !flush(&mut hub, &mut pending) {
                    done(false);
                    continue;
                }
                done(hub.save_profile_avatar(profile, avatar, photo).is_ok());
            }
            Some(Cmd::ProfileAvatarPhoto(profile, done)) => {
                done(hub.profile_avatar_photo(profile));
            }
            Some(Cmd::LoadPagePermissionCatalog(profile, done)) => {
                let outcome = match hub.load_page_permission_catalog(profile) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        eprintln!(
                            "store: profile {profile} page-permission catalog load failed: {error}"
                        );
                        PagePermissionCatalogLoadOutcome::Failed
                    }
                };
                done(outcome);
            }
            Some(Cmd::MutatePagePermissionCatalog(profile, expected, patch, _permit, done)) => {
                let outcome = match hub.mutate_page_permission_catalog(profile, expected, patch) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        eprintln!(
                            "store: profile {profile} page-permission catalog mutation failed: {error}"
                        );
                        PagePermissionCatalogMutationOutcome::Failed
                    }
                };
                done(outcome);
            }
            Some(Cmd::GetSetting(key, reply)) => {
                let _ = reply.send(hub.app_setting(&key));
            }
            Some(Cmd::ReadSetting(key, reply)) => {
                let _ = reply.send(hub.app_setting_readback(&key));
            }
            Some(Cmd::DownloadRecoveryProfiles(done)) => {
                done(hub.download_recovery_profiles());
            }
            Some(Cmd::DownloadCall(profile, call, done)) => {
                done(hub.download_call(profile, call));
            }
            Some(Cmd::ImportFavicons(profile, icons, reply)) => {
                let _ = reply.send(hub.import_favicons(profile, &icons));
            }
            Some(Cmd::DiscardSessionBackup(profile, done)) => {
                let result = if hub.knows(profile) || flush(&mut hub, &mut pending) {
                    hub.discard_session_backup(profile)
                } else {
                    Err(rusqlite::Error::InvalidQuery)
                };
                if let Err(error) = &result {
                    eprintln!("store: session backup disposal failed: {error}");
                }
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    done(result.is_ok());
                }));
            }
            Some(Cmd::ZoomLevels(profile, reply)) => {
                let _ = reply.send(hub.zoom_levels(profile));
            }
            Some(Cmd::SaveZoomLevel(profile, site, percent, set_at)) => {
                if hub.knows(profile) || flush(&mut hub, &mut pending) {
                    hub.save_zoom_level(profile, &site, percent, set_at);
                }
            }
            Some(Cmd::PopupSites(profile, reply)) => {
                let _ = reply.send(hub.popup_sites(profile));
            }
            Some(Cmd::SetPopupSite(profile, origin, allowed, reply)) => {
                let stored = (hub.knows(profile) || flush(&mut hub, &mut pending))
                    && hub.set_popup_site(profile, &origin, allowed);
                let _ = reply.send(stored);
            }
            Some(Cmd::FaviconAge(profile, origin, reply)) => {
                let _ = reply.send(hub.favicon_age(profile, &origin));
            }
            Some(Cmd::FaviconRasterWithAge(profile, origin, reply)) => {
                let _ = reply.send(hub.favicon_raster_with_age(profile, &origin));
            }
            Some(Cmd::SaveFavicon(profile, origin, content_type, bytes)) => {
                hub.save_favicon(profile, &origin, content_type.as_deref(), &bytes);
            }
            Some(Cmd::ForgetPageFavicon(profile, page)) => {
                hub.forget_page_favicon(profile, &page);
            }
            Some(Cmd::FaviconBytes(profile, origin, reply)) => {
                let _ = reply.send(hub.favicon_bytes(profile, &origin));
            }
            Some(Cmd::FaviconRasters(profile, origins, reply)) => {
                let rasters = origins
                    .into_iter()
                    .filter_map(|origin| {
                        hub.favicon_bytes(profile, &origin)
                            .map(|(_, bytes)| (origin, bytes))
                    })
                    .collect();
                let _ = reply.send(rasters);
            }
            Some(Cmd::PendingProfileDeletions(reply)) => {
                let result = match hub.reconcile_profile_deletion_journal() {
                    Ok(deletions) => {
                        if deletions.is_empty() {
                            if flush(&mut hub, &mut pending) {
                                ProfileDeletionLoad::Loaded(deletions)
                            } else {
                                ProfileDeletionLoad::Failed
                            }
                        } else {
                            // A durable authorization supersedes a retained
                            // pre-barrier snapshot that still contains any
                            // journaled profile. Never retry that stale snapshot
                            // before reporting the authoritative journal. Newer
                            // survivor-only state remains pending and is
                            // rescheduled by the application after tombstoning.
                            let conflicts = pending.as_ref().is_some_and(|save| {
                                deletions.iter().any(|deletion| {
                                    save.state
                                        .profiles
                                        .iter()
                                        .any(|profile| profile.id == deletion.profile)
                                })
                            });
                            if conflicts {
                                pending = None;
                            }
                            ProfileDeletionLoad::Loaded(deletions)
                        }
                    }
                    Err(error) => {
                        eprintln!("store: cannot reconcile profile deletion journal: {error}");
                        ProfileDeletionLoad::Failed
                    }
                };
                let _ = reply.send(result);
            }
            Some(Cmd::AuthorizeProfileDeletion(profile, session, reply)) => {
                let result = match hub.authorize_profile_deletion(profile, &session) {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        // SQLite commit errors can be durability-ambiguous.
                        // Never tell the coordinator authorization failed and
                        // invite it to infer the opposite; reconciliation via
                        // the durable journal is required.
                        eprintln!("store: profile {profile} deletion authorization returned an ambiguous error: {error}");
                        match hub.reconcile_profile_deletion_journal() {
                            Ok(deletions)
                                if deletions.iter().any(|deletion| deletion.profile == profile) =>
                            {
                                ProfileDeletionAuthorizeOutcome::Authorized
                            }
                            Ok(_) => ProfileDeletionAuthorizeOutcome::Failed,
                            Err(reconcile_error) => {
                                eprintln!("store: profile {profile} deletion authorization could not be reconciled: {reconcile_error}");
                                ProfileDeletionAuthorizeOutcome::OutcomeUnknown
                            }
                        }
                    }
                };
                if matches!(
                    result,
                    ProfileDeletionAuthorizeOutcome::Authorized
                        | ProfileDeletionAuthorizeOutcome::AlreadyAuthorized
                ) {
                    // This synchronous snapshot supersedes every coalesced
                    // save observed before the authorization command.
                    pending = None;
                }
                let _ = reply.send(result);
            }
            Some(Cmd::FinalizeProfileDeletion(profile, reply)) => {
                let result = if flush(&mut hub, &mut pending) {
                    match hub.finalize_profile_deletion(profile) {
                        Ok(true) => ProfileDeletionFinalizeOutcome::Completed,
                        Ok(false) => ProfileDeletionFinalizeOutcome::NotAuthorized,
                        Err(error) => {
                            eprintln!("store: cannot finalize profile {profile} deletion: {error}");
                            ProfileDeletionFinalizeOutcome::Failed
                        }
                    }
                } else {
                    ProfileDeletionFinalizeOutcome::Failed
                };
                let _ = reply.send(result);
            }
            Some(Cmd::Flush(ack)) => {
                let settings_durable =
                    flush_settings(&mut hub, &pending_settings, &mut setting_retry, true);
                let session_durable = flush(&mut hub, &mut pending);
                let _ = ack.send(settings_durable && session_durable);
            }
            Some(Cmd::Shutdown(ack)) => {
                let settings_durable =
                    flush_settings(&mut hub, &pending_settings, &mut setting_retry, true);
                let session_durable = flush(&mut hub, &mut pending);
                let durable = settings_durable && session_durable;
                let _ = ack.send(durable);
                if durable {
                    // Returning drops Hub and every SQLite connection before
                    // the wrapper thread publishes its exit proof.
                    return;
                }
            }
        }
        // Non-save traffic must not reset either deadline. Long-running reads
        // may overshoot it, so check again after every command as well.
        if pending
            .as_ref()
            .is_some_and(|pending| pending.due(Instant::now()))
        {
            let _ = flush(&mut hub, &mut pending);
        }
    }
    absorb_latest_session(&latest_session, &mut pending);
    // Never make the dropping/UI thread wait here. Normal shutdown already
    // used its caller-owned deadline barrier. On an unexpected sender drop,
    // this detached actor gets one best-effort terminal durability attempt.
    let _ = flush_settings(&mut hub, &pending_settings, &mut setting_retry, true);
    let _ = flush(&mut hub, &mut pending);
}

fn flush_settings(
    hub: &mut Hub,
    mailbox: &Mutex<PendingSettings>,
    retry: &mut WriteRetry,
    force: bool,
) -> bool {
    if !force
        && retry
            .retry_at
            .is_some_and(|retry_at| Instant::now() < retry_at)
    {
        return false;
    }
    let settings = {
        let mut mailbox = mailbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if mailbox.pending.is_empty() {
            retry.clear();
            return true;
        }
        let settings = std::mem::take(&mut mailbox.pending);
        mailbox.in_flight.extend(settings.keys().cloned());
        settings
    };

    let mut failed = PendingSettings::default();
    for (key, value) in settings {
        match hub.set_app_setting(&key, &value) {
            Ok(true) => {}
            Ok(false) => {
                // Admission and the actor's authoritative key registry should
                // make this unreachable. Treat external database divergence as
                // a failed durability barrier; never drop an accepted value or
                // acknowledge a clean shutdown.
                eprintln!(
                    "store: application-setting write was rejected after admission for key {key}"
                );
                failed.pending.insert(key, value);
            }
            Err(error) => {
                eprintln!("store: application-setting write failed: {error}");
                failed.pending.insert(key, value);
            }
        }
    }
    let had_failures = !failed.pending.is_empty();
    {
        let mut mailbox = mailbox
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Every key in this completed batch leaves the in-flight reservation.
        // If a concurrent caller admitted a newer value, it already occupies
        // `pending` and wins over a failed older write for the same key.
        mailbox.in_flight.clear();
        for (key, value) in failed.pending {
            mailbox.pending.entry(key).or_insert(value);
        }
    }
    if had_failures {
        retry.failed(Instant::now());
        false
    } else {
        retry.clear();
        true
    }
}

fn absorb_latest_session(
    latest: &Mutex<Option<SessionState>>,
    pending: &mut Option<PendingSession>,
) {
    let Some(state) = latest
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    else {
        return;
    };
    let now = Instant::now();
    match pending {
        Some(pending) => {
            pending.state = state;
            pending.latest = now;
        }
        None => *pending = Some(PendingSession::new(state, now)),
    }
}

fn flush(hub: &mut Hub, pending: &mut Option<PendingSession>) -> bool {
    let Some(mut save) = pending.take() else {
        return true;
    };
    match hub.save(&save.state) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("store: save failed: {e}");
            // Keep the latest snapshot retryable instead of acknowledging a
            // failed barrier and silently throwing the only in-memory copy
            // away. Exponential backoff bounds disk wakeups and log spam when
            // the failure is persistent; an explicit Flush still retries
            // immediately as a caller-owned durability barrier.
            save.failed(Instant::now());
            *pending = Some(save);
            false
        }
    }
}

/// The session the hub holds, as the shell receives it.
fn session_load(hub: &mut Hub) -> SessionLoad {
    match hub.load_authoritative() {
        Ok(Some(authoritative)) => {
            let profiles = hub.degraded_profile_ids();
            if profiles.is_empty() {
                SessionLoad::Loaded {
                    state: authoritative.state,
                }
            } else {
                SessionLoad::LoadedWithDegradedProfiles {
                    state: authoritative.state,
                    profiles,
                }
            }
        }
        Ok(None) => SessionLoad::Absent,
        Err(_) if hub.recovery_reason().is_some() => SessionLoad::RecoveryRequired {
            reason: hub
                .recovery_reason()
                .unwrap_or("authoritative session requires recovery")
                .to_owned(),
        },
        Err(error) => {
            eprintln!("store: session load failed: {error}");
            SessionLoad::Failed
        }
    }
}
