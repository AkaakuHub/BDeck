use super::*;
use zephium_core::ids::DeckAccountId;
use zephium_core::ports::engine::ComposerFocus;

const COMPOSE_URL: &str = "https://x.com/compose/post";

struct ComposerTab {
    account: DeckAccountId,
    profile: ProfileId,
    item: ItemId,
    ready: bool,
    height: f64,
    focus: ComposerFocus,
}

#[derive(Default)]
pub(in crate::shell) struct ComposerRuntime {
    tabs: Vec<ComposerTab>,
    active: Option<DeckAccountId>,
    last_account: Option<DeckAccountId>,
    pub(in crate::shell) return_to: Option<ItemId>,
}

impl ComposerRuntime {
    pub(in crate::shell) fn retire_profile(&mut self, profile: ProfileId) {
        let accounts = self
            .tabs
            .iter()
            .filter(|tab| tab.profile == profile)
            .map(|tab| tab.account)
            .collect::<Vec<_>>();
        self.tabs.retain(|tab| tab.profile != profile);
        if self
            .active
            .is_some_and(|account| accounts.contains(&account))
        {
            self.active = None;
            self.return_to = None;
        }
        if self
            .last_account
            .is_some_and(|account| accounts.contains(&account))
        {
            self.last_account = None;
        }
    }

    pub(in crate::shell) fn retained_items(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.tabs.iter().map(|tab| tab.item)
    }

    pub(in crate::shell) fn profile_for_item(&self, item: ItemId) -> Option<ProfileId> {
        self.tabs
            .iter()
            .find(|tab| tab.item == item)
            .map(|tab| tab.profile)
    }
    pub(in crate::shell) fn active_item(&self) -> Option<ItemId> {
        self.tabs
            .iter()
            .find(|tab| Some(tab.account) == self.active)
            .map(|tab| tab.item)
    }
}

impl Shell {
    fn prepare_composer(&mut self, account: &zephium_core::deck::DeckAccount) -> Option<ItemId> {
        if let Some(tab) = self
            .composer
            .tabs
            .iter()
            .find(|tab| tab.account == account.id)
        {
            return Some(tab.item);
        }
        let space = self.windows.focused()?.space;
        let item = ItemId::generate();
        if !self.items.insert_unloaded_tab(
            item,
            zephium_core::item::Placement::Space {
                space,
                section: zephium_core::item::SpaceSection::Today,
            },
            url::Url::parse(COMPOSE_URL).expect("static compose URL"),
            "X",
        ) {
            return None;
        }
        self.composer.tabs.push(ComposerTab {
            account: account.id,
            profile: account.profile,
            item,
            ready: false,
            height: 320.0,
            focus: ComposerFocus::Preserve,
        });
        Some(item)
    }

    pub(in crate::shell) fn prepare_composers(&mut self) {
        if !self.bootstrapped {
            return;
        }
        let accounts = self
            .deck
            .state()
            .map(|state| state.accounts.clone())
            .unwrap_or_default();
        let mut effects = Vec::new();
        for account in accounts {
            if account.identity.is_none()
                || self.accounts.active_profile() == Some(account.profile)
                || self.profile_deletion_quarantines(account.profile)
                || self.accounts.account_changed(account.profile)
            {
                continue;
            }
            if let Some(item) = self.prepare_composer(&account) {
                effects.extend(self.items.ensure_view(item));
            }
        }
        self.apply(effects);
    }

    fn focus_composer_editor(&mut self, item: ItemId) {
        let context = self
            .composer
            .tabs
            .iter_mut()
            .find(|tab| tab.item == item && tab.ready)
            .map(|tab| std::mem::take(&mut tab.focus))
            .unwrap_or_default();
        let _ = self.engine.focus_composer(item, context);
    }

    pub(in crate::shell) fn composer_ready(&mut self, item: ItemId) {
        if let Some(tab) = self.composer.tabs.iter_mut().find(|tab| tab.item == item) {
            tab.ready = true;
            if self.composer.active_item() == Some(item) && !self.settings_visible() {
                self.focus_composer_editor(item);
            }
        }
    }

    pub(in crate::shell) fn composer_height(&mut self, item: ItemId, height: f64) {
        if let Some(tab) = self.composer.tabs.iter_mut().find(|tab| tab.item == item) {
            if (tab.height - height).abs() >= 1.0 {
                tab.height = height;
                if self.composer.active_item() == Some(item) {
                    let _ = self.relayout();
                }
            }
        }
    }

    pub(in crate::shell) fn reset_composer_after_navigation(
        &mut self,
        item: ItemId,
        url: &url::Url,
    ) {
        if url.host_str() != Some("x.com") || url.path() != "/home" {
            return;
        }
        let Some(tab) = self
            .composer
            .tabs
            .iter_mut()
            .find(|tab| tab.item == item && tab.ready)
        else {
            return;
        };
        tab.ready = false;
        if self.composer.active_item() == Some(item) && self.accounts.active_item().is_none() {
            let _ = self.operation_close_composer();
        }
        let effects = self.items.navigate(item, COMPOSE_URL);
        self.apply(effects);
    }

