use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{apply, AccessLevel, Area, Context, Limits, Operation, Reply};
use crate::ExtensionId;

#[cfg(test)]
mod tests;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Installation {
    pub extension: ExtensionId,
    pub key: [u8; 16],
    pub unlimited: bool,
}

impl Installation {
    fn directory(&self, root: &Path) -> PathBuf {
        let key: String = self.key.iter().map(|byte| format!("{byte:02x}")).collect();
        root.join(self.extension.as_str()).join(key)
    }
}

pub struct Repository {
    root: PathBuf,
    installed: HashMap<ExtensionId, Installation>,
    sessions: HashMap<ExtensionId, Map<String, Value>>,
    access: HashMap<(ExtensionId, Area), AccessLevel>,
}

impl Repository {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            installed: HashMap::new(),
            sessions: HashMap::new(),
            access: HashMap::new(),
        }
    }

    pub fn admit(&mut self, installation: Installation) {
        if self
            .installed
            .get(&installation.extension)
            .is_some_and(|current| current.key != installation.key)
        {
            self.sessions.remove(&installation.extension);
            self.access
                .retain(|(id, _), _| id != &installation.extension);
        }
        self.installed
            .insert(installation.extension.clone(), installation);
    }

    pub fn revoke(&mut self, installation: &Installation) {
        if self
            .installed
            .get(&installation.extension)
            .is_some_and(|current| current.key == installation.key)
        {
            self.installed.remove(&installation.extension);
            self.sessions.remove(&installation.extension);
            self.access
                .retain(|(id, _), _| id != &installation.extension);
        }
    }

    pub fn erase(&mut self, installation: &Installation) -> Result<(), String> {
        self.revoke(installation);
        let directory = installation.directory(&self.root);
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => {
                if let Some(parent) = directory.parent() {
                    let _ = std::fs::remove_dir(parent);
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn allows_content_scripts(&self, installation: &Installation, area: Area) -> bool {
        self.installed
            .get(&installation.extension)
            .is_some_and(|current| current.key == installation.key)
            && self
                .access
                .get(&(installation.extension.clone(), area))
                .map_or(area != Area::Session, |level| {
                    *level == AccessLevel::TrustedAndUntrustedContexts
                })
    }

    pub fn execute(
        &mut self,
        installation: &Installation,
        area: Area,
        context: Context,
        operation: Operation,
    ) -> Result<Reply, String> {
        let admitted = self
            .installed
            .get(&installation.extension)
            .filter(|current| current.key == installation.key)
            .ok_or("The extension installation is no longer active.")?;
        let limits = Limits::for_area(area, admitted.unlimited);
        if context == Context::ContentScript && !self.allows_content_scripts(installation, area) {
            return Err(
                "Access to this storage area is restricted to trusted extension contexts.".into(),
            );
        }
        if let Operation::SetAccessLevel { access_level } = operation {
            if context != Context::Trusted {
                return Err("Only trusted extension contexts may change storage access.".into());
            }
            self.access
                .insert((installation.extension.clone(), area), access_level);
            return Ok(Reply {
                value: Value::Null,
                changes: Map::new(),
            });
        }
        if area == Area::Session {
            return apply(
                self.sessions
                    .entry(installation.extension.clone())
                    .or_default(),
                operation,
                limits,
            );
        }
        let path = installation.directory(&self.root).join(match area {
            Area::Local => "local.json",
            Area::Sync => "sync.json",
            Area::Session => unreachable!(),
        });
        let mut items = read(&path)?;
        let reply = apply(&mut items, operation, limits)?;
        if !reply.changes.is_empty() {
            write(&path, &items)?;
        }
        Ok(reply)
    }
}

fn read(path: &Path) -> Result<Map<String, Value>, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Extension storage exceeds the supported file size.".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("Invalid extension storage: {error}"))
}

fn write(path: &Path, items: &Map<String, Value>) -> Result<(), String> {
    let bytes = serde_json::to_vec(items).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("Extension storage exceeds the supported file size.".into());
    }
    let parent = path.parent().ok_or("Invalid extension storage path.")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.tmp");
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
