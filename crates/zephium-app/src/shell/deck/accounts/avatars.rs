use super::*;
use zephium_core::deck::avatars::{self, AccountAvatars};

impl AccountRuntime {
    pub(in crate::shell) fn load(store: &dyn zephium_core::ports::store::Store) -> Self {
        let avatars = store
            .app_setting(avatars::SETTING_KEY)
            .and_then(|value| serde_json::from_str::<AccountAvatars>(&value).ok())
            .filter(avatars::valid)
            .unwrap_or_default();
        Self {
            avatars,
            ..Self::default()
        }
    }
}

impl Shell {
    pub(in crate::shell) fn account_avatar_observed(&mut self, item: ItemId, url: Option<String>) {
        if url.as_ref().is_some_and(|url| !avatars::valid_url(url)) {
            return;
        }
        let Some(profile) = self
            .deck_profile_for_item(item)
            .filter(|profile| !self.profile_deletion_quarantines(*profile))
        else {
            return;
        };
        if self.accounts.account_changed(profile)
            || self.deck.state().is_none_or(|state| {
                !state
                    .accounts
                    .iter()
                    .any(|account| account.profile == profile)
            })
            || self.accounts.avatars.get(&profile) == url.as_ref()
        {
            return;
        }
        let mut next = self.accounts.avatars.clone();
        if let Some(url) = &url {
            next.insert(profile, url.clone());
        } else {
            next.remove(&profile);
        }
        if !avatars::valid(&next) {
            return;
        }
        let Ok(encoded) = serde_json::to_string(&next) else {
            return;
        };
        if !self
            .store
            .set_app_setting(avatars::SETTING_KEY.into(), encoded)
        {
            return;
        }
        self.accounts.avatars = next;
        let Ok(url) = serde_json::to_string(&url) else {
            return;
        };
        (self.emit)(Projection::UiCommand(format!(
            "account.avatar={profile}:{url}"
        )));
    }
}
