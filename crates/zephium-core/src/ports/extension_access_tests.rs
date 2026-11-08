use super::WebExtensionLoad;
use crate::ids::ExtensionInstallId;

fn installed() -> WebExtensionLoad {
    WebExtensionLoad {
        install: ExtensionInstallId::from(1),
        extension_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        root: "packages/current".into(),
        permissions: vec!["storage".into()],
        external_native_messaging: false,
        match_patterns: vec!["https://example.com/*".into()],
        start_background: false,
    }
}

#[test]
fn permission_and_site_additions_preserve_the_running_package() {
    let previous = installed();
    let mut wanted = previous.clone();
    wanted.permissions.push("alarms".into());
    assert!(wanted.extends_access_of(&previous));
    wanted = previous.clone();
    wanted.match_patterns.push("https://other.example/*".into());
    assert!(wanted.extends_access_of(&previous));
}

#[test]
fn removing_any_grant_requires_a_new_runtime_even_when_another_is_added() {
    let previous = installed();
    let mut wanted = previous.clone();
    wanted.permissions = vec!["alarms".into()];
    assert!(!wanted.extends_access_of(&previous));
    wanted = previous.clone();
    wanted.match_patterns = vec!["https://other.example/*".into()];
    assert!(!wanted.extends_access_of(&previous));
}

#[test]
fn retries_and_changed_package_or_native_authority_require_a_new_runtime() {
    let previous = installed();
    assert!(!previous.extends_access_of(&previous));
    for variant in 0..4 {
        let mut wanted = previous.clone();
        wanted.permissions.push("alarms".into());
        match variant {
            0 => wanted.install = ExtensionInstallId::from(2),
            1 => wanted.extension_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            2 => wanted.root = "packages/replacement".into(),
            _ => wanted.external_native_messaging = true,
        }
        assert!(!wanted.extends_access_of(&previous));
    }
}
