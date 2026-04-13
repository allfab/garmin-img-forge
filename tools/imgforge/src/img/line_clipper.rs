// Liang-Barsky polyline clipping to an axis-aligned bounding box.
// Faithful port of mkgmap LineClipper.java (uk.me.parabola.mkgmap.general).
//
// Purpose: when a polyline crosses subdivision boundaries, we must emit an
// intersection point that is bit-exact from both sides of the boundary.
// Liang-Barsky on shared edges produces that invariant because the boundary
// value is the same input on both sides and the arithmetic is deterministic.

use super::area::Area;
use super::coord::Coord;

/// Clip a polyline to a bbox. Returns `None` when the polyline is entirely
/// inside the box (fast path, no allocation — caller can reuse the input).
/// Returns `Some(segments)` otherwise; each inner `Vec<Coord>` is a connected
/// segment lying inside the box. An empty outer vec means the polyline lies
/// entirely outside.
pub fn clip_to_bbox(points: &[Coord], bbox: &Area) -> Option<Vec<Vec<Coord>>> {
    if points.len() < 2 {
        return None;
    }
    if points.iter().all(|c| bbox.contains_coord(c)) {
        return None;
    }

    let mut segments: Vec<Vec<Coord>> = Vec::new();
    let mut current: Option<Vec<Coord>> = None;

    for i in 0..points.len() - 1 {
        let a = points[i];
        let b = points[i + 1];
        if a.latitude() == b.latitude() && a.longitude() == b.longitude() {
            continue;
        }
        match clip_segment(a, b, bbox) {
            Some((p0, p1)) => {
                match current.as_mut() {
                    Some(line) => {
                        let last = *line.last().unwrap();
                        if last.latitude() == p0.latitude()
                            && last.longitude() == p0.longitude()
                        {
                            line.push(p1);
                        } else {
                            segments.push(current.take().unwrap());
                            let mut nl = Vec::with_capacity(4);
                            nl.push(p0);
                            nl.push(p1);
                            current = Some(nl);
                        }
                    }
                    None => {
                        let mut nl = Vec::with_capacity(4);
                        nl.push(p0);
                        nl.push(p1);
                        current = Some(nl);
                    }
                }
            }
            None => {
                if let Some(line) = current.take() {
                    segments.push(line);
                }
            }
        }
    }
    if let Some(line) = current.take() {
        segments.push(line);
    }
    Some(segments)
}

/// Clip a single segment `[a, b]` against `bbox` using Liang-Barsky.
/// Returns the (possibly clipped) endpoints, or `None` if fully outside.
fn clip_segment(a: Coord, b: Coord, bbox: &Area) -> Option<(Coord, Coord)> {
    let x0 = a.longitude() as i64;
    let y0 = a.latitude() as i64;
    let x1 = b.longitude() as i64;
    let y1 = b.latitude() as i64;

    let dx = x1 - x0;
    let dy = y1 - y0;

    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;

    // Left boundary:  x >= min_lon  →  p = -dx, q = -(min_lon - x0)
    if !check_side(&mut t0, &mut t1, -dx as f64, -(bbox.min_lon() as i64 - x0) as f64) {
        return None;
    }
    // Right boundary: x <= max_lon  →  p = dx,  q = max_lon - x0
    if !check_side(&mut t0, &mut t1, dx as f64, (bbox.max_lon() as i64 - x0) as f64) {
        return None;
    }
    // Bottom boundary: y >= min_lat →  p = -dy, q = -(min_lat - y0)
    if !check_side(&mut t0, &mut t1, -dy as f64, -(bbox.min_lat() as i64 - y0) as f64) {
        return None;
    }
    // Top boundary: y <= max_lat   →  p = dy,  q = max_lat - y0
    if !check_side(&mut t0, &mut t1, dy as f64, (bbox.max_lat() as i64 - y0) as f64) {
        return None;
    }

    let na = if t0 > 0.0 {
        Coord::new(
            calc_coord(y0, dy, t0) as i32,
            calc_coord(x0, dx, t0) as i32,
        )
    } else {
        a
    };
    let nb = if t1 < 1.0 {
        Coord::new(
            calc_coord(y0, dy, t1) as i32,
            calc_coord(x0, dx, t1) as i32,
        )
    } else {
        b
    };

    if t0 >= t1 {
        return None;
    }
    if na.latitude() == nb.latitude() && na.longitude() == nb.longitude() {
        return None;
    }
    Some((na, nb))
}

