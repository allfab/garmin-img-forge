//! Geometry generalization: smoothing and simplification.
//!
//! Applies Chaikin corner-cutting smoothing and/or Douglas-Peucker
//! simplification to feature geometries. Configured per-layer via
//! the `generalize` directive in source YAML configuration.

use crate::config::{GeneralizeConfig, GeneralizeProfile, LevelSpec, OverviewLevels};
use crate::pipeline::promotion;
use crate::pipeline::reader::{Feature, GeometryType};
use geo::{ChaikinSmoothing, LineString, Polygon, Simplify, SimplifyVw, SimplifyVwIdx, SimplifyVwPreserve};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use tracing::warn;

/// Apply generalization (smooth + simplify) to a feature in-place.
///
/// Only applies to LineString and Polygon geometries (Points are unchanged).
/// Returns `true` if the geometry was actually modified.
pub fn generalize_feature(feature: &mut Feature, config: &GeneralizeConfig) -> bool {
    match feature.geometry_type {
        GeometryType::Point => false,
        GeometryType::LineString => generalize_linestring(feature, config),
        GeometryType::Polygon => generalize_polygon(feature, config),
    }
}

/// Tech-spec #2 Task 9: resolve the effective list of levels for a feature,
/// honouring `when` clauses (first match wins). Returns `None` when no
/// applicable levels are found (profile empty both in default and all
/// unmatched branches).
fn resolve_levels<'a>(
    profile: &'a GeneralizeProfile,
    feature: &Feature,
) -> Option<&'a [LevelSpec]> {
    // Dispatch sur les attributs BDTOPO source (pré-règles) quand disponibles.
    let attrs = feature.source_attributes.as_ref().unwrap_or(&feature.attributes);
    for clause in &profile.when {
        if let Some(val) = attrs.get(&clause.field) {
            if clause.values.iter().any(|v| v == val) {
                return Some(&clause.levels);
            }
        }
    }
    if profile.levels.is_empty() {
        None
    } else {
        Some(&profile.levels)
    }
}

/// Tech-spec #2 Task 9: apply a multi-level profile to a feature in-place.
///
/// The raw geometry captured from the reader is kept in memory and used as
/// the source for every bucket (no stacking of simplifications between
/// levels). After running:
///   - `feature.geometry` contains the result for `n=0` (simplified if the
///     profile declares a `LevelSpec { n: 0, .. }`; otherwise left unchanged).
///   - `feature.additional_geometries[n]` is populated for every `n > 0`
///     declared in the resolved level list.
///
/// Point features are left unchanged. Returns the number of buckets
/// generated (including n=0 when it was derived).
pub fn apply_profile(feature: &mut Feature, profile: &GeneralizeProfile) -> usize {
    if matches!(feature.geometry_type, GeometryType::Point) {
        return 0;
    }
    let Some(levels) = resolve_levels(profile, feature) else {
        return 0;
    };

    // Capture raw geometry once — every bucket derives from it, not from the
    // previous level's output (clarification F17 of adversarial review).
    let raw = feature.geometry.clone();
    let mut generated = 0;

    for lvl in levels {
        let derived = match feature.geometry_type {
            GeometryType::LineString => apply_level_to_line(&raw, lvl),
            GeometryType::Polygon => apply_level_to_polygon(&raw, lvl),
            GeometryType::Point => None,
        };
        match derived {
            Some(coords) => {
                // H3 code review : passe par le setter unifié qui route n=0
                // vers `geometry` et n≥1 vers `additional_geometries`, en
                // garantissant l'invariant structurel.
                feature.set_level(lvl.n, coords);
                generated += 1;
            }
            None => {
                // M4 code review : alerte explicite quand un niveau devient
                // dégénéré (simplify trop agressif, raw trop court). Si c'est
                // le bucket `n=0`, `feature.geometry` reste = raw (silencieux
                // sinon, le consommateur ne sait pas pourquoi Data0 n'a pas
                // bougé).
                warn!(
                    source_layer = feature.source_layer.as_deref().unwrap_or(""),
                    n = lvl.n,
                    raw_points = raw.len(),
                    simplify = ?lvl.simplify,
                    iterations = lvl.iterations,
                    "level produced no valid geometry (too few points or empty after simplify); \
                     bucket skipped — for n=0, raw geometry preserved unchanged"
                );
            }
        }
    }
    generated
}

/// Build an ephemeral `GeneralizeConfig` mirroring the smooth/simplify knobs
/// of a `LevelSpec`, so the existing single-level helpers can be reused.
fn level_as_config(lvl: &LevelSpec) -> GeneralizeConfig {
    GeneralizeConfig {
        smooth: lvl.smooth.clone(),
        iterations: lvl.iterations,
        simplify: lvl.simplify,
    }
}

fn apply_level_to_line(raw: &[(f64, f64)], lvl: &LevelSpec) -> Option<Vec<(f64, f64)>> {
    if raw.len() < 2 {
        return None;
    }
    let cfg = level_as_config(lvl);
    let coords: Vec<geo::Coord<f64>> =
        raw.iter().map(|(x, y)| geo::Coord { x: *x, y: *y }).collect();
    let mut line = LineString::new(coords);
    if let Some(smoothed) = apply_smooth_line(line.clone(), &cfg) {
        line = smoothed;
    }
    if let Some(tolerance) = cfg.simplify {
        line = line.simplify(&tolerance);
    } else if let Some(tolerance) = lvl.simplify_vw {
        line = line.simplify_vw_preserve(&tolerance);
    }
    if line.0.len() < 2 {
        return None;
    }
    Some(line.0.iter().map(|c| (c.x, c.y)).collect())
}

fn apply_level_to_polygon(raw: &[(f64, f64)], lvl: &LevelSpec) -> Option<Vec<(f64, f64)>> {
    if raw.len() < 4 {
        return None;
    }
    let cfg = level_as_config(lvl);
    let coords: Vec<geo::Coord<f64>> =
        raw.iter().map(|(x, y)| geo::Coord { x: *x, y: *y }).collect();
    let ring = LineString::new(coords);
    let mut polygon = Polygon::new(ring, vec![]);
    if let Some(smoothed) = apply_smooth_polygon(polygon.clone(), &cfg) {
        polygon = smoothed;
    }
    if let Some(tolerance) = cfg.simplify {
        polygon = polygon.simplify(&tolerance);
    } else if let Some(tolerance) = lvl.simplify_vw {
        polygon = polygon.simplify_vw(&tolerance);
    }
    let exterior = polygon.exterior();
    if exterior.0.len() < 4 {
        return None;
    }
    Some(exterior.0.iter().map(|c| (c.x, c.y)).collect())
}

