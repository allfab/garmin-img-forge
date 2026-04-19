//! Geometry generalization: smoothing and simplification.
//!
//! Applies Chaikin corner-cutting smoothing and/or Douglas-Peucker
//! simplification to feature geometries. Configured per-layer via
//! the `generalize` directive in source YAML configuration.

use crate::config::{GeneralizeConfig, GeneralizeProfile, LevelSpec};
use crate::pipeline::reader::{Feature, GeometryType};
use geo::{ChaikinSmoothing, LineString, Polygon, Simplify};
use std::collections::{BTreeMap, HashMap};
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
    attributes: &HashMap<String, String>,
) -> Option<&'a [LevelSpec]> {
    for clause in &profile.when {
        if let Some(val) = attributes.get(&clause.field) {
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
    let Some(levels) = resolve_levels(profile, &feature.attributes) else {
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
        let Some(coords) = derived else { continue; };
        if lvl.n == 0 {
            feature.geometry = coords;
        } else {
            feature.additional_geometries.insert(lvl.n, coords);
        }
        generated += 1;
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
pub fn generalize_features_with_profiles(
    features: &mut [Feature],
    profile_map: &BTreeMap<String, GeneralizeProfile>,
) -> usize {
    if profile_map.is_empty() {
        return 0;
    }
    let mut count = 0;
    for feature in features.iter_mut() {
        let Some(layer_name) = feature.source_layer.as_deref() else { continue; };
        if let Some(profile) = profile_map.get(layer_name) {
            if apply_profile(feature, profile) > 0 {
                count += 1;
            }
        }
    }
    count
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
            source_layer: Some(layer.to_string()),
        }
    }

    fn make_linestring_feature(coords: Vec<(f64, f64)>, layer: &str) -> Feature {
        Feature {
            geometry_type: GeometryType::LineString,
            geometry: coords,
            additional_geometries: BTreeMap::new(),
            attributes: HashMap::new(),
            source_layer: Some(layer.to_string()),
        }
    }

    fn make_point_feature(coord: (f64, f64), layer: &str) -> Feature {
        Feature {
            geometry_type: GeometryType::Point,
            geometry: vec![coord],
            additional_geometries: BTreeMap::new(),
            attributes: HashMap::new(),
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
}
