use super::*;
use serde_json::json;

fn object(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}
fn local() -> Limits {
    Limits::for_area(Area::Local, false)
}

#[test]
fn defaults_preserve_stored_null_and_false_and_do_not_create_keys() {
    let mut items = object(json!({"null": null, "disabled": false}));
    let before = items.clone();
    let reply = apply(
        &mut items,
        Operation::Get {
            keys: Some(Keys::Defaults(object(
                json!({"null": 1, "disabled": true, "missing": 3}),
            ))),
        },
        local(),
    )
    .unwrap();
    assert_eq!(
        reply.value,
        json!({"null": null, "disabled": false, "missing": 3})
    );
    assert!(reply.changes.is_empty());
    assert_eq!(items, before);
}

#[test]
fn mutations_report_exact_changes_including_stored_null_and_ignore_noops() {
    let mut items = object(json!({"same": [1,2], "null": null}));
    let reply = apply(
        &mut items,
        Operation::Set {
            items: object(json!({"same": [1,2], "null": false, "new": null})),
        },
        local(),
    )
    .unwrap();
    assert_eq!(
        Value::Object(reply.changes),
        json!({"null": {"oldValue": null, "newValue": false}, "new": {"newValue": null}})
    );
    let reply = apply(
        &mut items,
        Operation::Remove {
            keys: RemoveKeys::Many(vec!["new".into(), "new".into(), "absent".into()]),
        },
        local(),
    )
    .unwrap();
    assert_eq!(
        Value::Object(reply.changes),
        json!({"new": {"oldValue": null}})
    );
    let reply = apply(&mut items, Operation::Clear, local()).unwrap();
    assert_eq!(
        Value::Object(reply.changes),
        json!({"same": {"oldValue": [1,2]}, "null": {"oldValue": false}})
    );
    assert!(items.is_empty());
    assert!(apply(&mut items, Operation::Clear, local())
        .unwrap()
        .changes
        .is_empty());
}

#[test]
fn quota_rejection_is_atomic_and_removal_releases_capacity() {
    let limits = Limits {
        bytes: Some(8),
        item_bytes: Some(5),
        items: Some(2),
    };
    let mut items = object(json!({"a": 1, "b": 2}));
    let before = items.clone();
    for next in [
        json!({"c": 3}),
        json!({"a": "long"}),
        json!({"a": "12", "b": "12"}),
    ] {
        assert!(apply(
            &mut items,
            Operation::Set {
                items: object(next)
            },
            limits
        )
        .is_err());
        assert_eq!(items, before);
    }
    apply(
        &mut items,
        Operation::Remove {
            keys: RemoveKeys::One("a".into()),
        },
        limits,
    )
    .unwrap();
    apply(
        &mut items,
        Operation::Set {
            items: object(json!({"c": 3})),
        },
        limits,
    )
    .unwrap();
    assert_eq!(items, object(json!({"b": 2, "c": 3})));
}

#[test]
fn serialized_size_counts_utf8_keys_and_values_and_only_requested_keys() {
    let mut items = object(json!({"日": "本", "other": true}));
    assert_eq!(
        apply(
            &mut items,
            Operation::GetBytesInUse {
                keys: Some(Keys::One("日".into()))
            },
            local()
        )
        .unwrap()
        .value,
        json!(8)
    );
    assert_eq!(
        apply(
            &mut items,
            Operation::GetBytesInUse {
                keys: Some(Keys::Many(vec!["日".into(), "日".into(), "missing".into()]))
            },
            local()
        )
        .unwrap()
        .value,
        json!(8)
    );
    assert_eq!(
        apply(&mut items, Operation::GetBytesInUse { keys: None }, local())
            .unwrap()
            .value,
        json!(17)
    );
}

#[test]
fn sync_limits_apply_even_with_unlimited_local_permission() {
    let mut items = Map::new();
    let large = object(json!({"a": "x".repeat(8192)}));
    assert!(apply(
        &mut items,
        Operation::Set {
            items: large.clone()
        },
        Limits::for_area(Area::Sync, true)
    )
    .is_err());
    assert!(items.is_empty());
    apply(
        &mut items,
        Operation::Set { items: large },
        Limits::for_area(Area::Local, true),
    )
    .unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn wire_commands_accept_null_and_empty_key_arrays_but_reject_invalid_mutations() {
    let mut items = object(json!({"a": 1}));
    let command = serde_json::from_value(json!({"method": "get", "keys": null})).unwrap();
    assert_eq!(
        apply(&mut items, command, local()).unwrap().value,
        json!({"a": 1})
    );
    let command = serde_json::from_value(json!({"method": "get", "keys": []})).unwrap();
    assert_eq!(
        apply(&mut items, command, local()).unwrap().value,
        json!({})
    );
    assert!(
        serde_json::from_value::<Operation>(json!({"method": "remove", "keys": null})).is_err()
    );
    assert!(serde_json::from_value::<Operation>(json!({"method": "set", "items": []})).is_err());
}
