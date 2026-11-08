//! Installing from the Chrome Web Store or from a file: download or read,
//! verification, staging and the user's confirmation.

use std::path::{Path, PathBuf};
use std::time::Duration;

use zephium_app::Handle;
use zephium_core::ids::ExtensionInstallId;
use zephium_webext::manifest::Manifest;
use zephium_webext::{archive, crx, permissions, store, ExtensionId};

use super::{compat_revision, prepare_package, MAX_PACKAGE_BYTES};
use crate::webext::{Entry, Original, Pending, WebExtensionReview, WebExtensions};

/// A client that talks only to Google's update and download hosts, where
/// the Web Store serves packages from.
pub(super) fn store_client() -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        let google = attempt.url().host_str().is_some_and(|host| {
            ["google.com", "googleusercontent.com", "gvt1.com"]
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        });
        if attempt.previous().len() >= 5 || !google {
            attempt.stop()
        } else {
            attempt.follow()
        }
    });
    reqwest::Client::builder()
        .https_only(true)
        .redirect(policy)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|_| "Downloads are unavailable.".to_string())
}

pub(super) async fn download(id: &ExtensionId) -> Result<Vec<u8>, String> {
    let client = store_client()?;
    let url = store::download_url(id, store::CHROME_VERSION);
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "The Chrome Web Store could not be reached.")?;
    if !response.status().is_success() {
        return Err(format!(
            "The Chrome Web Store answered {}.",
            response.status()
        ));
    }
    super::response::read_body(
        response,
        MAX_PACKAGE_BYTES,
        "The extension is too large.",
        "The download was interrupted.",
    )
    .await
}

pub(super) fn icon_data_url(dir: &Path, manifest: &Manifest) -> Option<String> {
    use base64::Engine as _;
    let path = manifest
        .icons()
        .best(48)
        .map(str::to_owned)?;
    let bytes = std::fs::read(dir.join(&path))
        .ok()
        .filter(|bytes| bytes.len() < 512 * 1024)?;
    let mime = if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "image/png"
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub(in crate::webext) async fn prepare(
    shell: &Handle,
    extensions: &WebExtensions,
    listing_url: &str,
) -> Result<WebExtensionReview, String> {
    let id = store::listing_id(listing_url)
        .ok_or("Chrome Web Storeの拡張機能URLを入力してください。")?;
    let bytes = download(&id).await?;
    review(shell, extensions, Source::Store(id, bytes)).await
}

/// Reviews an extension file or unpacked folder.
pub(in crate::webext) async fn prepare_file(
    shell: &Handle,
    extensions: &WebExtensions,
    path: PathBuf,
) -> Result<WebExtensionReview, String> {
    review(shell, extensions, Source::File(path)).await
}

pub(in crate::webext) async fn choose_file(
    app: &tauri::AppHandle,
    shell: &Handle,
    extensions: &WebExtensions,
    folder: bool,
) -> Result<Option<WebExtensionReview>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().file();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        if folder {
            dialog.blocking_pick_folder()
        } else {
            dialog
                .add_filter("Chrome extension", &["crx", "zip"])
                .blocking_pick_file()
        }
    })
    .await
    .map_err(|_| "The file couldn't be chosen.")?;
    let Some(path) = picked.and_then(|path| path.into_path().ok()) else {
        return Ok(None);
    };
    prepare_file(shell, extensions, path).await.map(Some)
}

/// Reviews the store's newest version of an extension whose update waits
/// for approval because it asks for more access.
pub(in crate::webext) async fn review_update(
    shell: &Handle,
    extensions: &WebExtensions,
    id: &str,
) -> Result<WebExtensionReview, String> {
    let id = ExtensionId::parse(id).ok_or("That extension isn't installed.")?;
    let bytes = download(&id).await?;
    review(shell, extensions, Source::Store(id, bytes)).await
}

async fn review(
    shell: &Handle,
    extensions: &WebExtensions,
    source: Source,
) -> Result<WebExtensionReview, String> {
    let root = extensions.root.clone();
    let installed = extensions.registry()?.extensions;
    #[cfg(target_os = "windows")]
    let enabled = installed.iter().filter(|entry| entry.enabled).count();
    #[cfg(target_os = "windows")]
    let enabled_ids: Vec<_> = installed
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.id.clone())
        .collect();
    let staged = tauri::async_runtime::spawn_blocking(move || stage(&root, source, &installed))
        .await
        .map_err(|_| "Installation failed.")??;
    if !shell.callback_handle().is_accepting_commands() {
        discard(&staged.pending);
        return Err("The browser is unavailable.".into());
    }
    let review = staged.review.clone();
    #[cfg(target_os = "windows")]
    let review = {
        let mut review = review;
        if enabled >= zephium_core::extensions::MAX_EXTENSION_INSTALLS_PER_PROFILE
            && !enabled_ids.contains(&review.id)
        {
            review.warnings.push("Windows can run eight extensions at once across all profiles. This extension will wait until you disable or remove another running extension.".into());
        }
        review
    };
    let mut pending = extensions
        .state
        .lock()
        .map_err(|_| "Installation failed.")?;
    if let Some(previous) = pending.replace(staged.pending) {
        discard(&previous);
    }
    Ok(review)
}

