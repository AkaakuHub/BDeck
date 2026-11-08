#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct AccountIdentity {
    pub user_id: String,
    pub display_name: String,
    pub screen_name: String,
}

impl AccountIdentity {
    pub fn valid(&self) -> bool {
        self.user_id
            .parse::<u64>()
            .is_ok_and(|id| id > 0 && id.to_string() == self.user_id)
            && !self.display_name.trim().is_empty()
            && self.display_name.chars().count() <= 100
            && !self.display_name.chars().any(char::is_control)
            && !self.screen_name.is_empty()
            && self.screen_name.len() <= 32
            && self
                .screen_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AccountLoginState {
    #[default]
    Unknown,
    LoggedIn,
    LoggedOut,
    AccountChanged,
}

impl AccountLoginState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::LoggedIn => "logged_in",
            Self::LoggedOut => "logged_out",
            Self::AccountChanged => "account_changed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_identifiers_without_losing_integer_precision() {
        let mut identity = AccountIdentity {
            user_id: "9007199254740993".into(),
            display_name: "Account".into(),
            screen_name: "account".into(),
        };
        assert!(identity.valid());
        for invalid in ["0", "-1", "1.5", "01", "18446744073709551616", "account"] {
            identity.user_id = invalid.into();
            assert!(!identity.valid());
        }
    }
}
