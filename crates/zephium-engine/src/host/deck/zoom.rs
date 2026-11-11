use zephium_core::deck::DeckAppearance;

pub(in crate::host) fn content_scale(appearance: DeckAppearance, address: &str) -> f64 {
    let Ok(url) = url::Url::parse(address) else {
        return 1.0;
    };
    if !super::allows_origin(&url) || url.path().starts_with("/compose/") {
        return 1.0;
    }
    f64::from(appearance.font_size) / 14.0
}

pub(in crate::host) fn sync(
    view: &super::super::ObservedView,
    appearance: DeckAppearance,
    address: &str,
) {
    let scale = content_scale(appearance, address);
    let previous = view.deck_zoom.replace(scale);
    if previous == scale {
        return;
    }
    if view.view.zoom(view.applied_zoom.get() * scale).is_err() {
        view.deck_zoom.set(previous);
        eprintln!("engine: native column zoom invocation failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_density_scales_only_twitter_content() {
        let appearance = DeckAppearance {
            font_size: 12,
            ..DeckAppearance::default()
        };
        for address in [
            "https://x.com/home",
            "https://x.com/notifications",
            "https://twitter.com/example",
        ] {
            assert_eq!(content_scale(appearance, address), 12.0 / 14.0);
        }
        for address in [
            "https://x.com/compose/post",
            "https://example.com/home",
            "about:blank",
            "https://x.com.example.org/home",
            "invalid",
        ] {
            assert_eq!(content_scale(appearance, address), 1.0);
        }
    }
}
