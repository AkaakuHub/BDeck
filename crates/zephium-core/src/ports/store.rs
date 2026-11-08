use crate::ids::ProfileId;
use crate::permissions::{
    PagePermissionCatalog, PagePermissionCatalogRevision, PagePermissionPatch,
    PagePermissionPatchResults,
};
use crate::session::SessionState;
use std::time::Instant;

pub const MAX_POPUP_SITES: usize = 512;

/// Maximum number of exact origins that browser chrome may hydrate in one
/// favicon-cache read. The returned raster for each origin is independently
/// fixed at `icon::RGBA32_BYTES`, bounding a batch to two MiB before small
/// collection overhead.
pub const MAX_FAVICON_BATCH_ORIGINS: usize = 512;

/// Durable cross-restart state for one profile deletion.
///
/// A row is created atomically with removal from the authoritative session
/// registry. `native_erasure_verified` becomes true only after the engine has
/// proved its platform-owned website data absent. The store must retain the
/// authorization until its own profile database has also been removed. A
/// platform adapter may retain an internal post-unlink tombstone beyond that
/// point for restart-time filesystem verification; such completed rows are
/// deliberately not returned as pending work to the current shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingProfileDeletion {
    pub profile: ProfileId,
    pub native_erasure_verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileDeletionLoad {
    Loaded(Vec<PendingProfileDeletion>),
    Failed,
}

/// Truthful result of the synchronous deletion-authorization barrier.
///
/// Native erasure may start only after `Authorized` or `AlreadyAuthorized`.
/// `OutcomeUnknown` means the bounded caller wait expired after the command
/// entered the storage actor; callers must reconcile through
/// `pending_profile_deletions` and must not assume either success or failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDeletionAuthorizeOutcome {
    Authorized,
    AlreadyAuthorized,
    NotRegistered,
    SessionConflict,
    InvalidSession,
    NotAdmitted,
    OutcomeUnknown,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileDeletionFinalizeOutcome {
    /// Native proof is durable and the exact local artifacts are absent for
    /// this process. The store may still retain a hidden completed tombstone
    /// until a fresh process verifies Windows filesystem recovery.
    Completed,
    NotAuthorized,
    NotAdmitted,
    OutcomeUnknown,
    Failed,
}

/// `SessionLoad::RecoveryRequired` for a session saved by a newer Zephium: it
/// opens again once that version is back, so it is never set aside.
pub const NEWER_SESSION_REASON: &str = "authoritative session is newer than supported";

/// Result of reading the authoritative browser session.
///
/// `Failed` is deliberately distinct from `Absent`: callers may initialize a
/// new profile only when no snapshot exists. A corrupt snapshot or storage I/O
/// failure must not be interpreted as first run and overwritten.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionLoad {
    Absent,
    Loaded {
        state: SessionState,
    },
    /// The authoritative session is exact and fully usable, but one or more
    /// registered per-profile ancillary databases could not be safely opened
    /// at their shipped schema. Their original files are preserved and every
    /// favicon operation for these profiles is disabled until an
    /// explicit repair, export, or deletion flow handles them.
    LoadedWithDegradedProfiles {
        state: SessionState,
        profiles: Vec<ProfileId>,
    },
    /// The authoritative bytes were preserved, but they do not describe an
    /// exact canonical session. The store is read-only until an explicit
    /// recovery flow exports, repairs, or discards the quarantined snapshot.
    RecoveryRequired {
        reason: String,
    },
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PagePermissionCatalogLoadOutcome {
    Loaded(PagePermissionCatalog),
    NotRegistered,
    /// The exact per-profile database was preserved but could not be safely
    /// opened at the shipped schema. No subset of its grants is returned.
    DegradedProfile,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagePermissionCatalogMutationApplied {
    pub catalog_revision: PagePermissionCatalogRevision,
    /// One bounded result for every change in the submitted patch, in patch
    /// order. Create/update carries the exact durable row; delete carries
    /// `None`.
    pub results: PagePermissionPatchResults,
}

/// Durable result of one atomic profile page-permission patch.
///
/// `OutcomeUnknown` means SQLite settlement could not be observed after the
/// transaction entered commit; callers must exact-load before attempting a
/// new mutation. `Invalid` and `LimitReached` prove no durable write or commit
/// was attempted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PagePermissionCatalogMutationOutcome {
    Applied(PagePermissionCatalogMutationApplied),
    Conflict {
        current: PagePermissionCatalogRevision,
    },
    NotRegistered,
    DegradedProfile,
    Invalid,
    LimitReached,
    RevisionExhausted,
    OutcomeUnknown,
    Failed,
}

/// Result of the store's terminal process-boundary protocol.
///
/// `RetryableFailure` proves the terminal command was not entered (normally
/// because the durability barrier failed), so the live actor may accept a
/// later retry. `Unclean` means terminal ownership may have transferred but
/// actor exit/resource release was not proved before the caller's deadline;
/// continued in-process use is unsafe and the process must exit non-zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreShutdownOutcome {
    RetryableFailure,
    Clean,
    Unclean,
}

