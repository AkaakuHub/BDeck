//! Composition-root ownership for privileged WebView2 runtime generations.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use zephium_core::webview2::{RuntimeGeneration, RuntimeGenerationKind};

static RUNTIME_STATE: OnceLock<RuntimeGeneration> = OnceLock::new();

#[derive(Debug)]
pub struct PreparedDirectories {
    pub main: PathBuf,
    pub column: PathBuf,
}

pub fn prepare(
    data_dir: &Path,
    main_label: &str,
    column_label: &str,
) -> io::Result<PreparedDirectories> {
    if main_label.is_empty() || column_label.is_empty() || main_label == column_label {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "privileged WebView label must not be empty",
        ));
    }
    let runtime = RuntimeGeneration::prepare(
        &data_dir.join("privileged-runtime"),
        RuntimeGenerationKind::Privileged,
    )?;
    let main = runtime.root().join(main_label);
    fs::create_dir(&main)?;
    let main = zephium_core::webview2::canonical_user_data_directory(&main)?;
    if main.parent() != Some(runtime.root()) {
        return Err(io::Error::other(
            "privileged WebView2 UDF escaped its owned runtime generation",
        ));
    }
    let column = runtime.root().join(column_label);
    fs::create_dir(&column)?;
    let column = zephium_core::webview2::canonical_user_data_directory(&column)?;
    if column.parent() != Some(runtime.root()) {
        return Err(io::Error::other(
            "privileged WebView2 UDF escaped its owned runtime generation",
        ));
    }
    RUNTIME_STATE
        .set(runtime)
        .map_err(|_| io::Error::other("privileged WebView2 runtime initialized twice"))?;
    Ok(PreparedDirectories { main, column })
}

/// Delete this run's generation only after the composition root has proven
/// the privileged environment exited.
pub fn cleanup_current_after_proven_exit() -> bool {
    let Some(runtime) = RUNTIME_STATE.get() else {
        return true;
    };
    let cleanup = runtime.cleanup_ticket();
    match cleanup.cleanup_after_proven_exit() {
        Ok(()) => true,
        Err(error) => {
            crate::write_diagnostic(format_args!(
                "privacy: could not remove proven-exited privileged WebView2 runtime data at {}: {error}",
                cleanup.root().display()
            ));
            false
        }
    }
}
