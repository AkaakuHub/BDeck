use super::super::{invalid_data, Hub, MAX_SETTING_VALUE_BYTES};
use zephium_core::{deck::DeckState, ports::store::DeckImportOutcome, session::SessionState};

impl Hub {
    pub(crate) fn import_deck(
        &mut self,
        session: &SessionState,
        deck: &DeckState,
    ) -> DeckImportOutcome {
        let result = self.commit_deck_import(session, deck);
        match result {
            Ok(()) => DeckImportOutcome::Applied,
            Err(error) => {
                eprintln!("store: layout import did not settle: {error}");
                let Ok(Some(stored)) = self.load() else {
                    return DeckImportOutcome::Unavailable;
                };
                let Some(stored_deck) = self.app_setting("deck.state") else {
                    return DeckImportOutcome::Unavailable;
                };
                let Ok(stored_deck) = serde_json::from_str::<DeckState>(&stored_deck) else {
                    return DeckImportOutcome::Unavailable;
                };
                if stored == *session && stored_deck == *deck {
                    DeckImportOutcome::Applied
                } else {
                    DeckImportOutcome::Rejected
                }
            }
        }
    }

    fn commit_deck_import(
        &mut self,
        session: &SessionState,
        deck: &DeckState,
    ) -> rusqlite::Result<()> {
        let prepared = self.prepare_session(session)?;
        if !deck.valid()
            || self
                .registry
                .difference(&prepared.registry)
                .next()
                .is_some()
            || deck
                .accounts
                .iter()
                .any(|account| !prepared.registry.contains(&account.profile))
        {
            return Err(invalid_data("invalid layout import registry"));
        }
        let previous = self
            .app_setting("deck.state")
            .and_then(|value| serde_json::from_str::<DeckState>(&value).ok())
            .filter(DeckState::valid)
            .ok_or_else(|| invalid_data("layout import requires an existing deck"))?;
        if !previous.preserves_column_profiles(deck)
            || previous
                .accounts
                .iter()
                .any(|account| !deck.accounts.contains(account))
        {
            return Err(invalid_data(
                "layout import cannot replace existing login partitions",
            ));
        }
        let encoded =
            serde_json::to_string(deck).map_err(|_| invalid_data("invalid imported layout"))?;
        if encoded.len() > MAX_SETTING_VALUE_BYTES {
            return Err(invalid_data("imported layout exceeds persistence limit"));
        }
        self.validate_session_transition(&prepared.registry)?;
        self.commit_prepared_session(prepared, None, Some(&encoded))
    }
}
