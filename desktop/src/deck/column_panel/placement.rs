#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Placement {
    pub left: f64,
    pub top: f64,
    max_height: f64,
}

impl Placement {
    pub fn new(anchor: (f64, f64), viewport: (f64, f64), width: f64, height: f64) -> Self {
        let height = height.ceil().min((viewport.1 - 16.0).max(100.0));
        let left = (anchor.0 - width).clamp(8.0, (viewport.0 - width - 8.0).max(8.0));
        let top = if anchor.1 + height + 8.0 <= viewport.1 {
            anchor.1 + 6.0
        } else {
            (anchor.1 - height - 32.0).max(8.0)
        };
        Self {
            left,
            top,
            max_height: (viewport.1 - top - 8.0).max(100.0),
        }
    }

    pub fn constrained(self, viewport: (f64, f64), width: f64) -> Self {
        let left = self.left.clamp(8.0, (viewport.0 - width - 8.0).max(8.0));
        let top = self.top.clamp(8.0, (viewport.1 - 108.0).max(8.0));
        Self {
            left,
            top,
            max_height: (viewport.1 - top - 8.0).max(100.0),
        }
    }

    pub fn height(self, requested: f64) -> f64 {
        requested.ceil().min(self.max_height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growing_content_keeps_the_original_position_and_uses_available_space() {
        let placement = Placement::new((500.0, 300.0), (1200.0, 900.0), 336.0, 400.0);
        assert_eq!((placement.left, placement.top), (164.0, 306.0));
        assert_eq!(placement.height(500.0), 500.0);
        assert_eq!(placement.height(640.0), 586.0);
        assert_eq!((placement.left, placement.top), (164.0, 306.0));
    }

    #[test]
    fn shrinking_an_above_panel_does_not_flip_it_below_its_button() {
        let placement = Placement::new((500.0, 700.0), (1200.0, 900.0), 336.0, 400.0);
        assert_eq!(placement.top, 268.0);
        assert_eq!(placement.height(100.0), 100.0);
        assert_eq!(placement.top, 268.0);
    }

    #[test]
    fn a_panel_at_the_viewport_edge_scrolls_instead_of_moving() {
        let placement = Placement::new((52.0, 750.0), (1200.0, 900.0), 336.0, 100.0);
        assert_eq!((placement.left, placement.top), (8.0, 756.0));
        assert_eq!(placement.height(400.0), 136.0);
    }

    #[test]
    fn panel_stays_inside_a_parent_that_was_resized_to_its_minimum_width() {
        let original = Placement::new((1100.0, 700.0), (1200.0, 900.0), 336.0, 400.0);
        let fitted = original.constrained((320.0, 480.0), 304.0);
        assert_eq!(fitted.left, 8.0);
        assert!(fitted.top >= 8.0);
        assert!(fitted.top + fitted.height(640.0) <= 472.0);
    }
}
