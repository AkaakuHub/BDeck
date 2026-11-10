pub(super) fn mention(url: &url::Url) -> Option<String> {
    if url.scheme() != "https" || !matches!(url.host_str(), Some("x.com" | "twitter.com")) {
        return None;
    }
    let mut path = url.path().trim_matches('/').split('/');
    let name = path.next()?;
    if name.is_empty() || name.len() > 32 || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        return None;
    }
    if matches!(name.to_ascii_lowercase().as_str(), "home" | "explore" | "notifications" | "messages" | "search" | "settings" | "compose" | "i" | "login" | "logout" | "signup" | "tos" | "privacy" | "intent") {
        return None;
    }
    if let Some(tab) = path.next() {
        if !matches!(tab, "with_replies" | "media" | "likes" | "highlights" | "articles") || path.next().is_some() {
            return None;
        }
    }
    Some(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::mention;
    #[test]
    fn uses_the_viewed_profile_and_its_tabs() {
        for path in ["/Viewed_User", "/Viewed_User/", "/Viewed_User/media", "/Viewed_User/with_replies"] {
            assert_eq!(mention(&url::Url::parse(&format!("https://x.com{path}")).unwrap()).as_deref(), Some("Viewed_User"));
        }
    }
    #[test]
    fn does_not_address_regular_pages_posts_or_external_origins() {
        for value in ["https://x.com/home", "https://x.com/explore", "https://x.com/i/lists/123", "https://x.com/Viewed_User/status/123", "https://x.com/Viewed_User/lists", "https://example.com/Viewed_User", "https://x.com/", "https://x.com/%40name", "https://x.com/name-with-dash"] {
            assert!(mention(&url::Url::parse(value).unwrap()).is_none(), "{value}");
        }
    }
}