/// Exact application-setting readback. Missing is distinct from an unavailable
/// database/actor so permission defaults never turn read failures into consent.
#[derive(Clone, Eq, PartialEq)]
pub enum AppSettingReadback {
    Value(String),
    Missing,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckImportOutcome {
    Applied,
    Rejected,
    Unavailable,
}

pub trait Store {
    fn import_deck(
        &self,
        _session: SessionState,
        _deck: crate::deck::DeckState,
        done: Box<dyn FnOnce(DeckImportOutcome) + Send>,
    ) {
        done(DeckImportOutcome::Rejected);
    }
    /// Every saved profile avatar, read once at startup. Profiles without a
    /// row are drawn with [`crate::profiles::ProfileAvatar::initial`].
    fn load_profile_avatars(&self) -> Vec<(ProfileId, crate::profiles::ProfileAvatar)> {
        Vec::new()
    }
    /// Replaces a profile's avatar. A photo style carries its downscaled
    /// bytes; any other style clears the stored photo.
    fn save_profile_avatar(
        &self,
        _profile: ProfileId,
        _avatar: crate::profiles::ProfileAvatar,
        _photo: Option<std::sync::Arc<Vec<u8>>>,
        done: Box<dyn FnOnce(bool) + Send>,
    ) {
        done(false);
    }
    fn profile_avatar_photo(
        &self,
        _profile: ProfileId,
        done: Box<dyn FnOnce(Option<Vec<u8>>) + Send>,
    ) {
        done(None);
    }
    fn save_session(&self, session: SessionState);
    /// Ordered session-durability barrier for shutdown and other process
    /// boundaries. Returns only after the latest session snapshot queued
    /// before this call has committed (`true`) or the adapter reports failure.
    fn flush(&self) -> bool;
    /// Deadline-aware form of the durability barrier. Adapters must not keep
    /// the caller blocked after `deadline`; they may continue an already
    /// admitted OS write on their private worker after returning `false`.
    fn flush_until(&self, _deadline: Instant) -> bool {
        self.flush()
    }
    /// Flushes every mutation ordered before this call and, for actor-backed
    /// stores, terminates and joins the actor while releasing its database
    /// handles. The implementation must use the caller's existing deadline;
    /// it must not start a fresh timeout after durability completes.
    fn shutdown_until(&self, deadline: Instant) -> StoreShutdownOutcome {
        if self.flush_until(deadline) {
            StoreShutdownOutcome::Clean
        } else {
            StoreShutdownOutcome::RetryableFailure
        }
    }
    fn load_session(&self) -> SessionLoad;
    /// After `RecoveryRequired`: keeps the unrestorable session's bytes in a
    /// file and restarts the session from the profile registry with no tabs.
    /// Returns the restarted load and the file, or `None` when it could not.
    fn set_aside_session(&self) -> Option<(SessionLoad, Option<std::path::PathBuf>)> {
        None
    }
    /// Loads one complete bounded page-permission authority catalog. `true`
    /// transfers exactly-once callback ownership; `false` proves the request
    /// was not admitted. Malformed durable state fails as a whole.
    fn load_page_permission_catalog(
        &self,
        _profile: ProfileId,
        _done: Box<dyn FnOnce(PagePermissionCatalogLoadOutcome) + Send>,
    ) -> bool {
        false
    }
    /// Atomically applies at most four page-permission changes after comparing
    /// the collection revision. The patch is already structurally bounded by
    /// its core constructor before it can enter an adapter mailbox.
    fn mutate_page_permission_catalog(
        &self,
        _profile: ProfileId,
        _expected: PagePermissionCatalogRevision,
        _patch: PagePermissionPatch,
        _done: Box<dyn FnOnce(PagePermissionCatalogMutationOutcome) + Send>,
    ) -> bool {
        false
    }
    /// Enumerates only registered durable profiles for startup download recovery.
    /// False means the callback was not retained.
    fn download_recovery_profiles(
        &self,
        _done: Box<dyn FnOnce(crate::downloads::DownloadStoreReply) + Send>,
    ) -> bool {
        false
    }
    /// Bounded asynchronous download persistence on the existing Store actor.
    /// Private profiles must never call this port. False means no callback is retained.
    fn download_call(
        &self,
        _profile: ProfileId,
        _call: crate::downloads::DownloadStoreCall,
        _done: Box<dyn FnOnce(crate::downloads::DownloadStoreReply) + Send>,
    ) -> bool {
        false
    }
    /// App-level settings live outside profiles.
    fn app_setting(&self, key: &str) -> Option<String>;
    /// Reads an authoritative value, including a confirmed absent setting.
    /// Adapters without this proof fail closed for permission consumers.
    fn app_setting_readback(&self, _key: &str) -> AppSettingReadback {
        AppSettingReadback::Unavailable
    }

    /// Enqueues an ordered application-setting mutation. `true` means the
    /// bounded adapter accepted the command; durability is established by a
    /// later `flush`/`flush_until` barrier. `false` is a definite rejection.
    fn set_app_setting(&self, key: String, value: String) -> bool;

