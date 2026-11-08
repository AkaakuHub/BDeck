use crate::ids::{DeckAccountId, DeckColumnId, DeckId, ProfileId};

pub const MIN_COLUMN_WIDTH: u16 = 240;
pub const MAX_COLUMN_WIDTH: u16 = 960;
pub const MAX_DECK_NAME_CHARS: usize = 64;
pub const MAX_COLUMN_QUERY_CHARS: usize = 256;
pub const MAX_DECKS: usize = 32;
pub const MAX_COLUMNS_PER_DECK: usize = 64;
pub const MAX_ACCOUNTS: usize = 32;
pub const MIN_ROW_RATIO: u8 = 20;
pub const MAX_ROW_RATIO: u8 = 80;
pub const DEFAULT_ROW_RATIO: u8 = 50;

pub mod account;
pub mod appearance;
pub mod avatars;
pub mod layout;
mod removal;
pub use appearance::DeckAppearance;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DeckLayout {
    #[default]
    Horizontal,
    TwoRows,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColumnTarget {
    Home,
    Notifications,
    Search { query: String },
    List { url: String },
    Url { url: String },
}

impl ColumnTarget {
    pub fn url(&self) -> Option<String> {
        match self {
            Self::Home => Some("https://x.com/home".into()),
            Self::Notifications => Some("https://x.com/notifications".into()),
            Self::Search { query } => {
                let encoded =
                    url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>();
                Some(format!("https://x.com/search?q={encoded}"))
            }
            Self::List { url } | Self::Url { url } => valid_x_url(url).then(|| url.clone()),
        }
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::Home | Self::Notifications => true,
            Self::Search { query } => {
                !query.trim().is_empty()
                    && query.chars().count() <= MAX_COLUMN_QUERY_CHARS
                    && query.chars().all(|character| !character.is_control())
            }
            Self::List { url } | Self::Url { url } => valid_x_url(url),
        }
    }
}

fn valid_x_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "x.com"
        || host.ends_with(".x.com")
        || host == "twitter.com"
        || host.ends_with(".twitter.com")
}

fn valid_name(value: &str) -> Option<String> {
    let name: String = value
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_DECK_NAME_CHARS)
        .collect();
    (!name.is_empty()).then_some(name)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DeckAccount {
    pub id: DeckAccountId,
    pub profile: ProfileId,
    pub identity: Option<account::AccountIdentity>,
}

