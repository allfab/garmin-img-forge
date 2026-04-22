//! Parité `RemoveEmpty.java` — rejette polyline ≤ 1 pt et shape ≤ 3 pts.

use crate::img::coord::Coord;

/// `true` si le feature passe le filtre (≥ 2 pts pour polyline, ≥ 4 pts pour shape).
pub fn passes_remove_empty(pts: &[Coord], is_shape: bool) -> bool {
    if is_shape {
        pts.len() > 3
    } else {
        pts.len() > 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::img::coord::Coord;

    fn c(x: i32, y: i32) -> Coord {
        Coord::new(y, x)
    }

    #[test]
    fn rejects_single_point_line() {
        assert!(!passes_remove_empty(&[c(0, 0)], false));
    }

    #[test]
    fn accepts_two_point_line() {
        assert!(passes_remove_empty(&[c(0, 0), c(1, 1)], false));
    }

    #[test]
    fn rejects_three_point_shape() {
        assert!(!passes_remove_empty(&[c(0, 0), c(1, 0), c(0, 1)], true));
    }

    #[test]
    fn accepts_four_point_shape() {
        assert!(passes_remove_empty(
            &[c(0, 0), c(1, 0), c(0, 1), c(0, 0)],
            true
        ));
    }
}
