//! Bounded asynchronous reads for favicon data.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use zephium_core::icon;
use zephium_core::ids::{ItemId, ProfileId, SpaceId};

use crate::{CallbackHandle, Command, SharedStore};

const MAX_PENDING_FAVICON_READS: usize = 64;
const MAX_PENDING_FAVICON_PROBE_READS: usize = 8;
pub(crate) const FAVICON_CACHE_MAX_AGE_SECONDS: i64 = 7 * 24 * 3600;

pub(crate) fn read_favicon(
    store: &dyn zephium_core::ports::store::Store,
    generation: u64,
    id: ItemId,
    profile: ProfileId,
    origin: String,
    page: Option<String>,
) -> StoreReadResult {
    let read = |key: &str| {
        let stored = store
            .favicon_raster_with_age(profile, key)
            .filter(|(bytes, _)| icon::validated_rgba32(bytes).is_some());
        StoredIcon {
            stale: stored
                .as_ref()
                .is_none_or(|(_, age)| *age > FAVICON_CACHE_MAX_AGE_SECONDS),
            rgba: stored.map(|(bytes, _)| bytes),
        }
    };
    let icon = read(&origin);
    let page = page.map(|key| {
        let icon = read(&key);
        Box::new(PageFavicon { key, icon })
    });
    StoreReadResult::Favicon {
        generation,
        id,
        profile,
        origin,
        icon,
        page,
    }
}

#[derive(Clone, Debug)]
pub struct StoredIcon {
    pub rgba: Option<Vec<u8>>,
    pub stale: bool,
}

#[derive(Clone, Debug)]
pub struct PageFavicon {
    pub key: String,
    pub icon: StoredIcon,
}

#[derive(Clone, Debug)]
pub enum StoreReadResult {
    Favicon {
        generation: u64,
        id: ItemId,
        profile: ProfileId,
        origin: String,
        icon: StoredIcon,
        page: Option<Box<PageFavicon>>,
    },
    FaviconBatch {
        generation: u64,
        profile: ProfileId,
        space: SpaceId,
        origins: Vec<String>,
        rasters: Vec<(String, Vec<u8>)>,
    },
    FaviconProbe {
        generation: u64,
        profile: ProfileId,
        origins: Vec<String>,
        rasters: Vec<(String, Vec<u8>, i64)>,
    },
}

enum Request {
    Favicon {
        generation: u64,
        id: ItemId,
        profile: ProfileId,
        origin: String,
        page: Option<String>,
    },
    FaviconBatch {
        generation: u64,
        profile: ProfileId,
        space: SpaceId,
        origins: Vec<String>,
    },
    FaviconProbe {
        generation: u64,
        profile: ProfileId,
        origins: Vec<String>,
    },
}

struct State {
    accepting: bool,
    stopped: bool,
    in_flight: bool,
    favicon_batch: Option<Request>,
    favicons: HashMap<ItemId, Request>,
    favicon_order: VecDeque<ItemId>,
    favicon_probes: VecDeque<Request>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            accepting: true,
            stopped: false,
            in_flight: false,
            favicon_batch: None,
            favicons: HashMap::new(),
            favicon_order: VecDeque::new(),
            favicon_probes: VecDeque::new(),
        }
    }
}

struct Inner {
    state: Mutex<State>,
    ready: Condvar,
}

#[derive(Clone)]
pub(crate) struct StoreReadQueue {
    inner: Arc<Inner>,
}

impl StoreReadQueue {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                ready: Condvar::new(),
            }),
        }
    }

    pub(crate) fn request_favicon(
        &self,
        generation: u64,
        id: ItemId,
        profile: ProfileId,
        origin: String,
        page: Option<String>,
    ) -> bool {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.stopped || !state.accepting {
            return false;
        }
        let request = Request::Favicon {
            generation,
            id,
            profile,
            origin,
            page,
        };
        if state.favicons.contains_key(&id) {
            state.favicons.insert(id, request);
        } else {
            if state.favicons.len() >= MAX_PENDING_FAVICON_READS {
                return false;
            }
            state.favicons.insert(id, request);
            state.favicon_order.push_back(id);
        }
        self.inner.ready.notify_one();
        true
    }

    pub(crate) fn cancel_favicon(&self, id: ItemId) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.favicons.remove(&id);
        state.favicon_order.retain(|candidate| *candidate != id);
    }

    pub(crate) fn request_favicon_batch(
        &self,
        generation: u64,
        profile: ProfileId,
        space: SpaceId,
        origins: Vec<String>,
    ) -> bool {
        if origins.len() > zephium_core::ports::store::MAX_FAVICON_BATCH_ORIGINS {
            return false;
        }
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.stopped || !state.accepting {
            return false;
        }
        state.favicon_batch = Some(Request::FaviconBatch {
            generation,
            profile,
            space,
            origins,
        });
        self.inner.ready.notify_one();
        true
    }

    pub(crate) fn request_favicon_probe(
        &self,
        generation: u64,
        profile: ProfileId,
        origins: Vec<String>,
    ) -> bool {
        if origins.len() > zephium_core::ports::store::MAX_FAVICON_BATCH_ORIGINS {
            return false;
        }
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.stopped
            || !state.accepting
            || state.favicon_probes.len() >= MAX_PENDING_FAVICON_PROBE_READS
        {
            return false;
        }
        state.favicon_probes.push_back(Request::FaviconProbe {
            generation,
            profile,
            origins,
        });
        self.inner.ready.notify_one();
        true
    }

    fn recv(&self) -> Option<Lease> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.stopped {
                return None;
            }
            if state.accepting && !state.in_flight {
                if let Some(request) = pop_browser_read(&mut state) {
                    state.in_flight = true;
                    return Some(Lease {
                        queue: self.clone(),
                        request: Some(request),
                    });
                }
            }
            state = self
                .inner
                .ready
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn finish_request(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.in_flight = false;
        self.inner.ready.notify_all();
    }

    pub(crate) fn quiesce_until(&self, deadline: Instant) -> bool {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.accepting = false;
        state.favicon_batch = None;
        state.favicons.clear();
        state.favicon_order.clear();
        state.favicon_probes.clear();
        while state.in_flight {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, timeout) = self
                .inner
                .ready
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next;
            if timeout.timed_out() && state.in_flight {
                return false;
            }
        }
        true
    }

    pub(crate) fn resume(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.stopped {
            state.accepting = true;
            self.inner.ready.notify_one();
        }
    }

    pub(crate) fn stop(&self) {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.stopped = true;
        state.accepting = false;
        state.favicon_batch = None;
        state.favicons.clear();
        state.favicon_order.clear();
        state.favicon_probes.clear();
        self.inner.ready.notify_all();
    }
}

