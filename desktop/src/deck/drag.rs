use tauri::{Manager, State, WebviewWindow};
use zephium_app::{Command, Handle};
use zephium_core::geometry::Rect;

#[tauri::command]
#[specta::specta]
pub(crate) fn deck_drag_preview(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    preview: Option<[f64; 4]>,
) -> bool {
    if !crate::authorize(&caller, crate::CallerPolicy::Main, "deck_drag_preview")
        || crate::shutdown_started(caller.app_handle()) {
        return false;
    }
    let size = crate::inner_logical(&caller);
    let rect = match preview {
        Some([x, y, width, height]) if [x, y, width, height].iter().all(|value| value.is_finite())
            && x >= 56.0 && y >= 48.0 && width > 0.0 && height > 0.0
            && x + width <= size.width && y + height <= size.height => Some(Rect::new(x, y, width, height)),
        Some(_) => return false,
        None => None,
    };
    shell.dispatch(Command::DeckDragPreview(rect))
}
