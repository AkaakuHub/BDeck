use super::*;

impl Shell {
    pub(in crate::shell) fn open_media(&mut self, item: ItemId) {
        if self.deck.profile_for_item(item).is_none()
            || !self.pane_tree().is_some_and(|tree| tree.contains(item))
        {
            return;
        }
        self.deck.overlay.set(Some(item));
        if let Some(window) = self.windows.focused() {
            let card = layout::hidden_card(window.size, window.metrics);
            let _ = self.engine.raise_chrome(window.id, false, card, false);
        }
        let _ = self.relayout();
    }

    pub(in crate::shell) fn close_media(&mut self, item: ItemId) {
        if self.deck.overlay.get() != Some(item) {
            return;
        }
        self.deck.overlay.set(None);
        let _ = self.relayout();
    }
}
