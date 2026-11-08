use objc2_foundation::MainThreadMarker;
use zephium_core::ports::engine::Partition;

pub fn webkit(view: &wry::WebView) -> objc2::rc::Retained<objc2_web_kit::WKWebView> {
    use wry::WebViewExtMacOS;
    // SAFETY: WryWebView is a WKWebView subclass; this is a plain upcast.
    unsafe { objc2::rc::Retained::cast_unchecked(view.webview()) }
}

pub(crate) fn set_warm_spare_layout(webview: &wry::WebView, spare: bool) {
    use objc2_app_kit::NSAutoresizingMaskOptions as Mask;
    // A zero-sized spare must flex its margins, not its dimensions. Otherwise
    // AppKit derives a minimum content size from its fixed launch-size margins,
    // preventing the entire window from shrinking below its initial dimensions.
    webkit(webview).setAutoresizingMask(if spare {
        Mask::ViewMaxXMargin | Mask::ViewMinYMargin
    } else {
        Mask::ViewWidthSizable | Mask::ViewHeightSizable
    });
}

pub fn configure(
    webview: &wry::WebView,
    radius: f64,
    partition: Partition,
    expected_ephemeral_store: Option<&super::WebsiteDataStore>,
) -> Result<(), String> {
    use objc2_app_kit::{NSAutoresizingMaskOptions as Mask, NSView};

    let wk = webkit(webview);
    let data_store = unsafe { wk.configuration().websiteDataStore() };
    let persistent = unsafe { data_store.isPersistent() };
    let identifier = unsafe { data_store.identifier() }.map(|identifier| identifier.as_bytes());
    validate_data_store_postcondition(
        partition,
        persistent,
        identifier,
        expected_ephemeral_store.is_some(),
    )?;
    if let Some(expected) = expected_ephemeral_store {
        if objc2::rc::Retained::as_ptr(&data_store) != objc2::rc::Retained::as_ptr(expected) {
            return Err("private WKWebView did not use its profile-owned data store".into());
        }
    }

    unsafe { wk.setInspectable(true) };
    super::location::serve(&wk);
    // Pinch magnifies the page the way Safari's does; page zoom stays separate.
    unsafe { wk.setAllowsMagnification(true) };
    let view: &NSView = &wk;
    // Fill the assigned region and follow window resize in AppKit's layout pass.
    view.setTranslatesAutoresizingMaskIntoConstraints(true);
    view.setAutoresizingMask(Mask::ViewWidthSizable | Mask::ViewHeightSizable);
    round_page(view, radius);
    Ok(())
}

/// A Browse page's own edge: its corner and the hairline that keeps it
/// readable.
pub(crate) fn round_page(view: &objc2_app_kit::NSView, radius: f64) {
    if let Some(layer) = view.layer() {
        layer.setCornerRadius(radius);
        // The window's own corners are continuous, so a circular page corner
        // beside them reads as a different shape at the one place they meet.
        // SAFETY: an immutable framework constant.
        layer.setCornerCurve(unsafe { objc2_quartz_core::kCACornerCurveContinuous });
        layer.setMasksToBounds(true);
        layer.setBorderWidth(0.0);
    }
}

fn validate_data_store_postcondition(
    partition: Partition,
    persistent: bool,
    identifier: Option<[u8; 16]>,
    has_expected_ephemeral_store: bool,
) -> Result<(), String> {
    match partition {
        Partition::Ephemeral(_) => {
            if !has_expected_ephemeral_store {
                return Err("ephemeral WKWebView has no profile-owned data-store proof".into());
            }
            if persistent {
                return Err("ephemeral WKWebView received a persistent website data store".into());
            }
            if identifier.is_some() {
                return Err("ephemeral WKWebView exposed a durable data-store identifier".into());
            }
        }
        Partition::Default(profile) | Partition::Persistent(profile) => {
            if has_expected_ephemeral_store {
                return Err("durable WKWebView received an ephemeral data-store proof".into());
            }
            if !persistent {
                return Err(
                    "durable WKWebView received a non-persistent website data store".into(),
                );
            }
            if identifier != Some(profile.bytes()) {
                return Err(
                    "durable WKWebView data-store identifier does not match its profile".into(),
                );
            }
        }
    }
    Ok(())
}

