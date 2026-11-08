//! Short coordination between extension publication and profile deletion.

use crate::ids::ProfileId;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};

const MAX_PUBLICATION_TICKETS: usize = 64;

struct TicketState {
    root: PathBuf,
    profile: Option<ProfileId>,
    live: AtomicBool,
}

/// Revocable authority for one pending install, never a profile lifetime lease.
pub struct ExtensionPublicationTicket(Arc<TicketState>);

impl ExtensionPublicationTicket {
    pub fn allows_browser(&self, root: &Path) -> bool {
        self.0.root == root && self.0.profile.is_none() && self.0.live.load(Ordering::Acquire)
    }

    /// Call only while holding `REGISTRY_IO` through the registry commit.
    pub fn allows(&self, root: &Path, profile: ProfileId) -> bool {
        self.0.root == root
            && self.0.profile == Some(profile)
            && self.0.live.load(Ordering::Acquire)
    }
}

pub struct ExtensionRegistryIo {
    tickets: Vec<Weak<TicketState>>,
}

impl ExtensionRegistryIo {
    /// `root` is the canonical webext directory. Register before final shell
    /// profile validation, so deletion on either side of validation wins.
    pub fn register(
        &mut self,
        root: PathBuf,
        profile: ProfileId,
    ) -> Option<ExtensionPublicationTicket> {
        self.tickets.retain(|ticket| ticket.strong_count() != 0);
        if self.tickets.len() >= MAX_PUBLICATION_TICKETS {
            return None;
        }
        let ticket = Arc::new(TicketState {
            root,
            profile: Some(profile),
            live: AtomicBool::new(true),
        });
        self.tickets.push(Arc::downgrade(&ticket));
        Some(ExtensionPublicationTicket(ticket))
    }

    pub fn register_browser(&mut self, root: PathBuf) -> Option<ExtensionPublicationTicket> {
        self.tickets.retain(|ticket| ticket.strong_count() != 0);
        if self.tickets.len() >= MAX_PUBLICATION_TICKETS {
            return None;
        }
        let ticket = Arc::new(TicketState {
            root,
            profile: None,
            live: AtomicBool::new(true),
        });
        self.tickets.push(Arc::downgrade(&ticket));
        Some(ExtensionPublicationTicket(ticket))
    }

    pub fn cancel(&mut self, root: &Path, profile: ProfileId) {
        self.tickets.retain(|ticket| {
            let Some(ticket) = ticket.upgrade() else {
                return false;
            };
            if ticket.root == root && ticket.profile == Some(profile) {
                ticket.live.store(false, Ordering::Release);
            }
            true
        });
    }
}

/// Hold only for registry commit/unlink, never preparation, network or native work.
pub static REGISTRY_IO: Mutex<ExtensionRegistryIo> = Mutex::new(ExtensionRegistryIo {
    tickets: Vec::new(),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_catalog_ticket_survives_account_deletion_without_authorizing_a_profile_write() {
        let mut state = ExtensionRegistryIo { tickets: vec![] };
        let ticket = state.register_browser("/one".into()).unwrap();
        state.cancel(Path::new("/one"), ProfileId::from(1));
        assert!(ticket.allows_browser(Path::new("/one")));
        assert!(!ticket.allows_browser(Path::new("/two")));
        assert!(!ticket.allows(Path::new("/one"), ProfileId::from(1)));
    }

    #[test]
    fn live_ticket_capacity_is_bounded_and_released_by_the_pending_owner() {
        let mut state = ExtensionRegistryIo { tickets: vec![] };
        let tickets: Vec<_> = (0..MAX_PUBLICATION_TICKETS)
            .map(|_| state.register("/one".into(), ProfileId::from(1)).unwrap())
            .collect();
        assert!(state.register("/one".into(), ProfileId::from(1)).is_none());
        drop(tickets);
        assert!(state.register("/one".into(), ProfileId::from(1)).is_some());
        assert_eq!(state.tickets.len(), 1);
    }

    #[test]
    fn deletion_cancels_only_live_tickets_for_its_exact_root_and_profile() {
        let mut state = ExtensionRegistryIo { tickets: vec![] };
        let first = state.register("/one".into(), ProfileId::from(1)).unwrap();
        let other_profile = state.register("/one".into(), ProfileId::from(2)).unwrap();
        let other_root = state.register("/two".into(), ProfileId::from(1)).unwrap();
        state.cancel(Path::new("/one"), ProfileId::from(1));
        assert!(!first.allows(Path::new("/one"), ProfileId::from(1)));
        assert!(other_profile.allows(Path::new("/one"), ProfileId::from(2)));
        assert!(other_root.allows(Path::new("/two"), ProfileId::from(1)));
        drop((first, other_profile, other_root));
        for _ in 0..MAX_PUBLICATION_TICKETS * 2 {
            drop(state.register("/one".into(), ProfileId::from(1)).unwrap());
        }
        assert_eq!(
            state.tickets.len(),
            1,
            "dead installs leave no historical tombstones"
        );
    }
}
