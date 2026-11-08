use super::*;

impl Shell {
    pub(in crate::shell) fn reveal_deck_column(&self, item: ItemId) {
        let region = self.content_region();
        let Some(viewport) = self.deck_viewport(Rect::new(0.0, 0.0, region.width, region.height))
        else {
            return;
        };
        let Some((_, rect)) = viewport
            .columns
            .iter()
            .find(|(id, _)| id.to_string() == item.to_string())
        else {
            return;
        };
        let delta = if rect.width >= region.width || rect.x < 0.0 {
            rect.x
        } else if rect.x + rect.width > region.width {
            rect.x + rect.width - region.width
        } else {
            0.0
        };
        if delta != 0.0 {
            self.deck
                .scroll_offset
                .set((viewport.offset + delta).clamp(0.0, viewport.max_offset));
            let _ = self.relayout();
        }
    }

    pub(in crate::shell) fn deck_viewport(
        &self,
        region: Rect,
    ) -> Option<zephium_core::deck::layout::ColumnViewport> {
        let state = self.deck.state()?;
        let deck = state
            .decks
            .iter()
            .find(|deck| deck.id == state.active)?
            .clone();
        let viewport = zephium_core::deck::layout::column_viewport(
            &deck,
            region,
            self.deck.scroll_offset.get(),
            1.0,
        );
        self.deck.scroll_offset.set(viewport.offset);
        Some(viewport)
    }

    pub(in crate::shell) fn visible_deck_panes(&self, region: Rect) -> Option<Pane> {
        let viewport = self.deck_viewport(region)?;
        let panes = viewport.columns.into_iter().filter_map(|(id, rect)| {
            (rect.x < region.x + region.width && rect.x + rect.width > region.x)
                .then(|| ItemId::parse(&id.to_string()).map(|id| (id, rect))).flatten()
        }).collect::<Vec<_>>();
        (!panes.is_empty()).then_some(Pane::Positioned(panes))
    }

    pub(in crate::shell) fn ensure_visible_deck_views(&mut self) -> Vec<Effect> {
        let region = self.content_region();
        let Some(panes) = self.visible_deck_panes(Rect::new(0.0, 0.0, region.width, region.height)) else {
            return Vec::new();
        };
        panes.tabs().into_iter().flat_map(|id| self.items.ensure_view(id)).collect()
    }
}
