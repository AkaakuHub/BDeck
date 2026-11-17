use rusqlite::{OptionalExtension, Transaction};
use serde_json::Value;

use crate::hub::MAX_SETTING_VALUE_BYTES;

pub(super) struct SettingMigration {
    key: &'static str,
    transform: fn(&mut Value) -> Result<(), &'static str>,
}

pub(super) fn apply(tx: &Transaction<'_>, migration: SettingMigration) -> rusqlite::Result<()> {
    let stored: Option<Option<String>> = tx.query_row(
        "SELECT CASE WHEN length(CAST(value AS BLOB)) <= ?2 THEN value END FROM settings WHERE key = ?1",
        rusqlite::params![migration.key, MAX_SETTING_VALUE_BYTES],
        |row| row.get(0),
    ).optional()?;
    let Some(stored) = stored else {
        return Ok(());
    };
    let invalid = |reason: &str| {
        rusqlite::Error::InvalidParameterName(format!(
            "setting migration {}: {reason}",
            migration.key
        ))
    };
    let stored = stored.ok_or_else(|| invalid("value exceeds persistence limit"))?;
    let mut value: Value = serde_json::from_str(&stored).map_err(|_| invalid("invalid JSON"))?;
    (migration.transform)(&mut value).map_err(invalid)?;
    let encoded = serde_json::to_string(&value).map_err(|_| invalid("encoding failed"))?;
    if encoded.len() > MAX_SETTING_VALUE_BYTES {
        return Err(invalid("result exceeds persistence limit"));
    }
    tx.execute(
        "UPDATE settings SET value = ?2 WHERE key = ?1",
        rusqlite::params![migration.key, encoded],
    )?;
    Ok(())
}

pub(super) const DECK_COLUMN_SECRET: SettingMigration = SettingMigration {
    key: "deck.state",
    transform: |value| {
        let decks = value
            .get_mut("decks")
            .and_then(Value::as_array_mut)
            .ok_or("invalid decks")?;
        for deck in decks {
            let columns = deck
                .get_mut("columns")
                .and_then(Value::as_array_mut)
                .ok_or("invalid columns")?;
            for column in columns {
                let column = column.as_object_mut().ok_or("invalid column")?;
                column.entry("secret").or_insert(Value::Bool(false));
            }
        }
        Ok(())
    },
};

#[cfg(test)]
mod tests {
    use crate::migrations::{apply, META};
    use rusqlite::Connection;
    use serde_json::{json, Value};

    #[test]
    fn upgrades_all_decks_and_preserves_existing_column_choices() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &META[..30]).unwrap();
        let original = json!({"accounts": [], "active": "a", "decks": [
            {"id": "a", "columns": [{"id": "one", "suspended": true}, {"id": "two", "secret": true}]},
            {"id": "b", "columns": [{"id": "three", "secret": false}]}
        ]});
        conn.execute(
            "INSERT INTO settings VALUES ('deck.state', ?1)",
            [original.to_string()],
        )
        .unwrap();
        apply(&mut conn, META).unwrap();
        let read = || -> Value {
            let encoded: String = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='deck.state'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            serde_json::from_str(&encoded).unwrap()
        };
        let mut expected = original;
        expected["decks"][0]["columns"][0]["secret"] = json!(false);
        assert_eq!(read(), expected);
        apply(&mut conn, META).unwrap();
        let encoded: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key='deck.state'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&encoded).unwrap(), expected);
    }

    #[test]
    fn invalid_settings_keep_the_previous_version_and_original_bytes() {
        for stored in ["broken", r#"{"decks":[{"columns":[{},null]}]}"#] {
            let mut conn = Connection::open_in_memory().unwrap();
            apply(&mut conn, &META[..30]).unwrap();
            conn.execute("INSERT INTO settings VALUES ('deck.state', ?1)", [stored])
                .unwrap();
            assert!(apply(&mut conn, META).is_err());
            let retained: String = conn
                .query_row(
                    "SELECT value FROM settings WHERE key='deck.state'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained, stored);
            assert_eq!(
                conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                30
            );
        }
    }
}
