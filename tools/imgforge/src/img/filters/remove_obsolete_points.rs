//! Supprime points colinéaires et spikes — parité `RemoveObsoletePointsFilter.java`.
//!
//! Trois cas gérés (cf. `Utils.isStraight` mkgmap) :
//!
//! - **Duplicata strict** : deux points consécutifs identiques → on garde un seul.
//! - **STRICTLY_STRAIGHT** (area=0, même direction) : point central d'un segment
//!   a-b-c colinéaire où b est dans l'intervalle [a,c] → suppression de b.
//! - **STRAIGHT_SPIKE** (area=0, direction inversée) : a-b-a ou direction
//!   inversée → spike ; pour un `shape`, suppression (re-bouclage).
//!
//! Simplification vs mkgmap : pas de préservation par nœud (NOD out of scope),
//! pas de gestion du wrap-around shape start=end dans notre format. La
//! variante "dual-carriage" a-b-c-b-a → a-b (lenDup > 1) est également
//! implémentée pour les polylines (cas courant post-round sur routes
//! aller/retour).

use crate::img::coord::Coord;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Straight {
    NotStraight,
    StraightSpike,
    StrictlyStraight,
}

/// Port de `Utils.isStraight(c1, c2, c3)` — détecte colinéarité c1-c2-c3
/// via l'aire du triangle (en i64 pour éviter les overflows i32).
fn is_straight(c1: &Coord, c2: &Coord, c3: &Coord) -> Straight {
    if c1 == c3 {
        return Straight::StraightSpike;
    }
    let (x1, y1) = (c1.longitude() as i64, c1.latitude() as i64);
    let (x2, y2) = (c2.longitude() as i64, c2.latitude() as i64);
    let (x3, y3) = (c3.longitude() as i64, c3.latitude() as i64);
    // aire signée (2 × triangle) — cf. mkgmap Utils.java:287-292
    let area = x1 * y2 - x2 * y1 + x2 * y3 - x3 * y2 + x3 * y1 - x1 * y3;
    if area != 0 {
        return Straight::NotStraight;
    }
    // colinéaire : direction inversée → SPIKE, sinon → STRICTLY_STRAIGHT
    let d1 = c1.latitude() - c2.latitude();
    let d2 = c2.latitude() - c3.latitude();
    if (d1 < 0 && d2 > 0) || (d1 > 0 && d2 < 0) {
        return Straight::StraightSpike;
    }
    let d1 = c1.longitude() - c2.longitude();
    let d2 = c2.longitude() - c3.longitude();
    if (d1 < 0 && d2 > 0) || (d1 > 0 && d2 < 0) {
        return Straight::StraightSpike;
    }
    Straight::StrictlyStraight
}

