use super::*;

impl Shell {
    pub(in crate::shell) fn handle_deck_shortcut(&mut self, item: ItemId, command: String) {
        match command.as_str() {
            "media.close" => self.close_media(item),
            "media.open" => self.open_media(item),
            value if value.starts_with("account.identity=") => {
                if let Some(identity) = value
                    .strip_prefix("account.identity=")
                    .and_then(|encoded| {
                        serde_json::from_str::<zephium_core::deck::account::AccountIdentity>(
                            encoded,
                        )
                        .ok()
                    })
                    .filter(zephium_core::deck::account::AccountIdentity::valid)
                {
                    self.account_identity_observed(item, identity);
                }
            }
            value if value.starts_with("account.avatar=") => {
                if let Some(url) = value
                    .strip_prefix("account.avatar=")
                    .and_then(|value| serde_json::from_str::<Option<String>>(value).ok())
                {
                    self.account_avatar_observed(item, url);
                }
            }
            "account.logged-in" => self.account_login_observed(
                item,
                zephium_core::deck::account::AccountLoginState::LoggedIn,
            ),
            "account.logged-out" => self.account_login_observed(
                item,
                zephium_core::deck::account::AccountLoginState::LoggedOut,
            ),
            value
                if value.starts_with("deck.scrollBy=")
                    && self.deck.profile_for_item(item).is_some()
                    && !self.settings_visible() =>
            {
                if let Some(delta) = value
                    .strip_prefix("deck.scrollBy=")
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|delta| delta.is_finite() && delta.abs() <= 1000.0)
                {
                    let offset = self.deck.scroll_offset.get() + delta;
                    let _ = self.operation_run_command(&format!("deck.scroll={offset}"));
                }
            }
            value if value.starts_with("composer.open=") => {
                let Some(source_url) = value
                    .strip_prefix("composer.open=")
                    .and_then(|encoded| serde_json::from_str::<String>(encoded).ok())
                    .and_then(|value| url::Url::parse(&value).ok())
                else {
                    return;
                };
                let profile = self.profile_of_item(item);
                let account = self
                    .deck
                    .state()
                    .and_then(|state| {
                        state
                            .accounts
                            .iter()
                            .find(|account| Some(account.profile) == profile)
                    })
                    .map(|account| account.id);
                let mention = super::profile::mention(&source_url);
                let result = self.operation_open_composer(account, mention);
                if self.deck.profile_for_item(item).is_some()
                    && matches!(
                        result.outcome,
                        OperationOutcome::Applied
                            | OperationOutcome::Deferred
                            | OperationOutcome::NoOp
                    )
                {
                    self.composer.return_to = Some(item);
                }
            }
            "composer.close"
                if self.composer.active_item() == Some(item)
                    || self.accounts.active_item() == Some(item) =>
            {
                let _ = self.operation_close_composer();
            }
            "composer.ready" => self.composer_ready(item),
            value if value.starts_with("composer.height=") => {
                if let Some(height) = value
                    .strip_prefix("composer.height=")
                    .and_then(|height| height.parse::<f64>().ok())
                    .filter(|height| height.is_finite() && (160.0..=640.0).contains(height))
                {
                    self.composer_height(item, height);
                }
            }
            _ => {}
        }
    }
}
