// overview_features.rs — extraction des features overview depuis un MP parsé
//
// Utilisé par imgforge Phase 2 pour alimenter build_overview_map en features réelles
// (polygones extraits des DataN wide-zoom produits par mpforge overview_levels).
// Phase 2 encode uniquement les polygones ; les polylignes sont réservées à une phase ultérieure.
//
// Prérequis mpforge : les MP doivent exposer des DataN pour N >= detail_max_level avec
// les EndLevel réécrits par overview_levels.promotion (cf. tech-spec mpforge-generalize-
// profiles-overview). Sans cela, extract_overview_features retourne Vec::new() et
// build_overview_map utilise le fallback bounding-box (comportement Phase 1).

use std::collections::BTreeMap;

use crate::img::coord::Coord;
use crate::parser::mp_types::MpFile;

/// Niveau de détail maximal pour les maps standard (7 niveaux 24/23/22/21/20/18/16).
/// Les features avec EndLevel >= cette valeur sont des features overview (DataN wide-zoom).
pub const OVERVIEW_DETAIL_MAX_LEVEL: u8 = 7;

/// Nombre de paliers overview Phase 2 : bits 10/12/14/16.
pub const OVERVIEW_NB_PALIERS: u8 = 4;

/// Feature overview : une entité extraite d'un bucket DataN du MP,
/// destinée à être encodée dans un subdiv de l'overview map.
#[derive(Debug, Clone)]
pub struct OverviewFeature {
    pub type_code: u32,
    pub end_level: u8,
    pub geometry: Vec<Coord>,
    pub is_polygon: bool,
    /// Index de palier : 0 = bits 16 (finest), 1 = bits 14, 2 = bits 12, 3 = bits 10 (coarsest).
    /// Déterminé par `end_level - detail_max_level` (clampé à nb_overview_levels-1).
    pub palier_index: u8,
}

/// Extrait les features overview (polygones uniquement) d'un MP parsé.
///
/// `detail_max_level` : niveau de détail maximal — cf. `OVERVIEW_DETAIL_MAX_LEVEL`.
/// Features avec EndLevel < detail_max_level → ignorées (features détail uniquement).
///
/// `nb_overview_levels` : nombre de paliers overview — cf. `OVERVIEW_NB_PALIERS`.
///
/// Retourne Vec::new() si aucune feature éligible → build_overview_map bascule sur
/// le fallback bounding-box Phase 1.
///
/// Note: seuls les polygones sont extraits. Les polylignes overview (routes, fleuves)
/// nécessitent un encodeur RGN polyline Phase 3 (non implémenté).
pub fn extract_overview_features(
    mp: &MpFile,
    detail_max_level: u8,
    nb_overview_levels: u8,
) -> Vec<OverviewFeature> {
    let mut features = Vec::new();
    let max_palier = nb_overview_levels.saturating_sub(1);

    for polygon in &mp.polygons {
        let end_level = match polygon.end_level {
            Some(el) if el >= detail_max_level => el,
            _ => continue,
        };
        let palier_index = end_level.saturating_sub(detail_max_level).min(max_palier);
        let target_n = detail_max_level.saturating_add(palier_index);
        let geom = pick_bucket(&polygon.geometries, target_n);
        if geom.is_empty() {
            continue;
        }
        features.push(OverviewFeature {
            type_code: polygon.type_code,
            end_level,
            geometry: geom.to_vec(),
            is_polygon: true,
            palier_index,
        });
    }

    if features.is_empty() {
        tracing::warn!(
            "Aucune feature overview polygone au niveau >={detail_max_level} — \
             build_overview_map utilisera le fallback bounding-box"
        );
    }

    features
}

