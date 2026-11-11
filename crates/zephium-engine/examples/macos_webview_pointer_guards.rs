//! Checks that parking WebKit tracking areas preserves AppKit tool tips.
#[cfg(target_os = "macos")]
objc2::define_class!(
    /// Reports itself key without the app ever activating: WebKit only
    /// tracks tool tips for a page in the key window.
    #[unsafe(super(objc2_app_kit::NSWindow, objc2_app_kit::NSResponder, objc2::runtime::NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "ZephiumPointerGuardsWindow"]
    struct KeyWindow;

    impl KeyWindow {
        #[unsafe(method(isKeyWindow))]
        fn is_key_window(&self) -> bool {
            true
        }
    }
);

#[cfg(target_os = "macos")]
fn main() {
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{msg_send, sel};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEvent,
        NSEventMask, NSEventModifierFlags, NSEventType, NSTrackingAreaOptions, NSWindow,
        NSWindowStyleMask,
    };
    use objc2_foundation::{
        MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize,
    };
    use raw_window_handle::{
        AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
    };
    use std::ptr::NonNull;
    use wry::{
        dpi::{LogicalPosition, LogicalSize},
        Rect, WebViewBuilder, WebViewExtMacOS, WryWebView,
    };

    struct Host(NonNull<std::ffi::c_void>);
    impl HasWindowHandle for Host {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let handle = AppKitWindowHandle::new(self.0);
            // SAFETY: the content view outlives every use of this handle.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(handle)) })
        }
    }

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();
    let pump = |secs: f64| {
        let until = NSDate::dateWithTimeIntervalSinceNow(secs);
        // SAFETY: plain event pump on the main thread.
        while let Some(event) = unsafe {
            app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask::Any,
                Some(&until),
                NSDefaultRunLoopMode,
                true,
            )
        } {
            autoreleasepool(|_| app.sendEvent(&event));
            if until.timeIntervalSinceNow() <= 0.0 {
                break;
            }
        }
    };
    let size = NSSize::new(600.0, 400.0);
    let frame = NSRect::new(
        NSPoint::new(-30_000.0, -30_000.0),
        size,
    );
    // SAFETY: NSWindow's designated initialiser on a plain subclass.
    let window: Retained<KeyWindow> = unsafe {
        msg_send![
            mtm.alloc::<KeyWindow>(),
            initWithContentRect: frame,
            styleMask: NSWindowStyleMask::Titled,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    let window: Retained<NSWindow> = window.into_super();
    // SAFETY: the window is released by the last `Retained`, not by `close`.
    unsafe { window.setReleasedWhenClosed(false) };
    let content = window.contentView().expect("content view");
    let host = Host(NonNull::new(Retained::as_ptr(&content) as *mut _).expect("view"));
    let build = || {
        WebViewBuilder::new()
            .with_html(
                "<body style='margin:0'>\
                 <div title='left' style='position:fixed;inset:0 50% 0 0'></div>\
                 <div title='right' style='position:fixed;inset:0 0 0 50%'></div></body>",
            )
            .with_bounds(Rect {
                position: LogicalPosition::new(0.0, 0.0).into(),
                size: LogicalSize::new(size.width, size.height).into(),
            })
            .build_as_child(&host)
            .expect("webview")
    };
    let view = build();
    // WebKit must treat the offscreen page as shown to install tool tips.
    window.setFrameOrigin(NSPoint::new(-30_000.0, -30_000.0));
    window.orderBack(None);
    pump(1.0);

    let mut failures = Vec::new();
    let mut check = |name: &str, ok: bool| {
        println!("{} {name}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            failures.push(name.to_owned());
        }
    };

    let page = view.webview();
    let tips = |page: &WryWebView| {
        page.trackingAreas()
            .iter()
            .filter(|area| {
                // SAFETY: an installed area's owner outlives it.
                let owner: Option<Retained<AnyObject>> = unsafe { msg_send![&**area, owner] };
                owner.is_some_and(|owner| owner.class().name().to_bytes() == b"NSToolTipManager")
            })
            .map(|area| Retained::as_ptr(&area).addr())
            .collect::<Vec<_>>()
    };
    let moving = |page: &WryWebView| {
        page.trackingAreas()
            .iter()
            .filter(|area| area.options().contains(NSTrackingAreaOptions::MouseMoved))
            .count()
    };
    // WebKit's own test hook: a mouse move straight into the page, so WebKit
    // replaces its tool tip exactly as it does under a real pointer.
    let hover = |page: &WryWebView, x: f64| {
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            NSEventType::MouseMoved,
            NSPoint::new(x, size.height / 2.0),
            NSEventModifierFlags::empty(),
            0.0,
            window.windowNumber(),
            None,
            0,
            0,
            0.0,
        )
        .expect("event");
        // SAFETY: WebKit SPI taking one mouse-moved event; checked below.
        let _: () = unsafe { msg_send![page, _simulateMouseMove: &*event] };
    };
    // SAFETY: plain selector query.
    let can_hover: bool =
        unsafe { msg_send![&*page, respondsToSelector: sel!(_simulateMouseMove:)] };
    check("WebKit can simulate a mouse move", can_hover);
    let hover_until = |page: &WryWebView, x: f64, done: &dyn Fn(&[usize]) -> bool| {
        for _ in 0..20 {
            hover(page, x);
            pump(0.25);
            if done(&tips(page)) {
                return true;
            }
        }
        false
    };
    let live_moving = moving(&page);
    check("page has WebKit mouse areas", live_moving > 0);
    check(
        "hovering a title installs a tool tip",
        hover_until(&page, size.width / 4.0, &|tips| tips.len() == 1),
    );
    let first = tips(&page);
    page.set_tracking_parked(true);
    check("parked page holds no mouse-moved areas", moving(&page) == 0);
    check("parked page keeps its tool tip area", tips(&page) == first);
    check(
        "WebKit replaces the tool tip while parked",
        hover_until(&page, size.width * 0.75, &|tips| {
            tips.len() == 1 && tips != first
        }),
    );
    pump(0.2);
    page.set_tracking_parked(false);
    check(
        "released page holds only its current tool tip area",
        tips(&page).len() == 1 && tips(&page) != first,
    );
    check(
        "released page has its mouse areas back",
        moving(&page) == live_moving,
    );
    page.setFrameSize(NSSize::new(size.width - 40.0, size.height));
    pump(0.2);
    page.setFrameSize(size);
    pump(0.2);

    drop(view);
    window.close();
    if !failures.is_empty() {
        eprintln!("failed: {failures:?}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
