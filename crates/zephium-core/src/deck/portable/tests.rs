use super::*;
use crate::deck::{ColumnTarget, DeckColumn, DeckLayout};

fn state(account: u128, profile: u128, user: &str) -> DeckState {
    let account = DeckAccount {
        id: DeckAccountId::from(account),
        profile: ProfileId::from(profile),
        identity: Some(AccountIdentity {
            user_id: user.into(),
            display_name: "Account".into(),
            screen_name: "account".into(),
        }),
    };
    let mut deck = Deck::new(DeckId::from(1), "main", DeckLayout::TwoRows).unwrap();
    deck.row_ratio = 35;
    deck.columns.push(DeckColumn {
        id: DeckColumnId::from(1),
        account: account.id,
        target: ColumnTarget::Home,
        width: 420,
        row: 1,
        suspended: true,
    });
    DeckState {
        accounts: vec![account],
        active: deck.id,
        decks: vec![deck],
    }
}

fn plan(archive: &LayoutArchive, local: &DeckState) -> Result<ImportPlan, ImportError> {
    let mut profile = 100;
    let mut account = 100;
    let mut column = 100;
    archive.plan(
        local,
        local.accounts.iter().map(|account| account.profile),
        || {
            profile += 1;
            ProfileId::from(profile)
        },
        || {
            account += 1;
            DeckAccountId::from(account)
        },
        || {
            column += 1;
            DeckColumnId::from(column)
        },
    )
}

#[test]
fn archive_roundtrips_layout_and_identity_without_local_profile_authority() {
    let original = state(10, 999, "9007199254740993");
    let archive = LayoutArchive::export(&original).unwrap();
    let encoded = serde_json::to_vec(&archive).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    let account = value["accounts"][0].as_object().unwrap();
    assert_eq!(account.len(), 2);
    assert!(account.contains_key("id"));
    assert!(account.contains_key("identity"));
    assert_eq!(LayoutArchive::decode(&encoded).unwrap(), archive);
    let imported = plan(&archive, &state(20, 888, "2")).unwrap();
    assert_eq!(imported.new_accounts.len(), 1);
    assert_ne!(
        imported.new_accounts[0].profile,
        original.accounts[0].profile
    );
    assert_eq!(imported.new_accounts[0].id, original.accounts[0].id);
    assert_eq!(imported.state.decks[0].columns[0].width, 420);
    assert_eq!(imported.state.decks[0].columns[0].row, 1);
    assert!(imported.state.decks[0].columns[0].suspended);
    assert_eq!(imported.state.decks[0].row_ratio, 35);
}

#[test]
fn unchanged_x_identity_reuses_local_partition_after_rename() {
    let archive = LayoutArchive::export(&state(10, 999, "1")).unwrap();
    let mut local = state(20, 888, "1");
    let identity = local.accounts[0].identity.as_mut().unwrap();
    identity.display_name = "Renamed".into();
    identity.screen_name = "new_handle".into();
    let imported = plan(&archive, &local).unwrap();
    assert!(imported.new_accounts.is_empty());
    assert_eq!(imported.state.accounts, local.accounts);
    assert_eq!(
        imported.state.decks[0].columns[0].account,
        local.accounts[0].id
    );
    assert_ne!(
        imported.state.decks[0].columns[0].id,
        local.decks[0].columns[0].id
    );
}

#[test]
fn matching_names_or_conflicting_bdeck_ids_never_reuse_another_account() {
    let archive = LayoutArchive::export(&state(10, 999, "1")).unwrap();
    for local in [state(20, 888, "2"), state(10, 888, "2")] {
        let imported = plan(&archive, &local).unwrap();
        assert_eq!(imported.new_accounts.len(), 1);
        assert_ne!(imported.new_accounts[0].profile, local.accounts[0].profile);
        assert_ne!(imported.new_accounts[0].id, local.accounts[0].id);
        assert_eq!(imported.state.accounts[0], local.accounts[0]);
    }
}

#[test]
fn repeated_import_does_not_create_more_partitions() {
    let archive = LayoutArchive::export(&state(10, 999, "1")).unwrap();
    let first = plan(&archive, &state(20, 888, "2")).unwrap();
    let second = plan(&archive, &first.state).unwrap();
    assert!(second.new_accounts.is_empty());
    assert_eq!(second.state.accounts, first.state.accounts);
}

#[test]
fn distinct_bdeck_accounts_keep_distinct_partitions_even_for_the_same_x_identity() {
    let mut original = state(10, 999, "1");
    original
        .accounts
        .push(state(11, 998, "1").accounts.remove(0));
    let mut column = original.decks[0].columns[0].clone();
    column.id = DeckColumnId::from(2);
    column.account = DeckAccountId::from(11);
    original.decks[0].columns.push(column);
    let archive = LayoutArchive::export(&original).unwrap();
    let imported = plan(&archive, &state(20, 888, "2")).unwrap();
    assert_eq!(imported.new_accounts.len(), 2);
    assert_ne!(
        imported.new_accounts[0].profile,
        imported.new_accounts[1].profile
    );
    let again = plan(&archive, &imported.state).unwrap();
    assert!(again.new_accounts.is_empty());
    assert_eq!(again.state.accounts, imported.state.accounts);
}

#[test]
fn ambiguous_accounts_and_id_exhaustion_fail_without_a_partial_plan() {
    let archive = LayoutArchive::export(&state(10, 999, "1")).unwrap();
    let mut local = state(20, 888, "1");
    local.accounts.push(state(30, 777, "1").accounts.remove(0));
    assert!(matches!(
        plan(&archive, &local),
        Err(ImportError::AmbiguousAccount)
    ));
    let local = state(20, 888, "2");
    assert!(matches!(
        archive.plan(
            &local,
            [ProfileId::from(100)],
            || ProfileId::from(100),
            || DeckAccountId::from(100),
            || DeckColumnId::from(100)
        ),
        Err(ImportError::IdentifierExhausted)
    ));
}

#[test]
fn rejects_invalid_or_unsupported_archives_and_excessive_accounts() {
    let archive = LayoutArchive::export(&state(10, 999, "1")).unwrap();
    for mutate in [
        |value: &mut serde_json::Value| {
            value["version"] = 2.into();
        },
        |value: &mut serde_json::Value| {
            value["accounts"][0]["profile"] = "local".into();
        },
        |value: &mut serde_json::Value| {
            value["accounts"][0]["identity"]["user_id"] = "01".into();
        },
        |value: &mut serde_json::Value| {
            value["decks"][0]["columns"][0]["width"] = 1.into();
        },
    ] {
        let mut value = serde_json::to_value(&archive).unwrap();
        mutate(&mut value);
        assert!(LayoutArchive::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    assert!(LayoutArchive::decode(&vec![b' '; MAX_LAYOUT_BYTES + 1]).is_err());
    let mut local = state(20, 888, "2");
    for id in 100..100 + MAX_ACCOUNTS as u128 - 1 {
        local.accounts.push(DeckAccount::new(
            DeckAccountId::from(id),
            ProfileId::from(id),
        ));
    }
    assert!(matches!(
        plan(&archive, &local),
        Err(ImportError::AccountLimit)
    ));
}
