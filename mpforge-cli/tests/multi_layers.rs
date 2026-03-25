//! Unit tests for multi-layer GeoPackage support
//! Story 5.5 - Fix GeoPackage Multi-Layers

use mpforge_cli::config::{Config, GridConfig, InputSource, OutputConfig};
use mpforge_cli::pipeline::reader::{GeometryType, SourceReader};
use std::path::PathBuf;

fn get_test_data_path(filename: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("integration");
    path.push("fixtures");
    path.push("test_data");
    path.push(filename);
    path.to_str().unwrap().to_string()
}

#[test]
fn test_geopackage_three_layers_configured() {
    // Task 4.1: Test GeoPackage avec 3 layers configurés
    // AC1: Toutes les features des 3 layers sont chargées
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let input = InputSource {
        path: Some(gpkg_path.clone()),
        layers: Some(vec![
            "pois".to_string(),
            "roads".to_string(),
            "buildings".to_string(),
        ]),
        connection: None,
        layer: None,
    };

    let result = SourceReader::read_file_source(&input);
    assert!(
        result.is_ok(),
        "Failed to read multi-layer GeoPackage: {:?}",
        result.err()
    );

    let (features, _unsupported, _multi_geom) = result.unwrap();

    // Total expected: 5 pois + 10 roads + 8 buildings = 23 features
    assert_eq!(
        features.len(),
        23,
        "Expected 23 features from 3 layers, got {}",
        features.len()
    );

    // Count by geometry type
    let point_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::Point)
        .count();
    let linestring_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::LineString)
        .count();
    let polygon_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::Polygon)
        .count();

    // AC1: Features de chaque layer conservent leurs attributs originaux
    assert_eq!(point_count, 5, "Expected 5 points from pois layer");
    assert_eq!(
        linestring_count, 10,
        "Expected 10 linestrings from roads layer"
    );
    assert_eq!(polygon_count, 8, "Expected 8 polygons from buildings layer");

    // Verify attributes are preserved
    for feature in &features {
        assert!(
            feature.attributes.contains_key("name"),
            "Feature should have 'name' attribute"
        );
        assert!(
            feature.attributes.contains_key("Type"),
            "Feature should have 'Type' attribute"
        );
    }
}

#[test]
fn test_geopackage_invalid_layer_continue_mode() {
    // Task 4.2: Test layer invalide en mode continue
    // AC3: En mode continue, les autres layers valides sont chargés
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let config = Config {
        version: 1,
        inputs: vec![InputSource {
            path: Some(gpkg_path.clone()),
            layers: Some(vec![
                "pois".to_string(),
                "invalid_layer".to_string(), // Layer inexistant
                "roads".to_string(),
            ]),
            connection: None,
            layer: None,
        }],
        grid: GridConfig {
            cell_size: 0.1,
            overlap: 0.01,
            origin: None,
        },
        output: OutputConfig {
            directory: "output".to_string(),
            filename_pattern: "tile_{x}_{y}.mp".to_string(),
            field_mapping_path: None,
            overwrite: None,
        },
        filters: None,
        error_handling: "continue".to_string(),
        header: None,
    };

    let result = SourceReader::read_all_sources(&config);
    assert!(
        result.is_ok(),
        "Should succeed in continue mode despite invalid layer"
    );

    let (features, _rtree, _unsupported, _multi_geom) = result.unwrap();

    // Should have features from valid layers only: pois (5) + roads (10) = 15
    assert_eq!(
        features.len(),
        15,
        "Expected 15 features from 2 valid layers (skipping invalid), got {}",
        features.len()
    );

    // Verify correct layers were loaded
    let point_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::Point)
        .count();
    let linestring_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::LineString)
        .count();

    assert_eq!(point_count, 5, "Expected 5 points from pois layer");
    assert_eq!(
        linestring_count, 10,
        "Expected 10 linestrings from roads layer"
    );
}

