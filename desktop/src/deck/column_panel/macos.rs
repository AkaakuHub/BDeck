use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior, NSWindowOrderingMode};
use objc2_foundation::MainThreadMarker;
use tauri::WebviewWindow;

pub(super) fn attach(main: &WebviewWindow, panel: &WebviewWindow) -> Result<(), String> {
    let _marker = MainThreadMarker::new().ok_or("panel attachment requires the main thread")?;
    let parent = main.ns_window().map_err(|error| error.to_string())?;
    let child = panel.ns_window().map_err(|error| error.to_string())?;
    // SAFETY: Tauri owns these live NSWindows throughout this main-thread call.
    let (parent, child) = unsafe { (&*parent.cast::<NSWindow>(), &*child.cast::<NSWindow>()) };
    child.setExcludedFromWindowsMenu(true);
    child.setMovable(false);
    child.setCollectionBehavior(
        NSWindowCollectionBehavior::Auxiliary
            | NSWindowCollectionBehavior::Transient
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    if let Some(current) = child.parentWindow() {
        if !std::ptr::eq(&*current, parent) {
            return Err("panel belongs to another window".into());
        }
    } else {
        // orderOut detaches child windows, so every presentation renews this relationship.
        // SAFETY: both windows are live on the main thread and the child has no parent.
        unsafe {
            parent.addChildWindow_ordered(child, NSWindowOrderingMode::Above);
        }
    }
    if !child
        .parentWindow()
        .is_some_and(|current| std::ptr::eq(&*current, parent))
    {
        return Err("panel attachment failed".into());
    }
    Ok(())
}