/// Tech-spec #2 Task 9: multi-profile entry point for the pipeline. Dispatches
/// each feature to its profile by `source_layer` (exact match). Returns the
/// total number of features that produced at least one bucket.
///
/// Après l'application du profil, les trous éventuels dans les index
/// `Data0..DataK` sont comblés par clonage du palier précédent (cf.
/// [`fill_level_gaps`]) — obligatoire pour le rendu sur certains firmwares
/// Garmin (Alpha 100 confirmé) qui exigent des sections `DataN=` contiguës
/// depuis `Data0`.
pub fn generalize_features_with_profiles(
    features: &mut [Feature],
    profile_map: &BTreeMap<String, GeneralizeProfile>,
    overview: Option<&OverviewLevels>,
) -> usize {
    if profile_map.is_empty() {
        return 0;
    }

    // Passe 1 (immutable) : collecter les ancres pour les couches topology: true.
    let topology_anchors: HashMap<String, HashSet<(u64, u64)>> = {
        let mut by_layer: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, f) in features.iter().enumerate() {
            let Some(layer) = f.source_layer.as_deref() else { continue };
            if profile_map.get(layer).map(|p| p.topology).unwrap_or(false) {
                by_layer.entry(layer.to_string()).or_default().push(i);
            }
        }
        by_layer
            .into_iter()
            .map(|(layer, idxs)| {
                let refs: Vec<&Feature> = idxs.iter().map(|&i| &features[i]).collect();
                (layer, collect_shared_vertices(&refs))
            })
            .collect()
    };

    let empty_anchors: HashSet<(u64, u64)> = HashSet::new();

    // Passe 2 (mutable) : application des profils.
    let mut count = 0;
    for feature in features.iter_mut() {
        let Some(layer_name) = feature.source_layer.as_deref() else { continue; };
        let layer_name = layer_name.to_string();
        // Features dégénérées en entrée (GDAL clipping aux bords de tuile) :
        // une LineString à 1 point est intrinsèquement invalide — skip silencieux
        // avant tout traitement pour éviter le warning AC17 du writer.
        if matches!(feature.geometry_type, GeometryType::LineString)
            && feature.geometry.len() < 2
        {
            continue;
        }
        if let Some(profile) = profile_map.get(&layer_name) {
            let generated = if profile.topology {
                let anchors = topology_anchors.get(&layer_name).unwrap_or(&empty_anchors);
                apply_profile_with_topology(feature, profile, anchors)
            } else {
                apply_profile(feature, profile)
            };
            if generated > 0 {
                // F1 fix : le max de remplissage est borné à la branche `when`
                // effectivement résolue pour cette feature (pas au max global
                // du profil). Sinon une feature qui match une branche courte
                // (ex: Communale n=0..6) serait paddée jusqu'au max d'une
                // autre branche (ex: Autoroute n=0..9) — brise l'AC7/8.
                let branch_levels = resolve_levels(profile, feature);
                let branch_max = branch_levels
                    .map(|ls| ls.iter().map(|l| l.n).max().unwrap_or(0))
                    .unwrap_or(0);

                // F4 fix : la promotion overview force le remplissage jusqu'au
                // palier cible MÊME si la branche ne déclare pas ce n. Les
                // paliers manquants sont clonés par `fill_level_gaps` depuis
                // le dernier palier effectivement produit — contrat Alpha 100.
                let promote_n = overview
                    .and_then(|ov| promotion::resolve_promotion(feature, &layer_name, ov))
                    .unwrap_or(0);

                let upper = branch_max.max(promote_n);
                if upper > 0 {
                    fill_level_gaps(feature, upper);
                }
                count += 1;
            }
        }
    }
    count
}

/// Comble les index `Data1..DataK` manquants après [`apply_profile`] en clonant
/// la géométrie du palier précédent disponible. Garantit que le writer émettra
/// des sections `Data0..DataK` contiguës dans le `.mp`, condition nécessaire au
/// rendu correct sur les firmwares Garmin sensibles aux trous d'index RGN
/// (Alpha 100 rend "moitié vide" si `Data1=` est absent alors que `Data2=`
/// existe — QMapShack est tolérant mais pas les devices).
///
/// Mutation no-op pour les points et pour les features avec géométrie vide.
pub fn fill_level_gaps(feature: &mut Feature, max_n: u8) {
    if matches!(feature.geometry_type, GeometryType::Point) {
        return;
    }
    if feature.geometry.is_empty() {
        return;
    }
    // Piste de parcours : `geometry` = Data0 ; additional_geometries[n] pour n>0.
    let mut last_coords: Vec<(f64, f64)> = feature.geometry.clone();
    for n in 1..=max_n {
        if let Some(coords) = feature.additional_geometries.get(&n) {
            last_coords = coords.clone();
        } else {
            feature.additional_geometries.insert(n, last_coords.clone());
        }
    }
}

