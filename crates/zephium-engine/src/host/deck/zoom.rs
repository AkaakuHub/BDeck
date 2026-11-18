use zephium_core::deck::DeckAppearance;

pub(in crate::host) fn content_scale(appearance: DeckAppearance, address: &str) -> f64 {
    let _ = (appearance, address);
    1.0
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
    fn column_density_does_not_apply_native_zoom() {
        let appearance = DeckAppearance {
            font_size: 12,
            ..DeckAppearance::default()
        };
        for address in [
            "https://x.com/home",
            "https://x.com/notifications",
            "https://twitter.com/example",
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
