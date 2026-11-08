//! Keeps each profile's running extensions equal to what the browser has
//! installed and enabled.

use std::collections::HashMap;

use zephium_core::ids::ExtensionInstallId;
use zephium_core::ports::engine::{WebExtensionLoad, WebExtensionLoaded};

use super::*;

#[derive(Default)]
pub(super) struct WebExtensionState {
    // Restored at launch before the session exists; applied once it does.
    shared: Option<SharedWebExtensions>,
    pub(super) pages: HashMap<ItemId, ExtensionPage>,
    loaded: HashMap<ProfileId, HashMap<ExtensionInstallId, WebExtensionLoad>>,
    status: HashMap<(ProfileId, ExtensionInstallId), WebExtensionStatus>,
    preparation_failures: HashMap<ProfileId, Vec<(ExtensionInstallId, String)>>,
}

#[derive(Clone, Copy)]
pub(super) struct ExtensionPage {
    pub profile: ProfileId,
    pub return_to: Option<ItemId>,
}

#[derive(Clone)]
struct SharedWebExtensions {
    extensions: Vec<WebExtensionLoad>,
    failures: Vec<(ExtensionInstallId, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebExtensionStatus {
    Loading,
    Running(WebExtensionLoaded),
    Failed(String),
}

/// Where an installation from the current tab would go.
#[derive(Clone, Debug)]
pub struct WebExtensionTarget {
    pub profile: ProfileId,
    /// The active tab's address when it is a Chrome Web Store listing.
    pub listing_url: Option<String>,
}

impl Shell {
    pub(super) fn web_extension_profile_admitted(&self, profile: ProfileId) -> bool {
        !self.profile_deletion.states.contains_key(&profile)
            && self
                .profiles
                .get(profile)
                .is_some_and(|profile| profile.kind != ProfileKind::Incognito)
    }

    pub(super) fn retire_web_extensions(&mut self, profile: ProfileId) {
        self.web_extensions.preparation_failures.remove(&profile);
        if let Some(installs) = self.web_extensions.loaded.remove(&profile) {
            for install in installs.keys() {
                self.web_extensions.status.remove(&(profile, *install));
            }
        }
    }

    pub(super) fn set_shared_web_extensions(
        &mut self,
        extensions: Vec<WebExtensionLoad>,
        failures: Vec<(ExtensionInstallId, String)>,
        retry: Option<ExtensionInstallId>,
    ) {
        self.web_extensions.shared = Some(SharedWebExtensions {
            extensions,
            failures,
        });
        if !self.bootstrapped {
            return;
        }
        let profiles: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| self.web_extension_profile_admitted(profile.id))
            .map(|profile| profile.id)
            .collect();
        for profile in profiles {
            self.apply_shared_web_extensions_for(profile, retry);
        }
    }

    pub(super) fn apply_shared_web_extensions_for(
        &mut self,
        profile: ProfileId,
        retry: Option<ExtensionInstallId>,
    ) {
        if let Some(shared) = self.web_extensions.shared.clone() {
            self.set_prepared_web_extensions(profile, shared.extensions, shared.failures, retry);
        }
    }

    pub(super) fn remove_shared_web_extension(&mut self, extension: WebExtensionLoad) {
        if let Some(shared) = self.web_extensions.shared.as_mut() {
            shared
                .extensions
                .retain(|load| load.install != extension.install);
            shared
                .failures
                .retain(|(install, _)| *install != extension.install);
        }
        let profiles: Vec<_> = self.web_extensions.loaded.keys().copied().collect();
        for profile in profiles {
            self.remove_web_extension(profile, extension.clone());
        }
    }

    pub(super) fn set_prepared_web_extensions(
        &mut self,
        profile: ProfileId,
        wanted: Vec<WebExtensionLoad>,
        failures: Vec<(ExtensionInstallId, String)>,
        retry: Option<ExtensionInstallId>,
    ) {
        self.set_web_extensions_for(profile, wanted, retry);
        if !self.bootstrapped || self.web_extension_profile_admitted(profile) {
            self.web_extensions
                .preparation_failures
                .insert(profile, failures);
        }
    }

    fn set_web_extensions_for(
        &mut self,
        profile: ProfileId,
        wanted: Vec<WebExtensionLoad>,
        retry: Option<ExtensionInstallId>,
    ) {
        if !self.bootstrapped {
            return;
        }
        if !self.web_extension_profile_admitted(profile) {
            return;
        }
        self.web_extensions.preparation_failures.remove(&profile);
        let current = self.web_extensions.loaded.entry(profile).or_default();
        let wanted: std::collections::BTreeMap<_, _> = wanted
            .into_iter()
            .map(|load| (load.install, load))
            .collect();
        let removed: Vec<_> = current
            .keys()
            .filter(|install| !wanted.contains_key(install))
            .copied()
            .collect();
        for install in removed {
            current.remove(&install);
            self.web_extensions.status.remove(&(profile, install));
            let _ = self.engine.unload_web_extension(profile, install);
        }
        for (install, load) in wanted {
            if current
                .get(&install)
                .is_some_and(|loaded| unchanged(loaded, &load))
                && retry != Some(install)
            {
                continue;
            }
            #[cfg(target_os = "windows")]
            let replacing = current.contains_key(&install)
                && matches!(
                    self.web_extensions.status.get(&(profile, install)),
                    Some(WebExtensionStatus::Loading | WebExtensionStatus::Running(_))
                );
            current.insert(install, load.clone());
            #[cfg(target_os = "windows")]
            {
                if replacing {
                    self.web_extensions
                        .status
                        .insert((profile, install), WebExtensionStatus::Loading);
                    if self.engine.load_web_extension(profile, load) != NativeDispatch::Scheduled {
                        self.web_extensions.status.insert(
                            (profile, install),
                            WebExtensionStatus::Failed("The browser could not start it.".into()),
                        );
                    }
                } else {
                    self.web_extensions.status.insert(
                        (profile, install),
                        WebExtensionStatus::Failed(
                            zephium_core::extensions::WINDOWS_EXTENSION_CAPACITY_MESSAGE.into(),
                        ),
                    );
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                self.web_extensions
                    .status
                    .insert((profile, install), WebExtensionStatus::Loading);
                if self.engine.load_web_extension(profile, load) != NativeDispatch::Scheduled {
                    self.web_extensions.status.insert(
                        (profile, install),
                        WebExtensionStatus::Failed("The browser could not start it.".into()),
                    );
                }
            }
        }

        #[cfg(target_os = "windows")]
        self.pump_waiting_windows_extensions();

        let mut active = zephium_core::extensions::ExtensionActiveProfiles::EMPTY;
        for (profile, installs) in &self.web_extensions.loaded {
            if !installs.is_empty() && !active.try_insert(*profile) {
                crate::diagnostic!("extensions: too many profiles run extensions");
            }
        }
        if self
            .extension_browser_surfaces
            .replace_active_profiles(active)
        {
            let sync = self.sync_extension_browser_surfaces();
            if sync.native.rejected {
                crate::diagnostic!("extensions: browser-surface synchronization awaits retry");
            }
        }
        if self.extension_actions.clear_projection(profile) {
            self.project_extension_actions(profile);
        }
        let _ = self.refresh_extension_actions(profile);
    }

    #[cfg(target_os = "windows")]
    fn pump_waiting_windows_extensions(&mut self) {
        // Admission is global on Windows. Queue in stable profile/install order;
        // only a capacity failure is retried automatically, never a bad package.
        let mut waiting: Vec<_> = self
            .web_extensions
            .loaded
            .iter()
            .flat_map(|(profile, installs)| {
                installs.iter().filter_map(|(id, load)| {
                matches!(self.web_extensions.status.get(&(*profile, *id)), Some(WebExtensionStatus::Failed(error))
                    if error == zephium_core::extensions::WINDOWS_EXTENSION_CAPACITY_MESSAGE)
                    .then_some((*profile, *id, load.clone()))
            })
            })
            .collect();
        waiting.sort_by_key(|(profile, id, _)| (*profile, *id));
        for (profile, id, load) in waiting {
            let active = self
                .web_extensions
                .status
                .values()
                .filter(|status| {
                    matches!(
                        status,
                        WebExtensionStatus::Loading | WebExtensionStatus::Running(_)
                    )
                })
                .count();
            if active >= zephium_core::extensions::MAX_EXTENSION_INSTALLS_PER_PROFILE {
                break;
            }
            self.web_extensions
                .status
                .insert((profile, id), WebExtensionStatus::Loading);
            if self.engine.load_web_extension(profile, load) != NativeDispatch::Scheduled {
                self.web_extensions.status.insert(
                    (profile, id),
                    WebExtensionStatus::Failed("The browser could not start it.".into()),
                );
            }
        }
    }

    pub(super) fn remove_web_extension(&mut self, profile: ProfileId, extension: WebExtensionLoad) {
        if let Some(current) = self.web_extensions.loaded.get_mut(&profile) {
            current.remove(&extension.install);
        }
        self.web_extensions
            .status
            .remove(&(profile, extension.install));
        if self.engine.remove_web_extension(profile, extension) != NativeDispatch::Scheduled {
            crate::diagnostic!("extensions: the browser could not erase a removed extension");
        }
        #[cfg(target_os = "windows")]
        self.pump_waiting_windows_extensions();
    }

    pub(super) fn apply_deferred_web_extensions(&mut self) {
        if let Some(shared) = self.web_extensions.shared.clone() {
            self.set_shared_web_extensions(shared.extensions, shared.failures, None);
        }
    }

    pub(super) fn on_web_extension_settled(
        &mut self,
        profile: ProfileId,
        install: ExtensionInstallId,
        result: Result<WebExtensionLoaded, String>,
    ) {
        if !self.web_extension_profile_admitted(profile) {
            return;
        }
        let still_wanted = self
            .web_extensions
            .loaded
            .get(&profile)
            .is_some_and(|installs| installs.contains_key(&install));
        if !still_wanted {
            return;
        }
        #[cfg(target_os = "windows")]
        let capacity_failure = result.as_ref().is_err_and(|error| {
            error == zephium_core::extensions::WINDOWS_EXTENSION_CAPACITY_MESSAGE
        });
        let status = match result {
            Ok(loaded) => WebExtensionStatus::Running(loaded),
            Err(error) => {
                crate::diagnostic!("extensions: an extension failed to start: {error}");
                WebExtensionStatus::Failed(error)
            }
        };
        self.web_extensions
            .status
            .insert((profile, install), status);
        #[cfg(target_os = "windows")]
        if !capacity_failure {
            self.pump_waiting_windows_extensions();
        }
        let _ = self.refresh_extension_actions(profile);
    }

    #[cfg(target_os = "windows")]
    pub(super) fn extension_document_may_close(&self, id: ItemId) -> bool {
        let Some(profile) = self.profile_of_item(id) else {
            return false;
        };
        let Some(extension) = self
            .items
            .tab(id)
            .and_then(|tab| tab.url.as_ref())
            .and_then(zephium_core::navigation::extension_document_id)
        else {
            return false;
        };
        self.web_extensions
            .loaded
            .get(&profile)
            .is_some_and(|installs| {
                installs.iter().any(|(id, load)| {
                    load.extension_id == extension
                        && matches!(
                            self.web_extensions.status.get(&(profile, *id)),
                            Some(WebExtensionStatus::Running(_))
                        )
                })
            })
    }

    pub(super) fn shared_web_extension_status(
        &self,
    ) -> Vec<(ExtensionInstallId, WebExtensionStatus)> {
        let mut statuses = HashMap::new();
        for profile in self
            .profiles
            .iter()
            .filter(|profile| self.web_extension_profile_admitted(profile.id))
        {
            for (install, status) in self.web_extension_status(profile.id) {
                let priority = |status: &WebExtensionStatus| match status {
                    WebExtensionStatus::Failed(_) => 2,
                    WebExtensionStatus::Loading => 1,
                    WebExtensionStatus::Running(_) => 0,
                };
                let current = statuses.entry(install).or_insert_with(|| status.clone());
                if priority(&status) > priority(current) {
                    *current = status;
                }
            }
        }
        statuses.into_iter().collect()
    }

    pub(super) fn web_extension_status(
        &self,
        profile: ProfileId,
    ) -> Vec<(ExtensionInstallId, WebExtensionStatus)> {
        if !self.web_extension_profile_admitted(profile) {
            return Vec::new();
        }
        let mut result: Vec<_> = self
            .web_extensions
            .loaded
            .get(&profile)
            .map(|installs| {
                installs
                    .keys()
                    .filter_map(|install| {
                        let status = self.web_extensions.status.get(&(profile, *install))?;
                        Some((*install, status.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if let Some(failures) = self.web_extensions.preparation_failures.get(&profile) {
            result.extend(
                failures
                    .iter()
                    .map(|(install, error)| (*install, WebExtensionStatus::Failed(error.clone()))),
            );
        }
        result
    }

    pub(super) fn web_extension_target(&self, tab: Option<ItemId>) -> Option<WebExtensionTarget> {
        if !self.bootstrapped {
            return None;
        }
        let window = self.windows.focused()?;
        let profile = window
            .active
            .and_then(|id| self.profile_of_item(id))
            .unwrap_or(window.profile);
        if !self.web_extension_profile_admitted(profile) {
            return None;
        }
        let listing_url = tab
            .filter(|tab| window.active == Some(*tab) && self.extension_tab_in_scope(*tab, profile))
            .and_then(|tab| self.items.tab(tab))
            .and_then(|tab| tab.url.as_ref())
            .filter(|url| super::extension_store::store_listing_url(url))
            .map(|url| url.as_str().to_owned());
        Some(WebExtensionTarget {
            profile,
            listing_url,
        })
    }
}

/// `start_background` asks for one background run after a package changed; a
/// later apply that clears it changes nothing that is loaded.
fn unchanged(loaded: &WebExtensionLoad, wanted: &WebExtensionLoad) -> bool {
    loaded.install == wanted.install
        && loaded.extension_id == wanted.extension_id
        && loaded.root == wanted.root
        && loaded.permissions == wanted.permissions
        && loaded.external_native_messaging == wanted.external_native_messaging
        && loaded.match_patterns == wanted.match_patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_the_background_run_does_not_reload_an_extension() {
        let loaded = WebExtensionLoad {
            install: ExtensionInstallId::from(7),
            extension_id: "abcdefghijklmnopabcdefghijklmnop".into(),
            root: "/packages/a/1.0-rev".into(),
            permissions: vec!["storage".into()],
            external_native_messaging: false,
            match_patterns: vec!["<all_urls>".into()],
            start_background: true,
        };
        let settled = WebExtensionLoad {
            start_background: false,
            ..loaded.clone()
        };
        assert!(unchanged(&loaded, &settled));
        let narrowed = WebExtensionLoad {
            match_patterns: vec!["*://github.com/*".into()],
            ..settled.clone()
        };
        assert!(!unchanged(&loaded, &narrowed));
        let authority_changed = WebExtensionLoad {
            external_native_messaging: true,
            ..settled
        };
        assert!(!unchanged(&loaded, &authority_changed));
    }
}
