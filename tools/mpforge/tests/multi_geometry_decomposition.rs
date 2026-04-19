//! Tests for multi-geometry decomposition functionality (Story 6.7).
//!
//! Tests cover:
//! - MultiGeometryStats accumulation, merge, and reporting
//! - Multi-geometry decomposition (MultiPoint, MultiLineString, MultiPolygon)
//! - GeometryCollection filtering
//! - Simple geometry non-regression
//! - JSON report serialization for multi_geometries_decomposed

use mpforge::config::InputSource;
use mpforge::pipeline::reader::{GeometryType, MultiGeometryStats, SourceReader};
use mpforge::report::{ExecutionReport, QualitySection, ReportStatus, UnsupportedTypeReport};
use std::collections::BTreeMap;
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

// ============================================================================
// Task 1 Tests: MultiGeometryStats Structure
// ============================================================================

#[test]
fn test_multi_geometry_stats_accumulation() {
    // AC6.7.5, AC6.7.6 - MultiGeometryStats can track decomposed multi-geometries by type
    let mut stats = MultiGeometryStats::default();

    // Simulate decomposing several multi-geometries
    stats.record("MultiPolygon".to_string());
    stats.record("MultiPolygon".to_string());
    stats.record("MultiPoint".to_string());

    // Verify counts
    assert_eq!(stats.total(), 3);
    assert_eq!(stats.by_type.get("MultiPolygon").unwrap().count, 2);
    assert_eq!(stats.by_type.get("MultiPoint").unwrap().count, 1);
}

#[test]
fn test_multi_geometry_stats_is_empty() {
    // Stats should report empty when no decompositions occurred
    let stats = MultiGeometryStats::default();
    assert!(stats.is_empty());

    let mut stats_with_data = MultiGeometryStats::default();
    stats_with_data.record("MultiLineString".to_string());
    assert!(!stats_with_data.is_empty());
}

#[test]
fn test_multi_geometry_stats_total() {
    // Total should sum all decomposed features across all types
    let mut stats = MultiGeometryStats::default();
    stats.record("MultiPoint".to_string());
    stats.record("MultiPoint".to_string());
    stats.record("MultiLineString".to_string());
    stats.record("MultiPolygon".to_string());
    stats.record("MultiPolygon".to_string());
    stats.record("MultiPolygon".to_string());

    assert_eq!(stats.total(), 6); // 2 + 1 + 3 = 6
}

#[test]
fn test_multi_geometry_stats_multiple_types() {
    // AC6.7.5 - Stats should breakdown by type correctly
    let mut stats = MultiGeometryStats::default();

    // Simulate a realistic dataset
    for _ in 0..38 {
        stats.record("MultiPolygon".to_string());
    }
    for _ in 0..12 {
        stats.record("MultiPoint".to_string());
    }
    for _ in 0..5 {
        stats.record("MultiLineString".to_string());
    }

    assert_eq!(stats.total(), 55);
    assert_eq!(stats.by_type.get("MultiPolygon").unwrap().count, 38);
    assert_eq!(stats.by_type.get("MultiPoint").unwrap().count, 12);
    assert_eq!(stats.by_type.get("MultiLineString").unwrap().count, 5);
}

#[test]
fn test_multi_geometry_stats_merge() {
    // Code Review M2: Test the new merge() method
    let mut stats1 = MultiGeometryStats::default();
    stats1.record("MultiPolygon".to_string());
    stats1.record("MultiPolygon".to_string());

    let mut stats2 = MultiGeometryStats::default();
    stats2.record("MultiPolygon".to_string());
    stats2.record("MultiPoint".to_string());

    stats1.merge(&stats2);

    assert_eq!(stats1.total(), 4);
    assert_eq!(stats1.by_type.get("MultiPolygon").unwrap().count, 3);
    assert_eq!(stats1.by_type.get("MultiPoint").unwrap().count, 1);
}

// ============================================================================
// Task 6 Tests: Multi-geometry Decomposition with real fixtures
// Code Review H2 Fix: Replace #[ignore] placeholders with real tests
// ============================================================================

