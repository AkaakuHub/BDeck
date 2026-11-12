use super::*;

impl Shell {
    pub(in crate::shell) fn set_media_overlay(&mut self, item: ItemId, open: bool) {
        if self.deck.profile_for_item(item).is_none() {
            return;
        }
        let next = if open {
            self.pane_tree().filter(|tree| tree.contains(item)).map(|_| item)
        } else {
            self.deck.overlay.get().filter(|current| *current != item)
        };
        if self.deck.overlay.replace(next) != next {
            let _ = self.relayout();
        }
    }
}