/// Sélectionne le bucket DataN le plus approprié pour un palier donné.
///
/// Priorité : exact target_n → coarser (N > target_n) → finer (N ≤ target_n, highest available).
/// Le fallback finer prend le bucket disponible avec le plus grand N inférieur à target_n,
/// c'est-à-dire la géométrie la moins simplifiée disponible en-dessous du palier cible.
fn pick_bucket<'a>(geometries: &'a BTreeMap<u8, Vec<Coord>>, target_n: u8) -> &'a [Coord] {
    if let Some(g) = geometries.get(&target_n) {
        if !g.is_empty() {
            return g;
        }
    }
    if let Some(next) = target_n.checked_add(1) {
        if let Some((_, g)) = geometries.range(next..).next() {
            if !g.is_empty() {
                tracing::debug!(
                    "pick_bucket: pas de bucket exact à {target_n}, fallback coarser"
                );
                return g;
            }
        }
    }
    if let Some((_, g)) = geometries.range(..=target_n).next_back() {
        if !g.is_empty() {
            tracing::debug!(
                "pick_bucket: pas de bucket coarser, fallback finer (highest available below target {target_n})"
            );
            return g;
        }
    }
    &[]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use crate::parser::mp_types::{MpFile, MpHeader, MpPolygon, MpPolyline};

    fn make_mp(polygons: Vec<MpPolygon>, polylines: Vec<MpPolyline>) -> MpFile {
        MpFile {
            header: MpHeader::default(),
            points: Vec::new(),
            polylines,
            polygons,
        }
    }

    fn make_polygon(
        type_code: u32,
        end_level: Option<u8>,
        buckets: &[(u8, Vec<Coord>)],
    ) -> MpPolygon {
        let mut geometries = BTreeMap::new();
        for (n, coords) in buckets {
            geometries.insert(*n, coords.clone());
        }
        MpPolygon {
            type_code,
            label: String::new(),
            geometries,
            end_level,
        }
    }

    fn c(lat: i32, lon: i32) -> Coord {
        Coord::new(lat, lon)
    }

    fn tri() -> Vec<Coord> {
        vec![c(2138930, 255409), c(2143196, 255409), c(2143196, 262632)]
    }

    #[test]
    fn test_extract_no_features_below_detail_max() {
        let mp = make_mp(
            vec![make_polygon(0x4A, Some(5), &[(5, tri())])],
            vec![],
        );
        let features = extract_overview_features(&mp, 7, 4);
        assert!(features.is_empty(), "EndLevel 5 < detail_max_level 7 → exclu");
    }

    #[test]
    fn test_extract_no_end_level_excluded() {
        let mp = make_mp(
            vec![make_polygon(0x4A, None, &[(7, tri())])],
            vec![],
        );
        let features = extract_overview_features(&mp, 7, 4);
        assert!(features.is_empty(), "EndLevel absent → exclu");
    }

    #[test]
    fn test_extract_palier_assignment() {
        let mp = make_mp(
            vec![
                make_polygon(0x4A, Some(7), &[(7, tri())]),
                make_polygon(0x4B, Some(8), &[(8, tri())]),
                make_polygon(0x50, Some(9), &[(9, tri())]),
                make_polygon(0x51, Some(10), &[(9, tri())]), // clamped to max_palier=3
            ],
            vec![],
        );
        let features = extract_overview_features(&mp, 7, 4);
        assert_eq!(features.len(), 4);
        let find = |tc: u32| features.iter().find(|f| f.type_code == tc).unwrap().palier_index;
        assert_eq!(find(0x4A), 0, "EndLevel=7 → palier 0 (bits16)");
        assert_eq!(find(0x4B), 1, "EndLevel=8 → palier 1 (bits14)");
        assert_eq!(find(0x50), 2, "EndLevel=9 → palier 2 (bits12)");
        assert_eq!(find(0x51), 3, "EndLevel=10 → palier 3 (bits10, clampé)");
    }

    #[test]
    fn test_extract_empty_geometry_skipped() {
        let mp = make_mp(
            vec![make_polygon(0x4A, Some(7), &[])],
            vec![],
        );
        let features = extract_overview_features(&mp, 7, 4);
        assert!(features.is_empty(), "Feature sans géométrie → ignorée");
    }

    #[test]
    fn test_pick_bucket_exact() {
        let mut g = BTreeMap::new();
        g.insert(7u8, tri());
        assert_eq!(pick_bucket(&g, 7).len(), 3, "bucket exact trouvé");
    }

    #[test]
    fn test_pick_bucket_fallback_coarser() {
        let mut g = BTreeMap::new();
        g.insert(9u8, tri()); // seul bucket disponible, plus grossier que target=7
        assert_eq!(pick_bucket(&g, 7).len(), 3, "fallback coarser (bucket 9 pour target 7)");
    }

    #[test]
    fn test_pick_bucket_fallback_finer() {
        let mut g = BTreeMap::new();
        g.insert(5u8, tri()); // seul bucket, plus fin que target=7
        assert_eq!(pick_bucket(&g, 7).len(), 3, "fallback finer (bucket 5 pour target 7)");
    }

    #[test]
    fn test_pick_bucket_empty_map() {
        let g: BTreeMap<u8, Vec<Coord>> = BTreeMap::new();
        assert!(pick_bucket(&g, 7).is_empty(), "map vide → slice vide");
    }

    // AC 9 : éligibilité par EndLevel (tech-spec Phase 2)
    #[test]
    fn test_ac9_eligibility_by_end_level() {
        // feature EndLevel=8, detail_max=7, nb_levels=4
        // → palier_index = 8-7 = 1 (bits14)
        // → apparaît dans bits14 mais PAS dans bits10 (palier_index=3)
        let mp = make_mp(
            vec![make_polygon(0x50, Some(8), &[(8, tri())])],
            vec![],
        );
        let features = extract_overview_features(&mp, 7, 4);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].palier_index, 1, "EndLevel=8 → palier 1 (bits14)");
        assert!(
            !features.iter().any(|f| f.palier_index == 3),
            "EndLevel=8 ne doit pas apparaître dans bits10 (palier_index=3)"
        );
    }
}