impl DeckAccount {
    pub fn new(id: DeckAccountId, profile: ProfileId) -> Self {
        Self {
            id,
            profile,
            identity: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DeckColumn {
    pub id: DeckColumnId,
    pub account: DeckAccountId,
    pub target: ColumnTarget,
    pub width: u16,
    pub row: u8,
    pub suspended: bool,
}

impl DeckColumn {
    pub fn valid(&self) -> bool {
        (MIN_COLUMN_WIDTH..=MAX_COLUMN_WIDTH).contains(&self.width)
            && self.row <= 1
            && self.target.valid()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct Deck {
    pub id: DeckId,
    pub name: String,
    pub layout: DeckLayout,
    pub row_ratio: u8,
    pub columns: Vec<DeckColumn>,
}

impl Deck {
    pub fn new(id: DeckId, name: &str, layout: DeckLayout) -> Option<Self> {
        Some(Self {
            id,
            name: valid_name(name)?,
            layout,
            row_ratio: DEFAULT_ROW_RATIO,
            columns: Vec::new(),
        })
    }

    pub fn valid(&self, accounts: &[DeckAccount]) -> bool {
        self.name.chars().count() <= MAX_DECK_NAME_CHARS
            && !self.name.is_empty()
            && (MIN_ROW_RATIO..=MAX_ROW_RATIO).contains(&self.row_ratio)
            && self.columns.len() <= MAX_COLUMNS_PER_DECK
            && self.columns.iter().all(|column| {
                column.valid() && accounts.iter().any(|account| account.id == column.account)
            })
            && unique_column_ids(&self.columns)
    }

    pub fn add_column(&mut self, column: DeckColumn) -> bool {
        if self.columns.len() >= MAX_COLUMNS_PER_DECK
            || !column.valid()
            || self.columns.iter().any(|current| current.id == column.id)
        {
            return false;
        }
        self.columns.push(column);
        true
    }

    pub fn remove_column(&mut self, id: DeckColumnId) -> bool {
        let Some(index) = self.columns.iter().position(|column| column.id == id) else {
            return false;
        };
        self.columns.remove(index);
        true
    }

    pub fn reorder_column(&mut self, id: DeckColumnId, before: Option<DeckColumnId>) -> bool {
        let Some(index) = self.columns.iter().position(|column| column.id == id) else {
            return false;
        };
        let column = self.columns.remove(index);
        let target = before
            .and_then(|before| self.columns.iter().position(|column| column.id == before))
            .unwrap_or(self.columns.len());
        self.columns.insert(target, column);
        true
    }

    pub fn set_width(&mut self, id: DeckColumnId, width: u16) -> bool {
        let Some(column) = self.columns.iter_mut().find(|column| column.id == id) else {
            return false;
        };
        let width = width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        if column.width == width {
            return false;
        }
        column.width = width;
        true
    }

    pub fn set_row(&mut self, id: DeckColumnId, row: u8) -> bool {
        let Some(column) = self.columns.iter_mut().find(|column| column.id == id) else {
            return false;
        };
        if row > 1 || column.row == row {
            return false;
        }
        column.row = row;
        true
    }

    pub fn set_account(&mut self, id: DeckColumnId, account: DeckAccountId) -> bool {
        let Some(column) = self.columns.iter_mut().find(|column| column.id == id) else {
            return false;
        };
        if column.account == account {
            return false;
        }
        column.account = account;
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct DeckState {
    pub accounts: Vec<DeckAccount>,
    pub decks: Vec<Deck>,
    pub active: DeckId,
}

impl DeckState {
    pub fn valid(&self) -> bool {
        let mut column_ids = std::collections::HashSet::new();
        self.accounts.len() <= MAX_ACCOUNTS
            && self.decks.len() <= MAX_DECKS
            && unique_accounts(&self.accounts)
            && self.accounts.iter().all(|account| {
                account
                    .identity
                    .as_ref()
                    .is_none_or(account::AccountIdentity::valid)
            })
            && unique_deck_ids(&self.decks)
            && self.decks.iter().all(|deck| deck.valid(&self.accounts))
            && self
                .decks
                .iter()
                .flat_map(|deck| &deck.columns)
                .all(|column| column_ids.insert(column.id))
            && self.decks.iter().any(|deck| deck.id == self.active)
    }

    pub fn preserves_column_profiles(&self, next: &Self) -> bool {
        self.accounts.iter().all(|account| {
            next.account(account.id).is_none_or(|candidate| {
                candidate.profile == account.profile
                    && account.identity.as_ref().is_none_or(|identity| {
                        candidate
                            .identity
                            .as_ref()
                            .is_some_and(|next| next.user_id == identity.user_id)
                    })
            })
        }) && self
            .decks
            .iter()
            .flat_map(|deck| &deck.columns)
            .all(|column| {
                let previous = self.account(column.account).map(|account| account.profile);
                next.decks
                    .iter()
                    .flat_map(|deck| &deck.columns)
                    .find(|candidate| candidate.id == column.id)
                    .is_none_or(|candidate| {
                        next.account(candidate.account)
                            .map(|account| account.profile)
                            == previous
                    })
            })
    }

    pub fn active_mut(&mut self) -> Option<&mut Deck> {
        self.decks.iter_mut().find(|deck| deck.id == self.active)
    }

    pub fn account(&self, id: DeckAccountId) -> Option<&DeckAccount> {
        self.accounts.iter().find(|account| account.id == id)
    }

    pub fn switch(&mut self, id: DeckId) -> bool {
        if self.decks.iter().any(|deck| deck.id == id) {
            self.active = id;
            true
        } else {
            false
        }
    }
}

fn unique_accounts(accounts: &[DeckAccount]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        accounts[..index]
            .iter()
            .all(|prior| prior.id != account.id && prior.profile != account.profile)
    })
}

fn unique_deck_ids(decks: &[Deck]) -> bool {
    decks
        .iter()
        .enumerate()
        .all(|(index, deck)| decks[..index].iter().all(|prior| prior.id != deck.id))
}

fn unique_column_ids(columns: &[DeckColumn]) -> bool {
    columns
        .iter()
        .enumerate()
        .all(|(index, column)| columns[..index].iter().all(|prior| prior.id != column.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: u128, profile: u128) -> DeckAccount {
        DeckAccount::new(DeckAccountId::from(id), ProfileId::from(profile))
    }

    fn column(id: u128, account: u128, target: ColumnTarget) -> DeckColumn {
        DeckColumn {
            id: DeckColumnId::from(id),
            account: DeckAccountId::from(account),
            target,
            width: 360,
            row: 0,
            suspended: false,
        }
    }

    #[test]
    fn built_in_targets_are_direct_x_urls() {
        assert_eq!(
            ColumnTarget::Home.url().as_deref(),
            Some("https://x.com/home")
        );
        assert_eq!(
            ColumnTarget::Search {
                query: "rust lang".into()
            }
            .url()
            .as_deref(),
            Some("https://x.com/search?q=rust+lang")
        );
    }

    #[test]
    fn arbitrary_target_rejects_non_x_origins_and_non_https() {
        assert!(!ColumnTarget::Url {
            url: "https://example.com".into()
        }
        .valid());
        assert!(!ColumnTarget::Url {
            url: "http://x.com/home".into()
        }
        .valid());
        assert!(ColumnTarget::Url {
            url: "https://mobile.x.com/i/lists/1".into()
        }
        .valid());
    }

    #[test]
    fn columns_can_be_reordered_resized_and_assigned_independently() {
        let mut deck = Deck::new(DeckId::from(1), "Main", DeckLayout::TwoRows).unwrap();
        assert!(deck.add_column(column(1, 1, ColumnTarget::Home)));
        assert!(deck.add_column(column(2, 2, ColumnTarget::Notifications)));
        assert!(deck.reorder_column(DeckColumnId::from(2), Some(DeckColumnId::from(1))));
        assert!(deck.set_width(DeckColumnId::from(2), 600));
        assert!(deck.set_row(DeckColumnId::from(2), 1));
        assert!(deck.set_account(DeckColumnId::from(2), DeckAccountId::from(1)));
        assert_eq!(deck.columns[0].id, DeckColumnId::from(2));
        assert_eq!(deck.columns[0].width, 600);
        assert_eq!(deck.columns[0].row, 1);
        assert_eq!(deck.columns[0].account, DeckAccountId::from(1));
    }

    #[test]
    fn state_validates_separate_account_and_deck_references() {
        let mut deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        assert!(deck.add_column(column(1, 1, ColumnTarget::Home)));
        let state = DeckState {
            accounts: vec![account(1, 10), account(2, 20)],
            decks: vec![deck],
            active: DeckId::from(1),
        };
        assert!(state.valid());
        assert_eq!(
            state.account(DeckAccountId::from(1)).unwrap().profile,
            ProfileId::from(10)
        );
    }

    #[test]
    fn columns_in_different_decks_cannot_share_a_native_view_identity() {
        let mut first = Deck::new(DeckId::from(1), "First", DeckLayout::Horizontal).unwrap();
        let mut second = Deck::new(DeckId::from(2), "Second", DeckLayout::Horizontal).unwrap();
        first.add_column(column(1, 1, ColumnTarget::Home));
        second.add_column(column(2, 2, ColumnTarget::Home));
        let mut state = DeckState {
            accounts: vec![account(1, 10), account(2, 20)],
            decks: vec![first, second],
            active: DeckId::from(1),
        };
        assert!(state.valid());
        state.decks[1].columns[0].id = state.decks[0].columns[0].id;
        assert!(!state.valid());
    }

    #[test]
    fn switch_refuses_unknown_decks() {
        let deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        let mut state = DeckState {
            accounts: vec![],
            decks: vec![deck],
            active: DeckId::from(1),
        };
        assert!(!state.switch(DeckId::from(2)));
        assert_eq!(state.active, DeckId::from(1));
    }
    #[test]
    fn different_accounts_cannot_share_a_cookie_partition() {
        let deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        let mut state = DeckState {
            accounts: vec![account(1, 10), account(2, 20)],
            decks: vec![deck],
            active: DeckId::from(1),
        };
        assert!(state.valid());
        state.accounts[1].profile = state.accounts[0].profile;
        assert!(!state.valid());
    }
    #[test]
    fn an_account_without_columns_cannot_change_cookie_partitions() {
        let deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        let original = DeckState {
            accounts: vec![account(1, 10)],
            decks: vec![deck],
            active: DeckId::from(1),
        };
        let mut next = original.clone();
        next.accounts[0].profile = ProfileId::from(20);
        assert!(next.valid());
        assert!(!original.preserves_column_profiles(&next));
    }
    #[test]
    fn account_identity_survives_display_and_screen_name_changes() {
        let deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        let mut owner = account(1, 10);
        owner.identity = Some(account::AccountIdentity {
            user_id: "9007199254740993".into(),
            display_name: "First".into(),
            screen_name: "first".into(),
        });
        let original = DeckState {
            accounts: vec![owner],
            decks: vec![deck],
            active: DeckId::from(1),
        };
        let mut next = original.clone();
        let identity = next.accounts[0].identity.as_mut().unwrap();
        identity.display_name = "Renamed".into();
        identity.screen_name = "renamed".into();
        assert!(next.valid());
        assert!(original.preserves_column_profiles(&next));
        next.accounts[0].identity.as_mut().unwrap().user_id = "9007199254740994".into();
        assert!(!original.preserves_column_profiles(&next));
        next.accounts[0].identity = None;
        assert!(!original.preserves_column_profiles(&next));
    }
    #[test]
    fn an_existing_column_cannot_change_cookie_partitions() {
        let mut original_deck = Deck::new(DeckId::from(1), "Main", DeckLayout::Horizontal).unwrap();
        original_deck.add_column(column(1, 1, ColumnTarget::Home));
        let original = DeckState {
            accounts: vec![account(1, 10), account(2, 20)],
            decks: vec![original_deck],
            active: DeckId::from(1),
        };
        let mut next = original.clone();
        next.decks[0].columns[0].account = DeckAccountId::from(2);
        assert!(!original.preserves_column_profiles(&next));
        next.decks[0].columns[0].id = DeckColumnId::from(2);
        assert!(original.preserves_column_profiles(&next));
    }
}