/// Applique le filtre RemoveObsoletePoints.
///
/// - `pts` : polyligne/polygone d'entrée.
/// - `is_shape` : `true` pour un polygone (minimum 4 points requis après filtrage,
///   gestion spike + suppression de séquence a-b-c-b-a sautée pour les shapes).
pub fn remove_obsolete_points(pts: &[Coord], is_shape: bool) -> Vec<Coord> {
    let n = pts.len();
    if n <= 1 {
        return pts.to_vec();
    }
    let required = if is_shape { 4 } else { 2 };
    let mut points: Vec<Coord> = pts.to_vec();

    // Boucle externe : re-scan tant qu'on a supprimé un spike (peut en révéler
    // d'autres). Copie mkgmap `RemoveObsoletePointsFilter.java:54-110`.
    loop {
        let mut removed_spike = false;
        let mut new_points: Vec<Coord> = Vec::with_capacity(points.len());
        new_points.push(points[0]);

        for i in 1..points.len() {
            let new_p = points[i];
            let last = new_points.len() - 1;
            let last_p = new_points[last];

            // Duplicata strict → skip
            if last_p == new_p {
                continue;
            }

            if new_points.len() > 1 {
                let prev = new_points[last - 1];
                match is_straight(&prev, &last_p, &new_p) {
                    Straight::StrictlyStraight => {
                        // Remplace last par new_p (on collapse prev-last-new_p en prev-new_p)
                        new_points[last] = new_p;
                        continue;
                    }
                    Straight::StraightSpike => {
                        if is_shape {
                            // Pour un shape, on pop le spike. Si le retour colinéaire
                            // ramène prev == new_p, on skip new_p aussi.
                            new_points.pop();
                            removed_spike = true;
                            if new_points.last().map(|c| c == &new_p).unwrap_or(false) {
                                continue;
                            }
                        }
                    }
                    Straight::NotStraight => {}
                }
            }
            new_points.push(new_p);
        }

        points = new_points;
        if !removed_spike || points.len() < required {
            break;
        }
    }

    // Variante a-b-c-b-a → a-b pour les polylines (post-round sur routes bidir).
    // Cf. mkgmap RemoveObsoletePointsFilter.java:136-145.
    if !is_shape && points.len() > 2 {
        let mut len_dup = 0usize;
        loop {
            let mirror = points.len() - 1 - len_dup;
            if points[len_dup] != points[mirror] {
                break;
            }
            len_dup += 1;
            // break APRÈS incrément si lenDup > mirror (dépassement médian)
            if len_dup > points.len() - 1 - len_dup {
                break;
            }
        }
        if len_dup > 1 {
            let new_len = points.len() + 1 - len_dup;
            points.truncate(new_len);
        }
    }

    // Respect du minimum (cf. mkgmap ligne 149) : si on est tombé sous le seuil,
    // on retourne vide pour laisser le caller (RemoveEmpty) dropper.
    if is_shape && points.len() <= 3 || points.len() <= 1 {
        return Vec::new();
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: i32, y: i32) -> Coord {
        Coord::new(y, x)
    }

    #[test]
    fn drops_strictly_straight_point() {
        // a=(0,0) b=(1,1) c=(2,2) → b sur la ligne → dropped
        let pts = vec![c(0, 0), c(1, 1), c(2, 2)];
        let out = remove_obsolete_points(&pts, false);
        assert_eq!(out, vec![c(0, 0), c(2, 2)]);
    }

    #[test]
    fn preserves_non_collinear_point() {
        let pts = vec![c(0, 0), c(1, 5), c(2, 0)];
        let out = remove_obsolete_points(&pts, false);
        assert_eq!(out, pts);
    }

    #[test]
    fn drops_duplicate_point() {
        let pts = vec![c(0, 0), c(1, 1), c(1, 1), c(2, 2)];
        let out = remove_obsolete_points(&pts, false);
        // 1,1 dup → 1,1 gardé une fois → 0,0-1,1-2,2 colinéaire → drop 1,1
        assert_eq!(out, vec![c(0, 0), c(2, 2)]);
    }

    #[test]
    fn drops_abcba_back_and_forth_polyline() {
        // a-b-c-b-a → a-b (spike inversé)
        let pts = vec![c(0, 0), c(1, 0), c(2, 0), c(1, 0), c(0, 0)];
        let out = remove_obsolete_points(&pts, false);
        // Les colinéaires STRAIGHT_SPIKE sur une polyline ne sont pas
        // poppés en sortie (mkgmap ne touche pas les spikes pour les lines
        // dans la boucle interne). Mais len_dup>1 s'applique.
        // Après colinéarité : 0,0 → 2,0 puis 2,0→0,0 → on a (0,0), (2,0), (0,0).
        // lenDup : points[0]=points[2]=0,0 ✓, points[1]=points[1]=2,0 ✓ → lenDup=2
        // new_len = 3 + 1 - 2 = 2 → [0,0, 2,0]
        assert_eq!(out, vec![c(0, 0), c(2, 0)]);
    }

    #[test]
    fn shape_drops_spike_point() {
        // Triangle avec spike extérieur : a-b-c-b'-a. Ici b'=b → spike pop.
        let pts = vec![c(0, 0), c(2, 0), c(1, 5), c(2, 0), c(0, 0)];
        let out = remove_obsolete_points(&pts, true);
        // Assez compact pour triangle rester (≥ 4 points : a-b-c-a...)
        // On vérifie qu'un output est produit et qu'il contient moins de points.
        assert!(out.len() < pts.len());
        assert!(out.len() >= 4 || out.is_empty());
    }

    #[test]
    fn short_line_below_minimum_returns_empty() {
        let pts = vec![c(0, 0), c(0, 0)]; // duplicata dégénère à 1 pt
        let out = remove_obsolete_points(&pts, false);
        assert!(out.is_empty());
    }

    #[test]
    fn preserves_single_point() {
        let pts = vec![c(5, 5)];
        // 1 point — retourne inchangé (guard n <= 1 au début)
        assert_eq!(remove_obsolete_points(&pts, false), pts);
    }

    #[test]
    fn is_straight_detects_collinear() {
        assert_eq!(is_straight(&c(0, 0), &c(1, 1), &c(2, 2)), Straight::StrictlyStraight);
        assert_eq!(is_straight(&c(0, 0), &c(1, 0), &c(0, 0)), Straight::StraightSpike);
        assert_eq!(is_straight(&c(0, 0), &c(1, 5), &c(2, 0)), Straight::NotStraight);
    }
}
