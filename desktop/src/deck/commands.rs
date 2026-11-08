pub(crate) fn admits_scroll(command: &str) -> bool {
    let limit = zephium_core::deck::MAX_COLUMNS_PER_DECK as f64
        * (f64::from(zephium_core::deck::MAX_COLUMN_WIDTH) + 1.0);
    command
        .strip_prefix("deck.scroll=")
        .and_then(|value| value.parse::<f64>().ok())
        .is_some_and(|offset| offset.is_finite() && (0.0..=limit).contains(&offset))
}

#[cfg(test)]
mod tests {
    use super::admits_scroll;
    #[test]
    fn admits_bounded_viewport_positions_only() {
        assert!(admits_scroll("deck.scroll=0"));
        assert!(admits_scroll("deck.scroll=320.5"));
        for command in [
            "deck.scroll=-1",
            "deck.scroll=NaN",
            "deck.scroll=inf",
            "deck.scroll=100000",
            "deck.scroll=",
            "composer.open",
            "deck.scroll=320;composer.open",
        ] {
            assert!(!admits_scroll(command));
        }
    }
}
