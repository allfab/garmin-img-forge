//! Quantification bits-world shift-aware — parité `RoundCoordsFilter.java`.
//!
//! Au level 5 (bits 18 → shift 6), les coordonnées sont arrondies au
//! bucket de 64 units (`(coord + 32) & !63`). Les points qui tombent
//! dans le même bucket sont collapsés en un seul. À shift=0 (level 0),
//! no-op et retour direct.
//!
//! Simplification par rapport à mkgmap : pas de CoordNode/preserved/
//! isNumberNode (NOD hors scope). Pas de branche contour `find best
//! match` (rare et coûteuse). Porte uniquement la quantification
//! standard + dédoublonnage successif.

use crate::img::coord::Coord;

/// Arrondit les coordonnées de `pts` au bucket de `1 << shift` units.
/// Dédoublonne les points successifs identiques après arrondi.
///
/// `shift` = 24 - bits_per_coord (0 pour level 0 → no-op).
pub fn round_coords(pts: &[Coord], shift: u32) -> Vec<Coord> {
    if shift == 0 || pts.is_empty() {
        return pts.to_vec();
    }
    let half: i32 = 1 << (shift - 1);
    let mask: i32 = !((1 << shift) - 1);
    let mut out: Vec<Coord> = Vec::with_capacity(pts.len());
    for p in pts {
        let lat = (p.latitude().wrapping_add(half)) & mask;
        let lon = (p.longitude().wrapping_add(half)) & mask;
        let rp = Coord::new(lat, lon);
        if out.last().map(|last| last == &rp).unwrap_or(false) {
            continue;
        }
        out.push(rp);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_zero_is_noop() {
        let pts = vec![Coord::new(100, 200), Coord::new(103, 204)];
        assert_eq!(round_coords(&pts, 0), pts);
    }

    #[test]
    fn shift_six_buckets_to_64() {
        // shift=6 ⇒ mask=!63=0xffff_ffc0, half=32.
        // (10 + 32) & !63 = 42 & 0xffffffc0 = 0
        // (50 + 32) & !63 = 82 & 0xffffffc0 = 64
        let pts = vec![Coord::new(10, 10), Coord::new(50, 50)];
        let out = round_coords(&pts, 6);
        assert_eq!(out, vec![Coord::new(0, 0), Coord::new(64, 64)]);
    }

    #[test]
    fn collapses_points_in_same_bucket() {
        // Three consecutive points all within shift=6 bucket → one output
        let pts = vec![
            Coord::new(10, 10),
            Coord::new(11, 12),
            Coord::new(20, 25),
        ];
        let out = round_coords(&pts, 6);
        assert_eq!(out, vec![Coord::new(0, 0)]);
    }

    #[test]
    fn negative_coords_rounded_correctly() {
        // wrapping_add handles i32 overflow; negative lat/lon must round
        // towards nearest bucket (still deterministic).
        let pts = vec![Coord::new(-10, -10), Coord::new(-70, -70)];
        let out = round_coords(&pts, 6);
        // -10 + 32 = 22, 22 & !63 = 0 → 0
        // -70 + 32 = -38, -38 & !63 = -64 → -64
        assert_eq!(out, vec![Coord::new(0, 0), Coord::new(-64, -64)]);
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(round_coords(&[], 6).is_empty());
    }
}
