use objc2::runtime::NSObjectProtocol;
use objc2::{msg_send, sel};
use objc2_web_kit::WKWebView;

pub(crate) fn enable_scrolling(view: &WKWebView) {
    if !view.respondsToSelector(sel!(_setMinimumLayoutWidth:))
        || !view.respondsToSelector(sel!(_setClipsToVisibleRect:))
    {
        return;
    }
    // WebKit bug 320810: popup autosizing disables main-frame scrolling.
    // Clear the minimum layout width to re-enable it, retaining autosizing and clipping.
    unsafe {
        let _: () = msg_send![view, _setMinimumLayoutWidth: 0.0_f64];
        let _: () = msg_send![view, _setClipsToVisibleRect: true];
    }
}
