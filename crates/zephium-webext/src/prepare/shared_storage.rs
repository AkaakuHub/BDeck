use super::{Background as ManifestBackground, Manifest, PrepareError, COMPAT_DIR};
use serde_json::{json, Value};

pub(super) struct Background {
    pub manifest: String,
    pub script: bool,
    pub page: Option<String>,
}

impl Background {
    pub fn plain(manifest: String) -> Self {
        Self {
            manifest,
            script: false,
            page: None,
        }
    }
}

pub(super) fn background(
    manifest: &Manifest,
    rewritten: String,
    resource: &str,
) -> Result<Background, PrepareError> {
    let requested = manifest
        .permissions()
        .iter()
        .chain(manifest.optional_permissions().iter())
        .any(|permission| permission == "storage");
    if !requested {
        return Ok(Background::plain(rewritten));
    }
    let bridge = format!("{COMPAT_DIR}/storage-background.js");
    let mut raw: Value =
        serde_json::from_str(&rewritten).map_err(|_| PrepareError::ManifestLayout)?;
    match manifest.background() {
        Some(ManifestBackground::ServiceWorker { .. }) => Ok(Background::plain(rewritten)),
        Some(ManifestBackground::Page(page)) => Ok(Background {
            manifest: rewritten,
            script: true,
            page: Some(page),
        }),
        Some(ManifestBackground::Scripts(_)) => {
            let first = raw
                .get_mut("background")
                .and_then(|background| background.get_mut("scripts"))
                .and_then(Value::as_array_mut)
                .and_then(|scripts| scripts.first_mut())
                .ok_or(PrepareError::ManifestLayout)?;
            if first.as_str() != Some(resource) {
                return Err(PrepareError::ManifestLayout);
            }
            *first = Value::String(bridge);
            Ok(Background {
                manifest: serde_json::to_string_pretty(&raw)
                    .map_err(|_| PrepareError::ManifestLayout)?,
                script: true,
                page: None,
            })
        }
        None => {
            raw["background"] = if manifest.manifest_version() == 3 {
                json!({"service_worker":bridge})
            } else {
                json!({"scripts":[bridge],"persistent":false})
            };
            Ok(Background {
                manifest: serde_json::to_string_pretty(&raw)
                    .map_err(|_| PrepareError::ManifestLayout)?,
                script: true,
                page: None,
            })
        }
    }
}
