use super::{invalid_data, MAX_SETTING_VALUE_BYTES};
use rusqlite::{Connection, OptionalExtension};
use zephium_core::{deck::DeckState, ids::ProfileId};

pub(super) fn remove_profile(meta: &Connection, profile: ProfileId) -> rusqlite::Result<()> {
    remove_avatar(meta, profile)?;
    let stored = meta.query_row(
        "SELECT CASE WHEN length(CAST(value AS BLOB)) <= ?1 THEN value END FROM settings WHERE key = 'deck.state'",
        [MAX_SETTING_VALUE_BYTES as i64], |row| row.get::<_, Option<String>>(0),
    ).optional()?;
    let Some(stored) = stored else {
        return Ok(());
    };
    let stored = stored.ok_or_else(|| invalid_data("deck state exceeds persistence limit"))?;
    let mut state = serde_json::from_str::<DeckState>(&stored)
        .ok()
        .filter(DeckState::valid)
        .ok_or_else(|| invalid_data("invalid deck state during profile deletion"))?;
    if state.remove_profile(profile) {
        let value = serde_json::to_string(&state)
            .map_err(|_| invalid_data("could not encode surviving deck state"))?;
        if value.len() > MAX_SETTING_VALUE_BYTES {
            return Err(invalid_data(
                "surviving deck state exceeds persistence limit",
            ));
        }
        if meta.execute(
            "UPDATE settings SET value = ?1 WHERE key = 'deck.state'",
            [value],
        )? != 1
        {
            return Err(invalid_data(
                "deck state disappeared during profile deletion",
            ));
        }
    }
    Ok(())
}

