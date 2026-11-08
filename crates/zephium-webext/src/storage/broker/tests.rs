use super::*;
use crate::ExtensionId;
use serde_json::json;
use std::time::Duration;

fn installation() -> Installation {
    Installation {
        extension: ExtensionId::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
        key: [1; 16],
        unlimited: false,
    }
}
fn operation(value: Value) -> Operation {
    Operation::Set {
        items: value.as_object().unwrap().clone(),
    }
}
fn execute(broker: &Broker, operation: Operation) -> Result<Value, String> {
    let (sender, receiver) = mpsc::channel();
    broker
        .execute(
            installation(),
            Area::Local,
            Context::Trusted,
            operation,
            Box::new(move |result| {
                sender.send(result).unwrap();
            }),
        )
        .unwrap();
    receiver.recv_timeout(Duration::from_secs(3)).unwrap()
}

#[test]
fn all_clients_observe_one_serialized_value_and_one_change_event_per_mutation() {
    let root = tempfile::tempdir().unwrap();
    let (events, received) = mpsc::channel();
    let broker = Broker::start(root.path().to_owned(), move |event| {
        events.send(event).unwrap();
    })
    .unwrap();
    broker.admit(installation()).unwrap();
    let other_profile = broker.clone();
    execute(&broker, operation(json!({"theme":"dark"}))).unwrap();
    assert_eq!(
        execute(&other_profile, Operation::Get { keys: None }).unwrap(),
        json!({"theme":"dark"})
    );
    let event = received.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(event.installation, installation());
    assert_eq!(event.area, Area::Local);
    assert_eq!(
        Value::Object(event.changes),
        json!({"theme":{"newValue":"dark"}})
    );
    execute(&other_profile, operation(json!({"theme":"dark"}))).unwrap();
    assert!(received.try_recv().is_err());
}

#[test]
fn uninstall_is_ordered_after_pending_writes_and_old_clients_cannot_resurrect_data() {
    let root = tempfile::tempdir().unwrap();
    let broker = Broker::start(root.path().to_owned(), |_| {}).unwrap();
    broker.admit(installation()).unwrap();
    execute(&broker, operation(json!({"setting":true}))).unwrap();
    let (sender, receiver) = mpsc::channel();
    broker
        .erase(
            installation(),
            Box::new(move |result| {
                sender.send(result).unwrap();
            }),
        )
        .unwrap();
    receiver
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert!(execute(&broker, operation(json!({"resurrect":true}))).is_err());
    assert!(!root.path().join(installation().extension.as_str()).exists());
}

#[test]
fn a_full_data_allowance_rejects_more_data_without_blocking_lifecycle_revocation() {
    let root = tempfile::tempdir().unwrap();
    let (entered, observed) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let broker = Broker::start(root.path().to_owned(), move |_| {
        entered.send(()).unwrap();
        resume.recv_timeout(Duration::from_secs(3)).unwrap();
    })
    .unwrap();
    broker.admit(installation()).unwrap();
    broker
        .execute(
            installation(),
            Area::Local,
            Context::Trusted,
            operation(json!({"a":1})),
            Box::new(|_| {}),
        )
        .unwrap();
    observed.recv_timeout(Duration::from_secs(3)).unwrap();
    let (completed, drained) = mpsc::channel();
    for _ in 0..(MAX_PENDING - 1) {
        let completed = completed.clone();
        broker
            .execute(
                installation(),
                Area::Local,
                Context::Trusted,
                Operation::Get { keys: None },
                Box::new(move |result| {
                    completed.send(result).unwrap();
                }),
            )
            .unwrap();
    }
    assert!(broker
        .execute(
            installation(),
            Area::Local,
            Context::Trusted,
            Operation::Get { keys: None },
            Box::new(|_| panic!("rejected operation ran"))
        )
        .is_err());
    broker.revoke(installation()).unwrap();
    release.send(()).unwrap();
    for _ in 0..(MAX_PENDING - 1) {
        drained
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .unwrap();
    }
    assert!(execute(&broker, Operation::Get { keys: None }).is_err());
}