/// WebKit revalidates the page and its subresources against the origin.
pub fn reload_ignoring_cache(view: &wry::WebView) -> bool {
    unsafe { webkit(view).reloadFromOrigin() }.is_some()
}

pub fn stop_loading(view: &wry::WebView) {
    unsafe { webkit(view).stopLoading() };
}

/// WebKit's back/forward list around the current entry, as (title, address)
/// in order, with the index of the current one. Only the nearest entries
/// each way are copied out of the list.
pub fn history_session(
    view: &wry::WebView,
    done: impl FnOnce(Option<(Vec<(String, String)>, usize)>),
) -> bool {
    use zephium_core::ports::engine::MAX_HISTORY_SIDE;
    let page = webkit(view);
    // SAFETY: public WebKit reads on the main thread.
    let list = unsafe { page.backForwardList() };
    let entry = |item: &objc2_web_kit::WKBackForwardListItem| {
        let address = unsafe { item.URL() }
            .absoluteString()
            .map(|url| url.to_string())
            .filter(|url| url.len() <= 8 * 1024)
            .unwrap_or_default();
        let title = unsafe { item.title() }
            .map(|title| title.to_string())
            .filter(|title| title.len() <= 8 * 1024)
            .unwrap_or_default();
        (title, address)
    };
    let Some(current) = (unsafe { list.currentItem() }) else {
        done(None);
        return true;
    };
    let behind: Vec<_> = unsafe { list.backList() }.iter().collect();
    let back: Vec<_> = behind
        .iter()
        .rev()
        .take(MAX_HISTORY_SIDE)
        .map(|item| entry(item))
        .collect();
    let forward: Vec<_> = unsafe { list.forwardList() }
        .iter()
        .take(MAX_HISTORY_SIDE)
        .map(|item| entry(&item))
        .collect();
    let current_at = back.len();
    let session = back
        .into_iter()
        .rev()
        .chain(std::iter::once(entry(&current)))
        .chain(forward)
        .collect();
    done(Some((session, current_at)));
    true
}

/// Goes to the entry `offset` steps from the current one when it is still at
/// `url`. WebKit counts the offset itself, so a list read earlier cannot send
/// the view to a different entry than the one shown.
pub fn go_to_history_entry(view: &wry::WebView, offset: i32, url: &str) -> bool {
    if offset == 0 {
        return false;
    }
    let page = webkit(view);
    // SAFETY: public WebKit access on the main thread.
    let list = unsafe { page.backForwardList() };
    let Some(item) = (unsafe { list.itemAtIndex(offset as isize) }) else {
        return false;
    };
    let at = unsafe { item.URL() }
        .absoluteString()
        .map(|at| at.to_string());
    at.as_deref() == Some(url) && unsafe { page.goToBackForwardListItem(&item) }.is_some()
}

/// Validate the real OS owner again at native consent admission/settlement.
pub(crate) fn permission_owner_is_focused(parent: raw_window_handle::RawWindowHandle) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    let raw_window_handle::RawWindowHandle::AppKit(parent) = parent else {
        return false;
    };
    // SAFETY: EngineHost owns the composition root's live main-window handle
    // throughout each child's lifetime. A consent modal deliberately detaches
    // the page from its stage, so focus belongs to this main-window owner.
    let view = unsafe { &*parent.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    objc2_app_kit::NSApplication::sharedApplication(mtm).isActive()
        && view.window().is_some_and(|window| window.isKeyWindow())
}

