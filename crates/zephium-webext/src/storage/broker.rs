use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};

use serde_json::{Map, Value};

use super::repository::{Installation, Repository};
use super::{Area, Context, Operation};

#[cfg(test)]
mod tests;

const MAX_PENDING: usize = 32;
const MAX_PENDING_BYTES: usize = 64 * 1024 * 1024;
pub type Completion<T> = Box<dyn FnOnce(Result<T, String>) + Send>;

#[derive(Debug)]
pub struct Changed {
    pub installation: Installation,
    pub area: Area,
    pub changes: Map<String, Value>,
    pub content_scripts: bool,
}

#[derive(Clone)]
pub struct Broker {
    sender: mpsc::Sender<Job>,
    pending_bytes: Arc<AtomicUsize>,
    pending_requests: Arc<AtomicUsize>,
}

#[derive(Clone)]
pub struct Client {
    pub broker: Broker,
    pub installation: Installation,
}

enum Job {
    Admit(Installation),
    Replace(Installation),
    Revoke(Installation),
    Erase(Installation, Completion<()>),
    Execute {
        installation: Installation,
        area: Area,
        context: Context,
        operation: Operation,
        done: Completion<Value>,
        _reservation: Reservation,
    },
}

struct Reservation {
    pending: Arc<AtomicUsize>,
    bytes: usize,
    requests: Arc<AtomicUsize>,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.pending.fetch_sub(self.bytes, Ordering::AcqRel);
        self.requests.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Broker {
    pub fn start(
        root: PathBuf,
        changed: impl Fn(Changed) + Send + 'static,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("extension-storage".into())
            .spawn(move || {
                let mut repository = Repository::new(root);
                while let Ok(job) = receiver.recv() {
                    match job {
                        Job::Admit(installation) => repository.admit(installation),
                        Job::Replace(installation) => {
                            repository.revoke(&installation);
                            repository.admit(installation);
                        }
                        Job::Revoke(installation) => repository.revoke(&installation),
                        Job::Erase(installation, done) => done(repository.erase(&installation)),
                        Job::Execute {
                            installation,
                            area,
                            context,
                            operation,
                            done,
                            _reservation,
                        } => match repository.execute(&installation, area, context, operation) {
                            Ok(reply) => {
                                if !reply.changes.is_empty() {
                                    changed(Changed {
                                        content_scripts: repository
                                            .allows_content_scripts(&installation, area),
                                        installation,
                                        area,
                                        changes: reply.changes,
                                    });
                                }
                                done(Ok(reply.value));
                            }
                            Err(error) => done(Err(error)),
                        },
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            sender,
            pending_bytes: Arc::new(AtomicUsize::new(0)),
            pending_requests: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn admit(&self, installation: Installation) -> Result<(), String> {
        self.send(Job::Admit(installation))
    }
    pub fn replace(&self, installation: Installation) -> Result<(), String> {
        self.send(Job::Replace(installation))
    }
    pub fn revoke(&self, installation: Installation) -> Result<(), String> {
        self.send(Job::Revoke(installation))
    }
    pub fn erase(&self, installation: Installation, done: Completion<()>) -> Result<(), String> {
        self.send(Job::Erase(installation, done))
    }

    pub fn execute(
        &self,
        installation: Installation,
        area: Area,
        context: Context,
        operation: Operation,
        done: Completion<Value>,
    ) -> Result<(), String> {
        let bytes = serde_json::to_vec(&operation)
            .map_err(|error| error.to_string())?
            .len();
        self.pending_requests
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1).filter(|count| *count <= MAX_PENDING)
            })
            .map_err(|_| "Extension storage is busy. Try again shortly.".to_owned())?;
        let mut reservation = Reservation {
            pending: self.pending_bytes.clone(),
            bytes: 0,
            requests: self.pending_requests.clone(),
        };
        self.pending_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current
                    .checked_add(bytes)
                    .filter(|total| *total <= MAX_PENDING_BYTES)
            })
            .map_err(|_| "Extension storage is busy. Try again shortly.".to_owned())?;
        reservation.bytes = bytes;
        self.send(Job::Execute {
            installation,
            area,
            context,
            operation,
            done,
            _reservation: reservation,
        })
    }

    fn send(&self, job: Job) -> Result<(), String> {
        // Browser lifecycle messages must still revoke access when data requests fill their allowance.
        self.sender
            .send(job)
            .map_err(|_| "Extension storage is unavailable.".into())
    }
}
