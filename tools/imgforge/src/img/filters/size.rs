//! Parité `SizeFilter.java` — rejette feature dont bbox max-dim < `size << shift`.
//!
//! Valeur `MIN_SIZE_LINE = 1` utilisée dans MapBuilder (ligne 136) :
//! au level 5 (shift 6), le seuil effectif est 64 units (≈ 0.9 m après
//! conversion mu24→degrés).
//!
//! Simplification vs mkgmap : pas de `keepRoads` (NOD hors scope) ni
//! `isSkipSizeFilter`. Application au shift courant uniquement.

use crate::img::coord::Coord;

/// `true` si bbox max-dim ≥ `size * (1 << shift)`.
/// À shift=0 ou pts vides, toujours `true` (no-op).
pub fn passes_size_filter(pts: &[Coord], shift: u32, size: u32) -> bool {
    if pts.is_empty() || shift == 0 {
        return true;
    }
    let threshold: i64 = (size as i64) * (1i64 << shift);
    let (mut min_lat, mut max_lat) = (i32::MAX, i32::MIN);
    let (mut min_lon, mut max_lon) = (i32::MAX, i32::MIN);
    for p in pts {
        let lat = p.latitude();
        let lon = p.longitude();
        if lat < min_lat { min_lat = lat; }
        if lat > max_lat { max_lat = lat; }
        if lon < min_lon { min_lon = lon; }
        if lon > max_lon { max_lon = lon; }
    }
    let dim_lat = (max_lat as i64) - (min_lat as i64);
    let dim_lon = (max_lon as i64) - (min_lon as i64);
    dim_lat.max(dim_lon) >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: i32, y: i32) -> Coord {
        Coord::new(y, x)
    }

    #[test]
    fn shift_zero_always_passes() {
        let pts = vec![c(0, 0), c(0, 0)];
        assert!(passes_size_filter(&pts, 0, 1));
    }

    #[test]
    fn rejects_below_threshold() {
        // shift=6, size=1 → threshold=64. bbox max-dim = 32 < 64.
        let pts = vec![c(0, 0), c(32, 32)];
        assert!(!passes_size_filter(&pts, 6, 1));
    }

    #[test]
    fn accepts_at_threshold() {
        // bbox max-dim = 64 = threshold → pass
        let pts = vec![c(0, 0), c(64, 0)];
        assert!(passes_size_filter(&pts, 6, 1));
    }

    #[test]
    fn accepts_above_threshold() {
        let pts = vec![c(0, 0), c(100, 100)];
        assert!(passes_size_filter(&pts, 6, 1));
    }

    #[test]
    fn empty_always_passes() {
        assert!(passes_size_filter(&[], 6, 1));
    }
}
