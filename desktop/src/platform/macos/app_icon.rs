use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;

pub(super) fn install() -> bool {
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let data = NSData::with_bytes(include_bytes!("../../../icons/icon.png"));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
        return false;
    };
    unsafe {
        NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
    }
    true
}
