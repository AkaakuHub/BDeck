use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[cfg(test)]
mod tests;

pub const BRIDGE_SCRIPT: &str = include_str!("bridge.js");

pub mod broker;
pub mod repository;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Area {
    Local,
    Sync,
    Session,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AccessLevel {
    TrustedContexts,
    TrustedAndUntrustedContexts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Context {
    Trusted,
    ContentScript,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Keys {
    One(String),
    Many(Vec<String>),
    Defaults(Map<String, Value>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RemoveKeys {
    One(String),
    Many(Vec<String>),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Get {
        keys: Option<Keys>,
    },
    GetKeys,
    SetAccessLevel {
        #[serde(rename = "accessLevel")]
        access_level: AccessLevel,
    },
    GetBytesInUse {
        keys: Option<Keys>,
    },
    Set {
        items: Map<String, Value>,
    },
    Remove {
        keys: RemoveKeys,
    },
    Clear,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<Value>,
}

#[derive(Debug, PartialEq)]
pub struct Reply {
    pub value: Value,
    pub changes: Map<String, Value>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: Option<usize>,
    pub item_bytes: Option<usize>,
    pub items: Option<usize>,
}

impl Limits {
    pub fn for_area(area: Area, unlimited: bool) -> Self {
        match area {
            Area::Sync => Self {
                bytes: Some(102_400),
                item_bytes: Some(8192),
                items: Some(512),
            },
            Area::Local if unlimited => Self {
                bytes: None,
                item_bytes: None,
                items: None,
            },
            Area::Local | Area::Session => Self {
                bytes: Some(10 * 1024 * 1024),
                item_bytes: None,
                items: None,
            },
        }
    }

    fn validate(self, items: &Map<String, Value>) -> Result<(), String> {
        if self.items.is_some_and(|limit| items.len() > limit) {
            return Err("MAX_ITEMS quota exceeded".into());
        }
        let mut total = 0usize;
        for (key, value) in items {
            let bytes = item_bytes(key, value);
            if self.item_bytes.is_some_and(|limit| bytes > limit) {
                return Err("QUOTA_BYTES_PER_ITEM quota exceeded".into());
            }
            total = total.saturating_add(bytes);
        }
        if self.bytes.is_some_and(|limit| total > limit) {
            return Err("QUOTA_BYTES quota exceeded".into());
        }
        Ok(())
    }
}

pub fn apply(
    items: &mut Map<String, Value>,
    operation: Operation,
    limits: Limits,
) -> Result<Reply, String> {
    let empty = || Map::new();
    match operation {
        Operation::SetAccessLevel { .. } => {
            Err("Access levels are owned by the extension storage repository.".into())
        }
        Operation::Get { keys } => Ok(Reply {
            value: Value::Object(select(items, keys)),
            changes: empty(),
        }),
        Operation::GetKeys => Ok(Reply {
            value: Value::Array(items.keys().cloned().map(Value::String).collect()),
            changes: empty(),
        }),
        Operation::GetBytesInUse { keys } => {
            let selected = match keys {
                Some(Keys::Defaults(_)) => {
                    return Err("getBytesInUse accepts a key, key array or null".into())
                }
                keys => select(items, keys),
            };
            let bytes: usize = selected
                .iter()
                .map(|(key, value)| item_bytes(key, value))
                .sum();
            Ok(Reply {
                value: Value::from(bytes),
                changes: empty(),
            })
        }
        mutation => {
            let mut next = items.clone();
            match mutation {
                Operation::Set { items } => {
                    next.extend(items);
                    limits.validate(&next)?;
                }
                Operation::Remove { keys } => {
                    let keys = match keys {
                        RemoveKeys::One(key) => vec![key],
                        RemoveKeys::Many(keys) => keys,
                    };
                    for key in keys {
                        next.remove(&key);
                    }
                }
                Operation::Clear => next.clear(),
                _ => unreachable!(),
            }
            let mut changes = Map::new();
            for key in items.keys().chain(next.keys()) {
                let old = items.get(key);
                let new = next.get(key);
                if old != new {
                    let change = Change {
                        old_value: old.cloned(),
                        new_value: new.cloned(),
                    };
                    changes.insert(
                        key.clone(),
                        serde_json::to_value(change).map_err(|error| error.to_string())?,
                    );
                }
            }
            *items = next;
            Ok(Reply {
                value: Value::Null,
                changes,
            })
        }
    }
}

fn select(items: &Map<String, Value>, keys: Option<Keys>) -> Map<String, Value> {
    match keys {
        None => items.clone(),
        Some(Keys::One(key)) => items
            .get(&key)
            .map(|value| Map::from_iter([(key, value.clone())]))
            .unwrap_or_default(),
        Some(Keys::Many(keys)) => keys
            .into_iter()
            .filter_map(|key| items.get(&key).map(|value| (key.clone(), value.clone())))
            .collect(),
        Some(Keys::Defaults(defaults)) => defaults
            .into_iter()
            .map(|(key, default)| {
                let value = items.get(&key).cloned().unwrap_or(default);
                (key, value)
            })
            .collect(),
    }
}

fn item_bytes(key: &str, value: &Value) -> usize {
    key.len().saturating_add(value.to_string().len())
}
