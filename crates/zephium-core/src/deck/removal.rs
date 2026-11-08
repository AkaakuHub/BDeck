use super::DeckState;
use crate::ids::ProfileId;

impl DeckState {
    pub fn remove_profile(&mut self, profile: ProfileId) -> bool {
        let removed = self
            .accounts
            .iter()
            .filter(|account| account.profile == profile)
            .map(|account| account.id)
            .collect::<Vec<_>>();
        if removed.is_empty() {
            return false;
        }
        self.accounts.retain(|account| account.profile != profile);
        for deck in &mut self.decks {
            deck.columns
                .retain(|column| !removed.contains(&column.account));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::{
        account::AccountIdentity, ColumnTarget, Deck, DeckAccount, DeckColumn, DeckLayout,
    };
    use crate::ids::{DeckAccountId, DeckColumnId, DeckId};

    fn column(id: u128, account: DeckAccountId) -> DeckColumn {
        DeckColumn {
            id: DeckColumnId::from(id),
            account,
            target: ColumnTarget::Home,
            width: 320,
            row: 0,
            suspended: false,
        }
    }

    #[test]
    fn removes_identity_and_all_related_columns_without_changing_survivors() {
        let deleted = DeckAccount::new(DeckAccountId::from(1), ProfileId::from(1));
        let mut survivor = DeckAccount::new(DeckAccountId::from(2), ProfileId::from(2));
        survivor.identity = Some(AccountIdentity {
            user_id: "9007199254740993".into(),
            display_name: "Survivor".into(),
            screen_name: "survivor".into(),
        });
        let mut deleted = deleted;
        deleted.identity = Some(AccountIdentity {
            user_id: "9007199254740994".into(),
            display_name: "Deleted".into(),
            screen_name: "deleted".into(),
        });
        let survivor_column = column(2, survivor.id);
        let mut state = DeckState {
            accounts: vec![deleted.clone(), survivor.clone()],
            decks: vec![
                Deck {
                    id: DeckId::from(1),
                    name: "First".into(),
                    layout: DeckLayout::Horizontal,
                    row_ratio: 50,
                    columns: vec![column(1, deleted.id), survivor_column.clone()],
                },
                Deck {
                    id: DeckId::from(2),
                    name: "Second".into(),
                    layout: DeckLayout::TwoRows,
                    row_ratio: 40,
                    columns: vec![column(3, deleted.id)],
                },
            ],
            active: DeckId::from(2),
        };
        assert!(state.valid());
        assert!(state.remove_profile(deleted.profile));
        assert!(state.valid());
        assert_eq!(state.accounts, vec![survivor]);
        assert_eq!(state.decks[0].columns, vec![survivor_column]);
        assert!(state.decks[1].columns.is_empty());
        assert_eq!(state.active, DeckId::from(2));
        assert_eq!(state.decks[1].row_ratio, 40);
        let unchanged = state.clone();
        assert!(!state.remove_profile(deleted.profile));
        assert_eq!(state, unchanged);
    }
}
