//! Accepted user-operation interpretation and completion disposition.

use super::*;

impl Shell {
    pub(super) fn handle_operation(&mut self, command: Command) -> OperationDisposition {
        match command {
            Command::Open => self.operation_open(),
            Command::InvokeExtensionAction { runtime, revision, anchor } =>
                self.operation_invoke_extension_action(runtime, revision, anchor),
            Command::Activate(id) => self.operation_activate(id),
            Command::Close(id) => self.operation_close(id),
            Command::CreateProfile {
                name,
                avatar,
                photo,
                switch,
            } => self.operation_create_profile(&name, avatar, photo, switch),
            Command::SwitchProfile(profile) => self.operation_switch_profile(profile),
            Command::RenameProfile { profile, name } => {
                self.operation_rename_profile(profile, &name)
            }
            Command::SetProfileAvatar {
                profile,
                avatar,
                photo,
            } => self.operation_set_profile_avatar(profile, avatar, photo),
            Command::Navigate { id, input } => self.operation_navigate(id, input),
            Command::Reload(id) => self.operation_reload(id),
            Command::AnswerPageRequest { id, decision } => {
                self.operation_answer_page_request(id, decision)
            }
            Command::AnswerHttpAuth {
                id,
                request,
                credentials,
            } => self.operation_answer_http_auth(id, request, credentials),
            Command::ContinuePastCertificate(id) => self.operation_continue_past_certificate(id),
            Command::GoBack(id) => self.operation_history(id, false),
            Command::GoForward(id) => self.operation_history(id, true),
            Command::SplitWith { other, axis } => self.operation_split(other, axis),
            Command::Unsplit => self.operation_unsplit(),
            Command::LeaveSplit(id) => self.operation_leave_split(id),
            Command::TabAction { id, action } => self.operation_tab_action(id, action),
            Command::Space(command) => self.operation_space(*command),
            Command::DropTab { id, x, y } => self.operation_drop_tab(id, x, y),
            Command::DividerRelease { x, y } => self.operation_divider_release(x.zip(y)),
            Command::Run(id) => self.operation_run_command(&id),
            Command::OpenUrl { input, new_tab } => self.operation_open_url(input, new_tab),
            Command::SetAppSetting { key, value } => self.operation_set_app_setting(key, value),
            Command::OpenComposer(account) => self.operation_open_composer(Some(account), None),
            Command::OpenAccount(account) => self.operation_open_account(account),
            _ => operation_result(
                OperationOutcome::Rejected,
                OperationReason::UnsupportedCommand,
            ),
        }
    }

