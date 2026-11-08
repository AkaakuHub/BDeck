use super::*;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::Duration;
use zephium_webext::storage::{Area, Context, Operation};

fn load(version: u8) -> WebExtensionLoad {
    WebExtensionLoad {
        install: ExtensionInstallId::from(1),
        extension_id: "a".repeat(32),
        root: format!("/package/{version}").into(),
        permissions: vec!["storage".into()],
        external_native_messaging: false,
        match_patterns: vec![],
        start_background: true,
    }
}
fn execute(client: &Client, area: Area, operation: Operation) -> Result<Value, String> {
    let (sender, receiver) = mpsc::channel();
    client
        .broker
        .execute(
            client.installation.clone(),
            area,
            Context::Trusted,
            operation,
            Box::new(move |result| {
                sender.send(result).unwrap();
            }),
        )
        .unwrap();
    receiver.recv_timeout(Duration::from_secs(3)).unwrap()
}
fn set(value: Value) -> Operation {
    Operation::Set {
        items: value.as_object().unwrap().clone(),
    }
}
fn get(client: &Client, area: Area) -> Value {
    execute(client, area, Operation::Get { keys: None }).unwrap()
}

#[test]
fn deleting_one_account_keeps_other_accounts_settings_and_active_session() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::new(root.path().to_owned(), Arc::new(|_| {}));
    let one = storage.load(ProfileId::from(1), &load(1)).unwrap();
    let two = storage.load(ProfileId::from(2), &load(1)).unwrap();
    execute(&one, Area::Local, set(json!({"theme":"dark"}))).unwrap();
    execute(&one, Area::Session, set(json!({"transient":true}))).unwrap();
    storage.retire_profile(ProfileId::from(1));
    assert_eq!(get(&two, Area::Local), json!({"theme":"dark"}));
    assert_eq!(get(&two, Area::Session), json!({"transient":true}));
    storage.retire_profile(ProfileId::from(2));
    let three = storage.load(ProfileId::from(3), &load(1)).unwrap();
    assert_eq!(get(&three, Area::Local), json!({"theme":"dark"}));
    assert_eq!(get(&three, Area::Session), json!({}));
}

#[test]
fn package_replacement_clears_session_once_and_preserves_shared_local_settings() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::new(root.path().to_owned(), Arc::new(|_| {}));
    let one = storage.load(ProfileId::from(1), &load(1)).unwrap();
    storage.load(ProfileId::from(2), &load(1)).unwrap();
    execute(&one, Area::Local, set(json!({"setting":true}))).unwrap();
    execute(&one, Area::Session, set(json!({"old":true}))).unwrap();
    let one = storage.load(ProfileId::from(1), &load(2)).unwrap();
    assert_eq!(get(&one, Area::Local), json!({"setting":true}));
    assert_eq!(get(&one, Area::Session), json!({}));
    execute(&one, Area::Session, set(json!({"new":true}))).unwrap();
    let two = storage.load(ProfileId::from(2), &load(2)).unwrap();
    assert_eq!(get(&two, Area::Session), json!({"new":true}));
}

#[test]
fn uninstall_cannot_erase_or_authorize_a_replacement_installation() {
    let root = tempfile::tempdir().unwrap();
    let mut storage = Storage::new(root.path().to_owned(), Arc::new(|_| {}));
    let old = load(1);
    let old_client = storage.load(ProfileId::from(1), &old).unwrap();
    execute(&old_client, Area::Local, set(json!({"old":true}))).unwrap();
    let mut replacement = load(2);
    replacement.install = ExtensionInstallId::from(2);
    let new = storage.load(ProfileId::from(1), &replacement).unwrap();
    execute(&new, Area::Local, set(json!({"new":true}))).unwrap();
    storage.erase(&old).unwrap();
    assert!(execute(&old_client, Area::Local, set(json!({"resurrect":true}))).is_err());
    assert_eq!(get(&new, Area::Local), json!({"new":true}));
    assert_eq!(
        std::fs::read_dir(root.path().join("a".repeat(32)))
            .unwrap()
            .count(),
        1
    );
}
