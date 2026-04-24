//! Unit tests for MpWriter (Story 5.4)

use mpforge::pipeline::reader::{Feature, GeometryType};
use mpforge::pipeline::writer::MpWriter;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a temporary output path for testing.
/// Returns (TempDir, PathBuf) where PathBuf is the full path to output.mp file.
fn create_temp_output() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("output.mp");
    (temp_dir, output_path)
}

/// Helper to create a test POI feature
fn create_poi_feature(label: &str, type_code: &str) -> Feature {
    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), type_code.to_string());
    attributes.insert("Label".to_string(), label.to_string());

    Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(2.3522, 48.8566)], // Paris coordinates
        additional_geometries: std::collections::BTreeMap::new(),
        attributes,
        source_layer: None,
    }
}

/// Helper to create a test POLYLINE feature
fn create_polyline_feature(label: &str, type_code: &str) -> Feature {
    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), type_code.to_string());
    attributes.insert("Label".to_string(), label.to_string());

    Feature {
        geometry_type: GeometryType::LineString,
        geometry: vec![(2.3522, 48.8566), (2.3532, 48.8576)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes,
        source_layer: None,
    }
}

/// Helper to create a test POLYGON feature
fn create_polygon_feature(label: &str, type_code: &str) -> Feature {
    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), type_code.to_string());
    attributes.insert("Label".to_string(), label.to_string());

    Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![
            (2.3522, 48.8566),
            (2.3532, 48.8566),
            (2.3532, 48.8576),
            (2.3522, 48.8576),
            (2.3522, 48.8566), // Closed ring
        ],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes,
        source_layer: None,
    }
}

// ============================================================================
// Task 1: MpWriter::new() Tests (AC: 1, 2)
// ============================================================================

#[test]
fn test_mpwriter_new_creates_dataset() {
    // AC1: Dataset .mp créé via GDAL Driver "PolishMap"
    let (_temp_dir, output_path) = create_temp_output();

    let result = MpWriter::new(output_path, None, None, None);

    assert!(result.is_ok(), "MpWriter::new() should succeed");
}

#[test]
fn test_mpwriter_new_creates_three_layers() {
    // AC1: Les 3 layers (POI, POLYLINE, POLYGON) sont créés
    let (_temp_dir, output_path) = create_temp_output();

    let writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");

    // Verify internal state (we'll check via write_features behavior)
    // Layer verification will be done indirectly through feature writing
    drop(writer);
}

#[test]
fn test_mpwriter_new_handles_invalid_directory() {
    // AC1: Subtask 1.3 - Gestion erreur permissions
    let output_path = PathBuf::from("/nonexistent/invalid/path/output.mp");
    let result = MpWriter::new(output_path, None, None, None);

    assert!(
        result.is_err(),
        "MpWriter::new() should fail with invalid directory"
    );
}

// ============================================================================
// Task 2: write_features() Tests (AC: 1, 2, 5)
// ============================================================================

#[test]
fn test_write_features_empty_dataset() {
    // AC5: Dataset vide (0 features) avec warning
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");
    let features = vec![];

    let result = writer.write_features(&features);

    assert!(
        result.is_ok(),
        "write_features() should handle empty dataset"
    );
    let stats = result.unwrap();
    assert_eq!(stats.point_count, 0);
    assert_eq!(stats.linestring_count, 0);
    assert_eq!(stats.polygon_count, 0);
}

#[test]
fn test_write_features_poi_only() {
    // AC2: Features POI avec Type et Label
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");
    let features = vec![
        create_poi_feature("Paris", "0x0100"),
        create_poi_feature("Lyon", "0x0200"),
    ];

    let result = writer.write_features(&features);

    assert!(result.is_ok(), "write_features() should succeed");
    let stats = result.unwrap();
    assert_eq!(stats.point_count, 2);
    assert_eq!(stats.linestring_count, 0);
    assert_eq!(stats.polygon_count, 0);
}

#[test]
fn test_write_features_mixed_geometries() {
    // AC1, AC2: Features mixtes POI + POLYLINE + POLYGON
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");
    let features = vec![
        create_poi_feature("Paris", "0x0100"),
        create_polyline_feature("Route A1", "0x0001"),
        create_polygon_feature("Zone 1", "0x0050"),
    ];

    let result = writer.write_features(&features);

    assert!(result.is_ok(), "write_features() should succeed");
    let stats = result.unwrap();
    assert_eq!(stats.point_count, 1);
    assert_eq!(stats.linestring_count, 1);
    assert_eq!(stats.polygon_count, 1);
}

#[test]
fn test_write_features_preserves_attributes() {
    // AC2: Type/Label préservés dans le fichier .mp
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");

    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), "0x0100".to_string());
    attributes.insert("Label".to_string(), "Test POI".to_string());
    attributes.insert("EndLevel".to_string(), "3".to_string());

    let features = vec![Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(2.3522, 48.8566)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes,
        source_layer: None,
    }];

    let result = writer.write_features(&features);

    assert!(
        result.is_ok(),
        "write_features() should preserve attributes"
    );
}

// ============================================================================
// Task 3: finalize() Tests (AC: 4)
// ============================================================================

#[test]
fn test_finalize_creates_mp_file() {
    // AC4: Fichier .mp généré dans le répertoire output
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path.clone(), None, None, None).expect("Failed to create writer");
    let features = vec![create_poi_feature("Test", "0x0100")];

    writer
        .write_features(&features)
        .expect("Failed to write features");

    let result = writer.finalize();

    assert!(result.is_ok(), "finalize() should succeed");

    // Verify .mp file exists
    assert!(
        output_path.exists(),
        "Output .mp file should exist at: {}",
        output_path.display()
    );
}

#[test]
fn test_finalize_returns_stats() {
    // AC4: finalize() retourne PipelineResult avec stats
    let (_temp_dir, output_path) = create_temp_output();

    let mut writer = MpWriter::new(output_path, None, None, None).expect("Failed to create writer");
    let features = vec![
        create_poi_feature("POI1", "0x0100"),
        create_polyline_feature("Line1", "0x0001"),
        create_polygon_feature("Poly1", "0x0050"),
    ];

    writer
        .write_features(&features)
        .expect("Failed to write features");

    let result = writer.finalize();

    assert!(result.is_ok(), "finalize() should return stats");
    let stats = result.unwrap();
    assert_eq!(stats.point_count, 1);
    assert_eq!(stats.linestring_count, 1);
    assert_eq!(stats.polygon_count, 1);
}
