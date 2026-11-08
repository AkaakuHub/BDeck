use super::*;

pub(super) struct PendingBrowserReturn {
    revision: u64,
    window: WindowId,
    profile: ProfileId,
    space: SpaceId,
    active: Option<ItemId>,
    splits: Option<Pane>,
}

impl Shell {
    pub(super) fn settings_visible(&self) -> bool {
        self.windows
            .focused()
            .is_some_and(|window| self.browser_page == Some(window.id))
    }

    pub(super) fn project_browser_page(&self) {
        let command = if self.settings_visible() {
            "browser.settings"
        } else {
            "browser.return"
        };
        (self.emit)(Projection::UiCommand(command.into()));
    }

    pub(super) fn operation_show_settings(&mut self) -> OperationDisposition {
        let Some(window) = self.windows.focused().map(|window| window.id) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let previous = self.browser_page;
        self.browser_return = None;
        self.browser_page = Some(window);
        self.drop_divider();
        self.cancel_page_permission_if_not_foreground();
        if self.relayout() != NativeDispatch::Scheduled {
            self.browser_page = previous;
            let _ = self.relayout();
            return operation_result(
                OperationOutcome::NativeAdmissionFailed,
                OperationReason::NativeDispatchRejected,
            );
        }
        self.project_browser_page();
        self.project_account_sessions();
        operation_result(OperationOutcome::Deferred, OperationReason::NativePending)
    }

    pub(super) fn request_browser_return(&mut self) -> OperationDisposition {
        if self.browser_return.is_some() {
            return operation_result(OperationOutcome::Deferred, OperationReason::NativePending);
        }
        let Some(items) = self.items_snapshot() else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let Some(window) = self.windows.focused() else {
            return operation_result(OperationOutcome::Rejected, OperationReason::NoFocusedWindow);
        };
        let Some(revision) = self.browser_return_revision.checked_add(1) else {
            return operation_result(OperationOutcome::Rejected, OperationReason::InvalidScope);
        };
        self.browser_return_revision = revision;
        self.browser_return = Some(PendingBrowserReturn {
            revision,
            window: window.id,
            profile: window.profile,
            space: window.space,
            active: window.active,
            splits: window.splits.clone(),
        });
        self.forget_offered_items();
        let callback = self.self_queue.as_ref().map(|queue| CallbackHandle {
            queue: Arc::downgrade(&queue.inner),
        });
        let dispatch = self.chrome.restore_browser_chrome(
            revision,
            items,
            Box::new(move |applied| {
                if let Some(callback) = callback {
                    let _ = callback.dispatch(Command::BrowserChromeRestored { revision, applied });
                }
            }),
        );
        match dispatch {
            ChromePresentationDispatch::Applied => self.browser_chrome_restored(revision, true),
            ChromePresentationDispatch::Rejected => self.browser_chrome_restored(revision, false),
            ChromePresentationDispatch::Scheduled => {}
        }
        operation_result(OperationOutcome::Deferred, OperationReason::NativePending)
    }

    pub(super) fn browser_chrome_restored(&mut self, revision: u64, applied: bool) {
        let Some(pending) = self.browser_return.as_ref() else {
            return;
        };
        if pending.revision != revision {
            return;
        }
        let exact = self.windows.focused().is_some_and(|window| {
            window.id == pending.window
                && window.profile == pending.profile
                && window.space == pending.space
                && window.active == pending.active
                && window.splits == pending.splits
        });
        self.browser_return = None;
        if !applied || !exact {
            self.project_browser_page();
            (self.emit)(Projection::UiCommand("browser.return-failed".into()));
            return;
        }
        let previous = self.browser_page.take();
        if self.relayout() != NativeDispatch::Scheduled {
            self.browser_page = previous;
            let _ = self.relayout();
            (self.emit)(Projection::UiCommand("browser.return-failed".into()));
        }
        self.project_browser_page();
    }
}
