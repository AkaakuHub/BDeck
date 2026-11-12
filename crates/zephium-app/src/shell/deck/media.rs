use super::*;

pub(in crate::shell) struct MediaView {
    pub(in crate::shell) item: ItemId,
    pub(in crate::shell) profile: ProfileId,
    pub(in crate::shell) source: ItemId,
}

#[derive(Default)]
pub(in crate::shell) struct MediaRuntime {
    pub(in crate::shell) view: Option<MediaView>,
    pub(in crate::shell) active: std::cell::Cell<bool>,
}

impl MediaRuntime {
    pub(in crate::shell) fn item(&self) -> Option<ItemId> {
        self.view.as_ref().map(|view| view.item)
    }

    pub(in crate::shell) fn active_item(&self) -> Option<ItemId> {
        self.item().filter(|_| self.active.get())
    }

    pub(in crate::shell) fn profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.view.as_ref().filter(|view| view.item == item).map(|view| view.profile)
    }
}

impl Shell {
    pub(in crate::shell) fn open_media(&mut self, source: ItemId, url: url::Url) {
        let Some(profile) = self.deck.profile_for_item(source) else { return; };
        if !self.pane_tree().is_some_and(|tree| tree.contains(source)) {
            return;
        }
        if self.deck.media.view.as_ref().is_some_and(|view| view.profile != profile) {
            if let Some(view) = self.deck.media.view.take() {
                self.deck.media.active.set(false);
                let effects = self.items.remove(view.item);
                self.apply(effects);
            }
        }
        if self.deck.media.view.is_none() {
            let Some(placement) = self.items.get(source).map(|item| item.placement) else { return; };
            let item = ItemId::generate();
            if !self.items.insert_unloaded_tab(item, placement, url.clone(), "X") {
                return;
            }
            self.deck.media.view = Some(MediaView { item, profile, source });
        }
        let Some(view) = self.deck.media.view.as_mut() else { return; };
        view.source = source;
        let item = view.item;
        self.deck.media.active.set(true);
        let loaded = self.items.tab(item).is_some_and(|tab| tab.has_view() && tab.url.as_ref() == Some(&url));
        let mut effects = if loaded { Vec::new() } else { self.items.navigate(item, url.as_str()) };
        effects.extend(self.items.ensure_view(item));
        self.apply(effects);
        let _ = self.relayout();
        self.maintain_views();
        let _ = self.engine.focus(item);
        let _ = self.engine.media_action(item, zephium_core::ports::engine::MediaAction::Play);
    }

    pub(in crate::shell) fn close_media(&mut self, item: ItemId) {
        if self.deck.media.item() != Some(item) || !self.deck.media.active.replace(false) {
            return;
        }
        let _ = self.engine.media_action(item, zephium_core::ports::engine::MediaAction::Pause);
        let _ = self.relayout();
        if let Some(view) = &self.deck.media.view {
            let _ = self.engine.focus(view.source);
        }
        self.maintain_views();
    }
}