    pub(in crate::shell) fn composer_frame(&self) -> Option<zephium_ipc::ComposerFrame> {
        let cached = self
            .composer
            .tabs
            .iter()
            .find(|cached| Some(cached.account) == self.composer.active)?;
        let tab = cached.item;
        let account = self.composer.active?;
        let window = self.windows.focused()?;
        let region = self.content_region();
        let width = 480.0_f64.min((region.width - 16.0).max(0.0));
        Some(zephium_ipc::ComposerFrame {
            tab: tab.to_string(),
            account: account.to_string(),
            managing: false,
            x: region.x + 8.0,
            y: region.y + 8.0,
            width,
            height: (cached.height + zephium_core::deck::layout::COMPOSER_HEADER_HEIGHT + 2.0)
                .min((window.size.height - region.y - 8.0).max(0.0)),
        })
    }

    pub(in crate::shell) fn operation_open_composer(
        &mut self,
        account: Option<DeckAccountId>,
        mention: Option<String>,
    ) -> OperationDisposition {
        let Some(account) = self
            .deck
            .state()
            .and_then(|state| match account {
                Some(id) => state.account(id),
                None => self
                    .composer
                    .last_account
                    .and_then(|id| state.account(id))
                    .filter(|account| self.account_available(account))
                    .or_else(|| {
                        state
                            .accounts
                            .iter()
                            .find(|account| self.account_available(account))
                    }),
            })
            .cloned()
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        if self.windows.focused().is_none() {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        }
        if !self.account_available(&account) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if self.composer.active.is_none() {
            self.composer.return_to = self.windows.focused().and_then(|window| window.active);
        }
        let Some(item) = self.prepare_composer(&account) else {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::ItemLimitReached,
            );
        };
        if let Some(tab) = self.composer.tabs.iter_mut().find(|tab| tab.item == item) {
            tab.focus = ComposerFocus::Recipient(mention);
        }
        let needs_navigation = self
            .items
            .tab(item)
            .and_then(|tab| tab.url.as_ref())
            .is_some_and(|url| {
                url.host_str() == Some("x.com") && !url.path().starts_with("/compose/")
            });
        self.composer.active = Some(account.id);
        self.composer.last_account = Some(account.id);
        let effects = self.focus_tab(item);
        let native = self.commit(effects);
        if !native.rejected && needs_navigation {
            return self.operation_navigate(item, COMPOSE_URL.into());
        }
        if !native.rejected {
            self.focus_composer_editor(item);
        }
        mutation_result(native)
    }

    pub(in crate::shell) fn operation_close_composer(&mut self) -> OperationDisposition {
        if self.accounts.active_item().is_some() {
            return self.operation_close_account();
        }
        if self.composer.active.take().is_none() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let target = self
            .composer
            .return_to
            .take()
            .filter(|id| self.item_in_focused_scope(*id))
            .or_else(|| self.deck.active_item());
        let effects = target.map_or_else(Vec::new, |id| self.focus_tab(id));
        if target.is_none() {
            if let Some(window) = self.windows.focused_mut() {
                window.active = None;
            }
        }
        let native = self.commit(effects);
        if let Some(item) = target {
            let _ = self.engine.focus(item);
        }
        mutation_result(native)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached() -> ComposerRuntime {
        ComposerRuntime {
            tabs: (1..=2)
                .map(|id| ComposerTab {
                    account: DeckAccountId::from(id),
                    profile: ProfileId::from(id),
                    item: ItemId::from(id),
                    ready: true,
                    height: 320.0,
                    focus: ComposerFocus::Preserve,
                })
                .collect(),
            active: Some(DeckAccountId::from(2)),
            last_account: Some(DeckAccountId::from(2)),
            return_to: Some(ItemId::from(10)),
        }
    }

    #[test]
    fn deleting_an_inactive_account_keeps_the_other_composer_and_its_focus() {
        let mut runtime = cached();
        runtime.retire_profile(ProfileId::from(1));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(2)]
        );
        assert_eq!(runtime.active_item(), Some(ItemId::from(2)));
        assert_eq!(runtime.last_account, Some(DeckAccountId::from(2)));
        assert_eq!(runtime.return_to, Some(ItemId::from(10)));
        assert_eq!(runtime.profile_for_item(ItemId::from(1)), None);
    }

    #[test]
    fn deleting_the_active_account_retires_its_cached_composer_without_losing_others() {
        let mut runtime = cached();
        runtime.retire_profile(ProfileId::from(2));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(1)]
        );
        assert_eq!(runtime.active_item(), None);
        assert_eq!(runtime.last_account, None);
        assert_eq!(runtime.return_to, None);
        runtime.retire_profile(ProfileId::from(2));
        assert_eq!(
            runtime.retained_items().collect::<Vec<_>>(),
            vec![ItemId::from(1)]
        );
    }
}