fn remove_avatar(meta: &Connection, profile: ProfileId) -> rusqlite::Result<()> {
    use zephium_core::deck::avatars::{self, AccountAvatars};
    let stored = meta.query_row("SELECT CASE WHEN length(CAST(value AS BLOB)) <= ?1 THEN value END FROM settings WHERE key = ?2", rusqlite::params![MAX_SETTING_VALUE_BYTES as i64, avatars::SETTING_KEY], |row| row.get::<_, Option<String>>(0)).optional()?;
    let Some(stored) = stored else {
        return Ok(());
    };
    let mut values = stored
        .and_then(|value| serde_json::from_str::<AccountAvatars>(&value).ok())
        .filter(avatars::valid)
        .ok_or_else(|| invalid_data("invalid account avatars during profile deletion"))?;
    if values.remove(&profile).is_some() {
        let encoded = serde_json::to_string(&values)
            .map_err(|_| invalid_data("could not encode surviving account avatars"))?;
        meta.execute(
            "UPDATE settings SET value = ?1 WHERE key = ?2",
            rusqlite::params![encoded, avatars::SETTING_KEY],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::Hub;
    use zephium_core::{
        deck::{account::AccountIdentity, ColumnTarget, Deck, DeckAccount, DeckColumn, DeckLayout},
        ids::{DeckAccountId, DeckColumnId, DeckId, SpaceId},
        ports::store::ProfileDeletionAuthorizeOutcome,
        profiles::ProfileKind,
        session::{self, PersistedProfile, PersistedSpace, SessionState},
    };

    fn session() -> SessionState {
        session::canonicalize(SessionState {
            profiles: vec![
                PersistedProfile {
                    id: ProfileId::from(1),
                    name: "main".into(),
                    kind: ProfileKind::Default,
                },
                PersistedProfile {
                    id: ProfileId::from(2),
                    name: "Other".into(),
                    kind: ProfileKind::Named,
                },
            ],
            spaces: vec![
                PersistedSpace {
                    id: SpaceId::from(1),
                    profile: ProfileId::from(1),
                    name: "main".into(),
                    look: Default::default(),
                },
                PersistedSpace {
                    id: SpaceId::from(2),
                    profile: ProfileId::from(2),
                    name: "Other".into(),
                    look: Default::default(),
                },
            ],
            active_space: Some(SpaceId::from(1)),
            ..SessionState::default()
        })
    }

    fn deck() -> DeckState {
        let accounts = (1..=2)
            .map(|id| {
                let mut account = DeckAccount::new(DeckAccountId::from(id), ProfileId::from(id));
                account.identity = Some(AccountIdentity {
                    user_id: (9007199254740992_u64 + id as u64).to_string(),
                    display_name: format!("Account{id}"),
                    screen_name: format!("account{id}"),
                });
                account
            })
            .collect();
        DeckState {
            accounts,
            decks: vec![Deck {
                id: DeckId::from(1),
                name: "Deck".into(),
                layout: DeckLayout::Horizontal,
                row_ratio: 50,
                columns: (1..=2)
                    .map(|id| DeckColumn {
                        id: DeckColumnId::from(id),
                        account: DeckAccountId::from(id),
                        target: ColumnTarget::Home,
                        width: 320,
                        row: 0,
                        suspended: false,
                    })
                    .collect(),
            }],
            active: DeckId::from(1),
        }
    }

    fn surviving_session() -> SessionState {
        let mut state = session();
        state
            .profiles
            .retain(|profile| profile.id != ProfileId::from(2));
        state
            .spaces
            .retain(|space| space.profile != ProfileId::from(2));
        state
    }

    #[test]
    fn deletion_and_deck_cleanup_share_the_same_transaction_and_retry_keeps_survivor_changes() {
        let mut hub = Hub::in_memory().unwrap();
        hub.save(&session()).unwrap();
        let original = serde_json::to_string(&deck()).unwrap();
        hub.set_app_setting("deck.state", &original).unwrap();
        let original_avatars = zephium_core::deck::avatars::AccountAvatars::from([
            (
                ProfileId::from(1),
                "https://pbs.twimg.com/profile_images/1/current_normal.jpg".into(),
            ),
            (
                ProfileId::from(2),
                "https://pbs.twimg.com/profile_images/2/deleted_normal.jpg".into(),
            ),
        ]);
        let encoded_avatars = serde_json::to_string(&original_avatars).unwrap();
        hub.set_app_setting(zephium_core::deck::avatars::SETTING_KEY, &encoded_avatars)
            .unwrap();

        hub.meta.execute_batch("CREATE TRIGGER reject_snapshot BEFORE UPDATE ON session_snapshot BEGIN SELECT RAISE(ABORT, 'write failure'); END;").unwrap();
        assert!(hub
            .authorize_profile_deletion(ProfileId::from(2), &surviving_session())
            .is_err());
        assert_eq!(hub.app_setting("deck.state"), Some(original));
        assert_eq!(
            hub.app_setting(zephium_core::deck::avatars::SETTING_KEY),
            Some(encoded_avatars)
        );
        assert!(hub.pending_profile_deletions().unwrap().is_empty());
        assert_eq!(hub.load().unwrap().unwrap().profiles.len(), 2);
        hub.meta
            .execute_batch("DROP TRIGGER reject_snapshot")
            .unwrap();
        assert_eq!(
            hub.authorize_profile_deletion(ProfileId::from(2), &surviving_session())
                .unwrap(),
            ProfileDeletionAuthorizeOutcome::Authorized
        );
        let mut expected = deck();
        expected.remove_profile(ProfileId::from(2));
        let stored: DeckState =
            serde_json::from_str(&hub.app_setting("deck.state").unwrap()).unwrap();
        assert_eq!(stored, expected);
        let avatars: zephium_core::deck::avatars::AccountAvatars = serde_json::from_str(
            &hub.app_setting(zephium_core::deck::avatars::SETTING_KEY)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(avatars.len(), 1);
        assert_eq!(
            avatars.get(&ProfileId::from(1)),
            original_avatars.get(&ProfileId::from(1))
        );
        expected.decks[0].columns[0].width = 480;
        hub.set_app_setting("deck.state", &serde_json::to_string(&expected).unwrap())
            .unwrap();
        assert_eq!(
            hub.authorize_profile_deletion(ProfileId::from(2), &surviving_session())
                .unwrap(),
            ProfileDeletionAuthorizeOutcome::AlreadyAuthorized
        );
        let stored: DeckState =
            serde_json::from_str(&hub.app_setting("deck.state").unwrap()).unwrap();
        assert_eq!(stored, expected);
    }

    #[test]
    fn invalid_deck_metadata_does_not_authorize_incomplete_cleanup() {
        let mut hub = Hub::in_memory().unwrap();
        hub.save(&session()).unwrap();
        hub.set_app_setting("deck.state", "invalid").unwrap();
        assert!(hub
            .authorize_profile_deletion(ProfileId::from(2), &surviving_session())
            .is_err());
        assert!(hub.pending_profile_deletions().unwrap().is_empty());
        assert_eq!(hub.load().unwrap().unwrap().profiles.len(), 2);
        assert_eq!(hub.app_setting("deck.state").as_deref(), Some("invalid"));
    }
}
