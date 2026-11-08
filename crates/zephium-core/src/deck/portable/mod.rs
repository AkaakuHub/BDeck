use std::collections::{HashMap, HashSet};

use super::{account::AccountIdentity, Deck, DeckAccount, DeckState, MAX_ACCOUNTS};
use crate::ids::{DeckAccountId, DeckColumnId, DeckId, ProfileId};

pub const MAX_LAYOUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutArchive {
    version: u8,
    accounts: Vec<Account>,
    decks: Vec<Deck>,
    active: DeckId,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    id: DeckAccountId,
    identity: Option<AccountIdentity>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    Invalid,
    AmbiguousAccount,
    AccountLimit,
    IdentifierExhausted,
}

pub struct ImportPlan {
    pub state: DeckState,
    pub new_accounts: Vec<DeckAccount>,
}

impl LayoutArchive {
    pub fn export(state: &DeckState) -> Option<Self> {
        state.valid().then(|| Self {
            version: 1,
            accounts: state
                .accounts
                .iter()
                .map(|account| Account {
                    id: account.id,
                    identity: account.identity.clone(),
                })
                .collect(),
            decks: state.decks.clone(),
            active: state.active,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ImportError> {
        if bytes.len() > MAX_LAYOUT_BYTES {
            return Err(ImportError::Invalid);
        }
        let archive: Self = serde_json::from_slice(bytes).map_err(|_| ImportError::Invalid)?;
        if !archive.valid() {
            return Err(ImportError::Invalid);
        }
        Ok(archive)
    }

    fn valid(&self) -> bool {
        self.version == 1
            && DeckState {
                accounts: self
                    .accounts
                    .iter()
                    .enumerate()
                    .map(|(index, account)| DeckAccount {
                        id: account.id,
                        profile: ProfileId::from(index as u128 + 1),
                        identity: account.identity.clone(),
                    })
                    .collect(),
                decks: self.decks.clone(),
                active: self.active,
            }
            .valid()
    }

    pub fn plan(
        &self,
        current: &DeckState,
        profiles: impl IntoIterator<Item = ProfileId>,
        mut profile_id: impl FnMut() -> ProfileId,
        mut account_id: impl FnMut() -> DeckAccountId,
        mut column_id: impl FnMut() -> DeckColumnId,
    ) -> Result<ImportPlan, ImportError> {
        if !self.valid() || !current.valid() {
            return Err(ImportError::Invalid);
        }
        let mut accounts = current.accounts.clone();
        let mut profile_ids: HashSet<_> = profiles.into_iter().collect();
        profile_ids.extend(accounts.iter().map(|account| account.profile));
        let mut account_ids: HashSet<_> = accounts.iter().map(|account| account.id).collect();
        let mut mapping = HashMap::new();
        let mut claimed = HashSet::new();
        let mut new_accounts = Vec::new();
        for imported in &self.accounts {
            let matched = matching_account(imported, &current.accounts)?
                .filter(|account| claimed.insert(account.id));
            let id = if let Some(account) = matched {
                account.id
            } else {
                if accounts.len() >= MAX_ACCOUNTS {
                    return Err(ImportError::AccountLimit);
                }
                let id = if account_ids.insert(imported.id) {
                    imported.id
                } else {
                    fresh_id(&mut account_ids, &mut account_id)?
                };
                let account = DeckAccount {
                    id,
                    profile: fresh_id(&mut profile_ids, &mut profile_id)?,
                    identity: imported.identity.clone(),
                };
                new_accounts.push(account.clone());
                accounts.push(account);
                id
            };
            mapping.insert(imported.id, id);
        }
        let mut column_ids: HashSet<_> = current
            .decks
            .iter()
            .flat_map(|deck| &deck.columns)
            .map(|column| column.id)
            .collect();
        let mut decks = self.decks.clone();
        for column in decks.iter_mut().flat_map(|deck| &mut deck.columns) {
            column.id = fresh_id(&mut column_ids, &mut column_id)?;
            column.account = *mapping.get(&column.account).ok_or(ImportError::Invalid)?;
        }
        let state = DeckState {
            accounts,
            decks,
            active: self.active,
        };
        if !state.valid() || !current.preserves_column_profiles(&state) {
            return Err(ImportError::Invalid);
        }
        Ok(ImportPlan {
            state,
            new_accounts,
        })
    }
}

fn matching_account<'a>(
    imported: &Account,
    accounts: &'a [DeckAccount],
) -> Result<Option<&'a DeckAccount>, ImportError> {
    let same_identity = |candidate: &&DeckAccount| match (&imported.identity, &candidate.identity) {
        (Some(imported), Some(current)) => imported.user_id == current.user_id,
        (None, None) => imported.id == candidate.id,
        _ => false,
    };
    if let Some(exact) = accounts
        .iter()
        .filter(same_identity)
        .find(|candidate| candidate.id == imported.id)
    {
        return Ok(Some(exact));
    }
    let mut candidates = accounts.iter().filter(same_identity);
    let first = candidates.next();
    if candidates.next().is_some() {
        return Err(ImportError::AmbiguousAccount);
    }
    Ok(first)
}

fn fresh_id<T: Copy + Eq + std::hash::Hash>(
    used: &mut HashSet<T>,
    generate: &mut impl FnMut() -> T,
) -> Result<T, ImportError> {
    (0..8)
        .map(|_| generate())
        .find(|id| used.insert(*id))
        .ok_or(ImportError::IdentifierExhausted)
}

#[cfg(test)]
mod tests;
