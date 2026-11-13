use super::model::{PanelSnapshot, PanelView};
use zephium_core::ids::ItemId;

#[derive(Default)]
pub(super) struct Presentation {
    pub view: Option<PanelView>,
    revision: u32,
    composer: Option<ItemId>,
    dismissed: Option<ItemId>,
}

impl Presentation {
    pub fn allows(&self, view: &PanelView) -> bool {
        if !view.composer {
            return self.composer.is_none();
        }
        view.id
            .as_deref()
            .and_then(ItemId::parse)
            .is_some_and(|item| self.composer == Some(item) && self.dismissed != Some(item))
    }

    pub fn observe_composer(&mut self, item: Option<ItemId>) -> bool {
        self.composer = item;
        if item.is_none() {
            self.dismissed = None;
        }
        if self.view.as_ref().is_some_and(|view| !self.allows(view)) {
            self.view = None;
            return true;
        }
        false
    }

    pub fn present(&mut self, view: PanelView) -> Result<PanelSnapshot, &'static str> {
        if !self.allows(&view) {
            return Err("stale panel presentation");
        }
        let same = self
            .view
            .as_ref()
            .is_some_and(|current| current.composer == view.composer && current.id == view.id);
        if !same {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or("panel revision exhausted")?;
        }
        self.view = Some(view);
        self.snapshot().ok_or("panel unavailable")
    }

    pub fn dismiss(&mut self, revision: u32) -> Option<PanelView> {
        self.snapshot_at(revision)?;
        let view = self.view.take()?;
        if view.composer {
            self.dismissed = self.composer;
        }
        Some(view)
    }

    pub fn snapshot(&self) -> Option<PanelSnapshot> {
        Some(PanelSnapshot {
            revision: self.revision,
            view: self.view.clone()?,
        })
    }

    pub fn snapshot_at(&self, revision: u32) -> Option<PanelSnapshot> {
        self.snapshot()
            .filter(|snapshot| snapshot.revision == revision && self.allows(&snapshot.view))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(composer: bool, id: Option<ItemId>) -> PanelView {
        PanelView {
            composer,
            id: id.map(|id| id.to_string()),
            content_height: composer.then_some(320),
            title: String::new(),
            index: 0,
            count: 0,
            account: String::new(),
            accounts: Vec::new(),
            width: 320,
            row: 0,
            two_rows: false,
            suspended: false,
            secret: false,
            failed: false,
        }
    }

    #[test]
    fn columns_cannot_replace_a_live_composer() {
        let mut state = Presentation::default();
        state.observe_composer(Some(ItemId::from(1)));
        assert!(state.present(view(false, None)).is_err());
        assert!(state.present(view(true, Some(ItemId::from(1)))).is_ok());
    }

    #[test]
    fn closing_the_composer_rejects_delayed_composer_updates_after_columns_open() {
        let mut state = Presentation::default();
        state.observe_composer(Some(ItemId::from(1)));
        state.present(view(true, Some(ItemId::from(1)))).unwrap();
        assert!(state.observe_composer(None));
        let columns = state.present(view(false, None)).unwrap();
        assert!(state.present(view(true, Some(ItemId::from(1)))).is_err());
        assert!(state.snapshot_at(columns.revision).is_some());
    }

    #[test]
    fn starting_a_composer_invalidates_the_column_panel_before_content_changes() {
        let mut state = Presentation::default();
        let columns = state.present(view(false, None)).unwrap();
        assert!(state.observe_composer(Some(ItemId::from(1))));
        assert!(state.snapshot_at(columns.revision).is_none());
        let composer = state.present(view(true, Some(ItemId::from(1)))).unwrap();
        assert_ne!(columns.revision, composer.revision);
    }

    #[test]
    fn closed_panel_rejects_old_ready_resize_close_and_action_revisions() {
        let mut state = Presentation::default();
        let first = state.present(view(false, None)).unwrap();
        assert!(state.dismiss(first.revision).is_some());
        assert!(state.snapshot_at(first.revision).is_none());
        let second = state.present(view(false, Some(ItemId::from(2)))).unwrap();
        assert!(state.snapshot_at(first.revision).is_none());
        assert!(state.dismiss(first.revision).is_none());
        assert!(state.snapshot_at(second.revision).is_some());
    }

    #[test]
    fn dismissed_composer_stays_suppressed_until_the_actor_observes_its_close() {
        let mut state = Presentation::default();
        let item = ItemId::from(1);
        state.observe_composer(Some(item));
        let first = state.present(view(true, Some(item))).unwrap();
        state.dismiss(first.revision).unwrap();
        state.observe_composer(Some(item));
        assert!(state.present(view(true, Some(item))).is_err());
        state.observe_composer(None);
        state.observe_composer(Some(item));
        assert!(state.present(view(true, Some(item))).is_ok());
    }

    #[test]
    fn account_switch_rejects_the_previous_accounts_composer() {
        let mut state = Presentation::default();
        state.observe_composer(Some(ItemId::from(1)));
        state.present(view(true, Some(ItemId::from(1)))).unwrap();
        assert!(state.observe_composer(Some(ItemId::from(2))));
        assert!(state.present(view(true, Some(ItemId::from(1)))).is_err());
        assert!(state.present(view(true, Some(ItemId::from(2)))).is_ok());
    }

    #[test]
    fn content_resizing_keeps_the_current_presentation() {
        let mut state = Presentation::default();
        let item = ItemId::from(1);
        state.observe_composer(Some(item));
        let first = state.present(view(true, Some(item))).unwrap();
        assert!(!state.observe_composer(Some(item)));
        for height in [500, 160, 640] {
            let mut next = view(true, Some(item));
            next.content_height = Some(height);
            let resized = state.present(next).unwrap();
            assert_eq!(first.revision, resized.revision);
        }
    }

    #[test]
    fn exhausted_revision_never_reuses_an_old_presentation() {
        let mut state = Presentation {
            revision: u32::MAX - 1,
            ..Presentation::default()
        };
        let last = state.present(view(false, None)).unwrap();
        state.dismiss(last.revision).unwrap();
        assert!(state.present(view(false, None)).is_err());
        assert!(state.snapshot_at(last.revision).is_none());
    }
}