fn pop_browser_read(state: &mut State) -> Option<Request> {
    state
        .favicon_batch
        .take()
        .or_else(|| {
            while let Some(id) = state.favicon_order.pop_front() {
                if let Some(request) = state.favicons.remove(&id) {
                    return Some(request);
                }
            }
            None
        })
        .or_else(|| state.favicon_probes.pop_front())
}

struct Lease {
    queue: StoreReadQueue,
    request: Option<Request>,
}

impl Lease {
    fn take(&mut self) -> Option<Request> {
        self.request.take()
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.queue.finish_request();
    }
}

pub(crate) struct StoreReaderStopGuard(StoreReadQueue);

impl StoreReaderStopGuard {
    pub(crate) fn new(queue: StoreReadQueue) -> Self {
        Self(queue)
    }
}

impl Drop for StoreReaderStopGuard {
    fn drop(&mut self) {
        self.0.stop();
    }
}

pub(crate) fn run(store: SharedStore, queue: StoreReadQueue, callback: CallbackHandle) {
    run_with(store, queue, move |result| {
        callback.dispatch(Command::StoreRead(result))
    });
}

fn run_with(
    store: SharedStore,
    queue: StoreReadQueue,
    mut deliver: impl FnMut(StoreReadResult) -> bool,
) {
    while let Some(mut lease) = queue.recv() {
        let Some(request) = lease.take() else {
            continue;
        };
        let result = match request {
            Request::Favicon {
                generation,
                id,
                profile,
                origin,
                page,
            } => read_favicon(&*store, generation, id, profile, origin, page),
            Request::FaviconBatch {
                generation,
                profile,
                space,
                origins,
            } => {
                let requested: std::collections::HashSet<&str> =
                    origins.iter().map(String::as_str).collect();
                let rasters = store
                    .favicon_rasters(profile, &origins)
                    .into_iter()
                    .take(zephium_core::ports::store::MAX_FAVICON_BATCH_ORIGINS)
                    .filter(|(origin, bytes)| {
                        requested.contains(origin.as_str())
                            && icon::validated_rgba32(bytes).is_some()
                    })
                    .collect();
                StoreReadResult::FaviconBatch {
                    generation,
                    profile,
                    space,
                    origins,
                    rasters,
                }
            }
            Request::FaviconProbe {
                generation,
                profile,
                origins,
            } => {
                let rasters = origins
                    .iter()
                    .filter_map(|origin| {
                        let (bytes, age) = store.favicon_raster_with_age(profile, origin)?;
                        icon::validated_rgba32(&bytes)?;
                        Some((origin.clone(), bytes, age))
                    })
                    .collect();
                StoreReadResult::FaviconProbe {
                    generation,
                    profile,
                    origins,
                    rasters,
                }
            }
        };
        if !deliver(result) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favicon_admission_is_bounded_and_replacement_is_per_item() {
        let queue = StoreReadQueue::new();
        let profile = ProfileId::from(1);
        for raw in 1..=MAX_PENDING_FAVICON_READS as u128 {
            assert!(queue.request_favicon(
                raw as u64,
                ItemId::from(raw),
                profile,
                format!("https://{raw}.example"),
                None
            ));
        }
        assert!(queue.request_favicon(
            999,
            ItemId::from(1),
            profile,
            "https://replacement.example".into(),
            Some("https://replacement.example/page".into())
        ));
        assert!(!queue.request_favicon(
            1000,
            ItemId::from(999),
            profile,
            "https://overflow.example".into(),
            None
        ));
    }

    #[test]
    fn repeated_favicon_cancel_and_replace_keeps_order_metadata_bounded() {
        let queue = StoreReadQueue::new();
        let profile = ProfileId::from(1);
        let id = ItemId::from(1);
        for generation in 1..=10_000 {
            assert!(queue.request_favicon(
                generation,
                id,
                profile,
                format!("https://{generation}.example"),
                None
            ));
            queue.cancel_favicon(id);
        }
        let state = queue.inner.state.lock().unwrap();
        assert!(state.favicons.is_empty());
        assert!(state.favicon_order.is_empty());
    }
}
