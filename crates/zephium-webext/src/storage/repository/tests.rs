use super::*;
use serde_json::json;

fn installation(id: &str, key: u8) -> Installation {
    Installation {
        extension: ExtensionId::parse(id).unwrap(),
        key: [key; 16],
        unlimited: false,
    }
}
fn set(value: Value) -> Operation {
    Operation::Set {
        items: value.as_object().unwrap().clone(),
    }
}
fn get(repository: &mut Repository, installation: &Installation, area: Area) -> Value {
    repository
        .execute(
            installation,
            area,
            Context::Trusted,
            Operation::Get { keys: None },
        )
        .unwrap()
        .value
}

#[test]
fn shared_installation_reuses_data_and_other_extensions_cannot_read_it() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let two = installation("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 2);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    repository.admit(two.clone());
    repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            set(json!({"theme":"dark"})),
        )
        .unwrap();
    repository.admit(one.clone());
    assert_eq!(
        get(&mut repository, &one, Area::Local),
        json!({"theme":"dark"})
    );
    assert_eq!(get(&mut repository, &two, Area::Local), json!({}));
    assert_eq!(get(&mut repository, &one, Area::Sync), json!({}));
}

#[test]
fn restart_keeps_persistent_areas_and_clears_session_data() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    for area in [Area::Local, Area::Sync, Area::Session] {
        repository
            .execute(&one, area, Context::Trusted, set(json!({"setting": true})))
            .unwrap();
    }
    drop(repository);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    assert_eq!(
        get(&mut repository, &one, Area::Local),
        json!({"setting": true})
    );
    assert_eq!(
        get(&mut repository, &one, Area::Sync),
        json!({"setting": true})
    );
    assert_eq!(get(&mut repository, &one, Area::Session), json!({}));
}

#[test]
fn uninstall_revokes_old_requests_without_erasing_a_new_installation() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut replacement = one.clone();
    replacement.key = [2; 16];
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            set(json!({"old":true})),
        )
        .unwrap();
    repository.admit(replacement.clone());
    repository
        .execute(
            &replacement,
            Area::Local,
            Context::Trusted,
            set(json!({"new":true})),
        )
        .unwrap();
    repository.erase(&one).unwrap();
    assert!(!one.directory(root.path()).exists());
    assert!(repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            set(json!({"resurrect":true}))
        )
        .is_err());
    assert_eq!(
        get(&mut repository, &replacement, Area::Local),
        json!({"new":true})
    );
}

#[test]
fn corrupt_data_is_reported_without_overwriting_or_dispatching_changes() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    let directory = one.directory(root.path());
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("local.json");
    std::fs::write(&path, b"broken").unwrap();
    assert!(repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            set(json!({"replace":true}))
        )
        .is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"broken");
}

#[test]
fn request_cannot_escalate_unlimited_storage_permission() {
    let root = tempfile::tempdir().unwrap();
    let mut one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    one.unlimited = true;
    assert!(repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            set(json!({"oversized": "a".repeat(10 * 1024 * 1024)}))
        )
        .is_err());
    assert_eq!(get(&mut repository, &one, Area::Local), json!({}));
}

#[test]
fn content_scripts_cannot_read_session_data_without_a_trusted_grant() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    repository
        .execute(
            &one,
            Area::Session,
            Context::Trusted,
            set(json!({"secret": true})),
        )
        .unwrap();
    assert!(repository
        .execute(
            &one,
            Area::Session,
            Context::ContentScript,
            Operation::Get { keys: None }
        )
        .is_err());
    assert!(repository
        .execute(
            &one,
            Area::Session,
            Context::ContentScript,
            Operation::SetAccessLevel {
                access_level: AccessLevel::TrustedAndUntrustedContexts
            }
        )
        .is_err());
    repository
        .execute(
            &one,
            Area::Session,
            Context::Trusted,
            Operation::SetAccessLevel {
                access_level: AccessLevel::TrustedAndUntrustedContexts,
            },
        )
        .unwrap();
    assert_eq!(
        repository
            .execute(
                &one,
                Area::Session,
                Context::ContentScript,
                Operation::Get { keys: None }
            )
            .unwrap()
            .value,
        json!({"secret":true})
    );
    assert!(repository.allows_content_scripts(&one, Area::Session));
    repository.revoke(&one);
    repository.admit(one.clone());
    assert!(!repository.allows_content_scripts(&one, Area::Session));
    assert_eq!(get(&mut repository, &one, Area::Session), json!({}));
}

#[test]
fn restricting_local_access_preserves_settings_and_rejects_untrusted_writes() {
    let root = tempfile::tempdir().unwrap();
    let one = installation("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1);
    let mut repository = Repository::new(root.path().to_owned());
    repository.admit(one.clone());
    repository
        .execute(
            &one,
            Area::Local,
            Context::ContentScript,
            set(json!({"theme":"dark"})),
        )
        .unwrap();
    repository
        .execute(
            &one,
            Area::Local,
            Context::Trusted,
            Operation::SetAccessLevel {
                access_level: AccessLevel::TrustedContexts,
            },
        )
        .unwrap();
    assert!(!repository.allows_content_scripts(&one, Area::Local));
    assert!(repository
        .execute(
            &one,
            Area::Local,
            Context::ContentScript,
            set(json!({"theme":"light"}))
        )
        .is_err());
    assert!(repository
        .execute(
            &one,
            Area::Local,
            Context::ContentScript,
            Operation::SetAccessLevel {
                access_level: AccessLevel::TrustedAndUntrustedContexts
            }
        )
        .is_err());
    assert_eq!(
        get(&mut repository, &one, Area::Local),
        json!({"theme":"dark"})
    );
}
