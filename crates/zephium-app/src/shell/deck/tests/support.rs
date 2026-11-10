use crate::api::{
    ChromePresentation, ChromePresentationCallback, ChromePresentationDispatch, PresentationChrome,
};
use crate::shell::*;
use std::sync::Mutex;
use zephium_core::extensions::ExtensionBrowserRequestId;
use zephium_core::ids::*;
use zephium_core::ports::chrome::{Chrome, ChromeFrame};
use zephium_core::ports::engine::*;
use zephium_core::windows::WindowKind;

type EraseDone = Box<dyn FnOnce(ProfileDataErasureOutcome) + Send>;

#[derive(Default)]
pub(super) struct TestEngine {
    pub views: Mutex<Vec<(ItemId, ProfileId)>>,
    pub navigations: Mutex<Vec<(ItemId, String)>>,
    pub closed: Mutex<Vec<ItemId>>,
    pub composer_focus: Mutex<Vec<(ItemId, ComposerFocus)>>,
    pub extension_loads: Mutex<Vec<(ProfileId, ExtensionInstallId)>>,
    pub extension_unloads: Mutex<Vec<(ProfileId, ExtensionInstallId)>>,
    pub extension_removals: Mutex<Vec<(ProfileId, ExtensionInstallId)>>,
    pub erasures: Mutex<Vec<(ProfileId, EraseDone)>>,
    pub extension_surfaces: Mutex<Vec<zephium_core::extensions::ExtensionBrowserSurface>>,
}
impl TestEngine {
    pub fn verify_erasure(&self, profile: ProfileId) {
        self.views
            .lock()
            .unwrap()
            .retain(|(_, owner)| *owner != profile);
        let mut erasures = self.erasures.lock().unwrap();
        let position = erasures
            .iter()
            .position(|(owner, _)| *owner == profile)
            .unwrap();
        let (_, done) = erasures.remove(position);
        drop(erasures);
        done(ProfileDataErasureOutcome::Verified);
    }
}
impl Engine for TestEngine {
    fn set_extension_browser_surface(
        &self,
        surface: zephium_core::extensions::ExtensionBrowserSurface,
    ) -> NativeDispatch {
        self.extension_surfaces.lock().unwrap().push(surface);
        NativeDispatch::Scheduled
    }