#[test]
fn test_multipoint_decomposition_with_attributes() {
    // AC6.7.1 - MultiPoint → N Features Point with attributes preserved
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec!["multi_points".to_string()]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, _unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // 2 MultiPoints: mp1 has 3 Points, mp2 has 2 Points → 5 features total
    assert_eq!(
        features.len(),
        5,
        "Expected 5 decomposed point features from 2 MultiPoints"
    );

    // All should be Point type
    for feature in &features {
        assert_eq!(feature.geometry_type, GeometryType::Point);
        assert!(!feature.geometry.is_empty());
    }

    // AC6.7.1: Attributes preserved on each decomposed feature
    let mp1_features: Vec<_> = features
        .iter()
        .filter(|f| f.attributes.get("name") == Some(&"mp1".to_string()))
        .collect();
    assert_eq!(
        mp1_features.len(),
        3,
        "mp1 MultiPoint(3) should produce 3 features"
    );
    for f in &mp1_features {
        assert_eq!(
            f.attributes.get("Type"),
            Some(&"0x0100".to_string()),
            "Type attribute must be preserved on each decomposed point"
        );
    }

    let mp2_features: Vec<_> = features
        .iter()
        .filter(|f| f.attributes.get("name") == Some(&"mp2".to_string()))
        .collect();
    assert_eq!(
        mp2_features.len(),
        2,
        "mp2 MultiPoint(2) should produce 2 features"
    );

    // Stats should show 2 MultiPoint decompositions
    assert_eq!(multi_geom.total(), 2);
    assert_eq!(multi_geom.by_type.get("MultiPoint").unwrap().count, 2);
}

#[test]
fn test_multilinestring_decomposition() {
    // AC6.7.2 - MultiLineString → N Features LineString
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec!["multi_lines".to_string()]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, _unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // 1 MultiLineString with 2 LineStrings → 2 features
    assert_eq!(
        features.len(),
        2,
        "Expected 2 decomposed linestring features from 1 MultiLineString"
    );

    for feature in &features {
        assert_eq!(feature.geometry_type, GeometryType::LineString);
        assert!(
            feature.geometry.len() >= 2,
            "LineString should have at least 2 points"
        );
        // Attributes preserved
        assert_eq!(feature.attributes.get("name"), Some(&"ml1".to_string()));
        assert_eq!(feature.attributes.get("Type"), Some(&"0x0001".to_string()));
    }

    // Stats
    assert_eq!(multi_geom.total(), 1);
    assert_eq!(multi_geom.by_type.get("MultiLineString").unwrap().count, 1);
}

#[test]
fn test_multipolygon_decomposition() {
    // AC6.7.3 - MultiPolygon → N Features Polygon
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec!["multi_polygons".to_string()]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, _unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // 2 MultiPolygons: mpoly1 has 2 Polygons, mpoly2 has 3 Polygons → 5 features
    assert_eq!(
        features.len(),
        5,
        "Expected 5 decomposed polygon features from 2 MultiPolygons"
    );

    for feature in &features {
        assert_eq!(feature.geometry_type, GeometryType::Polygon);
        assert!(
            feature.geometry.len() >= 4,
            "Polygon should have at least 4 points (closed ring)"
        );
    }

    // Verify attribute preservation per parent
    let mpoly1_features: Vec<_> = features
        .iter()
        .filter(|f| f.attributes.get("name") == Some(&"mpoly1".to_string()))
        .collect();
    assert_eq!(mpoly1_features.len(), 2);
    for f in &mpoly1_features {
        assert_eq!(f.attributes.get("Type"), Some(&"0x0003".to_string()));
    }

    let mpoly2_features: Vec<_> = features
        .iter()
        .filter(|f| f.attributes.get("name") == Some(&"mpoly2".to_string()))
        .collect();
    assert_eq!(mpoly2_features.len(), 3);

    // Stats: 2 MultiPolygon decompositions
    assert_eq!(multi_geom.total(), 2);
    assert_eq!(multi_geom.by_type.get("MultiPolygon").unwrap().count, 2);
}

#[test]
fn test_geometry_collection_filtered() {
    // AC6.7.4 - GeometryCollection → filtered (vec![]) with warning
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec!["geom_collection".to_string()]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // GeometryCollection should be filtered out
    assert_eq!(
        features.len(),
        0,
        "GeometryCollection should be filtered (0 features)"
    );

    // Should appear in unsupported_types stats
    assert!(
        !unsupported.is_empty(),
        "GeometryCollection should be recorded as unsupported"
    );
    assert!(
        unsupported.by_type.contains_key("GeometryCollection"),
        "Should track GeometryCollection as unsupported type"
    );

    // Should NOT appear in multi-geometry stats
    assert!(multi_geom.is_empty());
}

