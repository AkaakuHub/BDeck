pub fn external_target(target: &str) -> Option<url::Url> {
    if target.len() > 8192 {
        return None;
    }
    let url = url::Url::parse(target).ok()?;
    let host = url.host_str()?.trim_end_matches('.');
    (matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && host != "x.com"
        && !host.ends_with(".x.com")
        && host != "twitter.com"
        && !host.ends_with(".twitter.com"))
    .then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_web_links_exclude_in_deck_navigation_and_non_web_schemes() {
        for link in [
            "https://example.com/path",
            "https://t.co/abc",
            "http://example.com/",
        ] {
            assert!(external_target(link).is_some());
        }
        for link in [
            "https://x.com/home",
            "https://twitter.com/user/status/1",
            "https://mobile.x.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://name:secret@example.com",
            "mailto:a@example.com",
        ] {
            assert!(external_target(link).is_none());
        }
    }
}