    fn settle_extension_browser_request(
        &self,
        _profile: ProfileId,
        _request: ExtensionBrowserRequestId,
        settlement: ExtensionBrowserRequestSettlement,
        _first_url_after_reply: Option<(Arc<str>, NavigationRequestId)>,
    ) -> NativeDispatch {
        if let ExtensionBrowserRequestSettlement::Applied(
            zephium_core::extensions::ExtensionBrowserRequestResult::ExtensionPageAuthorized {
                tab,
                ..
            },
        ) = settlement
        {
            assert!(self
                .extension_surfaces
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .windows()
                .iter()
                .any(|window| window.tabs().iter().any(|published| published.id() == tab)));
        }
        NativeDispatch::Scheduled
    }
    fn load_web_extension(
        &self,
        profile: ProfileId,
        extension: WebExtensionLoad,
    ) -> NativeDispatch {
        self.extension_loads
            .lock()
            .unwrap()
            .push((profile, extension.install));
        NativeDispatch::Scheduled
    }
    fn unload_web_extension(
        &self,
        profile: ProfileId,
        install: ExtensionInstallId,
    ) -> NativeDispatch {
        self.extension_unloads
            .lock()
            .unwrap()
            .push((profile, install));
        NativeDispatch::Scheduled
    }
    fn remove_web_extension(
        &self,
        profile: ProfileId,
        extension: WebExtensionLoad,
    ) -> NativeDispatch {
        self.extension_removals
            .lock()
            .unwrap()
            .push((profile, extension.install));
        NativeDispatch::Scheduled
    }
    fn create_view(&self, id: ItemId, partition: Partition, _url: &str, _bounds: Rect) -> bool {
        self.views.lock().unwrap().push((id, partition.profile()));
        true
    }
    fn navigate(&self, id: ItemId, url: &str, _request: NavigationRequestId) -> bool {
        self.navigations.lock().unwrap().push((id, url.into()));
        true
    }
    fn reload(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn stop(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn go_back(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn go_forward(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn close(&self, id: ItemId) -> NativeDispatch {
        self.closed.lock().unwrap().push(id);
        NativeDispatch::Scheduled
    }
    fn set_content(
        &self,
        _window: WindowId,
        _tree: Option<Pane>,
        _region: Option<Rect>,
    ) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn set_composer(
        &self,
        _item: Option<ItemId>,
        _region: Option<Rect>,
        _floating: bool,
    ) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn focus(&self, _item: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn focus_composer(&self, item: ItemId, context: ComposerFocus) -> NativeDispatch {
        self.composer_focus.lock().unwrap().push((item, context));
        NativeDispatch::Scheduled
    }
    fn set_drop_indicator(
        &self,
        _window: WindowId,
        _preview: Option<DropPreview>,
    ) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn zoom(&self, _id: ItemId, _scale: f64, _request: ZoomRequestId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn set_muted(&self, _id: ItemId, _muted: bool) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn find(&self, _id: ItemId, _request: Option<FindRequest>) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn capture(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn discover_favicon(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn rasterize_favicon(&self, _id: ItemId, _url: &str) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn probe_discard_safety(&self, _id: ItemId, _probe: DiscardProbeId) -> bool {
        false
    }
    fn discard_view(&self, _id: ItemId, _probe: DiscardProbeId) -> bool {
        false
    }
    fn print(&self, _id: ItemId) -> NativeDispatch {
        NativeDispatch::Scheduled
    }
    fn erase_profile_data(&self, profile: ProfileId, done: EraseDone) {
        self.erasures.lock().unwrap().push((profile, done));
    }
}
struct TestChrome;
impl Chrome for TestChrome {
    fn position(&self, _frame: ChromeFrame) -> bool {
        true
    }
}
impl PresentationChrome for TestChrome {
    fn apply_tab_for_presentation(
        &self,
        _presentation: ChromePresentation,
        _done: ChromePresentationCallback,
    ) -> ChromePresentationDispatch {
        ChromePresentationDispatch::Applied
    }
}

pub(super) struct Fixture {
    pub shell: Shell,
    pub store: Arc<zephium_store::SqliteStore>,
    pub engine: Arc<TestEngine>,
    pub events: Arc<Mutex<Vec<Projection>>>,
}
impl Fixture {
    pub(super) fn authenticate_accounts(&mut self) {
        let mut state = self.shell.deck.state().unwrap().clone();
        for (index, account) in state.accounts.iter_mut().enumerate() {
            account.identity = Some(zephium_core::deck::account::AccountIdentity {
                user_id: (index + 1).to_string(),
                display_name: "Account".into(),
                screen_name: format!("account{index}"),
            });
            self.shell.account_login_observed(
                ItemId::from(101 + index as u128),
                zephium_core::deck::account::AccountLoginState::LoggedIn,
            );
        }
        self.shell.deck.set_state(state);
    }

    pub fn new(accounts: u128) -> Self {
        let engine = Arc::new(TestEngine::default());
        let store = Arc::new(zephium_store::SqliteStore::in_memory().unwrap());
        let events = Arc::new(Mutex::new(Vec::new()));
        let collected = events.clone();
        let mut shell = Shell::new(
            engine.clone(),
            store.clone(),
            Arc::new(TestChrome),
            Box::new(move |event| collected.lock().unwrap().push(event)),
        );
        let mut deck = zephium_core::deck::Deck::new(
            zephium_core::ids::DeckId::from(1),
            "Main",
            zephium_core::deck::DeckLayout::Horizontal,
        )
        .unwrap();
        let mut identities = Vec::new();
        for id in 1..=accounts {
            let profile = ProfileId::from(id);
            shell.profiles.insert(Profile {
                id: profile,
                name: "Account".into(),
                kind: if id == 1 {
                    ProfileKind::Default
                } else {
                    ProfileKind::Named
                },
            });
            shell.spaces.insert(Space {
                id: SpaceId::from(id),
                profile,
                name: "Space".into(),
                look: Default::default(),
            });
            identities.push(zephium_core::deck::DeckAccount::new(
                DeckAccountId::from(id),
                profile,
            ));
            deck.add_column(zephium_core::deck::DeckColumn {
                id: DeckColumnId::from(100 + id),
                account: DeckAccountId::from(id),
                target: zephium_core::deck::ColumnTarget::Home,
                width: 320,
                row: 0,
                suspended: false,
            });
        }
        shell.windows.create(
            WindowKind::Main,
            ProfileId::from(1),
            SpaceId::from(1),
            Size::new(1200.0, 800.0),
        );
        shell.bootstrapped = true;
        let state = zephium_core::deck::DeckState {
            accounts: identities,
            active: deck.id,
            decks: vec![deck],
        };
        assert!(shell.apply_deck_state(state.clone()).is_some());
        shell.persist();
        assert!(zephium_core::ports::store::Store::set_app_setting(
            &*store,
            "deck.state".into(),
            serde_json::to_string(&state).unwrap()
        ));
        assert!(store.flush());
        Self {
            shell,
            store,
            engine,
            events,
        }
    }
}
