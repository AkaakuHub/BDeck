use zephium_core::deck::account::AccountIdentity;

pub(in crate::host) fn allows_origin(origin: &url::Url) -> bool {
    origin.scheme() == "https"
        && origin.host_str().is_some_and(|host| {
            host == "x.com"
                || host.ends_with(".x.com")
                || host == "twitter.com"
                || host.ends_with(".twitter.com")
        })
}

pub(in crate::host) fn accepts_command(command: &str) -> bool {
    let identity = command
        .strip_prefix("account.identity=")
        .filter(|encoded| encoded.len() <= 1024)
        .and_then(|encoded| serde_json::from_str::<AccountIdentity>(encoded).ok())
        .is_some_and(|identity| identity.valid());
    let avatar = command
        .strip_prefix("account.avatar=")
        .filter(|encoded| encoded.len() <= 1100)
        .and_then(|encoded| serde_json::from_str::<Option<String>>(encoded).ok())
        .is_some_and(|url| {
            url.as_ref()
                .is_none_or(|url| zephium_core::deck::avatars::valid_url(url))
        });
    let scroll = command
        .strip_prefix("deck.scrollBy=")
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|delta| delta.is_finite() && delta.abs() <= 1000.0);
    let composer = command
        .strip_prefix("composer.open=")
        .filter(|encoded| encoded.len() <= 8192)
        .and_then(|encoded| serde_json::from_str::<String>(encoded).ok())
        .and_then(|value| url::Url::parse(&value).ok())
        .is_some_and(|url| allows_origin(&url));
    identity
        || composer
        || avatar
        || scroll
        || command
            .strip_prefix("composer.height=")
            .and_then(|height| height.parse::<f64>().ok())
            .is_some_and(|height| height.is_finite() && (160.0..=640.0).contains(&height))
        || matches!(
            command,
            "media.open" | "media.close" | "composer.close"
                | "composer.ready"
                | "account.logged-in"
                | "account.logged-out"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_x_origins_can_report_local_ui_events() {
        for origin in [
            "https://x.com/home",
            "https://twitter.com/home",
            "https://mobile.x.com/",
        ] {
            assert!(allows_origin(&url::Url::parse(origin).unwrap()));
        }
        for origin in [
            "http://x.com/",
            "https://x.com.evil.test/",
            "https://x.com@evil.test/",
        ] {
            assert!(!allows_origin(&url::Url::parse(origin).unwrap()));
        }
    }

    #[test]
    fn admits_bounded_ui_gestures_without_social_actions() {
        assert!(accepts_command(
            r#"composer.open="https://x.com/Viewed_User""#
        ));
        assert!(accepts_command(r#"composer.open="https://x.com/home""#));
        assert!(accepts_command("composer.ready"));
        assert!(accepts_command("composer.height=320"));
        assert!(accepts_command("deck.scrollBy=-10.5"));
        assert!(accepts_command(
            "account.avatar=\"https://pbs.twimg.com/profile_images/1/avatar.jpg\""
        ));
        assert!(!accepts_command(
            "account.avatar=\"https://evil.test/avatar.jpg\""
        ));
        for command in [
            "composer.open",
            "composer.open=not-json",
            r#"composer.open="https://x.com.evil.test/user""#,
            r#"composer.open="http://x.com/user""#,
            "tweet.send",
            "account.follow",
            "deck.scrollBy=NaN",
            "deck.scrollBy=inf",
            "deck.scrollBy=1001",
            "composer.open;tweet.send",
            "composer.height=NaN",
            "composer.height=159",
            "composer.height=641",
        ] {
            assert!(!accepts_command(command));
        }
    }
}
