use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use zephium_core::ids::{ExtensionInstallId, ProfileId};
use zephium_core::ports::engine::WebExtensionLoad;
use zephium_webext::storage::broker::{Broker, Changed, Client};
use zephium_webext::storage::repository::Installation;
use zephium_webext::ExtensionId;

#[cfg(test)]
mod tests;

struct Installed {
    installation: Installation,
    root: PathBuf,
    profiles: HashSet<ProfileId>,
}

pub(super) struct Storage {
    root: PathBuf,
    changed: Arc<dyn Fn(Changed) + Send + Sync>,
    broker: Option<Broker>,
    installed: HashMap<ExtensionId, Installed>,
}

impl Storage {
    pub(super) fn new(root: PathBuf, changed: Arc<dyn Fn(Changed) + Send + Sync>) -> Self {
        Self {
            root,
            changed,
            broker: None,
            installed: HashMap::new(),
        }
    }

    fn broker(&mut self) -> Result<Broker, String> {
        if let Some(broker) = &self.broker {
            return Ok(broker.clone());
        }
        let changed = self.changed.clone();
        let broker = Broker::start(self.root.clone(), move |event| changed(event))?;
        self.broker = Some(broker.clone());
        Ok(broker)
    }

    pub(super) fn load(
        &mut self,
        profile: ProfileId,
        load: &WebExtensionLoad,
    ) -> Result<Client, String> {
        let extension =
            ExtensionId::parse(&load.extension_id).ok_or("Invalid extension identity.")?;
        let installation = Installation {
            extension: extension.clone(),
            key: load.install.bytes(),
            unlimited: load
                .permissions
                .iter()
                .any(|permission| permission == "unlimitedStorage"),
        };
        let broker = self.broker()?;
        let current = self.installed.get(&extension);
        if current.is_some_and(|current| {
            current.installation.key == installation.key && current.root != load.root
        }) {
            broker.replace(installation.clone())?;
        } else if current.is_none_or(|current| current.installation != installation) {
            broker.admit(installation.clone())?;
        }
        let current = self
            .installed
            .entry(extension)
            .or_insert_with(|| Installed {
                installation: installation.clone(),
                root: load.root.clone(),
                profiles: HashSet::new(),
            });
        if current.installation.key != installation.key {
            current.profiles.clear();
        }
        current.installation = installation.clone();
        current.root = load.root.clone();
        current.profiles.insert(profile);
        Ok(Client {
            broker,
            installation,
        })
    }

    pub(super) fn unload(&mut self, profile: ProfileId, install: ExtensionInstallId) {
        let extension = self.installed.iter_mut().find_map(|(id, entry)| {
            if entry.installation.key != install.bytes() {
                return None;
            }
            entry.profiles.remove(&profile);
            entry.profiles.is_empty().then(|| id.clone())
        });
        if let Some(extension) = extension {
            if let Some(entry) = self.installed.remove(&extension) {
                if let Some(broker) = &self.broker {
                    if let Err(error) = broker.revoke(entry.installation) {
                        eprintln!("extensions: storage retirement failed: {error}");
                    }
                }
            }
        }
    }

    pub(super) fn retire_profile(&mut self, profile: ProfileId) {
        let installs: Vec<_> = self
            .installed
            .values()
            .filter(|entry| entry.profiles.contains(&profile))
            .map(|entry| ExtensionInstallId::from(u128::from_be_bytes(entry.installation.key)))
            .collect();
        for install in installs {
            self.unload(profile, install);
        }
    }

    pub(super) fn erase(&mut self, load: &WebExtensionLoad) -> Result<(), String> {
        let extension =
            ExtensionId::parse(&load.extension_id).ok_or("Invalid extension identity.")?;
        if self
            .installed
            .get(&extension)
            .is_some_and(|entry| entry.installation.key == load.install.bytes())
        {
            self.installed.remove(&extension);
        }
        let installation = Installation {
            extension,
            key: load.install.bytes(),
            unlimited: false,
        };
        self.broker()?.erase(
            installation,
            Box::new(|result| {
                if let Err(error) = result {
                    eprintln!("extensions: storage erasure failed: {error}");
                }
            }),
        )
    }
}
