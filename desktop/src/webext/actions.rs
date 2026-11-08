use tauri::{Manager, State, WebviewWindow};
use zephium_app::{Command, Handle};
use zephium_core::ids::{ProfileId, ExtensionInstallId};
use zephium_core::extensions::{ExtensionRuntimeGeneration, ExtensionActionRevision, ExtensionRuntimeInstance, ExtensionPopupAnchor};
use zephium_core::geometry::Rect;
use crate::{authorize, shutdown_started, bounded, rejected_operation, dispatch_operation, CallerPolicy};
const MAX_WINDOW_COORDINATE: f64 = 1_000_000.0;

#[tauri::command]
#[specta::specta]
pub(crate) fn extension_action_invoke(
    caller: WebviewWindow,
    shell: State<'_, Handle>,
    profile_id: String,
    install_id: String,
    runtime_generation: String,
    action_revision: String,
    anchor_x: f64,
    anchor_y: f64,
    anchor_width: f64,
    anchor_height: f64,
) -> zephium_ipc::OperationAdmission {
    if !authorize(&caller, CallerPolicy::Main, "extension_action_invoke")
        || shutdown_started(caller.app_handle())
        || !bounded(&profile_id, 128)
        || !bounded(&install_id, 128)
    {
        return rejected_extension_action("caller-or-identity-boundary");
    }
    let Some(profile) =
        ProfileId::parse(&profile_id).filter(|profile| profile.to_string() == profile_id)
    else {
        return rejected_extension_action("profile-identity");
    };
    let Some(install) =
        ExtensionInstallId::parse(&install_id).filter(|install| install.to_string() == install_id)
    else {
        return rejected_extension_action("install-identity");
    };
    let Some(generation) =
        fixed_nonzero_hex(&runtime_generation).and_then(ExtensionRuntimeGeneration::new)
    else {
        return rejected_extension_action("runtime-generation");
    };
    let Some(revision) = fixed_nonzero_hex(&action_revision).and_then(ExtensionActionRevision::new)
    else {
        return rejected_extension_action("action-revision");
    };
    let window = crate::inner_logical(&caller);
    let Some(anchor) = extension_popup_anchor_in_bounds(
        anchor_x,
        anchor_y,
        anchor_width,
        anchor_height,
        window.width,
        window.height,
    ) else {
        return rejected_extension_action("window-anchor-bounds");
    };
    let admission = dispatch_operation(
        caller.app_handle(),
        &shell,
        Command::InvokeExtensionAction {
            runtime: ExtensionRuntimeInstance::new(profile, install, generation),
            revision,
            anchor,
        },
    );
    if !admission.accepted {
        crate::write_diagnostic(format_args!("extensions: toolbar action IPC rejected at actor-queue-admission"));
    }
    admission
}

fn rejected_extension_action(stage: &'static str) -> zephium_ipc::OperationAdmission {
    crate::write_diagnostic(format_args!("extensions: toolbar action IPC rejected at {stage}"));
    rejected_operation()
}

fn fixed_nonzero_hex(value: &str) -> Option<u64> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let parsed = u64::from_str_radix(value, 16).ok()?;
    (parsed != 0).then_some(parsed)
}

fn extension_popup_anchor_in_bounds(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    window_width: f64,
    window_height: f64,
) -> Option<ExtensionPopupAnchor> {
    let values = [x, y, width, height, window_width, window_height];
    if values.iter().any(|value| !value.is_finite())
        || x < 0.0
        || y < 0.0
        || width <= 0.0
        || height <= 0.0
        || window_width <= 0.0
        || window_height <= 0.0
        || window_width > MAX_WINDOW_COORDINATE
        || window_height > MAX_WINDOW_COORDINATE
    {
        return None;
    }
    let right = x + width;
    let bottom = y + height;
    if !right.is_finite() || !bottom.is_finite() || right > window_width || bottom > window_height {
        return None;
    }
    ExtensionPopupAnchor::new(Rect::new(x, y, width, height)).ok()
}