/// Liang-Barsky side check. Returns `false` if the segment must be discarded.
fn check_side(t0: &mut f64, t1: &mut f64, p: f64, q: f64) -> bool {
    if p == 0.0 {
        return q >= 0.0;
    }
    let r = q / p;
    if p < 0.0 {
        if r > *t1 {
            return false;
        } else if r > *t0 {
            *t0 = r;
        }
    } else if r < *t0 {
        return false;
    } else if r < *t1 {
        *t1 = r;
    }
    true
}

/// Mirror of mkgmap LineClipper.calcCoord — rounds away from zero by 0.5.
fn calc_coord(base: i64, delta: i64, t: f64) -> i64 {
    let y = base as f64 + t * delta as f64;
    if y >= 0.0 {
        (y + 0.5) as i64
    } else {
        (y - 0.5) as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(lat: i32, lon: i32) -> Coord {
        Coord::new(lat, lon)
    }

    #[test]
    fn fully_inside_returns_none() {
        let bbox = Area::new(0, 0, 100, 100);
        let line = vec![c(10, 10), c(50, 50), c(90, 90)];
        assert!(clip_to_bbox(&line, &bbox).is_none());
    }

    #[test]
    fn fully_outside_returns_empty_vec() {
        let bbox = Area::new(0, 0, 100, 100);
        let line = vec![c(200, 200), c(300, 300)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn crossing_horizontal_produces_boundary_point() {
        let bbox = Area::new(0, 0, 100, 100);
        // Segment crosses the right edge at lon=100
        let line = vec![c(50, 50), c(50, 150)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 2);
        assert_eq!(out[0][0], c(50, 50));
        assert_eq!(out[0][1], c(50, 100));
    }

    #[test]
    fn crossing_is_bit_exact_across_adjacent_bboxes() {
        // Two bboxes sharing edge at lon=100.
        // A diagonal segment crossing from (50,50) to (150,150) must
        // produce the same intersection coordinate when clipped against
        // each bbox.
        let left = Area::new(0, 0, 200, 100);
        let right = Area::new(0, 100, 200, 200);
        let line = vec![c(50, 50), c(150, 150)];

        let l_out = clip_to_bbox(&line, &left).unwrap();
        let r_out = clip_to_bbox(&line, &right).unwrap();

        // Left segment ends at the boundary; right segment starts at it.
        let last_left = *l_out[0].last().unwrap();
        let first_right = r_out[0][0];
        assert_eq!(last_left.longitude(), 100);
        assert_eq!(first_right.longitude(), 100);
        assert_eq!(last_left.latitude(), first_right.latitude());
    }

    #[test]
    fn meandering_polyline_produces_multiple_segments() {
        let bbox = Area::new(0, 0, 100, 100);
        // Enter, exit right at lat=10, re-enter right at lat=90 (different
        // boundary points) → two disconnected segments.
        let line = vec![c(10, 50), c(10, 200), c(90, 200), c(90, 50)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0][0], c(10, 50));
        assert_eq!(out[0][1], c(10, 100));
        assert_eq!(out[1][0], c(90, 100));
        assert_eq!(out[1][1], c(90, 50));
    }

    #[test]
    fn parallel_outside_discarded() {
        let bbox = Area::new(0, 0, 100, 100);
        // Segment above the top edge, parallel to it
        let line = vec![c(200, 10), c(200, 90)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn corner_clipping() {
        let bbox = Area::new(0, 0, 100, 100);
        // From (-50, -50) to (150, 150) crosses through the rectangle diagonally
        let line = vec![c(-50, -50), c(150, 150)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0][0], c(0, 0));
        assert_eq!(out[0][1], c(100, 100));
    }

    #[test]
    fn short_polyline_ignored() {
        let bbox = Area::new(0, 0, 100, 100);
        assert!(clip_to_bbox(&[c(50, 50)], &bbox).is_none());
        assert!(clip_to_bbox(&[], &bbox).is_none());
    }

    #[test]
    fn colinear_with_edge_inside() {
        let bbox = Area::new(0, 0, 100, 100);
        // Segment lying exactly on the left edge (lon=0)
        let line = vec![c(20, 0), c(80, 0)];
        // Entire segment is on the boundary → considered inside
        assert!(clip_to_bbox(&line, &bbox).is_none());
    }

    #[test]
    fn entry_from_outside_to_inside() {
        let bbox = Area::new(0, 0, 100, 100);
        let line = vec![c(-50, 50), c(50, 50)];
        let out = clip_to_bbox(&line, &bbox).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0][0], c(0, 50));
        assert_eq!(out[0][1], c(50, 50));
    }
}
