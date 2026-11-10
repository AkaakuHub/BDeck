//! Authoritative window geometry, split layout, and divider capture.

use super::*;

#[derive(Clone, Debug)]
pub(super) struct GrabbedDivider {
    window: WindowId,
    topology: Pane,
    divider: split::Divider,
    /// Where the pointer took the gutter, from its leading edge, so the
    /// divider does not jump to the pointer on pickup.
    anchor: (f64, f64),
    /// The ratio the guide shows; applied to the panes once, on release.
    ratio: Option<f64>,
}

impl Shell {
    pub(super) fn resolve_drop(&self, x: f64, y: f64) -> Option<split::Drop> {
        let win = self.windows.focused()?;
        let tree = self.pane_tree()?;
        let region =
            layout::compute(win.size, win.mode, win.metrics, self.present(&tree)).content?;
        let local = Rect::new(0.0, 0.0, region.width, region.height);
        split::drop_target(&tree, local, win.metrics.gap, x - region.x, y - region.y)
    }

    pub(super) fn relayout(&self) -> NativeDispatch {
        self.relayout_with(false)
    }

    /// A relayout that is one step of a deliberate change of shape, which
    /// the chrome and the content carry out as a journey.
    pub(super) fn relayout_with(&self, travel: bool) -> NativeDispatch {
        let filling = self.window_filling_fullscreen();
        let Some(win) = self.windows.focused() else {
            return NativeDispatch::Rejected;
        };
        let mut tree = if let Some(id) = filling {
            Some(Pane::Leaf(id))
        } else {
            self.pane_tree()
        };
        let present = tree.as_ref().is_some_and(|t| self.present(t));
        let mut l = layout::compute(win.size, win.mode, win.metrics, present);
        // A browser-owned consent sheet is window-modal. Native page views
        // are sibling views above chrome on every platform, so CSS alone
        // cannot prevent a page from obscuring the sheet or receiving input
        // behind it. Remove content from the native stage and expand
        // privileged chrome for exactly the lifetime of the retained sheet.
        // A prompt in the sidebar needs neither: no page view overlaps it.
        let privileged_overlay_active = self.page_permission_modal() || self.settings_visible();
        // `Items` marks a prospective view resident before its CreateView
        // effect is dispatched. While the profile's first explicit native
        // policy is still compiling/installing, that effect is intentionally
        // held. Do not expose the logical leaf to the engine lifecycle gate
        // until policy settlement releases the create and reserves its native
        // item token. The settlement path applies held effects before calling
        // `relayout`, so the first visible layout remains correctly ordered
        // after native policy registration and view admission.
        if !self.window_visible || privileged_overlay_active {
            l.content = None;
        }
        if filling.is_some() {
            l.content = Some(Rect::new(0.0, 0.0, win.size.width, win.size.height));
        }
        // Raw native children still receive their final geometry while a
        // first navigation is provisional, but macOS must not shrink the
        // privileged chrome away from a fresh New Tab surface until at least
        // one visible leaf completed its exact chrome-verification transition.
        // The content stage itself is transparent and presentation-gated, so
        // it can sit above this real UI without painting an artificial box.
        if !self.chrome.position(ChromeFrame {
            rect: Rect::new(0.0, 0.0, win.size.width, win.size.height),
            fill_width: true,
            travel,
        }) {
            return NativeDispatch::Rejected;
        }
        let extension_page = win.active.is_some_and(|id| {
            self.items
                .tab(id)
                .is_some_and(|tab| tab.content == zephium_core::item::TabContent::ExtensionOwned)
        });
        let mut scroll = None;
        if filling.is_none() && !extension_page {
            if let Some(region) = l.content {
                if let Some(viewport) =
                    self.deck_viewport(Rect::new(0.0, 0.0, region.width, region.height))
                {
                    if viewport.max_offset > 0.0 {
                        let height = region.height.min(6.0);
                        scroll = Some(zephium_ipc::DeckScrollFrame {
                            x: region.x,
                            y: region.y
                                + region
                                    .height
                                    .min(zephium_core::deck::layout::COLUMN_HEADER_HEIGHT)
                                - height,
                            width: region.width,
                            height,
                            offset: viewport.offset,
                            max_offset: viewport.max_offset,
                        });
                    }
                    tree = Some(Pane::Positioned(
                        viewport
                            .columns
                            .into_iter()
                            .filter(|(_, rect)| rect.x < region.width && rect.x + rect.width > 0.0)
                            .filter_map(|(id, rect)| {
                                ItemId::parse(&id.to_string()).map(|id| (id, rect))
                            })
                            .collect(),
                    ));
                }
            }
        }
        let dividers = match (&tree, l.content) {
            (Some(tree), Some(region)) => {
                let local = Rect::new(0.0, 0.0, region.width, region.height);
                split::dividers(tree, local, win.metrics.gap)
                    .into_iter()
                    .map(|d| DividerView {
                        x: region.x + d.strip.x,
                        y: region.y + d.strip.y,
                        width: d.strip.width,
                        height: d.strip.height,
                        vertical: d.axis == Axis::Row,
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let columns = match (&tree, l.content) {
            (Some(tree), Some(region)) if filling.is_none() => {
                split::layout(tree, region, win.metrics.gap)
                    .into_iter()
                    .map(|(id, rect)| {
                        let header = zephium_core::deck::layout::header_rect(rect);
                        zephium_ipc::DeckColumnFrame {
                            id: id.to_string(),
                            x: header.x,
                            y: header.y,
                            width: header.width,
                            height: header.height,
                        }
                    })
                    .collect()
            }
            _ => Vec::new(),
        };
        let composer = if self.accounts.active_item().is_some()
            && filling.is_none()
            && !self.page_permission_modal()
        {
            self.account_frame()
        } else if l.content.is_some() && filling.is_none() {
            self.composer_frame()
        } else {
            None
        };
        (self.emit)(Projection::Layout(LayoutState {
            dividers,
            columns,
            composer: composer.clone(),
            scroll,
        }));
        let applied = self.engine.set_content(win.id, tree, l.content);
        let composer_item = composer
            .as_ref()
            .and_then(|frame| ItemId::parse(&frame.tab));
        let floating = composer.as_ref().is_some_and(|frame| !frame.managing);
        let composer_region = composer.map(|frame| {
            let inset = zephium_core::deck::layout::COMPOSER_HEADER_HEIGHT
                - zephium_core::deck::layout::COLUMN_HEADER_HEIGHT;
            Rect::new(
                frame.x,
                frame.y + inset,
                frame.width,
                (frame.height - inset).max(0.0),
            )
        });
        let composer_applied = self
            .engine
            .set_composer(composer_item, composer_region, floating);
        self.reconcile_page_permission_placement();
        if composer_applied == NativeDispatch::Rejected {
            NativeDispatch::Rejected
        } else {
            applied
        }
    }

    pub(super) fn locate_divider(&self, x: f64, y: f64) -> Option<GrabbedDivider> {
        let win = self.windows.focused()?;
        let tree = self.pane_tree()?;
        let region =
            layout::compute(win.size, win.mode, win.metrics, self.present(&tree)).content?;
        let local = Rect::new(0.0, 0.0, region.width, region.height);
        let divider = split::divider_at(&tree, local, win.metrics.gap, x - region.x, y - region.y)?;
        let anchor = (
            x - region.x - divider.strip.x,
            y - region.y - divider.strip.y,
        );
        Some(GrabbedDivider {
            window: win.id,
            topology: tree,
            divider,
            anchor,
            ratio: None,
        })
    }

    /// Ends a divider capture without applying it, and takes its guide away.
    pub(super) fn drop_divider(&mut self) {
        if let Some(grabbed) = self.divider.take() {
            if grabbed.ratio.is_some() {
                let _ = self.engine.set_resize_guide(grabbed.window, None);
            }
        }
    }

    /// Applies the guided ratio to the panes in one layout, then clears the
    /// guide. Returns false when nothing was captured.
    pub(super) fn commit_divider(&mut self) -> bool {
        let Some(grabbed) = self.divider.take() else {
            return false;
        };
        if let Some(ratio) = grabbed.ratio {
            let _ = self.engine.set_resize_guide(grabbed.window, None);
            if let Some(win) = self.windows.focused_mut() {
                if win.id == grabbed.window {
                    if let Some(tree) = win.splits.as_mut() {
                        tree.set_ratio(&grabbed.divider.path, ratio);
                    }
                }
            }
            let _ = self.relayout();
        }
        true
    }

    /// Moves the guide only: page views keep their size until release, as on
    /// macOS, so no pane relayouts or repaints on every pointer frame.
    pub(super) fn divider_drag(&mut self, x: f64, y: f64) {
        let Some(grabbed) = self.divider.as_ref() else {
            return;
        };
        let grabbed_window = grabbed.window;
        let grabbed_path = grabbed.divider.path.clone();
        let Some(win) = self.windows.focused() else {
            self.drop_divider();
            return;
        };
        let current_tree = self.pane_tree();
        let topology_is_current = win.id == grabbed_window
            && current_tree
                .as_ref()
                .is_some_and(|tree| grabbed.topology.same_topology(tree));
        if !topology_is_current {
            // Focus/topology changed while the pointer was captured. The same
            // binary path may now identify a different live branch.
            self.drop_divider();
            return;
        }
        let Some(region) = layout::compute(win.size, win.mode, win.metrics, true).content else {
            return;
        };
        let gap = win.metrics.gap;
        let Some(tree) = current_tree.as_ref() else {
            self.drop_divider();
            return;
        };
        let local = Rect::new(0.0, 0.0, region.width, region.height);
        let Some(current) = split::divider_at_path(tree, local, gap, &grabbed_path) else {
            // The split tree changed while the pointer was captured. Its old
            // path is no longer authority for any live branch.
            self.drop_divider();
            return;
        };
        let (anchor_x, anchor_y) = grabbed.anchor;
        let ratio = split::ratio_for(
            current.axis,
            current.rect,
            gap,
            x - region.x - anchor_x,
            y - region.y - anchor_y,
        );
        let mut preview = tree.clone();
        preview.set_ratio(&grabbed_path, ratio);
        let Some(target) = split::divider_at_path(&preview, local, gap, &grabbed_path) else {
            return;
        };
        let (r, strip) = (target.rect, target.strip);
        let guide = match target.axis {
            Axis::Row => Rect::new(
                region.x + strip.x + strip.width / 2.0 - 1.0,
                region.y + r.y,
                2.0,
                r.height,
            ),
            Axis::Col => Rect::new(
                region.x + r.x,
                region.y + strip.y + strip.height / 2.0 - 1.0,
                r.width,
                2.0,
            ),
        };
        let window = win.id;
        if let Some(grabbed) = self.divider.as_mut() {
            grabbed.ratio = Some(ratio);
        }
        let _ = self.engine.set_resize_guide(window, Some(guide));
    }

    fn present(&self, tree: &Pane) -> bool {
        tree.tabs()
            .iter()
            .any(|id| self.items.tab(*id).is_some_and(TabState::shows_page))
    }

    // The split group persists across tab switches (Arc model): members show
    // the whole group, other tabs show alone, the group is a tab away.
    pub(super) fn pane_tree(&self) -> Option<Pane> {
        let win = self.windows.focused()?;
        let active = if win.active == self.composer.active_item() {
            self.composer.return_to.or_else(|| self.deck.active_item())
        } else {
            win.active
        }?;
        if !self.item_in_scope(active, win.profile, win.space) {
            return None;
        }
        if self.deck.profile_for_item(active).is_some() {
            let region = self.content_region();
            return self.visible_deck_panes(Rect::new(0.0, 0.0, region.width, region.height));
        }
        if let Some(tree) = win.splits.clone() {
            if tree.contains(active)
                && self.pane_in_scope(&tree, win.profile, win.space)
                && tree
                    .tabs()
                    .into_iter()
                    .all(|id| self.items.tab(id).is_some_and(TabState::shows_page))
            {
                return Some(tree);
            }
        }
        // A view a failure covers keeps its history but stays off the
        // stage, where chrome explains the failure instead.
        self.items
            .tab(active)
            .is_some_and(TabState::shows_page)
            .then_some(Pane::Leaf(active))
    }

    pub(super) fn content_region(&self) -> Rect {
        let Some(win) = self.windows.focused() else {
            return Rect::default();
        };
        layout::compute(win.size, win.mode, win.metrics, true)
            .content
            .unwrap_or_default()
    }
}