#[test]
fn test_geopackage_invalid_layer_fail_fast_mode() {
    // Task 4.3: Test layer invalide en mode fail-fast
    // AC3: En mode fail-fast, le pipeline s'arrête immédiatement
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let config = Config {
        version: 1,
        inputs: vec![InputSource {
            path: Some(gpkg_path.clone()),
            layers: Some(vec![
                "pois".to_string(),
                "invalid_layer".to_string(), // Layer inexistant
                "roads".to_string(),
            ]),
            connection: None,
            layer: None,
        }],
        grid: GridConfig {
            cell_size: 0.1,
            overlap: 0.01,
            origin: None,
        },
        output: OutputConfig {
            directory: "output".to_string(),
            filename_pattern: "tile_{x}_{y}.mp".to_string(),
            field_mapping_path: None,
            overwrite: None,
        },
        filters: None,
        error_handling: "fail-fast".to_string(),
        header: None,
    };

    let result = SourceReader::read_all_sources(&config);
    assert!(
        result.is_err(),
        "Should fail immediately in fail-fast mode with invalid layer"
    );

    // Verify error message mentions the invalid layer
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("invalid_layer") || error_msg.to_lowercase().contains("layer"),
        "Error message should mention the invalid layer, got: {}",
        error_msg
    );
}

#[test]
fn test_geopackage_empty_layers_list() {
    // Task 4.4: Test layers liste vide (default layer 0)
    // AC4: Avec layers: [], utiliser layer 0 par défaut avec warning
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let input = InputSource {
        path: Some(gpkg_path.clone()),
        layers: Some(vec![]), // Empty list
        connection: None,
        layer: None,
    };

    let result = SourceReader::read_file_source(&input);
    assert!(
        result.is_ok(),
        "Should succeed with empty layers list (fallback to layer 0)"
    );

    let (features, _unsupported, _multi_geom) = result.unwrap();

    // Should load default layer (layer 0) which is "pois" (5 features)
    assert_eq!(
        features.len(),
        5,
        "Expected 5 features from default layer 0 (pois), got {}",
        features.len()
    );

    // Verify it's the pois layer (all points)
    let point_count = features
        .iter()
        .filter(|f| f.geometry_type == GeometryType::Point)
        .count();
    assert_eq!(
        point_count, 5,
        "Expected 5 points from pois layer (default layer 0)"
    );
}

#[test]
fn test_geopackage_layers_none_backward_compat() {
    // Task 4.5: Test layers None (backward compat, pas de warning)
    // AC5: Avec layers: None, utiliser layer 0 sans warning (comportement actuel)
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let input = InputSource {
        path: Some(gpkg_path.clone()),
        layers: None, // No layers specified
        connection: None,
        layer: None,
    };

    let result = SourceReader::read_file_source(&input);
    assert!(
        result.is_ok(),
        "Should succeed with layers None (backward compat)"
    );

    let (features, _unsupported, _multi_geom) = result.unwrap();

    // Should load default layer (layer 0) which is "pois" (5 features)
    assert_eq!(
        features.len(),
        5,
        "Expected 5 features from default layer 0 (pois), got {}",
        features.len()
    );
}

#[test]
fn test_geopackage_mixed_geometry_types() {
    // Task 4.6: Test accumulation features cross-layers (types géométriques mixés)
    // Verify that features from different layers with different geometry types
    // are correctly accumulated in a single vector
    let gpkg_path = get_test_data_path("multi_layers.gpkg");

    let input = InputSource {
        path: Some(gpkg_path.clone()),
        layers: Some(vec![
            "pois".to_string(),      // Points
            "roads".to_string(),     // LineStrings
            "buildings".to_string(), // Polygons
        ]),
        connection: None,
        layer: None,
    };

    let result = SourceReader::read_file_source(&input);
    assert!(
        result.is_ok(),
        "Should succeed loading mixed geometry types"
    );

    let (features, _unsupported, _multi_geom) = result.unwrap();

    // Verify we have all three geometry types
    let has_points = features
        .iter()
        .any(|f| f.geometry_type == GeometryType::Point);
    let has_linestrings = features
        .iter()
        .any(|f| f.geometry_type == GeometryType::LineString);
    let has_polygons = features
        .iter()
        .any(|f| f.geometry_type == GeometryType::Polygon);

    assert!(has_points, "Should have Point features from pois layer");
    assert!(
        has_linestrings,
        "Should have LineString features from roads layer"
    );
    assert!(
        has_polygons,
        "Should have Polygon features from buildings layer"
    );

    // Verify total count matches expected
    assert_eq!(
        features.len(),
        23,
        "Expected 23 total features (5+10+8), got {}",
        features.len()
    );
}
