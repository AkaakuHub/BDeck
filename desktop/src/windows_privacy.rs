//! Opens one fixed Windows settings pane; chrome cannot choose a shell URI.
#[cfg(target_os = "windows")]
use super::{emit_to_privileged, MAIN_LABEL};

#[cfg(target_os = "windows")]
static STARTUP_CHOICE: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();

#[cfg(any(target_os = "windows", test))]
fn required(readback: zephium_core::ports::store::AppSettingReadback) -> Option<bool> {
    use zephium_core::ports::store::AppSettingReadback;
    match readback {
        AppSettingReadback::Value(value) => Some(zephium_core::preferences::smartscreen_required(
            Some(&value),
        )),
        AppSettingReadback::Missing => Some(true),
        AppSettingReadback::Unavailable => None,
    }
}

#[cfg(any(target_os = "windows", test))]
fn startup_choice(
    read: impl FnOnce() -> zephium_core::ports::store::AppSettingReadback + Send + 'static,
    budget: std::time::Duration,
) -> Option<bool> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("zephium-reputation-preference".into())
        .spawn(move || {
            let _ = send.send(required(read()));
        })
        .is_err()
    {
        return None;
    }
    // A late OFF reply cannot change the process choice after views are created.
    receive.recv_timeout(budget).ok().flatten()
}

#[cfg(target_os = "windows")]
pub(crate) fn initialize(
    app: &tauri::AppHandle,
    store: std::sync::Arc<dyn zephium_core::ports::store::Store + Send + Sync>,
) {
    let app = app.clone();
    zephium_engine::on_windows_reputation_managed(move || {
        emit_to_privileged(
            &app,
            MAIN_LABEL,
            "zephium:windows-reputation-managed",
            &true,
        );
    });
    let choice = *STARTUP_CHOICE.get_or_init(|| {
        startup_choice(
            move || store.app_setting_readback("privacy.smartscreen"),
            std::time::Duration::from_millis(100),
        )
    });
    if choice.is_none() {
        eprintln!("Windows reputation preference was unavailable at launch; retaining default ON for this launch");
    }
    if !zephium_engine::initialize_windows_reputation_checking(choice.unwrap_or(true)) {
        eprintln!(
            "Windows reputation preference was already initialized; retaining startup choice"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zephium_core::ports::store::AppSettingReadback;
    #[test]
    fn preference_readback_preserves_default_on_and_unavailable_provenance() {
        assert_eq!(required(AppSettingReadback::Unavailable), None);
        assert_eq!(required(AppSettingReadback::Missing), Some(true));
        assert_eq!(
            required(AppSettingReadback::Value("true".into())),
            Some(true)
        );
        assert_eq!(
            required(AppSettingReadback::Value("false".into())),
            Some(false)
        );
    }
    #[test]
    fn unavailable_launch_read_stays_distinct_from_a_missing_preference() {
        let budget = std::time::Duration::from_secs(1);
        let unavailable = startup_choice(|| AppSettingReadback::Unavailable, budget);
        assert_eq!(unavailable, None);
        assert!(unavailable.unwrap_or(true));
        assert_eq!(
            startup_choice(|| AppSettingReadback::Missing, budget),
            Some(true)
        );
    }
    #[test]
    fn slow_preference_read_cannot_disable_the_launch_default_later() {
        let (send, receive) = std::sync::mpsc::channel();
        let choice = startup_choice(
            move || {
                receive.recv().unwrap();
                AppSettingReadback::Value("false".into())
            },
            std::time::Duration::from_millis(5),
        );
        assert_eq!(choice, None);
        assert!(choice.unwrap_or(true));
        send.send(()).unwrap();
        assert_eq!(
            startup_choice(
                || AppSettingReadback::Value("false".into()),
                std::time::Duration::from_secs(1)
            ),
            Some(false)
        );
    }
}