/// Arrondit une coordonnée à la grille 1e-6° (≈ 11 cm à 45°N).
///
/// Utilisé pour la détection de vertices partagés afin de tolérer la dérive
/// floating-point introduite par GDAL/GEOS lors du clipping de tuile, ainsi que
/// les légères imprecisions des exports Shapefile BDTOPO (frontières nominalement
/// partagées mais non bit-exactes). La précision 1e-6° est plus fine que la
/// résolution Garmin 24-bit (≈ 2.4 m) et sans effet visible sur le rendu.
#[inline]
fn snap_coord(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

/// Collecte les vertices partagés entre features d'une même couche (topologie intra-cellule).
///
/// Retourne un `HashSet` de coordonnées snappées `(snap(x).to_bits(), snap(y).to_bits())`
/// présentes dans au moins deux features distinctes. Le snap à 1e-6° tolère la dérive
/// GEOS et les différences sub-millimétriques des exports Shapefile BDTOPO — sans quoi
/// la comparaison bit-exacte manquerait les vertices nominalement partagés.
pub fn collect_shared_vertices(features: &[&Feature]) -> HashSet<(u64, u64)> {
    let mut counts: HashMap<(u64, u64), usize> = HashMap::new();
    for feature in features {
        let mut seen: HashSet<(u64, u64)> = HashSet::new();
        for &(x, y) in &feature.geometry {
            let key = (snap_coord(x).to_bits(), snap_coord(y).to_bits());
            if seen.insert(key) {
                *counts.entry(key).or_insert(0) += 1;
            }
        }
    }
    counts.into_iter().filter(|&(_, c)| c >= 2).map(|(k, _)| k).collect()
}

/// Applique VW avec ancrage topologique sur l'anneau extérieur d'un polygone.
///
/// Les vertices partagés avec d'autres features (encodés dans `anchors` sous forme
/// de clés snappées via [`snap_coord`]) sont injectés dans les indices retenus par
/// VW avant reconstruction, garantissant que les frontières communes sont identiques
/// dans les features voisines. La lookup dans `anchors` utilise le même snap que
/// [`collect_shared_vertices`] pour tolérer la dérive floating-point GEOS.
fn apply_level_to_polygon_topo(
    raw: &[(f64, f64)],
    lvl: &LevelSpec,
    anchors: &HashSet<(u64, u64)>,
) -> Option<Vec<(f64, f64)>> {
    if raw.len() < 4 {
        return None;
    }
    let coords: Vec<geo::Coord<f64>> =
        raw.iter().map(|(x, y)| geo::Coord { x: *x, y: *y }).collect();
    let ring = LineString::new(coords);
    if let Some(tolerance) = lvl.simplify_vw {
        let vw_indices = ring.simplify_vw_idx(&tolerance);
        let mut must_keep: BTreeSet<usize> = vw_indices.into_iter().collect();
        for (i, coord) in ring.0.iter().enumerate() {
            if anchors.contains(&(snap_coord(coord.x).to_bits(), snap_coord(coord.y).to_bits())) {
                must_keep.insert(i);
            }
        }
        let result: Vec<(f64, f64)> =
            must_keep.iter().map(|&i| (ring.0[i].x, ring.0[i].y)).collect();
        if result.len() < 4 {
            return None;
        }
        Some(result)
    } else {
        Some(raw.to_vec())
    }
}

/// Applique VW avec ancrage topologique sur une linestring.
///
/// Voir [`apply_level_to_polygon_topo`] — même contrat sur `anchors` (clés snappées).
fn apply_level_to_line_topo(
    raw: &[(f64, f64)],
    lvl: &LevelSpec,
    anchors: &HashSet<(u64, u64)>,
) -> Option<Vec<(f64, f64)>> {
    if raw.len() < 2 {
        return None;
    }
    let coords: Vec<geo::Coord<f64>> =
        raw.iter().map(|(x, y)| geo::Coord { x: *x, y: *y }).collect();
    let line = LineString::new(coords);
    if let Some(tolerance) = lvl.simplify_vw {
        let vw_indices = line.simplify_vw_idx(&tolerance);
        let mut must_keep: BTreeSet<usize> = vw_indices.into_iter().collect();
        for (i, coord) in line.0.iter().enumerate() {
            if anchors.contains(&(snap_coord(coord.x).to_bits(), snap_coord(coord.y).to_bits())) {
                must_keep.insert(i);
            }
        }
        let result: Vec<(f64, f64)> =
            must_keep.iter().map(|&i| (line.0[i].x, line.0[i].y)).collect();
        if result.len() < 2 {
            return None;
        }
        Some(result)
    } else {
        Some(raw.to_vec())
    }
}

/// Miroir de [`apply_profile`] routant vers les variantes topologiques.
///
/// `anchors` contient les vertices partagés avec d'autres features de la même
/// couche, collectés par [`collect_shared_vertices`] lors de la passe 1 de
/// [`generalize_features_with_profiles`].
pub fn apply_profile_with_topology(
    feature: &mut Feature,
    profile: &GeneralizeProfile,
    anchors: &HashSet<(u64, u64)>,
) -> usize {
    if matches!(feature.geometry_type, GeometryType::Point) {
        return 0;
    }
    let Some(levels) = resolve_levels(profile, feature) else {
        return 0;
    };

    let raw = feature.geometry.clone();
    let mut generated = 0;

    for lvl in levels {
        let derived = match feature.geometry_type {
            GeometryType::LineString => apply_level_to_line_topo(&raw, lvl, anchors),
            GeometryType::Polygon => apply_level_to_polygon_topo(&raw, lvl, anchors),
            GeometryType::Point => None,
        };
        match derived {
            Some(coords) => {
                feature.set_level(lvl.n, coords);
                generated += 1;
            }
            None => {
                warn!(
                    source_layer = feature.source_layer.as_deref().unwrap_or(""),
                    n = lvl.n,
                    raw_points = raw.len(),
                    simplify_vw = ?lvl.simplify_vw,
                    "topology level produced degenerate geometry (< min vertices after anchored simplify); \
                     bucket skipped"
                );
            }
        }
    }
    generated
}

/// Apply generalization to all features that have a matching layer config.
pub fn generalize_features(
    features: &mut [Feature],
    generalize_map: &HashMap<String, GeneralizeConfig>,
) -> usize {
    let mut count = 0;
    for feature in features.iter_mut() {
        let layer_name = feature.source_layer.as_deref().unwrap_or("");
        if let Some(config) = generalize_map.get(layer_name) {
            if generalize_feature(feature, config) {
                count += 1;
            }
        }
    }
    count
}

/// Apply smoothing algorithm to a LineString, returning the smoothed result.
/// Returns `None` if the algorithm is unknown (warning emitted).
fn apply_smooth_line(line: LineString<f64>, config: &GeneralizeConfig) -> Option<LineString<f64>> {
    match config.smooth.as_deref() {
        Some("chaikin") => Some(line.chaikin_smoothing(config.iterations)),
        Some(other) => {
            warn!(algorithm = %other, "Unknown smoothing algorithm, skipping");
            None
        }
        None => None,
    }
}

/// Apply smoothing algorithm to a Polygon, returning the smoothed result.
/// Returns `None` if the algorithm is unknown (warning emitted).
fn apply_smooth_polygon(polygon: Polygon<f64>, config: &GeneralizeConfig) -> Option<Polygon<f64>> {
    match config.smooth.as_deref() {
        Some("chaikin") => Some(polygon.chaikin_smoothing(config.iterations)),
        Some(other) => {
            warn!(algorithm = %other, "Unknown smoothing algorithm, skipping");
            None
        }
        None => None,
    }
}

fn generalize_linestring(feature: &mut Feature, config: &GeneralizeConfig) -> bool {
    if feature.geometry.len() < 3 {
        return false;
    }

    let coords: Vec<geo::Coord<f64>> = feature
        .geometry
        .iter()
        .map(|(x, y)| geo::Coord { x: *x, y: *y })
        .collect();
    let mut line = LineString::new(coords);
    let mut modified = false;

    if let Some(smoothed) = apply_smooth_line(line.clone(), config) {
        line = smoothed;
        modified = true;
    }

    if let Some(tolerance) = config.simplify {
        let simplified = line.simplify(&tolerance);
        if simplified.0.len() != line.0.len() {
            modified = true;
        }
        line = simplified;
    }

    if !modified || line.0.len() < 2 {
        return false;
    }
    feature.geometry = line.0.iter().map(|c| (c.x, c.y)).collect();
    true
}

fn generalize_polygon(feature: &mut Feature, config: &GeneralizeConfig) -> bool {
    if feature.geometry.len() < 4 {
        return false;
    }

    let coords: Vec<geo::Coord<f64>> = feature
        .geometry
        .iter()
        .map(|(x, y)| geo::Coord { x: *x, y: *y })
        .collect();
    let ring = LineString::new(coords);
    let mut polygon = Polygon::new(ring, vec![]);
    let mut modified = false;

    if let Some(smoothed) = apply_smooth_polygon(polygon.clone(), config) {
        polygon = smoothed;
        modified = true;
    }

    if let Some(tolerance) = config.simplify {
        let simplified = polygon.simplify(&tolerance);
        if simplified.exterior().0.len() != polygon.exterior().0.len() {
            modified = true;
        }
        polygon = simplified;
    }

    if !modified {
        return false;
    }

    let exterior = polygon.exterior();
    if exterior.0.len() < 4 {
        return false;
    }
    feature.geometry = exterior.0.iter().map(|c| (c.x, c.y)).collect();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_polygon_feature(coords: Vec<(f64, f64)>, layer: &str) -> Feature {
        Feature {
            geometry_type: GeometryType::Polygon,
            geometry: coords,
            additional_geometries: BTreeMap::new(),
            attributes: HashMap::new(),
            source_attributes: None,
            source_layer: Some(layer.to_string()),
        }
    }

    fn make_linestring_feature(coords: Vec<(f64, f64)>, layer: &str) -> Feature {
        Feature {
            geometry_type: GeometryType::LineString,
            geometry: coords,
            additional_geometries: BTreeMap::new(),
            attributes: HashMap::new(),
            source_attributes: None,
            source_layer: Some(layer.to_string()),
        }
    }

    fn make_point_feature(coord: (f64, f64), layer: &str) -> Feature {
        Feature {
            geometry_type: GeometryType::Point,
            geometry: vec![coord],
            additional_geometries: BTreeMap::new(),
            attributes: HashMap::new(),
            source_attributes: None,
            source_layer: Some(layer.to_string()),
        }
    }

    /// Simple square polygon for testing
    fn square_coords() -> Vec<(f64, f64)> {
        vec![
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
        ]
    }

    /// Zigzag linestring for testing
    fn zigzag_coords() -> Vec<(f64, f64)> {
        vec![
            (0.0, 0.0),
            (1.0, 1.0),
            (2.0, 0.0),
            (3.0, 1.0),
            (4.0, 0.0),
        ]
    }

    // =================================================================
    // H1/H2 fix: verify return value is false when nothing is modified
    // =================================================================

    #[test]
    fn test_point_unchanged() {
        let mut feature = make_point_feature((5.0, 45.0), "LAYER");
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 2,
            simplify: None,
        };
        assert!(!generalize_feature(&mut feature, &config));
        assert_eq!(feature.geometry, vec![(5.0, 45.0)]);
    }

    #[test]
    fn test_no_smooth_no_simplify_returns_false() {
        let original = square_coords();
        let mut feature = make_polygon_feature(original.clone(), "ZONE");
        let config = GeneralizeConfig {
            smooth: None,
            iterations: 1,
            simplify: None,
        };
        // No operation configured → must return false
        assert!(!generalize_feature(&mut feature, &config));
        assert_eq!(feature.geometry, original);
    }

    #[test]
    fn test_unknown_algorithm_returns_false() {
        let original = square_coords();
        let mut feature = make_polygon_feature(original.clone(), "ZONE");
        let config = GeneralizeConfig {
            smooth: Some("unknown_algo".into()),
            iterations: 1,
            simplify: None,
        };
        // Unknown algo + no simplify → nothing modified → false
        assert!(!generalize_feature(&mut feature, &config));
        assert_eq!(feature.geometry, original);
    }

    #[test]
    fn test_no_smooth_no_simplify_linestring_returns_false() {
        let original = zigzag_coords();
        let mut feature = make_linestring_feature(original.clone(), "ROUTE");
        let config = GeneralizeConfig {
            smooth: None,
            iterations: 1,
            simplify: None,
        };
        assert!(!generalize_feature(&mut feature, &config));
        assert_eq!(feature.geometry, original);
    }

    // =================================================================
    // Chaikin smoothing
    // =================================================================

    #[test]
    fn test_chaikin_polygon_modifies_geometry() {
        let mut feature = make_polygon_feature(square_coords(), "ZONE");
        let original_len = feature.geometry.len();
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 1,
            simplify: None,
        };
        assert!(generalize_feature(&mut feature, &config));
        assert!(feature.geometry.len() > original_len);
    }

    #[test]
    fn test_chaikin_linestring_modifies_geometry() {
        let mut feature = make_linestring_feature(zigzag_coords(), "ROUTE");
        let original_len = feature.geometry.len();
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 1,
            simplify: None,
        };
        assert!(generalize_feature(&mut feature, &config));
        assert!(feature.geometry.len() > original_len);
    }

    #[test]
    fn test_chaikin_two_iterations_more_vertices() {
        let mut f1 = make_polygon_feature(square_coords(), "ZONE");
        let mut f2 = make_polygon_feature(square_coords(), "ZONE");

        let config1 = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 1,
            simplify: None,
        };
        let config2 = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 2,
            simplify: None,
        };

        generalize_feature(&mut f1, &config1);
        generalize_feature(&mut f2, &config2);

        assert!(f2.geometry.len() > f1.geometry.len());
    }

    // =================================================================
    // Simplification
    // =================================================================

    #[test]
    fn test_simplify_only_reduces_vertices() {
        // Create a polygon with many vertices (smoothed first)
        let mut feature = make_polygon_feature(square_coords(), "ZONE");
        let smooth_config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 3,
            simplify: None,
        };
        generalize_feature(&mut feature, &smooth_config);
        let smoothed_len = feature.geometry.len();

        // Now simplify
        let simplify_config = GeneralizeConfig {
            smooth: None,
            iterations: 1,
            simplify: Some(0.1),
        };
        generalize_feature(&mut feature, &simplify_config);
        assert!(feature.geometry.len() <= smoothed_len);
    }

    #[test]
    fn test_chaikin_then_simplify_combined() {
        let mut feature = make_polygon_feature(square_coords(), "ZONE");
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 2,
            simplify: Some(0.05),
        };
        assert!(generalize_feature(&mut feature, &config));
        // Should still have a valid polygon (>= 4 coords for closed ring)
        assert!(feature.geometry.len() >= 4);
    }

    // =================================================================
    // Batch generalization by layer
    // =================================================================

    #[test]
    fn test_generalize_features_by_layer() {
        let mut features = vec![
            make_polygon_feature(square_coords(), "ZONE_D_HABITATION"),
            make_polygon_feature(square_coords(), "BATIMENT"),
            make_linestring_feature(zigzag_coords(), "ZONE_D_HABITATION"),
            make_point_feature((5.0, 45.0), "ZONE_D_HABITATION"),
        ];

        let mut gen_map = HashMap::new();
        gen_map.insert(
            "ZONE_D_HABITATION".to_string(),
            GeneralizeConfig {
                smooth: Some("chaikin".into()),
                iterations: 1,
                simplify: None,
            },
        );

        let count = generalize_features(&mut features, &gen_map);
        // Should have generalized the polygon and linestring (not point, not BATIMENT)
        assert_eq!(count, 2);
    }

    #[test]
    fn test_generalize_features_empty_map() {
        let mut features = vec![make_polygon_feature(square_coords(), "ZONE")];
        let gen_map = HashMap::new();
        let count = generalize_features(&mut features, &gen_map);
        assert_eq!(count, 0);
    }

    // =================================================================
    // Edge cases: too few coordinates
    // =================================================================

    #[test]
    fn test_too_few_coords_polygon() {
        let mut feature = make_polygon_feature(vec![(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)], "ZONE");
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 1,
            simplify: None,
        };
        assert!(!generalize_feature(&mut feature, &config));
    }

    #[test]
    fn test_too_few_coords_linestring() {
        let mut feature = make_linestring_feature(vec![(0.0, 0.0), (1.0, 1.0)], "ROUTE");
        let config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 1,
            simplify: None,
        };
        assert!(!generalize_feature(&mut feature, &config));
    }

    // =================================================================
    // Tech-spec #2 Task 11 — apply_profile (multi-bucket + dispatch)
    // =================================================================

    fn profile_from_yaml(yaml: &str) -> GeneralizeProfile {
        serde_yml::from_str(yaml).expect("valid profile YAML")
    }

    #[test]
    fn test_apply_profile_generates_three_buckets() {
        let zigzag: Vec<(f64, f64)> = (0..20)
            .map(|i| (i as f64 * 0.1, (i as f64 * 0.1).sin()))
            .collect();
        let mut feature = make_linestring_feature(zigzag, "HYDRO");
        let profile = profile_from_yaml(
            "levels:\n  - { n: 0, simplify: 0.00001 }\n  - { n: 2, simplify: 0.0001 }\n  - { n: 4, simplify: 0.0005 }\n",
        );
        let generated = apply_profile(&mut feature, &profile);
        assert_eq!(generated, 3);
        // n=0 replaces primary geometry
        assert!(!feature.geometry.is_empty());
        // n=2 and n=4 land in additional_geometries
        assert!(feature.additional_geometries.contains_key(&2));
        assert!(feature.additional_geometries.contains_key(&4));
        assert!(!feature.additional_geometries.contains_key(&1));
    }

    #[test]
    fn test_apply_profile_dispatch_when_matches_first() {
        let coords: Vec<(f64, f64)> =
            (0..10).map(|i| (i as f64 * 0.1, 0.0)).collect();
        let mut feature = make_linestring_feature(coords, "TRONCON_DE_ROUTE");
        feature
            .attributes
            .insert("CL_ADMIN".to_string(), "Autoroute".to_string());

        let profile = profile_from_yaml(
            r#"
when:
  - field: CL_ADMIN
    values: [Autoroute]
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 3, simplify: 0.00009 }
  - field: CL_ADMIN
    values: [Chemin]
    levels:
      - { n: 0, simplify: 0.0001 }
      - { n: 5, simplify: 0.0009 }
levels:
  - { n: 0, simplify: 0.00005 }
"#,
        );
        apply_profile(&mut feature, &profile);
        // First `when` branch matched → n=3 bucket created (not n=5, not default)
        assert!(feature.additional_geometries.contains_key(&3));
        assert!(!feature.additional_geometries.contains_key(&5));
    }

    #[test]
    fn test_apply_profile_falls_back_to_default_when_no_match() {
        let coords: Vec<(f64, f64)> =
            (0..10).map(|i| (i as f64 * 0.1, 0.0)).collect();
        let mut feature = make_linestring_feature(coords, "TRONCON_DE_ROUTE");
        feature
            .attributes
            .insert("CL_ADMIN".to_string(), "Rue".to_string());

        let profile = profile_from_yaml(
            r#"
when:
  - field: CL_ADMIN
    values: [Autoroute]
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 3, simplify: 0.00009 }
levels:
  - { n: 0, simplify: 0.00005 }
  - { n: 2, simplify: 0.0001 }
"#,
        );
        apply_profile(&mut feature, &profile);
        assert!(feature.additional_geometries.contains_key(&2));
        assert!(!feature.additional_geometries.contains_key(&3));
    }

    #[test]
    fn test_apply_profile_single_n0_no_additional_buckets() {
        let coords = zigzag_coords();
        let mut feature = make_linestring_feature(coords.clone(), "ZONE");
        let profile = profile_from_yaml(
            "levels:\n  - { n: 0, smooth: chaikin, iterations: 1 }\n",
        );
        apply_profile(&mut feature, &profile);
        assert!(feature.additional_geometries.is_empty());
        assert_ne!(feature.geometry, coords, "primary geom should be smoothed");
    }

    #[test]
    fn test_apply_profile_point_is_noop() {
        let mut feature = make_point_feature((2.0, 48.0), "POI");
        let profile = profile_from_yaml(
            "levels:\n  - { n: 0, simplify: 0.0001 }\n  - { n: 2, simplify: 0.0005 }\n",
        );
        let generated = apply_profile(&mut feature, &profile);
        assert_eq!(generated, 0);
        assert!(feature.additional_geometries.is_empty());
        assert_eq!(feature.geometry, vec![(2.0, 48.0)]);
    }

    #[test]
    fn test_generalize_features_with_profiles_dispatches_by_layer() {
        let mut features = vec![
            make_linestring_feature(zigzag_coords(), "HYDRO"),
            make_linestring_feature(zigzag_coords(), "BATIMENT"),
        ];
        let mut map = BTreeMap::new();
        map.insert(
            "HYDRO".to_string(),
            profile_from_yaml(
                "levels:\n  - { n: 0, simplify: 0.0001 }\n  - { n: 2, simplify: 0.0005 }\n",
            ),
        );
        let count = generalize_features_with_profiles(&mut features, &map, None);
        assert_eq!(count, 1);
        assert!(features[0].additional_geometries.contains_key(&2));
        assert!(features[1].additional_geometries.is_empty());
    }

    #[test]
    fn test_generalize_features_with_profiles_fills_level_gaps() {
        // Profil avec trou explicite (n=0, n=2 sans n=1). Après
        // generalize_features_with_profiles, fill_level_gaps doit combler n=1
        // pour garantir la contiguïté des sections DataN= dans le .mp
        // (requis par le firmware Alpha 100).
        let mut features = vec![make_linestring_feature(zigzag_coords(), "HYDRO")];
        let mut map = BTreeMap::new();
        map.insert(
            "HYDRO".to_string(),
            profile_from_yaml(
                "levels:\n  - { n: 0, simplify: 0.0001 }\n  - { n: 2, simplify: 0.0005 }\n",
            ),
        );
        generalize_features_with_profiles(&mut features, &map, None);
        let f = &features[0];
        assert!(
            f.additional_geometries.contains_key(&1),
            "n=1 must be backfilled for Data0..DataK contiguity"
        );
        assert!(f.additional_geometries.contains_key(&2));
        // Le clone de n=1 doit provenir du palier précédent (n=0 = feature.geometry)
        assert_eq!(
            f.additional_geometries.get(&1).unwrap(),
            &f.geometry,
            "n=1 (backfilled) should clone n=0 geometry verbatim"
        );
    }

    // =================================================================
    // L4 code review — edge cases du smoother
    // =================================================================

    #[test]
    fn test_fill_level_gaps_fills_up_to_branch_max_even_if_some_levels_degenerate() {
        // F1 régression : une feature dont une branche `when` déclare
        // n=0..6 mais dont un niveau intermédiaire dégénère (simplify trop
        // agressif → apply_level_to_line retourne None) doit quand même être
        // paddée jusqu'au branch_max=6 (contiguïté Alpha 100).
        // On simule en utilisant un raw très court sur lequel simplify=0.0005
        // produit None pour les niveaux hauts. Le filling doit clone depuis
        // le dernier palier valide.
        let short_raw = vec![(0.0, 0.0), (0.0001, 0.0), (0.0002, 0.0)];
        let mut features = vec![make_linestring_feature(short_raw, "SHORT_LAYER")];
        let mut map = BTreeMap::new();
        map.insert(
            "SHORT_LAYER".to_string(),
            profile_from_yaml(
                "levels:\n  \
                 - { n: 0, simplify: 0.00001 }\n  \
                 - { n: 3, simplify: 0.00001 }\n  \
                 - { n: 6, simplify: 0.0009 }\n",
            ),
        );
        generalize_features_with_profiles(&mut features, &map, None);
        let f = &features[0];
        // Branch_max = 6 même si n=6 dégénère → pad contiguïté 1..=6.
        for n in 1u8..=6 {
            assert!(
                f.additional_geometries.contains_key(&n),
                "n={n} must be present for Alpha 100 contiguity even if intermediate levels degenerate"
            );
        }
    }

    #[test]
    fn test_overview_promotion_pads_up_to_promote_to_even_without_lvlspec() {
        // F4 régression : promotion overview force le padding jusqu'à
        // promote_to même si le profil ne déclare pas ce n dans la branche
        // résolue. fill_level_gaps clone depuis le dernier palier produit.
        use crate::config::{OverviewLevels, PromotionRule};
        let mut features = vec![make_linestring_feature(
            (0..10).map(|i| (i as f64 * 0.001, 0.0)).collect(),
            "TRONCON_DE_ROUTE",
        )];
        features[0]
            .attributes
            .insert("CL_ADMIN".to_string(), "Autoroute".to_string());
        let mut map = BTreeMap::new();
        map.insert(
            "TRONCON_DE_ROUTE".to_string(),
            // Profil DELIBEREMENT SANS n=9 — le promote_to:9 doit quand même
            // produire Data9 par clonage (F4 fix).
            profile_from_yaml("levels:\n  - { n: 0, simplify: 0.00001 }\n"),
        );
        let mut promotion_rules = BTreeMap::new();
        let mut match_map = BTreeMap::new();
        match_map.insert("CL_ADMIN".to_string(), vec!["Autoroute".to_string()]);
        promotion_rules.insert(
            "TRONCON_DE_ROUTE".to_string(),
            vec![PromotionRule {
                match_: match_map,
                promote_to: 9,
            }],
        );
        let ov = OverviewLevels {
            header_extension: vec![14, 12, 10],
            promotion: promotion_rules,
        };
        generalize_features_with_profiles(&mut features, &map, Some(&ov));
        let f = &features[0];
        for n in 1u8..=9 {
            assert!(
                f.additional_geometries.contains_key(&n),
                "n={n} must be filled up to promote_to=9 via cloning (F4)"
            );
        }
    }

    #[test]
    fn test_fill_level_gaps_up_to_n9() {
        // Tech-spec overview wide-zoom Task 9 : un profil déclarant n=0 et n=9
        // (trous explicites n=1..8) doit produire 10 paliers contigus après
        // generalize_features_with_profiles. Garantit que fill_level_gaps est
        // bien level-agnostic jusqu'à u8 arbitraire — condition Alpha 100.
        let mut features = vec![make_linestring_feature(
            (0..20)
                .map(|i| (i as f64 * 0.001, 0.0))
                .collect(),
            "TRONCON_DE_ROUTE",
        )];
        let mut map = BTreeMap::new();
        map.insert(
            "TRONCON_DE_ROUTE".to_string(),
            profile_from_yaml(
                "levels:\n  - { n: 0, simplify: 0.00001 }\n  - { n: 9, simplify: 0.005 }\n",
            ),
        );
        generalize_features_with_profiles(&mut features, &map, None);
        let f = &features[0];
        // n=0 dans geometry, n=1..=9 dans additional_geometries (10 paliers
        // au total, tous clonés à partir du précédent pour combler les trous).
        for n in 1u8..=9 {
            assert!(
                f.additional_geometries.contains_key(&n),
                "n={n} must be backfilled (Alpha 100 contiguïté)"
            );
        }
        // Niveaux intermédiaires 1..8 clonés depuis n=0 (pas de simplify
        // spécifique). Le n=9 diffère (simplify=0.005).
        assert_eq!(
            f.additional_geometries.get(&8).unwrap(),
            &f.geometry,
            "n=8 (backfilled) should clone n=0 geometry verbatim"
        );
    }

    #[test]
    fn test_apply_profile_when_value_not_listed_falls_back_to_default() {
        // Attribut présent mais valeur non listée → branche default.
        let coords: Vec<(f64, f64)> =
            (0..8).map(|i| (i as f64 * 0.05, 0.0)).collect();
        let mut feature = make_linestring_feature(coords, "TRONCON_DE_ROUTE");
        feature
            .attributes
            .insert("CL_ADMIN".to_string(), "Bretelle".to_string());

        let profile = profile_from_yaml(
            r#"
when:
  - field: CL_ADMIN
    values: [Autoroute, Nationale]
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 3, simplify: 0.00009 }
levels:
  - { n: 0, simplify: 0.00005 }
  - { n: 2, simplify: 0.00015 }
"#,
        );
        apply_profile(&mut feature, &profile);
        assert!(
            feature.additional_geometries.contains_key(&2),
            "default levels should apply when no when-branch matches"
        );
        assert!(!feature.additional_geometries.contains_key(&3));
    }

    #[test]
    fn test_apply_profile_attribute_key_missing_uses_default() {
        // Attribut requis par `when` absent → default.
        let coords: Vec<(f64, f64)> =
            (0..8).map(|i| (i as f64 * 0.05, 0.0)).collect();
        let mut feature = make_linestring_feature(coords, "TRONCON_DE_ROUTE");
        // pas d'insertion de CL_ADMIN dans attributes

        let profile = profile_from_yaml(
            r#"
when:
  - field: CL_ADMIN
    values: [Autoroute]
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 3, simplify: 0.00009 }
levels:
  - { n: 0, simplify: 0.00005 }
  - { n: 2, simplify: 0.00015 }
"#,
        );
        apply_profile(&mut feature, &profile);
        assert!(feature.additional_geometries.contains_key(&2));
        assert!(!feature.additional_geometries.contains_key(&3));
    }

    #[test]
    fn test_resolve_levels_uses_source_attributes() {
        // Cas clé du fix source_attributes : CL_ADMIN absent de attributes (post-règles)
        // mais présent dans source_attributes (pré-règles) → la branche when: doit matcher.
        let profile = profile_from_yaml(r#"
when:
  - field: CL_ADMIN
    values: [Autoroute]
    levels:
      - { n: 0, simplify: 0.00001 }
      - { n: 9, simplify: 0.005 }
levels:
  - { n: 0, simplify: 0.00005 }
"#);
        let coords: Vec<(f64, f64)> = (0..15).map(|i| (i as f64 * 0.001, 0.0)).collect();
        let mut f = make_linestring_feature(coords, "TRONCON_DE_ROUTE");
        // Simule l'état post-règles : CL_ADMIN absent de attributes
        f.attributes.insert("Type".to_string(), "0x01".to_string());
        f.attributes.insert("EndLevel".to_string(), "9".to_string());
        // Mais présent dans source_attributes (snapshot pré-règles)
        f.source_attributes = Some({
            let mut m = HashMap::new();
            m.insert("CL_ADMIN".to_string(), "Autoroute".to_string());
            m
        });
        apply_profile(&mut f, &profile);
        assert!(
            f.additional_geometries.contains_key(&9),
            "when: dispatch doit utiliser source_attributes — n=9 doit être généré"
        );
    }

    // =================================================================
    // T5a/T5b — Algorithme VW standard (per-feature)
    // =================================================================

    #[test]
    fn test_simplify_vw_polygon_reduces_vertices() {
        let mut feature = make_polygon_feature(square_coords(), "COMMUNE");
        let smooth_config = GeneralizeConfig {
            smooth: Some("chaikin".into()),
            iterations: 3,
            simplify: None,
        };
        generalize_feature(&mut feature, &smooth_config);
        let raw_len = feature.geometry.len();
        let profile = profile_from_yaml("levels:\n  - { n: 0, simplify_vw: 0.05 }\n");
        apply_profile(&mut feature, &profile);
        assert!(feature.geometry.len() < raw_len, "VW doit réduire le nombre de vertices");
        assert!(feature.geometry.len() >= 4, "polygone reste valide");
    }

    #[test]
    fn test_simplify_vw_linestring_reduces_vertices() {
        // Zigzag avec aire triangle ≈5e-7 << epsilon=0.05 : VW simplifie massivement.
        let zigzag: Vec<(f64, f64)> =
            (0..30).map(|i| (i as f64 * 0.001, if i % 2 == 0 { 0.0 } else { 0.001 })).collect();
        let raw_len = zigzag.len();
        let mut feature = make_linestring_feature(zigzag, "HYDRO");
        let profile = profile_from_yaml("levels:\n  - { n: 0, simplify_vw: 0.05 }\n");
        apply_profile(&mut feature, &profile);
        assert!(feature.geometry.len() < raw_len, "VW doit réduire le nombre de vertices");
        assert!(feature.geometry.len() >= 2, "linestring reste valide");
    }

    // =================================================================
    // T5c/T5d/T5e — Validation config
    // =================================================================

    #[test]
    fn test_simplify_vw_out_of_range_detail_level() {
        use crate::config::LevelSpec;
        // n=3 (palier détail) : MAX_PROFILE_SIMPLIFY_VW_DETAIL = 0.005. 0.006 doit être rejeté.
        let lvl = LevelSpec {
            n: 3,
            smooth: None,
            iterations: 1,
            simplify: None,
            simplify_vw: Some(0.006),
        };
        let result = lvl.validate("test");
        assert!(result.is_err(), "simplify_vw=0.006 pour n=3 doit échouer la validation");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("simplify_vw"), "msg: {msg}");
    }

    #[test]
    fn test_simplify_dp_and_vw_mutually_exclusive() {
        use crate::config::LevelSpec;
        let lvl = LevelSpec {
            n: 0,
            smooth: None,
            iterations: 1,
            simplify: Some(0.0001),
            simplify_vw: Some(0.0001),
        };
        let result = lvl.validate("test");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("mutuellement exclusifs"), "msg: {msg}");
    }

    #[test]
    fn test_topology_with_smooth_fails_validation() {
        use crate::config::{GeneralizeProfile, LevelSpec};
        let profile = GeneralizeProfile {
            topology: true,
            levels: vec![LevelSpec {
                n: 0,
                smooth: Some("chaikin".into()),
                iterations: 1,
                simplify: None,
                simplify_vw: Some(0.0002),
            }],
            when: vec![],
        };
        assert!(profile.validate("test").is_err());
    }

    #[test]
    fn test_topology_with_dp_fails_validation() {
        use crate::config::{GeneralizeProfile, LevelSpec};
        let profile = GeneralizeProfile {
            topology: true,
            levels: vec![LevelSpec {
                n: 0,
                smooth: None,
                iterations: 1,
                simplify: Some(0.0002),
                simplify_vw: None,
            }],
            when: vec![],
        };
        assert!(profile.validate("test").is_err());
    }

    // =================================================================
    // snap_coord — unité
    // =================================================================

    #[test]
    fn test_snap_coord_exact_values_unchanged() {
        assert_eq!(snap_coord(1.0), 1.0, "entier exact inchangé");
        assert_eq!(snap_coord(0.5), 0.5, "demi-entier exact inchangé");
        assert_eq!(snap_coord(-1.0), -1.0, "négatif exact inchangé");
        assert_eq!(snap_coord(0.0), 0.0, "zéro inchangé");
    }

    #[test]
    fn test_snap_coord_small_offset_rounds_to_grid() {
        // 1e-7 < demi-case (5e-7) → snappe vers 1.0
        assert_eq!(snap_coord(1.0000001), 1.0, "dérive 1e-7 vers 1.0");
        // 3e-7 < 5e-7 → snappe vers 1.0
        assert_eq!(snap_coord(1.0000003), 1.0, "dérive 3e-7 vers 1.0");
    }

    #[test]
    fn test_snap_coord_large_offset_stays_in_different_cell() {
        // 2e-6 > demi-case → snappe vers 1.000002, pas 1.0
        let snapped = snap_coord(1.000002);
        assert_ne!(snapped, 1.0, "dérive 2e-6 reste dans sa propre case");
        assert!((snapped - 1.000002).abs() < 1e-9, "snappe vers 1.000002");
    }

    // =================================================================
    // T5f/T5g — collect_shared_vertices
    // =================================================================

    #[test]
    fn test_collect_shared_vertices_two_adjacent_polygons() {
        let f1 = make_polygon_feature(
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            "COMMUNE",
        );
        let f2 = make_polygon_feature(
            vec![(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 0.0)],
            "COMMUNE",
        );
        let anchors = collect_shared_vertices(&[&f1, &f2]);
        assert!(anchors.contains(&(1.0_f64.to_bits(), 0.0_f64.to_bits())));
        assert!(anchors.contains(&(1.0_f64.to_bits(), 1.0_f64.to_bits())));
        assert_eq!(anchors.len(), 2, "exactement 2 vertices partagés");
    }

    #[test]
    fn test_collect_shared_vertices_no_sharing() {
        let f1 = make_polygon_feature(
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            "COMMUNE",
        );
        let f2 = make_polygon_feature(
            vec![(2.0, 0.0), (3.0, 0.0), (3.0, 1.0), (2.0, 1.0), (2.0, 0.0)],
            "COMMUNE",
        );
        let anchors = collect_shared_vertices(&[&f1, &f2]);
        assert!(anchors.is_empty(), "aucun vertex partagé");
    }

    // T5g-bis — tolérance epsilon : vertices nominalement partagés mais non bit-exacts
    // (simule la dérive GEOS/Shapefile BDTOPO < 5e-7°)
    #[test]
    fn test_collect_shared_vertices_epsilon_tolerance() {
        // Frontière commune nominalement à x=1.0, mais avec dérive sub-µ° entre les deux communes.
        let f1 = make_polygon_feature(
            vec![(0.0, 0.0), (1.0000001, 0.0), (1.0000001, 1.0), (0.0, 1.0), (0.0, 0.0)],
            "COMMUNE",
        );
        let f2 = make_polygon_feature(
            vec![(1.0000002, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0000002, 1.0), (1.0000002, 0.0)],
            "COMMUNE",
        );
        // Les deux paires de vertices diffèrent de 1e-7° < grille snap 1e-6° → reconnus comme partagés.
        let anchors = collect_shared_vertices(&[&f1, &f2]);
        assert_eq!(anchors.len(), 2, "2 vertices epsilon-partagés reconnus comme ancres");
        // Les deux frontières snapent vers (1.0, 0.0) et (1.0, 1.0).
        assert!(
            anchors.contains(&(1.0_f64.to_bits(), 0.0_f64.to_bits())),
            "ancre (1.0, 0.0) présente"
        );
        assert!(
            anchors.contains(&(1.0_f64.to_bits(), 1.0_f64.to_bits())),
            "ancre (1.0, 1.0) présente"
        );
    }

    // T5g-ter — les vertices diffèrent de plus d'une case snap (> 5e-7°) → ne doivent PAS fusionner
    #[test]
    fn test_collect_shared_vertices_no_merge_above_threshold() {
        // Différence de 2e-6° > demi-grille snap (5e-7°) → snapent vers des cases distinctes.
        let f1 = make_polygon_feature(
            vec![(0.0, 0.0), (1.000000, 0.0), (1.000000, 1.0), (0.0, 1.0), (0.0, 0.0)],
            "COMMUNE",
        );
        let f2 = make_polygon_feature(
            vec![(1.000002, 0.0), (2.0, 0.0), (2.0, 1.0), (1.000002, 1.0), (1.000002, 0.0)],
            "COMMUNE",
        );
        let anchors = collect_shared_vertices(&[&f1, &f2]);
        assert!(
            anchors.is_empty(),
            "vertices éloignés de 2e-6° ne doivent pas être traités comme partagés"
        );
    }

    // =================================================================
    // T5h — apply_level_to_polygon_topo préserve les ancres
    // =================================================================

    #[test]
    fn test_apply_level_to_polygon_topo_preserves_anchor() {
        // Polygon avec un vertex collinéaire (0.5, 0.0) que VW supprimerait sans ancrage
        let raw = vec![
            (0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0),
        ];
        let mut anchors = HashSet::new();
        anchors.insert((0.5_f64.to_bits(), 0.0_f64.to_bits()));
        use crate::config::LevelSpec;
        // epsilon=0.25 : supprime (0.5,0) [area=0] mais conserve les angles du carré [area=0.5>0.25]
        let lvl = LevelSpec {
            n: 3,
            smooth: None,
            iterations: 1,
            simplify: None,
            simplify_vw: Some(0.25),
        };
        let result = apply_level_to_polygon_topo(&raw, &lvl, &anchors);
        assert!(result.is_some(), "résultat non-dégénéré");
        let coords = result.unwrap();
        let has_anchor = coords
            .iter()
            .any(|&(x, y)| x.to_bits() == 0.5_f64.to_bits() && y.to_bits() == 0.0_f64.to_bits());
        assert!(has_anchor, "vertex ancré (0.5, 0.0) doit être préservé");
    }

    // =================================================================
    // T5i — topologie batch : frontières partagées bit-identiques
    // =================================================================

    #[test]
    fn test_topology_mode_shared_border_identical_after_simplification() {
        // Deux rectangles adjacents partageant un bord vertical à x=1.0.
        // (1.0, 0.5) est collinéaire → VW le supprimerait en mode normal.
        let f1 = make_polygon_feature(
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            "COMMUNE",
        );
        let f2 = make_polygon_feature(
            vec![(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 0.5), (1.0, 0.0)],
            "COMMUNE",
        );
        let mut features = vec![f1, f2];
        let mut map = BTreeMap::new();
        map.insert(
            "COMMUNE".to_string(),
            profile_from_yaml(
                "topology: true\nlevels:\n  - { n: 0 }\n  - { n: 1, simplify_vw: 0.1 }\n",
            ),
        );
        generalize_features_with_profiles(&mut features, &map, None);

        let f1_n1 = features[0].additional_geometries.get(&1).expect("f1 n=1 présent");
        let f2_n1 = features[1].additional_geometries.get(&1).expect("f2 n=1 présent");

        let has_mid_f1 = f1_n1.iter().any(|&(x, y)| {
            x.to_bits() == 1.0_f64.to_bits() && y.to_bits() == 0.5_f64.to_bits()
        });
        let has_mid_f2 = f2_n1.iter().any(|&(x, y)| {
            x.to_bits() == 1.0_f64.to_bits() && y.to_bits() == 0.5_f64.to_bits()
        });
        assert!(has_mid_f1, "f1 n=1 doit contenir le vertex partagé (1.0, 0.5)");
        assert!(has_mid_f2, "f2 n=1 doit contenir le vertex partagé (1.0, 0.5)");
    }

    #[test]
    fn test_apply_profile_polygon_too_few_points_bucket_dropped() {
        // Polygon < 4 points → None retourné par apply_level_to_polygon,
        // bucket additionnel absent, n=0 conserve la géométrie brute.
        let raw = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]; // 4 pts (min)
        let mut f = make_polygon_feature(raw.clone(), "TEST_TINY");
        let profile = profile_from_yaml(
            // simplify agressif : risque de dégénération à tous les niveaux
            "levels:\n  - { n: 0, simplify: 0.0009 }\n  - { n: 2, simplify: 0.0009 }\n",
        );
        apply_profile(&mut f, &profile);
        // Post-condition : invariant structurel maintenu même en cas de
        // dégénération (M4) — pas de n=0 fantôme dans additional.
        #[cfg(debug_assertions)]
        f.assert_multi_bucket_invariant();
    }
}