    pub(super) fn operation_tab_action(
        &mut self,
        id: ItemId,
        action: TabAction,
    ) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        match action {
            TabAction::Duplicate => self.operation_duplicate(id),
            TabAction::CloseOthers => self.operation_close_around(id, false),
            TabAction::CloseBelow => self.operation_close_around(id, true),
        }
    }

    fn operation_duplicate(&mut self, id: ItemId) -> OperationDisposition {
        let Some(url) = self
            .items
            .tab(id)
            .filter(|tab| tab.content == zephium_core::item::TabContent::Web)
            .and_then(|tab| tab.url.as_ref())
            .map(ToString::to_string)
        else {
            return operation_result(OperationOutcome::NoOp, OperationReason::InvalidInput);
        };
        let Some(space) = self.windows.focused().map(|window| window.space) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let Some((copy, mut effects)) = self.open_tab_with_id() else {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::ItemLimitReached,
            );
        };
        let today = Placement::Space {
            space,
            section: SpaceSection::Today,
        };
        let roots = self.items.roots(today);
        if let Some(at) = roots.iter().position(|root| *root == id) {
            let before = roots[at + 1..].iter().copied().find(|root| *root != copy);
            self.items.move_tab_to_root(copy, today, before);
        }
        effects.extend(self.items.navigate(copy, &url));
        mutation_result(self.commit(effects))
    }

    /// Closes the space's open tabs other than `id`, or only those after it.
    /// Kept and pinned tabs stay, as they do when a day's tabs are cleared.
    fn operation_close_around(&mut self, id: ItemId, below: bool) -> OperationDisposition {
        let Some((space, active)) = self
            .windows
            .focused()
            .map(|window| (window.space, window.active))
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let today = self.today_tabs(space);
        let closing: Vec<ItemId> = if below {
            let Some(at) = today.iter().position(|tab| *tab == id) else {
                return operation_result(OperationOutcome::NoOp, OperationReason::InvalidScope);
            };
            today[at + 1..].to_vec()
        } else {
            today.into_iter().filter(|tab| *tab != id).collect()
        };
        if closing.is_empty() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let mut native = NativeWork::default();
        // Hand the window to the tab that stays first, so closing the one in
        // front never wakes each of the others in turn on its way out.
        if active.is_some_and(|active| closing.contains(&active)) {
            let effects = self.focus_tab(id);
            native.merge(self.commit(effects));
        }
        for tab in closing {
            native.merge(self.close(tab));
        }
        mutation_result(native)
    }

    /// A tab dragged out of a split stands alone; a split left with one tab
    /// is no split at all.
    pub(super) fn operation_leave_split(&mut self, id: ItemId) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        let Some(win) = self.windows.focused_mut() else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let Some(tree) = win.splits.take_if(|tree| tree.contains(id)) else {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        };
        win.splits = tree.remove(id).filter(|rest| rest.tabs().len() > 1);
        self.drop_divider();
        mutation_result(self.commit(Vec::new()))
    }

    fn operation_invoke_extension_action(
        &mut self,
        runtime: zephium_core::extensions::ExtensionRuntimeInstance,
        revision: zephium_core::extensions::ExtensionActionRevision,
        anchor: zephium_core::extensions::ExtensionPopupAnchor,
    ) -> OperationDisposition {
        match self.invoke_extension_action(runtime, revision, anchor) {
            Ok(_) => operation_result(
                OperationOutcome::Deferred,
                OperationReason::NativePending,
            ),
            Err(
                reason @ zephium_core::extensions::ExtensionActionRejection::NativeAdmissionFailed,
            ) => {
                crate::diagnostic!(
                    "extensions: toolbar action synchronously rejected with typed reason {reason:?}"
                );
                self.project_extension_action_failure(runtime.profile(), None, reason);
                operation_result(
                    OperationOutcome::NativeAdmissionFailed,
                    OperationReason::NativeDispatchRejected,
                )
            }
            Err(
                reason @ (zephium_core::extensions::ExtensionActionRejection::UnsupportedPlatform
                | zephium_core::extensions::ExtensionActionRejection::PopupUnavailable),
            ) => {
                crate::diagnostic!(
                    "extensions: toolbar action synchronously rejected with typed reason {reason:?}"
                );
                self.project_extension_action_failure(runtime.profile(), None, reason);
                operation_result(
                    OperationOutcome::Rejected,
                    OperationReason::UnsupportedCommand,
                )
            }
            Err(reason) => {
                crate::diagnostic!(
                    "extensions: toolbar action synchronously rejected with typed reason {reason:?}"
                );
                self.project_extension_action_failure(runtime.profile(), None, reason);
                operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope)
            }
        }
    }

    pub(super) fn operation_open(&mut self) -> OperationDisposition {
        if self.windows.focused().is_none() {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        }
        let Some((_id, effects)) = self.open_tab_with_id() else {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::ItemLimitReached,
            );
        };
        mutation_result(self.commit(effects))
    }

    /// Opens an address in the focused window without naming a tab.
    pub(super) fn operation_open_url(
        &mut self,
        input: String,
        new_tab: bool,
    ) -> OperationDisposition {
        let Some(input) = zephium_core::navigation::web_address(&input).map(|url| url.to_string())
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput);
        };
        let Some(active) = self.windows.focused().map(|window| window.active) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        if let Some(id) = active.filter(|_| !new_tab) {
            // Navigating in place also returns from a browser page, which is
            // what opening a row from the history library should do.
            return self.operation_navigate(id, input);
        }
        self.open_url_in_new_tab_at(input, self.tab_preferences.new_position)
    }

    /// `input` is already a classified address.
    fn open_url_in_new_tab_at(
        &mut self,
        input: String,
        position: tabs::NewTabPosition,
    ) -> OperationDisposition {
        if self.tab_preferences.switch_to_open {
            if let Some(open) = self.open_tab_for(&input) {
                return self.operation_activate(open);
            }
        }
        let Some((id, mut effects)) = self.open_tab_at(position) else {
            // Reaching the bounded item limit must not repurpose and navigate
            // the caller's existing active tab.
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::ItemLimitReached,
            );
        };
        effects.extend(self.items.navigate(id, &input));
        mutation_result(self.commit(effects))
    }

    /// Opens an address in a new tab of the focused window without selecting
    /// it, as a modified click on a link does.
    /// Finds in the page in front. Moving to another page ends the search in
    /// the one before, so its highlights never linger out of sight.
    pub(super) fn find_in_page(
        &mut self,
        request: Option<zephium_core::ports::engine::FindRequest>,
    ) {
        let page = self.windows.focused().and_then(|window| window.active);
        if let Some(previous) = self
            .find_target
            .filter(|previous| Some(*previous) != page || request.is_none())
        {
            let _ = self.engine.find(previous, None);
            self.find_target = None;
        }
        let (Some(page), Some(request)) = (page, request) else {
            return;
        };
        if self.engine.find(page, Some(request))
            != zephium_core::ports::engine::NativeDispatch::Rejected
        {
            self.find_target = Some(page);
        }
    }

    /// Opens what another application handed over, each in its own tab and
    /// the last one in front, the way a clicked link opens elsewhere.
    pub(super) fn open_external(&mut self, urls: Vec<String>) {
        let limit = zephium_core::navigation::MAX_EXTERNAL_TARGETS;
        if !self.bootstrapped {
            let room = limit.saturating_sub(self.pending_external.len());
            self.pending_external.extend(urls.into_iter().take(room));
            return;
        }
        // Another application's link is not a private page: it opens with
        // the regular tabs, the way a private window leaves it to another.
        if self.private_shown() {
            let _ = self.operation_leave_private();
        }
        let position = self
            .tab_preferences
            .external_position
            .unwrap_or(self.tab_preferences.new_position);
        for url in urls.into_iter().take(limit) {
            if let Some(url) = zephium_core::navigation::external_target(&url) {
                let _ = self.open_url_in_new_tab_at(url.to_string(), position);
            }
        }
    }

    /// Opens the startup list. The tab launch put in front is the first
    /// page's, the rest follow it, and the first stays in front.
    pub(super) fn open_startup_pages(&mut self) {
        let mut pages = self.tab_preferences.startup_pages.clone().into_iter();
        let blank = self
            .windows
            .focused()
            .and_then(|window| window.active)
            .filter(|id| self.items.tab(*id).is_some_and(|tab| tab.url.is_none()));
        let Some(first) = pages.next() else {
            return;
        };
        let front = match blank {
            Some(id) => {
                let effects = self.items.navigate(id, &first);
                let _ = self.commit(effects);
                Some(id)
            }
            None => {
                let _ = self.open_url_in_new_tab_at(first, tabs::NewTabPosition::End);
                self.windows.focused().and_then(|window| window.active)
            }
        };
        for page in pages {
            let _ = self.open_url_in_new_tab_at(page, tabs::NewTabPosition::End);
        }
        if let Some(front) = front {
            let _ = self.operation_activate(front);
        }
    }

    pub(super) fn operation_activate(&mut self, id: ItemId) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        self.reveal_deck_column(id);
        let active = self.windows.focused().and_then(|window| window.active);
        let has_view = self.items.tab(id).is_some_and(TabState::has_view);
        let discard_closing = matches!(
            self.residency.discard_probes.get(&id),
            Some(PendingDiscardProbe::Closing { .. })
        );
        if active == Some(id) && has_view && !discard_closing {
            self.touch(id);
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let effects = self.focus_tab(id);
        let native = self.commit(effects);
        if discard_closing {
            operation_result(
                OperationOutcome::Deferred,
                OperationReason::DiscardCompletionPending,
            )
        } else {
            mutation_result(native)
        }
    }

    pub(super) fn operation_close(&mut self, id: ItemId) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if self.is_lone_new_tab(id) {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let discard_closing = matches!(
            self.residency.discard_probes.get(&id),
            Some(PendingDiscardProbe::Closing { .. })
        );
        let native = self.close(id);
        if discard_closing && !native.rejected {
            operation_result(
                OperationOutcome::Deferred,
                OperationReason::DiscardCompletionPending,
            )
        } else {
            mutation_result(native)
        }
    }

    pub(super) fn operation_navigate(&mut self, id: ItemId, input: String) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        let Some(input) = zephium_core::navigation::web_address(&input).map(|url| url.to_string())
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput);
        };
        if let Some(PendingDiscardProbe::Closing {
            probe,
            recreate,
            deferred_navigation,
            reload_on_refusal,
            ..
        }) = self.residency.discard_probes.get_mut(&id)
        {
            *recreate = true;
            *deferred_navigation = Some(input);
            *reload_on_refusal = false;
            self.engine.cancel_discard(id, *probe);
            return operation_result(
                OperationOutcome::Deferred,
                OperationReason::DiscardCompletionPending,
            );
        }
        self.cancel_discard_probe(id);
        let effects = self.items.navigate(id, &input);
        if effects.is_empty() {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput);
        }
        self.cancel_page_permission_for_item(id);
        mutation_result(self.commit(effects))
    }

    pub(super) fn operation_reload(&mut self, id: ItemId) -> OperationDisposition {
        if let Some(url) = self.deck.column_url(id) {
            return self.operation_navigate(id, url);
        }
        self.operation_reload_with(id, false)
    }

    fn operation_reload_with(&mut self, id: ItemId, ignoring_cache: bool) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if self.recreate_after_inflight_discard(id) {
            if let Some(PendingDiscardProbe::Closing {
                reload_on_refusal,
                deferred_navigation,
                ..
            }) = self.residency.discard_probes.get_mut(&id)
            {
                *reload_on_refusal = true;
                *deferred_navigation = None;
            }
            return operation_result(
                OperationOutcome::Deferred,
                OperationReason::DiscardCompletionPending,
            );
        }
        self.cancel_discard_probe(id);
        self.cancel_page_permission_for_item(id);
        if !self.items.tab(id).is_some_and(TabState::has_view) {
            let effects = self.items.ensure_view(id);
            if effects.is_empty() {
                return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
            }
            return mutation_result(self.commit(effects));
        }
        let mut native = NativeWork::default();
        native.record(if ignoring_cache {
            self.engine.hard_reload(id)
        } else {
            self.engine.reload(id)
        });
        mutation_result(native)
    }

    pub(super) fn operation_history(&mut self, id: ItemId, forward: bool) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        let Some(tab) = self.items.tab(id) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        let available = if forward {
            tab.can_go_forward
        } else {
            tab.can_go_back
        };
        if !available || !tab.has_view() {
            return operation_result(OperationOutcome::NoOp, OperationReason::HistoryUnavailable);
        }
        if self.recreate_after_inflight_discard(id) {
            // Native back/forward history belongs to the controller that is
            // already closing and cannot be reconstructed by URL alone.
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::HistoryUnavailable,
            );
        }
        self.cancel_discard_probe(id);
        self.cancel_page_permission_for_item(id);
        let admission = if forward {
            self.engine.go_forward(id)
        } else {
            self.engine.go_back(id)
        };
        let mut native = NativeWork::default();
        native.record(admission);
        mutation_result(native)
    }

    pub(super) fn operation_split(&mut self, other: ItemId, axis: Axis) -> OperationDisposition {
        let Some((active, profile, space)) = self
            .windows
            .focused()
            .and_then(|win| win.active.map(|active| (active, win.profile, win.space)))
        else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        if !self.item_in_scope(active, profile, space) || !self.item_in_scope(other, profile, space)
        {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        if active == other {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let Some(mut tree) = self.pane_tree() else {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::LayoutUnavailable,
            );
        };
        if !self.pane_in_scope(&tree, profile, space) || tree.tabs().len() >= MAX_VISIBLE_PANES {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::LayoutUnavailable,
            );
        }
        if tree.contains(other) {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let closing = [active, other].into_iter().any(|id| {
            matches!(
                self.residency.discard_probes.get(&id),
                Some(PendingDiscardProbe::Closing { .. })
            )
        });
        let effects = self.items.ensure_view(other);
        let mut native = self.apply(effects);
        if !self.items.tab(other).is_some_and(TabState::has_view) {
            // A synchronous create-dispatch refusal already rolled back the
            // optimistic view bit. Do not install a split whose new leaf can
            // never be represented by the native layout admitted below.
            return mutation_result(native);
        }
        self.touch(other);
        if !tree.split(active, other, axis, false) {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        if let Some(win) = self.windows.focused_mut() {
            win.splits = Some(tree);
        }
        native.merge(self.commit(Vec::new()));
        if closing && !native.rejected {
            operation_result(
                OperationOutcome::Deferred,
                OperationReason::DiscardCompletionPending,
            )
        } else {
            mutation_result(native)
        }
    }

    pub(super) fn operation_unsplit(&mut self) -> OperationDisposition {
        let Some(win) = self.windows.focused_mut() else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        if win.splits.take().is_none() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        mutation_result(self.commit(Vec::new()))
    }

    pub(super) fn operation_drop_tab(
        &mut self,
        id: ItemId,
        x: f64,
        y: f64,
    ) -> OperationDisposition {
        if !self.item_in_focused_scope(id) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        }
        let Some(window) = self.windows.focused().map(|window| window.id) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let Some(drop) = self.resolve_drop(x, y) else {
            let _ = self.engine.set_drop_indicator(window, None);
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        };
        let result = self.apply_drop(drop.tab, id, drop.edge);
        let _ = self.engine.set_drop_indicator(window, None);
        result
    }

    pub(super) fn operation_divider_release(
        &mut self,
        final_pointer: Option<(f64, f64)>,
    ) -> OperationDisposition {
        if self.divider.is_none() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        if let Some((x, y)) = final_pointer {
            self.divider_drag(x, y);
        }
        if !self.commit_divider() {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        self.schedule_persist();
        operation_result(OperationOutcome::Applied, OperationReason::MutationApplied)
    }

    pub(super) fn operation_set_app_setting(
        &mut self,
        key: String,
        value: String,
    ) -> OperationDisposition {
        if self.deck.pending_import.is_some() {
            return operation_result(OperationOutcome::Rejected, OperationReason::StorePending);
        }
        if !zephium_core::preferences::value_allowed(&key, &value) {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput);
        }
        let deck_state = if key == deck::SETTING_KEY {
            let Some(state) = serde_json::from_str::<zephium_core::deck::DeckState>(&value).ok()
            else {
                return operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput);
            };
            if !self.can_apply_deck(&state) {
                return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
            }
            if state.decks.iter().flat_map(|deck| &deck.columns).any(|column| {
                let existing = self.deck.state().is_some_and(|previous| previous.decks.iter()
                    .flat_map(|deck| &deck.columns)
                    .any(|saved| saved.id == column.id && saved.account == column.account));
                !existing && state.account(column.account).is_none_or(|account| !self.account_available(account))
            }) {
                return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
            }
            Some(state)
        } else {
            None
        };
        if !self.store.set_app_setting(key.clone(), value.clone()) {
            return operation_result(
                OperationOutcome::Rejected,
                OperationReason::StoreAdmissionRejected,
            );
        }
        self.tab_preferences.apply(&key, &value);
        if self.proxy.apply(&key, &value) {
            self.engine.set_proxy(self.proxy.server());
        }
        if key == "tabs.automatic-pip" && value == "false" {
            self.engine.cancel_automatic_pip();
        }
        if key == "deck.appearance"
            || key == zephium_core::deck::appearance::HIDE_ADVERTISEMENTS_SETTING
        {
            let (appearance, hidden) = deck::view_preferences(&*self.store);
            self.engine.set_deck_appearance(appearance, hidden);
        }
        if let Some(state) = deck_state {
            let result = self.apply_deck_state(state).map_or_else(
                || operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope),
                mutation_result,
            );
            self.project_deck_state();
            return result;
        }
        // This projection is downstream of truthful store-queue admission.
        // The desktop composition root applies native theme state from this
        // signal, never optimistically from the IPC request itself.
        let command = if key == "appearance" {
            format!("theme.{value}")
        } else {
            format!("preference.{key}={value}")
        };
        (self.emit)(Projection::UiCommand(command));
        operation_result(OperationOutcome::Deferred, OperationReason::StorePending)
    }

    pub(super) fn operation_run_command(&mut self, id: &str) -> OperationDisposition {
        let active = self.windows.focused().and_then(|window| window.active);
        match id {
            value if value.starts_with("deck.scroll=") => {
                let Some(offset) = value
                    .strip_prefix("deck.scroll=")
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|offset| offset.is_finite())
                else {
                    return operation_result(
                        OperationOutcome::Rejected,
                        OperationReason::InvalidInput,
                    );
                };
                self.deck.scroll_offset.set(offset);
                let native = self.commit(Vec::new());
                mutation_result(native)
            }
            "account.status" => {
                self.project_account_sessions();
                operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged)
            }
            "composer.open" => self.operation_open_composer(None, None),
            "composer.close" => self.operation_close_composer(),
            "browser.settings" => self.operation_show_settings(),
            "browser.return" => self.request_browser_return(),
            "tab.new" => self.operation_open(),
            "window.newPrivate" => self.operation_enter_private(),
            "window.closePrivate" => self.operation_close_private(),
            "window.togglePrivate" => self.operation_toggle_private(),
            "window.showRegular" => self.operation_show_regular(),
            "window.showPrivate" => self.operation_show_private(),
            "tab.close" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| self.operation_close(id),
            ),
            "tab.reopen" => self.operation_reopen_closed_tab(None),
            _ if id.starts_with("tab.reopen=") => {
                match zephium_core::ids::ClosedSessionId::parse(&id["tab.reopen=".len()..]) {
                    Some(named) => self.operation_reopen_closed_tab(Some(named)),
                    None => {
                        operation_result(OperationOutcome::Rejected, OperationReason::InvalidInput)
                    }
                }
            }
            "tab.next" | "tab.below" => self.operation_cycle_tab(1),
            "tab.previous" | "tab.above" => self.operation_cycle_tab(-1),
            "nav.back" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| self.operation_history(id, false),
            ),
            "nav.forward" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| self.operation_history(id, true),
            ),
            "nav.reload" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| self.operation_reload(id),
            ),
            "nav.hardReload" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| self.operation_reload_with(id, true),
            ),
            "nav.home" => match (active, self.tab_preferences.home.clone()) {
                (Some(id), Some(home)) => self.operation_navigate(id, home),
                (None, Some(home)) => {
                    let position = self.tab_preferences.new_position;
                    self.open_url_in_new_tab_at(home, position)
                }
                (_, None) => self.operation_open(),
            },
            "nav.stop" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| {
                    let mut native = NativeWork::default();
                    native.record(self.engine.stop(id));
                    mutation_result(native)
                },
            ),
            "space.next" => self.operation_space(crate::SpaceCommand::Step(1)),
            "space.previous" => self.operation_space(crate::SpaceCommand::Step(-1)),
            id if id.starts_with("space.select.") => id["space.select.".len()..]
                .parse::<usize>()
                .ok()
                .filter(|position| (1..=9).contains(position))
                .map_or_else(
                    || {
                        operation_result(
                            OperationOutcome::Rejected,
                            OperationReason::UnsupportedCommand,
                        )
                    },
                    |position| self.operation_select_space(position),
                ),
            "tab.select.last" => self.operation_select_tab(None),
            id if id.starts_with("tab.select.") => id["tab.select.".len()..]
                .parse::<usize>()
                .ok()
                .filter(|position| (1..=8).contains(position))
                .map_or_else(
                    || {
                        operation_result(
                            OperationOutcome::Rejected,
                            OperationReason::UnsupportedCommand,
                        )
                    },
                    |position| self.operation_select_tab(Some(position)),
                ),
            "page.print" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| {
                    let mut native = NativeWork::default();
                    native.record(self.engine.print(id));
                    mutation_result(native)
                },
            ),
            "page.devtools" => active.map_or_else(
                || operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow),
                |id| {
                    let mut native = NativeWork::default();
                    native.record(self.engine.open_devtools(id));
                    mutation_result(native)
                },
            ),
            "url.focus" => {
                (self.emit)(Projection::UiCommand("url.focus".into()));
                operation_result(OperationOutcome::Applied, OperationReason::MutationApplied)
            }
            _ => operation_result(
                OperationOutcome::Rejected,
                OperationReason::UnsupportedCommand,
            ),
        }
    }

    /// Reopens the newest closed tab of the focused profile, or the one named,
    /// in the space it came from. Nothing to restore is a no-op, not a
    /// failure.
    pub(super) fn operation_reopen_closed_tab(
        &mut self,
        named: Option<zephium_core::ids::ClosedSessionId>,
    ) -> OperationDisposition {
        let Some(profile) = self.windows.focused().map(|window| window.profile) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        match self.restore_recently_closed_tab(profile, named) {
            Some((_, native)) => mutation_result(native),
            None => operation_result(OperationOutcome::NoOp, OperationReason::MutationApplied),
        }
    }

    fn operation_cycle_tab(&mut self, step: isize) -> OperationDisposition {
        let Some(win) = self.windows.focused() else {
            return operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow);
        };
        let Some(active) = win.active else {
            return operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow);
        };
        let tabs = self.keyboard_tabs(win.space);
        let Some(position) = tabs.iter().position(|id| *id == active) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        if tabs.len() < 2 {
            return operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged);
        }
        let next = (position as isize + step).rem_euclid(tabs.len() as isize) as usize;
        self.operation_activate(tabs[next])
    }

    /// Selects by sidebar position, counting from one. `None` is the last tab,
    /// however many there are.
    fn operation_select_tab(&mut self, position: Option<usize>) -> OperationDisposition {
        let Some(win) = self.windows.focused() else {
            return operation_result(OperationOutcome::NoOp, OperationReason::NoFocusedWindow);
        };
        let tabs = self.keyboard_tabs(win.space);
        let target = match position {
            Some(position) => position.checked_sub(1).and_then(|index| tabs.get(index)),
            None => tabs.last(),
        };
        match target {
            Some(id) if win.active != Some(*id) => self.operation_activate(*id),
            _ => operation_result(OperationOutcome::NoOp, OperationReason::StateUnchanged),
        }
    }
}
