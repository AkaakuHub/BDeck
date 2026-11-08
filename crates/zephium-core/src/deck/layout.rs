use super::{Deck, DeckLayout};
use crate::geometry::Rect;
use crate::ids::DeckColumnId;

pub const COLUMN_HEADER_HEIGHT: f64 = 30.0;
pub const COMPOSER_HEADER_HEIGHT: f64 = 40.0;

pub fn header_rect(pane: Rect) -> Rect {
    Rect::new(
        pane.x,
        pane.y,
        pane.width,
        pane.height.min(COLUMN_HEADER_HEIGHT),
    )
}

pub fn content_rect(pane: Rect) -> Rect {
    let header = header_rect(pane);
    Rect::new(
        pane.x,
        pane.y + header.height,
        pane.width,
        pane.height - header.height,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColumnViewport {
    pub columns: Vec<(DeckColumnId, Rect)>,
    pub offset: f64,
    pub max_offset: f64,
}

pub fn column_viewport(
    deck: &Deck,
    viewport: Rect,
    requested_offset: f64,
    gap: f64,
) -> ColumnViewport {
    let gap = if gap.is_finite() { gap.max(0.0) } else { 0.0 };
    let rows = deck.layout == DeckLayout::TwoRows;
    let mut widths = [0.0_f64; 2];
    let mut counts = [0; 2];
    for column in deck.columns.iter().filter(|column| !column.suspended) {
        let row = usize::from(rows && column.row == 1);
        widths[row] += f64::from(column.width) + if counts[row] == 0 { 0.0 } else { gap };
        counts[row] += 1;
    }
    let max_offset = (widths[0].max(widths[1]) - viewport.width).max(0.0);
    let offset = if requested_offset.is_finite() {
        requested_offset.clamp(0.0, max_offset)
    } else {
        0.0
    };
    let split = rows && counts[0] > 0 && counts[1] > 0;
    let upper_height = if split {
        (viewport.height - gap).max(0.0) * f64::from(deck.row_ratio) / 100.0
    } else {
        viewport.height
    };
    let mut positions = [viewport.x - offset; 2];
    let columns = deck
        .columns
        .iter()
        .filter(|column| !column.suspended)
        .map(|column| {
            let row = usize::from(rows && column.row == 1);
            let lower = split && row == 1;
            let rect = Rect::new(
                positions[row],
                viewport.y + if lower { upper_height + gap } else { 0.0 },
                f64::from(column.width),
                if lower {
                    (viewport.height - upper_height - gap).max(0.0)
                } else {
                    upper_height
                },
            );
            positions[row] += rect.width + gap;
            (column.id, rect)
        })
        .collect();
    ColumnViewport {
        columns,
        offset,
        max_offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn deck() -> Deck {
        use crate::deck::{ColumnTarget, DeckColumn};
        let mut deck =
            Deck::new(crate::ids::DeckId::from(1), "test", DeckLayout::Horizontal).expect("deck");
        deck.columns = (1..=3)
            .map(|id| DeckColumn {
                id: DeckColumnId::from(id),
                account: crate::ids::DeckAccountId::from(1),
                target: ColumnTarget::Home,
                width: 320,
                row: 0,
                suspended: false,
            })
            .collect();
        deck
    }

    #[test]
    fn column_widths_survive_a_narrow_window_and_scroll_to_the_end() {
        let layout = column_viewport(&deck(), Rect::new(57.0, 48.0, 640.0, 752.0), 9999.0, 1.0);
        assert_eq!(layout.max_offset, 322.0);
        assert_eq!(layout.offset, 322.0);
        assert!(layout.columns.iter().all(|(_, rect)| rect.width == 320.0));
        let last = layout.columns.last().expect("last column").1;
        assert_eq!(last.x + last.width, 697.0);
    }

    #[test]
    fn two_rows_keep_independent_widths_and_share_scroll_offset() {
        let mut deck = deck();
        deck.layout = DeckLayout::TwoRows;
        deck.row_ratio = 60;
        deck.columns[2].row = 1;
        deck.columns[2].width = 480;
        let layout = column_viewport(&deck, Rect::new(0.0, 0.0, 500.0, 601.0), 140.0, 1.0);
        assert_eq!(layout.max_offset, 141.0);
        assert_eq!(layout.columns[0].1, Rect::new(-140.0, 0.0, 320.0, 360.0));
        assert_eq!(layout.columns[2].1, Rect::new(-140.0, 361.0, 480.0, 240.0));
    }

    #[test]
    fn suspended_columns_do_not_reserve_scroll_space() {
        let mut deck = deck();
        deck.columns[1].suspended = true;
        let layout = column_viewport(&deck, Rect::new(0.0, 0.0, 800.0, 600.0), 100.0, 1.0);
        assert_eq!(layout.max_offset, 0.0);
        assert_eq!(layout.offset, 0.0);
        assert_eq!(layout.columns.len(), 2);
        assert_eq!(layout.columns[1].1.x, 321.0);
    }

    #[test]
    fn header_and_page_tile_the_native_pane() {
        let pane = Rect::new(62.0, 420.0, 320.0, 380.0);
        let header = header_rect(pane);
        let page = content_rect(pane);
        assert_eq!(header, Rect::new(62.0, 420.0, 320.0, 30.0));
        assert_eq!(page, Rect::new(62.0, 450.0, 320.0, 350.0));
        assert_eq!(header.height + page.height, pane.height);
    }
    #[test]
    fn short_panes_never_create_negative_native_sizes() {
        assert_eq!(
            content_rect(Rect::new(0.0, 0.0, 200.0, 12.0)),
            Rect::new(0.0, 12.0, 200.0, 0.0)
        );
    }
}
