use crate::ids::ProfileId;
use std::collections::BTreeMap;

pub const SETTING_KEY: &str = "deck.avatars";
pub type AccountAvatars = BTreeMap<ProfileId, String>;

pub fn valid_url(value: &str) -> bool {
    value.len() <= 1024
        && url::Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("pbs.twimg.com")
                && url.port().is_none()
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.path().starts_with("/profile_images/")
        })
}

pub fn valid(avatars: &AccountAvatars) -> bool {
    avatars.len() <= super::MAX_ACCOUNTS && avatars.values().all(|url| valid_url(url))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_accepts_the_profile_image_origin_and_path() {
        assert!(valid_url(
            "https://pbs.twimg.com/profile_images/123/avatar_normal.jpg"
        ));
        for value in [
            "http://pbs.twimg.com/profile_images/123/a.jpg",
            "https://pbs.twimg.com/media/a.jpg",
            "https://pbs.twimg.com.evil.example/profile_images/a.jpg",
            "https://user@pbs.twimg.com/profile_images/a.jpg",
            "https://pbs.twimg.com:444/profile_images/a.jpg",
            "https://pbs.twimg.com/profile_images/a.jpg#old",
        ] {
            assert!(!valid_url(value));
        }
    }
}