#[test]
fn test_simple_geometries_unchanged() {
    // AC6.7.7 - Simple geometries pass through without modification
    // Code Review H2 Fix: Real test replacing assert!(true) placeholder
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec!["simple_points".to_string()]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // 2 simple points should pass through unchanged
    assert_eq!(features.len(), 2, "2 simple points should pass unchanged");

    for feature in &features {
        assert_eq!(feature.geometry_type, GeometryType::Point);
        assert_eq!(feature.geometry.len(), 1, "Simple point has 1 coordinate");
        assert!(feature.attributes.contains_key("name"));
        assert!(feature.attributes.contains_key("Type"));
    }

    // No unsupported types, no multi-geometry decompositions
    assert!(unsupported.is_empty());
    assert!(multi_geom.is_empty());
}

#[test]
fn test_mix_simple_and_multi_geometries() {
    // AC6.7.7 - Mix of simple + multi: simple pass untouched, multi decomposed
    let gpkg_path = get_test_data_path("multi_geom.gpkg");

    let input = InputSource {
        path: Some(gpkg_path),
        layers: Some(vec![
            "simple_points".to_string(),
            "multi_points".to_string(),
        ]),
        connection: None,
        layer: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        dedup_by_field: None,
    };

    let (features, _unsupported, multi_geom) = SourceReader::read_file_source(&input).unwrap();

    // 2 simple points + 5 decomposed points = 7 total
    assert_eq!(
        features.len(),
        7,
        "2 simple + 5 decomposed = 7 point features"
    );

    // Only multi-geometries counted in stats
    assert_eq!(
        multi_geom.total(),
        2,
        "Only 2 MultiPoint features decomposed (not simple points)"
    );
}

// ============================================================================
// Code Review H3 Fix: JSON report serialization tests
// ============================================================================

#[test]
fn test_json_report_with_multi_geometries_decomposed() {
    // AC6.7.5 - JSON report contains multi_geometries_decomposed with breakdown
    let mut decomposed = BTreeMap::new();
    decomposed.insert("MultiPolygon".to_string(), 38_usize);
    decomposed.insert("MultiPoint".to_string(), 12_usize);
    decomposed.insert("MultiLineString".to_string(), 5_usize);

    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 100,
        tiles_failed: 0,
        tiles_skipped: 0,
        features_processed: 10000,
        duration_seconds: 42.5,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types: BTreeMap::new(),
            multi_geometries_decomposed: Some(decomposed),
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();

    // Verify multi_geometries_decomposed is present and correct
    assert!(json.contains("\"multi_geometries_decomposed\""));
    assert!(json.contains("\"MultiPolygon\": 38"));
    assert!(json.contains("\"MultiPoint\": 12"));
    assert!(json.contains("\"MultiLineString\": 5"));

    // Code Review M3 Fix: Empty unsupported_types should NOT appear in JSON
    assert!(
        !json.contains("\"unsupported_types\""),
        "Empty unsupported_types should be skipped in JSON"
    );

    // Verify via deserialization
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["quality"]["multi_geometries_decomposed"]["MultiPolygon"],
        38
    );
    assert_eq!(
        parsed["quality"]["multi_geometries_decomposed"]["MultiPoint"],
        12
    );
}

#[test]
fn test_json_report_without_multi_geometries() {
    // AC6.7.5 - multi_geometries_decomposed omitted when None (skip_serializing_if)
    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 50,
        tiles_failed: 0,
        tiles_skipped: 0,
        features_processed: 5000,
        duration_seconds: 10.0,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types: BTreeMap::new(),
            multi_geometries_decomposed: None,
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(
        !json.contains("\"multi_geometries_decomposed\""),
        "None multi_geometries_decomposed should not appear in JSON"
    );
}

#[test]
fn test_json_report_with_both_quality_fields() {
    // Both unsupported_types and multi_geometries_decomposed populated
    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "GeometryCollection".to_string(),
        UnsupportedTypeReport {
            count: 2,
            sources: vec!["mixed.gpkg".to_string()],
            total_sources: None,
        },
    );

    let mut decomposed = BTreeMap::new();
    decomposed.insert("MultiPolygon".to_string(), 100_usize);

    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 200,
        tiles_failed: 0,
        tiles_skipped: 0,
        features_processed: 20000,
        duration_seconds: 60.0,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types,
            multi_geometries_decomposed: Some(decomposed),
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"unsupported_types\""));
    assert!(json.contains("\"GeometryCollection\""));
    assert!(json.contains("\"multi_geometries_decomposed\""));
    assert!(json.contains("\"MultiPolygon\": 100"));
}