/// Where a package comes from.
pub(super) enum Source {
    /// A download from the Chrome Web Store, which must be signed for `id`.
    Store(ExtensionId, Vec<u8>),
    /// A `.crx` or `.zip` file, or an unpacked folder, the user chose.
    File(PathBuf),
}

pub(super) struct Staged {
    pub(super) review: WebExtensionReview,
    pub(super) pending: Pending,
}

/// Verifies and unpacks a package into the staging area, prepares it for
/// WebKit and describes it for review; `installed` are the profile's
/// extensions, one of which it may update.
pub(super) fn stage(root: &Path, source: Source, installed: &[Entry]) -> Result<Staged, String> {
    let staging = root.join("staging");
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let review_id = ExtensionInstallId::generate().to_string();
    let dir = staging.join(&review_id);
    let from_file = matches!(source, Source::File(_));
    let result = (|| {
        let (id, original) = unpack(source, &dir)?;
        let existing = installed.iter().find(|entry| entry.id == id.as_str());
        // An unsigned package takes its identity from a manifest key anyone
        // can copy. It may update a file install, never a store extension,
        // whose access, data and native-app trust it would otherwise inherit.
        if matches!(original, Original::Zip(_) | Original::Folder(_))
            && existing.is_some_and(|entry| !entry.sideloaded)
        {
            return Err("An extension from the Chrome Web Store with this identity is already installed. Remove it first to install this file.".into());
        }
        let source = original_identity(&original)?;
        let manifest = Manifest::load(&dir)
            .map_err(|error| format!("The extension's manifest is invalid ({error})."))?;
        let warnings = permissions::warnings(&manifest);
        let icon = icon_data_url(&dir, &manifest);
        let mut hosts = manifest.host_permissions();
        for script in manifest.content_scripts() {
            hosts.extend(script.matches);
        }
        hosts.sort();
        hosts.dedup();
        let version = manifest
            .version()
            .ok_or("The extension has no valid version.")?
            .to_owned();
        let name = manifest.name().unwrap_or_else(|| id.to_string());
        let description = manifest.description().unwrap_or_default().to_owned();
        let access = existing
            .map(|entry| entry.access.clone())
            .unwrap_or_default();
        let report = prepare_package(&dir, &access)
            .map_err(|error| format!("The extension can't be prepared ({error})."))?;
        let reviewed_permissions = manifest.permissions();
        let publisher_verified = matches!(original, Original::Crx(_));
        let mut permissions = reviewed_permissions.clone();
        permissions.extend(report.added_permissions);
        let revision = compat_revision(&access);
        let prepared_digest =
            zephium_webext::package::tree_digest(&dir).map_err(|e| e.to_string())?;
        let entry = Entry {
            install: existing
                .map(|entry| entry.install.clone())
                .unwrap_or_else(|| ExtensionInstallId::generate().to_string()),
            id: id.to_string(),
            name: name.clone(),
            version: version.clone(),
            description: description.clone(),
            enabled: true,
            package: zephium_webext::package::publication_identity(
                &source,
                &revision,
                &prepared_digest,
            ),
            source,
            prepared_digest,
            compat: revision,
            started: String::new(),
            permissions,
            reviewed_permissions,
            publisher_verified,
            hosts,
            held_update: None,
            access,
            sideloaded: from_file,
        };
        Ok(Staged {
            review: WebExtensionReview {
                review_id: review_id.clone(),
                id: id.to_string(),
                name,
                version,
                description,
                warnings,
                icon,
                update: existing.is_some(),
                from_file,
            },
            pending: Pending {
                review_id,
                entry,
                staged: dir.clone(),
                original,
                previous: existing.map(|entry| (entry.install.clone(), entry.source.clone())),
            },
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(dir.with_extension("src"));
    }
    result
}

/// Unpacks `source` into `dir` and identifies it: store and `.crx` packages
/// by their signature, others by their manifest's `key`, or else by where
/// they came from, as Chrome identifies unpacked extensions.
fn unpack(source: Source, dir: &Path) -> Result<(ExtensionId, Original), String> {
    let limits = archive::Limits::default();
    let unsafe_package =
        |error: archive::ArchiveError| format!("The package can't be unpacked safely ({error}).");
    match source {
        Source::Store(id, bytes) => {
            let verified = crx::verify(&bytes, Some(&id))
                .map_err(|_| "The package isn't correctly signed by its publisher.")?;
            archive::extract(verified.zip, dir, &limits).map_err(unsafe_package)?;
            super::platform::record_signed_key(dir, verified.public_key)?;
            Ok((id, Original::Crx(bytes)))
        }
        Source::File(path) => {
            let path = path
                .canonicalize()
                .map_err(|_| "That file can't be read.")?;
            if path.is_dir() {
                archive::copy_dir(&path, dir, &limits).map_err(unsafe_package)?;
                // Freeze the reviewed source before compatibility preparation.
                // Confirmation must never reread a developer's changing folder.
                let snapshot = dir.with_extension("src");
                archive::copy_dir(dir, &snapshot, &limits).map_err(unsafe_package)?;
                return Ok((unsigned_id(dir, &path)?, Original::Folder(snapshot)));
            }
            let bytes = read_package(&path)?;
            if bytes.starts_with(b"Cr24") {
                let verified =
                    crx::verify(&bytes, None).map_err(|_| "The package isn't correctly signed.")?;
                let id = verified.id.clone();
                archive::extract(verified.zip, dir, &limits).map_err(unsafe_package)?;
                super::platform::record_signed_key(dir, verified.public_key)?;
                Ok((id, Original::Crx(bytes)))
            } else {
                archive::extract(&bytes, dir, &limits).map_err(unsafe_package)?;
                Ok((unsigned_id(dir, &path)?, Original::Zip(bytes)))
            }
        }
    }
}

fn read_package(path: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(path)
        .map_err(|_| "That file can't be read.")?
        .len();
    if size > MAX_PACKAGE_BYTES {
        return Err("The extension is too large.".into());
    }
    std::fs::read(path).map_err(|_| "That file can't be read.".into())
}

fn unsigned_id(dir: &Path, source: &Path) -> Result<ExtensionId, String> {
    use base64::Engine as _;
    let manifest = Manifest::load(dir)
        .map_err(|error| format!("The extension's manifest is invalid ({error})."))?;
    let key = manifest
        .raw()
        .get("key")
        .and_then(|key| key.as_str())
        .and_then(|key| {
            base64::engine::general_purpose::STANDARD
                .decode(key.trim())
                .ok()
        });
    Ok(match key {
        Some(key) => ExtensionId::from_public_key(&key),
        None if cfg!(target_os = "windows") => return Err("On Windows, use a signed CRX or an unpacked extension with a manifest key. A stable native identity is required.".into()),
        None => ExtensionId::from_source_path(&source.to_string_lossy()),
    })
}

pub(in crate::webext) fn confirm(
    shell: &Handle,
    extensions: &WebExtensions,
    mut pending: Pending,
    ticket: zephium_core::extensions::ExtensionPublicationTicket,
) -> Result<(), String> {
    let id = pending.entry.id.clone();
    let publication = super::PUBLICATION
        .lock()
        .map_err(|_| "Installation failed.")?;
    let mut registry = extensions.registry()?;
    if !review_is_current(&pending, &registry) {
        discard(&pending);
        return Err(
            "The extension changed since review. Review it again before installing.".into(),
        );
    }
    if let Some(current) = registry.extensions.iter().find(|entry| entry.id == id) {
        pending.entry.access = current.access.clone();
        pending.entry.enabled = current.enabled;
    }
    let packages = extensions.packages(&id);
    if let Err(error) = publish(&packages, &pending) {
        discard(&pending);
        return Err(error);
    }

    registry.extensions.retain(|entry| entry.id != id);
    registry.extensions.push(pending.entry);
    if pending.previous.is_some() {
        extensions.save_existing(&registry)?;
    } else {
        extensions.save_new(&registry, &ticket)?;
    }
    drop(publication);
    extensions.apply(shell, &mut registry);
    Ok(())
}

pub(in crate::webext) fn take_pending(
    extensions: &WebExtensions,
    id: &str,
    review_id: &str,
) -> Result<Pending, String> {
    let mut state = extensions
        .state
        .lock()
        .map_err(|_| "Installation failed.")?;
    if !state
        .as_ref()
        .is_some_and(|pending| pending.entry.id == id && pending.review_id == review_id)
    {
        return Err("Nothing is waiting to be installed.".to_owned());
    }
    state
        .take()
        .ok_or_else(|| "Nothing is waiting to be installed.".to_owned())
}

fn review_is_current(pending: &Pending, registry: &crate::webext::Registry) -> bool {
    pending.previous
        == registry
            .extensions
            .iter()
            .find(|entry| entry.id == pending.entry.id)
            .map(|entry| (entry.install.clone(), entry.source.clone()))
}

/// Stores what a package was installed from, so it can be rebuilt when the
/// compatibility layer changes.
pub(super) fn original_identity(original: &Original) -> Result<String, String> {
    use zephium_webext::package::{bytes_digest, tree_digest};
    Ok(match original {
        Original::Crx(bytes) => format!("crx-{}", bytes_digest(bytes)),
        Original::Zip(bytes) => format!("zip-{}", bytes_digest(bytes)),
        Original::Folder(path) => format!("src-{}", tree_digest(path).map_err(|e| e.to_string())?),
    })
}

pub(super) fn keep_original(
    packages: &Path,
    source: &str,
    original: &Original,
) -> Result<(), String> {
    if original_identity(original)? != source {
        return Err("The reviewed package source changed.".into());
    }
    match original {
        Original::Crx(bytes) | Original::Zip(bytes) => {
            let kind = if matches!(original, Original::Crx(_)) {
                "crx"
            } else {
                "zip"
            };
            let target = packages.join(format!("{source}.{kind}"));
            if target.exists() {
                let existing = read_package(&target)?;
                if existing == *bytes {
                    return Ok(());
                }
                return Err("A different package already occupies the source identity.".into());
            }
            let temporary = packages.join(format!(".{}.tmp", ExtensionInstallId::generate()));
            let result = (|| -> std::io::Result<()> {
                use std::io::Write as _;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                file.write_all(bytes)?;
                file.sync_all()?;
                // A hard link publishes a complete original without replacing
                // anything another profile may already reference.
                match std::fs::hard_link(&temporary, &target) {
                    Ok(()) => Ok(()),
                    Err(error)
                        if error.kind() == std::io::ErrorKind::AlreadyExists
                            && std::fs::read(&target)? == *bytes =>
                    {
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            })();
            let _ = std::fs::remove_file(temporary);
            result.map_err(|e| e.to_string())
        }
        Original::Folder(path) => {
            let target = packages.join(format!("{source}.src"));
            // Copy because a compatibility rebuild may share this original.
            let temporary = packages.join(format!(".{}.src", ExtensionInstallId::generate()));
            archive::copy_dir(path, &temporary, &archive::Limits::default())
                .map_err(|e| e.to_string())?;
            zephium_webext::package::publish_directory(&temporary, &target)
                .map_err(|e| e.to_string())
        }
    }
}

pub(in crate::webext) fn discard(pending: &Pending) {
    crate::webext::discard_pending(pending);
}

pub(super) fn publish(packages: &Path, pending: &Pending) -> Result<(), String> {
    std::fs::create_dir_all(packages).map_err(|e| e.to_string())?;
    keep_original(packages, &pending.entry.source, &pending.original)?;
    let target = packages.join(&pending.entry.package);
    if zephium_webext::package::tree_digest(&pending.staged).map_err(|e| e.to_string())?
        != pending.entry.prepared_digest
    {
        return Err("The prepared extension changed after review.".into());
    }
    zephium_webext::package::publish_directory(&pending.staged, &target)
        .map_err(|e| e.to_string())?;
    if let Original::Folder(path) = &pending.original {
        let _ = std::fs::remove_dir_all(path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use zephium_core::ids::ProfileId;

    #[test]
    fn management_and_review_use_the_declared_package_icon() {
        let root = tempfile::tempdir().unwrap();
        let png = base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+nmFQAAAAASUVORK5CYII=").unwrap();
        std::fs::write(root.path().join("package.png"), &png).unwrap();
        let manifest = serde_json::json!({"manifest_version":3,"name":"Package icon","version":"1.0","icons":{"48":"package.png"},"action":{"default_icon":"unused.png"}});
        std::fs::write(root.path().join("manifest.json"), manifest.to_string()).unwrap();
        assert!(icon_data_url(root.path(), &Manifest::load(root.path()).unwrap()).is_some());
    }

    #[test]
    fn confirmation_for_another_id_preserves_the_managed_pending_review() {
        let root = tempfile::tempdir().unwrap();
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(signed, None).unwrap();
        let staged = stage(
            root.path(),
            Source::Store(verified.id, signed.to_vec()),
            &[],
        )
        .unwrap();
        let id = staged.pending.entry.id.clone();
        let review_id = staged.review.review_id.clone();
        let path = staged.pending.staged.clone();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(Some(staged.pending)),
        };
        assert!(take_pending(&extensions, "another-extension", &review_id).is_err());
        let pending = take_pending(&extensions, &id, &review_id)
            .expect("the same owner must retain its reviewed package");
        assert_eq!(pending.staged, path);
        assert!(path.is_dir());
        discard(&pending);
    }

    #[test]
    fn a_stale_review_cannot_confirm_a_new_review_of_the_same_extension_in_another_profile() {
        let root = tempfile::tempdir().unwrap();
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let stage_review = |profile| {
            let verified = crx::verify(signed, None).unwrap();
            stage(
                root.path(),
                Source::Store(verified.id, signed.to_vec()),
                &[],
            )
            .unwrap()
        };
        let first = stage_review(ProfileId::from(1));
        let second = stage_review(ProfileId::from(2));
        assert_eq!(first.review.id, second.review.id);
        assert_ne!(first.review.review_id, second.review.review_id);
        let id = second.review.id.clone();
        let review_id = second.review.review_id.clone();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(Some(second.pending)),
        };
        assert!(
            take_pending(&extensions, &id, &first.review.review_id).is_err(),
            "extension identity alone is not approval of a particular package/profile"
        );
        assert!(crate::webext::cancel_pending(&extensions, &first.review.review_id).is_none());
        let pending = take_pending(&extensions, &id, &review_id).unwrap();
        assert_eq!(pending.review_id, review_id);
        discard(&pending);
        discard(&first.pending);
    }

    #[test]
    fn cancellation_consumes_only_the_matching_pending_review() {
        let root = tempfile::tempdir().unwrap();
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(signed, None).unwrap();
        let staged = stage(
            root.path(),
            Source::Store(verified.id, signed.to_vec()),
            &[],
        )
        .unwrap();
        let review_id = staged.review.review_id.clone();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(Some(staged.pending)),
        };
        assert!(crate::webext::cancel_pending(&extensions, "stale-review").is_none());
        let pending = crate::webext::cancel_pending(&extensions, &review_id).unwrap();
        assert_eq!(pending.review_id, review_id);
        assert!(crate::webext::cancel_pending(&extensions, &review_id).is_none());
        discard(&pending);
    }

    #[test]
    fn copied_identity_in_another_profile_never_replaces_published_code() {
        let root = tempfile::tempdir().unwrap();
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(signed, None).unwrap();
        let key = base64::engine::general_purpose::STANDARD.encode(verified.public_key);
        let folder = |name: &str, code: &str| {
            let path = root.path().join(name);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(
                path.join("manifest.json"),
                serde_json::json!({
                    "manifest_version": 3, "name": "Fixture", "version": "1.2.3",
                    "key": key
                })
                .to_string(),
            )
            .unwrap();
            std::fs::write(path.join("worker.js"), code).unwrap();
            path
        };
        let profile_a = ProfileId::generate();
        let profile_b = ProfileId::generate();
        let first = stage(
            root.path(),
            Source::Store(verified.id, signed.to_vec()),
            &[],
        )
        .unwrap();
        let trusted = first.pending.entry.clone();
        assert!(!trusted.sideloaded);
        assert!(trusted.publisher_verified);
        assert_eq!(
            trusted.permissions.contains(&"nativeMessaging".to_owned()),
            cfg!(target_os = "macos"),
            "only macOS adds native transport to this storage-only fixture"
        );
        assert!(
            !zephium_webext::permissions::external_native_messaging_allowed(
                &trusted.reviewed_permissions,
                trusted.publisher_verified
            )
        );
        let packages = root.path().join("packages").join(&trusted.id);
        publish(&packages, &first.pending).unwrap();
        let second_source = folder("b", "replacement code");
        assert!(stage(
            root.path(),
            Source::File(second_source.clone()),
            std::slice::from_ref(&trusted)
        )
        .is_err());
        let second = stage(root.path(), Source::File(second_source), &[]).unwrap();
        publish(&packages, &second.pending).unwrap();
        assert_eq!(
            std::fs::read_to_string(packages.join(&trusted.package).join("worker.js")).unwrap(),
            "verified code"
        );
        assert!(!trusted.sideloaded);
        assert!(trusted.publisher_verified);
        assert_eq!(trusted.reviewed_permissions, ["storage"]);
        assert!(trusted.source.starts_with("crx-"));
        assert_ne!(trusted.package, second.pending.entry.package);
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(None),
        };
        let next_load = extensions.load(
            &trusted,
            ExtensionInstallId::parse(&trusted.install).unwrap(),
            true,
        );
        assert_eq!(next_load.root, packages.join(&trusted.package));
        assert_eq!(next_load.permissions, trusted.permissions);
        assert!(!next_load.external_native_messaging);
        // A legacy Store record must never fall back to a same-version
        // unsigned original if its verified CRX is unavailable.
        let mut legacy = trusted.clone();
        legacy.source.clear();
        legacy.prepared_digest.clear();
        archive::copy_dir(
            &packages.join(format!("{}.src", second.pending.entry.source)),
            &packages.join(format!("{}.src", legacy.version)),
            &archive::Limits::default(),
        )
        .unwrap();
        assert!(extensions.rebuild(&mut legacy, &trusted.compat).is_err());
    }
    #[test]
    fn legacy_store_rebuild_recovers_only_verified_granted_permissions() {
        for (name, expected) in [("webext-native-app", true), ("webext-native", false)] {
            let root = tempfile::tempdir().unwrap();
            let extensions = WebExtensions {
                root: root.path().to_path_buf(),
                state: Default::default(),
            };
            let bytes: &[u8] = if name == "webext-native-app" {
                include_bytes!("../../../tests/fixtures/webext-native-app.crx")
            } else {
                include_bytes!("../../../tests/fixtures/webext-native.crx")
            };
            let verified = crx::verify(bytes, None).unwrap();
            let profile = ProfileId::from(1);
            let staged =
                stage(root.path(), Source::Store(verified.id, bytes.to_vec()), &[]).unwrap();
            let mut entry = staged.pending.entry.clone();
            let packages = extensions.packages(&entry.id);
            publish(&packages, &staged.pending).unwrap();
            entry.reviewed_permissions.clear();
            entry.publisher_verified = false;
            entry.source.clear();
            entry.prepared_digest.clear();
            entry.permissions = vec![
                "storage".into(),
                "nativeMessaging".into(),
                "activeTab".into(),
            ];
            std::fs::write(packages.join(format!("{}.crx", entry.version)), bytes).unwrap();
            let revision = entry.compat.clone();
            extensions.rebuild(&mut entry, &revision).unwrap();
            assert!(entry.publisher_verified);
            assert_eq!(
                entry
                    .reviewed_permissions
                    .contains(&"nativeMessaging".into()),
                expected
            );
            assert!(!entry.reviewed_permissions.contains(&"activeTab".into()));
            assert_eq!(
                extensions
                    .load(
                        &entry,
                        ExtensionInstallId::parse(&entry.install).unwrap(),
                        true
                    )
                    .external_native_messaging,
                expected
            );
            entry
                .permissions
                .retain(|permission| permission != "nativeMessaging");
            entry.reviewed_permissions.clear();
            entry.publisher_verified = false;
            extensions.rebuild(&mut entry, &revision).unwrap();
            assert!(!entry
                .reviewed_permissions
                .contains(&"nativeMessaging".into()));
        }
    }

    #[test]
    fn registry_save_failure_keeps_ready_extensions_loading() {
        let root = tempfile::tempdir().unwrap();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: Default::default(),
        };
        let profile = ProfileId::from(1);
        let mut entries = Vec::new();
        for bytes in [
            include_bytes!("../../../tests/fixtures/webext-native.crx").as_slice(),
            include_bytes!("../../../tests/fixtures/webext-native-app.crx").as_slice(),
        ] {
            let verified = crx::verify(bytes, None).unwrap();
            let staged =
                stage(root.path(), Source::Store(verified.id, bytes.to_vec()), &[]).unwrap();
            publish(
                &extensions.packages(&staged.pending.entry.id),
                &staged.pending,
            )
            .unwrap();
            entries.push(staged.pending.entry);
        }
        entries[0].started = entries[0].compat.clone();
        entries[1].compat.clear();
        let mut registry = crate::webext::Registry {
            extensions: entries,
        };
        extensions.save(&registry).unwrap();
        std::fs::create_dir(extensions.registry_path().with_extension("json.tmp")).unwrap();
        let mut loads = Vec::new();
        assert!(extensions.apply_with(
            &mut registry,
            || true,
            |command| {
                let zephium_app::Command::SetWebExtensions { extensions, .. } = command else {
                    panic!("wrong command")
                };
                loads = extensions;
                true
            }
        ));
        assert_eq!(
            loads.len(),
            2,
            "both unchanged and successfully rebuilt packages remain usable"
        );
        assert!(!loads[0].start_background);
        assert!(loads[1].start_background);
    }

    fn installed_fixture() -> (tempfile::TempDir, WebExtensions, ProfileId, Entry) {
        let root = tempfile::tempdir().unwrap();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: Default::default(),
        };
        let profile = ProfileId::generate();
        let bytes = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(bytes, None).unwrap();
        let staged = stage(root.path(), Source::Store(verified.id, bytes.to_vec()), &[]).unwrap();
        publish(
            &extensions.packages(&staged.pending.entry.id),
            &staged.pending,
        )
        .unwrap();
        std::fs::remove_dir_all(extensions.root.join("staging")).unwrap();
        let mut entry = staged.pending.entry;
        entry.compat.clear();
        extensions
            .save(&crate::webext::Registry {
                extensions: vec![entry.clone()],
            })
            .unwrap();
        (root, extensions, profile, entry)
    }

    #[test]
    fn rebuild_failure_reaches_the_profile_install_status_without_a_native_load() {
        let (_root, extensions, profile, entry) = installed_fixture();
        std::fs::remove_file(
            extensions
                .packages(&entry.id)
                .join(format!("{}.crx", entry.source)),
        )
        .unwrap();
        let mut registry = crate::webext::Registry::default();
        assert!(extensions.apply_with(
            &mut registry,
            || true,
            |command| {
                let zephium_app::Command::SetWebExtensions {
                    extensions: loads,
                    failures,
                    ..
                } = command
                else {
                    panic!("wrong command")
                };
                assert!(loads.is_empty());
                assert_eq!(failures.len(), 1);
                assert_eq!(failures[0].0.to_string(), entry.install);
                assert!(failures[0].1.contains("could not be prepared"));
                true
            }
        ));
    }

    #[test]
    fn retry_does_not_prepare_a_replaced_or_disabled_installation() {
        let (_root, extensions, profile, entry) = installed_fixture();
        let original = std::fs::read(extensions.registry_path()).unwrap();
        let mut registry = crate::webext::Registry::default();
        assert!(extensions
            .prepare_catalog(&mut registry, Some(ExtensionInstallId::generate()))
            .is_none());
        assert_eq!(std::fs::read(extensions.registry_path()).unwrap(), original);
        assert!(registry.extensions[0].compat.is_empty());
        registry.extensions[0].enabled = false;
        extensions.save(&registry).unwrap();
        assert!(extensions
            .prepare_catalog(&mut registry, ExtensionInstallId::parse(&entry.install))
            .is_none());
        assert!(registry.extensions[0].compat.is_empty());
    }

    #[test]
    fn hardening_ordinary_mutations_cannot_recreate_a_deleted_registry() {
        let (_root, extensions, profile, _entry) = installed_fixture();
        let mut registry = extensions.registry().unwrap();
        std::fs::remove_file(extensions.registry_path()).unwrap();
        registry.extensions[0].enabled = false;
        assert!(extensions.save_existing(&registry).is_err());
        assert!(!extensions.registry_path().exists());
        assert!(!extensions
            .registry_path()
            .with_extension("json.tmp")
            .exists());
    }

    #[test]
    fn deleting_an_account_does_not_revoke_the_shared_catalog() {
        let (_root, extensions, profile, _entry) = installed_fixture();
        let registry = extensions.registry().unwrap();
        let ticket = crate::webext::imp::publication_ticket(&extensions).unwrap();
        {
            let root = std::fs::canonicalize(&extensions.root).unwrap();
            let mut io = zephium_core::extensions::REGISTRY_IO.lock().unwrap();
            io.cancel(&root, profile);
        }
        assert!(extensions.save_new(&registry, &ticket).is_ok());
        assert!(extensions.registry_path().is_file());
    }

    #[test]
    fn completed_rebuild_cannot_restore_a_deleted_or_changed_registry() {
        for change in ["delete", "disable", "access", "uninstall", "shutdown"] {
            let (_root, extensions, profile, entry) = installed_fixture();
            let mut registry = crate::webext::Registry::default();
            let prepared = extensions.prepare_catalog(&mut registry, None).unwrap();
            let mut current = extensions.registry().unwrap();
            match change {
                "delete" => {
                    let _io = zephium_core::extensions::REGISTRY_IO.lock().unwrap();
                    std::fs::remove_file(extensions.registry_path()).unwrap();
                }
                "disable" => {
                    current.extensions[0].enabled = false;
                    extensions.save(&current).unwrap();
                }
                "access" => {
                    current.extensions[0].access = crate::webext::Access::Click;
                    extensions.save(&current).unwrap();
                }
                "uninstall" => {
                    current.extensions.clear();
                    extensions.save(&current).unwrap();
                }
                _ => {}
            }
            let baseline = std::fs::read(extensions.registry_path()).ok();
            assert!(!extensions.commit_prepared(
                &registry,
                prepared,
                || change != "shutdown",
                |_| panic!("stale rebuild reached native admission")
            ));
            assert_eq!(std::fs::read(extensions.registry_path()).ok(), baseline);
            assert!(!extensions
                .root
                .join("staging")
                .join(profile.to_string())
                .exists());
            assert_eq!(entry.install, registry.extensions[0].install);
        }
    }

    #[tokio::test]
    async fn cancelling_a_folder_review_erases_both_prepared_and_frozen_trees() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("developer");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(
            source.join("manifest.json"),
            r#"{"manifest_version":3,"name":"Fixture","version":"1.0","key":"Zml4dHVyZQ=="}"#,
        )
        .unwrap();
        std::fs::write(source.join("worker.js"), "original source").unwrap();
        let staged = stage(root.path(), Source::File(source.clone()), &[]).unwrap();
        let prepared = staged.pending.staged.clone();
        let frozen = match &staged.pending.original {
            Original::Folder(path) => path.clone(),
            _ => panic!("folder source required"),
        };
        let review_id = staged.review.review_id;
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(Some(staged.pending)),
        };
        let pending = crate::webext::cancel_pending(&extensions, &review_id).unwrap();
        crate::webext::discard_cancelled(pending).await;
        assert!(!prepared.exists());
        assert!(
            !frozen.exists(),
            "cancellation retained the frozen developer source"
        );
        assert_eq!(
            std::fs::read_to_string(source.join("worker.js")).unwrap(),
            "original source"
        );
    }

    #[test]
    fn saturated_confirmation_retains_the_managed_folder_review() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("developer");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(
            source.join("manifest.json"),
            r#"{"manifest_version":3,"name":"Fixture","version":"1.0","key":"Zml4dHVyZQ=="}"#,
        )
        .unwrap();
        let staged = stage(root.path(), Source::File(source), &[]).unwrap();
        let id = staged.pending.entry.id.clone();
        let review = staged.pending.review_id.clone();
        let prepared = staged.pending.staged.clone();
        let frozen = match &staged.pending.original {
            Original::Folder(path) => path.clone(),
            _ => panic!("folder source required"),
        };
        let extensions = WebExtensions {
            root: root.path().to_owned(),
            state: std::sync::Mutex::new(Some(staged.pending)),
        };
        let jobs = std::sync::atomic::AtomicUsize::new(0);
        let mut slots = (0..8)
            .map(|_| crate::webext::PublicationSlot::acquire(&jobs).unwrap())
            .collect::<Vec<_>>();
        assert!(
            crate::webext::take_pending_for_publication(&extensions, &id, &review, &jobs).is_err()
        );
        assert!(
            extensions
                .state
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|pending| pending.review_id == review),
            "saturated confirmation consumed the only managed review"
        );
        assert!(prepared.exists() && frozen.exists());
        drop(slots.pop());
        let (_slot, pending) =
            crate::webext::take_pending_for_publication(&extensions, &id, &review, &jobs).unwrap();
        assert_eq!(pending.review_id, review);
        discard(&pending);
        assert!(!prepared.exists() && !frozen.exists());
    }

    #[test]
    fn folder_confirmation_uses_the_frozen_reviewed_original() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("developer");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(
            source.join("manifest.json"),
            serde_json::json!({
                "manifest_version":3, "name":"Fixture", "version":"1.2.3",
                "key":base64::engine::general_purpose::STANDARD.encode(b"fixture key")
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(source.join("worker.js"), "reviewed").unwrap();
        let staged = stage(root.path(), Source::File(source.clone()), &[]).unwrap();
        std::fs::write(source.join("worker.js"), "changed after review").unwrap();
        let packages = root.path().join("packages").join(&staged.pending.entry.id);
        publish(&packages, &staged.pending).unwrap();
        let original = packages.join(format!("{}.src", staged.pending.entry.source));
        assert_eq!(
            std::fs::read_to_string(original.join("worker.js")).unwrap(),
            "reviewed"
        );
        assert_eq!(
            std::fs::read_to_string(
                packages
                    .join(&staged.pending.entry.package)
                    .join("worker.js")
            )
            .unwrap(),
            "reviewed"
        );
    }
    #[test]
    fn apply_rejoins_current_registry_after_an_intervening_mutation() {
        let root = tempfile::tempdir().unwrap();
        let extensions = WebExtensions {
            root: root.path().to_path_buf(),
            state: std::sync::Mutex::new(None),
        };
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(signed, None).unwrap();
        let profile = ProfileId::generate();
        let staged = stage(
            root.path(),
            Source::Store(verified.id, signed.to_vec()),
            &[],
        )
        .unwrap();
        let captured = crate::webext::Registry {
            extensions: vec![staged.pending.entry.clone()],
        };
        let mut current = captured.clone();
        current.extensions[0].enabled = false;
        current.extensions[0].permissions.clear();
        extensions.save(&current).unwrap();
        let mut applied = captured.clone();
        let prepared = extensions.prepare_catalog(&mut applied, None).unwrap();
        assert!(!applied.extensions[0].enabled);
        assert!(applied.extensions[0].permissions.is_empty());
        assert!(prepared.loads.is_empty());
        drop(prepared);
        extensions
            .save(&crate::webext::Registry::default())
            .unwrap();
        let prepared = extensions.prepare_catalog(&mut applied, None).unwrap();
        assert!(applied.extensions.is_empty());
        assert!(prepared.loads.is_empty());
    }

    #[test]
    fn pending_review_cannot_resurrect_a_removed_or_replaced_install() {
        let root = tempfile::tempdir().unwrap();
        let signed = include_bytes!("../../../tests/fixtures/webext-native.crx");
        let verified = crx::verify(signed, None).unwrap();
        let profile = ProfileId::generate();
        let fresh = stage(
            root.path(),
            Source::Store(verified.id.clone(), signed.to_vec()),
            &[],
        )
        .unwrap();
        let installed = fresh.pending.entry;
        let update = stage(
            root.path(),
            Source::Store(verified.id, signed.to_vec()),
            std::slice::from_ref(&installed),
        )
        .unwrap();
        let mut current = crate::webext::Registry {
            extensions: vec![installed],
        };
        assert!(review_is_current(&update.pending, &current));
        current.extensions[0].source.push_str("changed");
        assert!(!review_is_current(&update.pending, &current));
        current.extensions[0].source = update.pending.previous.as_ref().unwrap().1.clone();
        current.extensions[0].install = ExtensionInstallId::generate().to_string();
        assert!(!review_is_current(&update.pending, &current));
        current.extensions.clear();
        assert!(!review_is_current(&update.pending, &current));
    }
}
