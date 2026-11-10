#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Direction {
    Back,
    Forward,
}

#[derive(Default)]
pub(super) struct Gesture {
    x: f64,
    y: f64,
    finished: bool,
}

impl Gesture {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn advance(&mut self, x: f64, y: f64) -> Option<Direction> {
        if self.finished || !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.x += x;
        self.y += y;
        if self.y.abs() >= 12.0 && self.y.abs() >= self.x.abs() {
            self.finished = true;
            return None;
        }
        if self.x.abs() < 48.0 || self.x.abs() < self.y.abs() * 1.5 {
            return None;
        }
        self.finished = true;
        Some(if self.x > 0.0 {
            Direction::Back
        } else {
            Direction::Forward
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horizontal_gesture_navigates_once_without_waiting_for_release() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.advance(20.0, 1.0), None);
        assert_eq!(gesture.advance(30.0, 2.0), Some(Direction::Back));
        assert_eq!(gesture.advance(100.0, 0.0), None);
        gesture.reset();
        assert_eq!(gesture.advance(-50.0, 0.0), Some(Direction::Forward));
    }

    #[test]
    fn a_new_touch_rearms_immediately_without_a_time_based_cooldown() {
        let mut gesture = Gesture::default();
        for _ in 0..3 {
            gesture.reset();
            assert_eq!(gesture.advance(50.0, 0.0), Some(Direction::Back));
            assert_eq!(gesture.advance(50.0, 0.0), None);
        }
    }

    #[test]
    fn vertical_scrolling_cannot_turn_into_a_history_gesture() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.advance(4.0, 20.0), None);
        assert_eq!(gesture.advance(100.0, 0.0), None);
    }

    #[test]
    fn small_or_invalid_deltas_do_not_navigate() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.advance(f64::NAN, 0.0), None);
        assert_eq!(gesture.advance(f64::INFINITY, 0.0), None);
        assert_eq!(gesture.advance(12.0, 1.0), None);
        assert_eq!(gesture.advance(-12.0, -1.0), None);
    }
}