    /// Site icons another browser held, as fixed 32x32 rasters by origin. An
    /// origin that already has one keeps it. Returns how many were added.
    fn import_favicons(&self, _profile: ProfileId, _icons: Vec<(String, Vec<u8>)>) -> Option<u32> {
        None
    }
    /// Disposes recovery copies that may contain this profile's browsing
    /// data. False means queue refusal, with no completion; admitted calls
    /// report whether disposal committed. Call for site-data erasure too.
    fn discard_session_backup(
        &self,
        _profile: ProfileId,
        _done: Box<dyn FnOnce(bool) + Send>,
    ) -> bool {
        false
    }
    /// Age in seconds of the cached icon for a favicon key, None when absent.
    /// A key is a page origin, or one page of it (`icon::page_key`) whose own
    /// icon differs from its origin's.
    fn favicon_age(&self, profile: ProfileId, origin: &str) -> Option<i64>;
    fn save_favicon(
        &self,
        profile: ProfileId,
        origin: String,
        content_type: Option<String>,
        bytes: Vec<u8>,
    );
    fn favicon_bytes(&self, profile: ProfileId, origin: &str) -> Option<(Option<String>, Vec<u8>)>;
    /// Drops the icon kept for one page, as when it now matches its origin's.
    fn forget_page_favicon(&self, _profile: ProfileId, _page: String) {}

    /// Loads one already-decoded favicon with the age of the stored copy.
    /// Age decides whether to refresh, never whether to display: an old
    /// raster is still the right thing to draw while a newer one is fetched.
    fn favicon_raster_with_age(&self, profile: ProfileId, origin: &str) -> Option<(Vec<u8>, i64)> {
        let age = self.favicon_age(profile, origin)?;
        self.favicon_bytes(profile, origin)
            .map(|(_, bytes)| (bytes, age))
    }

    /// Loads already-decoded favicon rasters for a bounded authoritative set
    /// of origins. Actor-backed stores should override this to perform one
    /// mailbox round trip; the default is suitable for simple test adapters.
    fn favicon_rasters(&self, profile: ProfileId, origins: &[String]) -> Vec<(String, Vec<u8>)> {
        if origins.len() > MAX_FAVICON_BATCH_ORIGINS {
            return Vec::new();
        }
        let mut seen = std::collections::HashSet::with_capacity(origins.len());
        origins
            .iter()
            .filter(|origin| seen.insert((*origin).clone()))
            .filter_map(|origin| {
                self.favicon_bytes(profile, origin)
                    .map(|(_, bytes)| (origin.clone(), bytes))
            })
            .collect()
    }

    /// The page zoom a profile saved per site, newest first.
    fn zoom_levels(&self, _profile: ProfileId) -> Vec<crate::zoom::SavedLevel> {
        Vec::new()
    }
    /// Saves a site's page zoom, or forgets it when `percent` is none. The
    /// oldest sites beyond `zoom::MAX_SITE_LEVELS` are dropped. False when
    /// not admitted.
    fn save_zoom_level(
        &self,
        _profile: ProfileId,
        _site: String,
        _percent: Option<u16>,
        _set_at: i64,
    ) -> bool {
        false
    }

    /// The sites `profile` lets open new tabs without a click, or None when
    /// they cannot be read.
    fn popup_sites(&self, _profile: ProfileId) -> Option<Vec<crate::permissions::PageOrigin>> {
        None
    }
    /// Adds or removes one site from that list. False when nothing was
    /// written, including when the list is full.
    fn set_popup_site(
        &self,
        _profile: ProfileId,
        _origin: crate::permissions::PageOrigin,
        _allowed: bool,
    ) -> bool {
        false
    }

    /// Returns the bounded, durable deletion work that survived the previous
    /// process. An empty default keeps non-persistent test adapters inert.
    fn pending_profile_deletions(&self) -> ProfileDeletionLoad {
        ProfileDeletionLoad::Loaded(Vec::new())
    }

    /// Atomically commits the exact canonical post-removal session and a
    /// durable deletion authorization. This is the ordering barrier between
    /// aggregate removal and native website-data erasure.
    ///
    /// The store validates persistence invariants only; policy such as whether
    /// a default/last profile may be deleted belongs to the application.
    fn authorize_profile_deletion(
        &self,
        _profile: ProfileId,
        _filtered_session: SessionState,
        _deadline: Instant,
    ) -> ProfileDeletionAuthorizeOutcome {
        ProfileDeletionAuthorizeOutcome::NotRegistered
    }

    /// Records the engine's authoritative native-erasure proof and removes
    /// the exact journal-authorized SQLite profile. Implementations must write
    /// the proof before touching the file and clear the journal only after
    /// deletion succeeds, so every crash point remains idempotently resumable.
    fn finalize_profile_deletion(
        &self,
        _profile: ProfileId,
        _deadline: Instant,
    ) -> ProfileDeletionFinalizeOutcome {
        ProfileDeletionFinalizeOutcome::NotAuthorized
    }
}
