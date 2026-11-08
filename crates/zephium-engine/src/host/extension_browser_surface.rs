//! Host-side ownership of Shell-projected extension window/tab routing.

#[cfg(target_os = "macos")]
use zephium_core::extensions::{ExtensionBrowserRequestId, ExtensionBrowserRequestSettlement};
use zephium_core::extensions::{ExtensionBrowserSurface, MAX_EXTENSION_BROWSER_WINDOWS};
use zephium_core::ids::ProfileId;

use super::EngineHost;

impl EngineHost {
    #[cfg(target_os = "macos")]
    pub(crate) fn settle_extension_browser_request(
        &mut self,
        profile: ProfileId,
        request: ExtensionBrowserRequestId,
        settlement: ExtensionBrowserRequestSettlement,
        page_permit: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
        first_url_after_reply: Option<(
            std::sync::Arc<str>,
            zephium_core::ports::engine::NavigationRequestId,
        )>,
    ) -> bool {
        match self
            .webext
            .settle_browser_request(profile, request, settlement)
        {
            super::webext::BrowserRequestOutcome::NotOurs => false,
            super::webext::BrowserRequestOutcome::Settled => {
                if let (
                    ExtensionBrowserRequestSettlement::Applied(
                        zephium_core::extensions::ExtensionBrowserRequestResult::CreatedTab(tab),
                    ),
                    Some((url, intent)),
                ) = (settlement, first_url_after_reply)
                {
                    self.sink.emit(
                        zephium_core::ports::engine::EngineEvent::ExtensionCreatedTabReplied {
                            profile,
                            request,
                            tab,
                            url,
                            intent,
                        },
                    );
                }
                true
            }
            super::webext::BrowserRequestOutcome::Page {
                extension_id,
                url,
                done,
            } => {
                let ExtensionBrowserRequestSettlement::Applied(
                    zephium_core::extensions::ExtensionBrowserRequestResult::ExtensionPageAuthorized {
                        tab,
                        window,
                    },
                ) = settlement
                else {
                    return true;
                };
                let sink = self.sink.clone();
                let presented = match (self.ensure_stage(window), page_permit) {
                    (Some(stage), Some(permit)) => self.webext.present_page(
                        profile,
                        tab,
                        stage,
                        permit,
                        &extension_id,
                        &url,
                        &sink,
                    ),
                    _ => Err("the page presentation is no longer active".to_owned()),
                };
                match presented {
                    Ok(()) => {
                        if let Some(done) = done {
                            done(Ok(Some(self.webext.tab_number(profile, tab))));
                        }
                    }
                    Err(error) => {
                        eprintln!("extensions: could not show {extension_id} page: {error}");
                        if let Some(done) = done {
                            done(Err(error));
                        }
                        self.sink.emit(
                            zephium_core::ports::engine::EngineEvent::ExtensionPageClosed {
                                profile,
                                id: tab,
                            },
                        );
                    }
                }
                true
            }
        }
    }

    pub(crate) fn set_extension_browser_surface(
        &mut self,
        surface: ExtensionBrowserSurface,
    ) -> bool {
        let profile = surface.profile();
        if self.erasure_tombstones.contains(&profile)
            || (!self.extension_browser_surfaces.contains_key(&profile)
                && self.extension_browser_surfaces.len() >= MAX_EXTENSION_BROWSER_WINDOWS)
        {
            return false;
        }
        if let Some(current) = self.extension_browser_surfaces.get(&profile) {
            if surface.generation() < current.generation() {
                // A coalesced or delayed replaceable fact cannot roll the
                // native graph backward.
                return true;
            }
            if surface.generation() == current.generation() {
                return &surface == current;
            }
        }
        if surface.tabs().any(|tab| {
            self.partitions
                .get(&tab.id())
                .is_some_and(|partition| partition.profile() != profile)
        }) {
            return false;
        }

        if surface.tabs().next().is_none() {
            self.retire_extension_browser_surface(profile);
            return true;
        }

        #[cfg(target_os = "macos")]
        {
            let views = &self.views;
            let partitions = &self.partitions;
            self.webext.publish(&surface, |id| {
                partitions
                    .get(&id)
                    .filter(|partition| partition.profile() == profile)
                    .and_then(|_| views.get(&id))
                    .map(|view| crate::platform::imp::native_webview(&view.view))
            });
        }
        #[cfg(target_os = "windows")]
        self.reconcile_windows_extension_popup(&surface);
        self.extension_browser_surfaces.insert(profile, surface);
        true
    }

    pub(super) fn retire_extension_browser_surface(&mut self, profile: ProfileId) {
        #[cfg(target_os = "macos")]
        self.webext.cancel_profile_auth_flows(profile);
        self.extension_browser_surfaces.remove(&profile);
    }
}
