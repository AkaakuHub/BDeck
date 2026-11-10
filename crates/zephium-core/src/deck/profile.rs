use super::DeckAccount;

pub(super) fn url(account: &DeckAccount) -> Option<String> {
    let identity = account.identity.as_ref().filter(|identity| identity.valid())?;
    Some(format!("https://x.com/i/user/{}", identity.user_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::{account::AccountIdentity, ColumnTarget};
    use crate::ids::{DeckAccountId, ProfileId};

    #[test]
    fn own_profile_requires_a_verified_identity_and_keeps_the_same_user_after_a_rename() {
        let mut account = DeckAccount::new(DeckAccountId::from(1), ProfileId::from(2));
        assert!(ColumnTarget::Profile.url(&account).is_none());
        account.identity = Some(AccountIdentity {
            user_id: "9007199254740993".into(),
            display_name: "First".into(),
            screen_name: "first_user".into(),
        });
        let expected = "https://x.com/i/user/9007199254740993";
        assert_eq!(ColumnTarget::Profile.url(&account).as_deref(), Some(expected));
        account.identity.as_mut().unwrap().screen_name = "renamed_user".into();
        assert_eq!(ColumnTarget::Profile.url(&account).as_deref(), Some(expected));
        account.identity.as_mut().unwrap().user_id = "../settings".into();
        assert!(ColumnTarget::Profile.url(&account).is_none());
    }
}