/// Cross-check renderer heuristics with WebKit's public media playback and
/// capture state. Playback is asynchronous; the caller's existing bounded
/// deadline handles a missing native completion without retaining the view.
/// `None` while capturing or once the view is gone; otherwise whether the
/// page may be producing sound. WebKit's audibility SPI answers exactly,
/// child frames and Web Audio included. Without it the public playback state
/// also counts muted media, so callers pair it with the renderer's report.
pub fn query_document_playback(
    view: &wry::WebView,
    done: impl FnOnce(Option<bool>) + 'static,
) -> bool {
    use std::cell::RefCell;
    use std::rc::Rc;

    use objc2_web_kit::{WKMediaCaptureState, WKMediaPlaybackState};

    let wk = webkit(view);
    let capturing = unsafe {
        wk.cameraCaptureState() != WKMediaCaptureState::None
            || wk.microphoneCaptureState() != WKMediaCaptureState::None
    };
    if capturing {
        done(None);
        return true;
    }
    // SAFETY: main-thread WebKit access; the SPI is used only when present.
    let audibility: bool =
        unsafe { objc2::msg_send![&*wk, respondsToSelector: objc2::sel!(_isPlayingAudio)] };
    if audibility {
        let audible: bool = unsafe { objc2::msg_send![&*wk, _isPlayingAudio] };
        done(Some(audible));
        return true;
    }

    let completion = Rc::new(RefCell::new(Some(done)));
    let callback_completion = completion.clone();
    let weak = objc2::rc::Weak::from_retained(&wk);
    let callback = block2::RcBlock::new(move |state: WKMediaPlaybackState| {
        if let Some(done) = callback_completion.borrow_mut().take() {
            let still_idle = weak.load().is_some_and(|page| unsafe {
                page.cameraCaptureState() == WKMediaCaptureState::None
                    && page.microphoneCaptureState() == WKMediaCaptureState::None
            });
            done(still_idle.then_some(state == WKMediaPlaybackState::Playing));
        }
    });
    unsafe { wk.requestMediaPlaybackStateWithCompletionHandler(&callback) };
    true
}

/// The renderer process footprint, as Activity Monitor reports it. WebKit
/// exposes the process only through SPI; without it no page counts as heavy.
pub(crate) fn page_footprint(view: &wry::WebView) -> Option<u64> {
    let page = webkit(view);
    // SAFETY: main-thread WebKit access; the SPI is used only when present.
    let supported: bool =
        unsafe { objc2::msg_send![&*page, respondsToSelector: objc2::sel!(_webProcessIdentifier)] };
    if !supported {
        return None;
    }
    let pid: libc::pid_t = unsafe { objc2::msg_send![&*page, _webProcessIdentifier] };
    if pid <= 0 {
        return None;
    }
    // SAFETY: a correctly sized, zeroed rusage_info_v4 for this flavor.
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    let status = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V4,
            (&mut info as *mut libc::rusage_info_v4).cast(),
        )
    };
    (status == 0).then_some(info.ri_phys_footprint)
}

/// Hidden pages are throttled by default and suspended once dormant. WebKit
/// keeps a page running while it is audible or capturing, whatever this says,
/// and resumes it as soon as it becomes visible.
pub(crate) fn set_background_suspension(view: &wry::WebView, suspend: bool) {
    use objc2_web_kit::WKInactiveSchedulingPolicy as Policy;
    let target = if suspend {
        Policy::Suspend
    } else {
        Policy::Throttle
    };
    let page = webkit(view);
    // SAFETY: main-thread WebKit access; each view owns its preferences
    // object, so this changes only this page's background scheduling.
    unsafe {
        let preferences = page.configuration().preferences();
        if preferences.inactiveSchedulingPolicy() != target {
            preferences.setInactiveSchedulingPolicy(target);
        }
    }
}

pub(crate) fn native_discard_idle(view: &wry::WebView) -> bool {
    use objc2_web_kit::WKMediaCaptureState;
    let page = webkit(view);
    page.isHiddenOrHasHiddenAncestor()
        && page
            .window()
            .is_none_or(|window| window.attachedSheet().is_none())
        && unsafe {
            !page.isLoading()
                && page.cameraCaptureState() == WKMediaCaptureState::None
                && page.microphoneCaptureState() == WKMediaCaptureState::None
        }
}
