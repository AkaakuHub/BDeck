mod state;

use std::cell::RefCell;
use std::ptr::NonNull;

use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventPhase};
use objc2_foundation::{NSError, NSNumber, NSString};
use objc2_web_kit::WKWebView;

use state::{horizontal_scroll, Direction, Gesture};

pub(super) struct HistoryGesture {
    monitor: Retained<AnyObject>,
}

impl HistoryGesture {
    pub(super) fn install(
        page: &Retained<WKWebView>,
        on_scroll: impl Fn(&WKWebView, f64) -> bool + 'static,
    ) -> Result<Self, &'static str> {
        let weak = Weak::from_retained(page);
        let gesture = RefCell::new(Gesture::default());
        let handler = block2::RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit lends this event to its main-thread local monitor.
            let event_ref = unsafe { event.as_ref() };
            let Some(page) = weak.load() else {
                return event.as_ptr();
            };
            let shift_scroll = event_ref
                .modifierFlags()
                .contains(NSEventModifierFlags::Shift)
                && !event_ref
                    .modifierFlags()
                    .intersects(NSEventModifierFlags::Control | NSEventModifierFlags::Command);
            if shift_scroll {
                let precise = event_ref.hasPreciseScrollingDeltas();
                let (x, y) = if precise {
                    (event_ref.scrollingDeltaX(), event_ref.scrollingDeltaY())
                } else {
                    (event_ref.deltaX(), event_ref.deltaY())
                };
                if let Some(delta) = horizontal_scroll(x, y, precise) {
                    if targets_page(&page, event_ref) && on_scroll(&page, delta) {
                        gesture.borrow_mut().reset();
                        return std::ptr::null_mut();
                    }
                }
                return event.as_ptr();
            }
            if !NSEvent::isSwipeTrackingFromScrollEventsEnabled()
                || !event_ref.hasPreciseScrollingDeltas()
                || event_ref.phase().is_empty()
                || !event_ref.momentumPhase().is_empty()
            {
                return event.as_ptr();
            }
            let phase = event_ref.phase();
            if phase.intersects(
                NSEventPhase::Began
                    | NSEventPhase::MayBegin
                    | NSEventPhase::Ended
                    | NSEventPhase::Cancelled,
            ) {
                gesture.borrow_mut().reset();
            }
            if !phase.intersects(NSEventPhase::Began | NSEventPhase::Changed) {
                return event.as_ptr();
            }
            let modifiers = NSEventModifierFlags::Shift
                | NSEventModifierFlags::Control
                | NSEventModifierFlags::Command
                | NSEventModifierFlags::Option;
            if event_ref.modifierFlags().intersects(modifiers) {
                gesture.borrow_mut().reset();
                return event.as_ptr();
            }
            let direction = gesture
                .borrow_mut()
                .advance(event_ref.scrollingDeltaX(), event_ref.scrollingDeltaY());
            let Some(direction) = direction else {
                return event.as_ptr();
            };
            let targeted = targets_page(&page, event_ref);
            if !targeted {
                gesture.borrow_mut().reset();
                return event.as_ptr();
            }
            navigate(&page, event_ref, direction);
            event.as_ptr()
        });
        // SAFETY: the monitor is local to this app and owned on the main thread.
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::ScrollWheel,
                &handler,
            )
        }
        .ok_or("history gesture monitor unavailable")?;
        Ok(Self { monitor })
    }
}

impl Drop for HistoryGesture {
    fn drop(&mut self) {
        // SAFETY: retire the exact registration before its WebView is destroyed.
        unsafe {
            NSEvent::removeMonitor(&self.monitor);
        }
    }
}

fn targets_page(page: &WKWebView, event: &NSEvent) -> bool {
    let Some(window) = page.window() else {
        return false;
    };
    !page.isHiddenOrHasHiddenAncestor()
        && event.windowNumber() == window.windowNumber()
        && window
            .contentView()
            .and_then(|content| {
                let parent = unsafe { content.superview() }?;
                let point = parent.convertPoint_fromView(event.locationInWindow(), None);
                content.hitTest(point)
            })
            .is_some_and(|hit| hit.isDescendantOf(page))
}

fn navigate(page: &Retained<WKWebView>, event: &NSEvent, direction: Direction) {
    let point = page.convertPoint_fromView(event.locationInWindow(), None);
    let bounds = page.bounds();
    if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
        return;
    }
    let x = point.x / bounds.size.width;
    let y = if page.isFlipped() {
        point.y
    } else {
        bounds.size.height - point.y
    } / bounds.size.height;
    let script = format!(
        "(() => {{ const pointX = {x}; const pointY = {y}; return {} }})()",
        include_str!("eligible.js").trim_end_matches([';', '\n'])
    );
    let weak = Weak::from_retained(page);
    let Some(url) = (unsafe { page.URL() }) else {
        return;
    };
    let callback = block2::RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
        if !error.is_null() {
            return;
        }
        // SAFETY: WebKit lends the JavaScript result for this completion callback.
        let eligible = unsafe { value.as_ref() }
            .and_then(|value| value.downcast_ref::<NSNumber>())
            .is_some_and(|value| value.boolValue());
        let Some(page) = weak
            .load()
            .filter(|page| !page.isHiddenOrHasHiddenAncestor())
        else {
            return;
        };
        if !eligible || unsafe { page.URL() }.as_deref() != Some(&url) {
            return;
        }
        // SAFETY: the same live main-thread WebView and source URL passed the hit test.
        unsafe {
            match direction {
                Direction::Back if page.canGoBack() => {
                    let previous = page
                        .backForwardList()
                        .backItem()
                        .and_then(|item| item.URL().absoluteString());
                    if previous.is_some_and(|value| value.to_string() != "about:blank") {
                        page.goBack();
                    }
                }
                Direction::Forward if page.canGoForward() => {
                    page.goForward();
                }
                _ => {}
            }
        }
    });
    // SAFETY: the script checks the pointer target without changing page content.
    unsafe {
        page.evaluateJavaScript_completionHandler(&NSString::from_str(&script), Some(&callback));
    }
}
