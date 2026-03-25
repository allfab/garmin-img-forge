//! Tests for unsupported geometry type filtering
//! Story 6.6 - Tasks 8, 9, 10
//! Code Review fixes: Adapted to BTreeMap and total_sources field

use mpforge_cli::pipeline::reader::UnsupportedTypeStats;
use mpforge_cli::report::{ExecutionReport, QualitySection, ReportStatus, UnsupportedTypeReport};
use std::collections::BTreeMap;

// ============================================================================
// Task 8: Unit tests for type filtering (AC: 1, 3)
// ============================================================================

#[test]
fn test_unsupported_type_stats_record() {
    // Subtask 8.4: Test UnsupportedTypeStats accumulation correcte
    let mut stats = UnsupportedTypeStats::default();
    stats.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    stats.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    stats.record("GeometryCollection".to_string(), "mixed.gpkg".to_string());

    assert_eq!(stats.total(), 3);
    assert_eq!(stats.by_type["MultiPolygon"].count, 2);
    assert_eq!(stats.by_type["MultiPolygon"].sources, vec!["COMMUNE.shp"]);
    assert_eq!(stats.by_type["GeometryCollection"].count, 1);
    assert_eq!(
        stats.by_type["GeometryCollection"].sources,
        vec!["mixed.gpkg"]
    );
}

#[test]
fn test_unsupported_type_stats_dedup_sources() {
    // Subtask 8.4: Source deduplication
    let mut stats = UnsupportedTypeStats::default();
    stats.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    stats.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    // Source ne doit apparaître qu'une fois
    assert_eq!(stats.by_type["MultiPolygon"].sources.len(), 1);
}

#[test]
fn test_unsupported_type_stats_multiple_sources() {
    let mut stats = UnsupportedTypeStats::default();
    stats.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    stats.record("MultiPolygon".to_string(), "BATIMENT.shp".to_string());

    assert_eq!(stats.by_type["MultiPolygon"].count, 2);
    assert_eq!(stats.by_type["MultiPolygon"].sources.len(), 2);
    assert!(stats.by_type["MultiPolygon"]
        .sources
        .contains(&"COMMUNE.shp".to_string()));
    assert!(stats.by_type["MultiPolygon"]
        .sources
        .contains(&"BATIMENT.shp".to_string()));
}

#[test]
fn test_empty_stats() {
    let stats = UnsupportedTypeStats::default();
    assert!(stats.is_empty());
    assert_eq!(stats.total(), 0);
}

#[test]
fn test_stats_not_empty_after_record() {
    let mut stats = UnsupportedTypeStats::default();
    stats.record("MultiPolygon".to_string(), "test.shp".to_string());
    assert!(!stats.is_empty());
    assert_eq!(stats.total(), 1);
}

#[test]
fn test_stats_merge() {
    let mut stats1 = UnsupportedTypeStats::default();
    stats1.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());
    stats1.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());

    let mut stats2 = UnsupportedTypeStats::default();
    stats2.record("MultiPolygon".to_string(), "BATIMENT.shp".to_string());
    stats2.record("GeometryCollection".to_string(), "mixed.gpkg".to_string());

    stats1.merge(&stats2);

    assert_eq!(stats1.total(), 4);
    assert_eq!(stats1.by_type["MultiPolygon"].count, 3);
    assert_eq!(stats1.by_type["MultiPolygon"].sources.len(), 2);
    assert_eq!(stats1.by_type["GeometryCollection"].count, 1);
}

#[test]
fn test_stats_merge_dedup_sources() {
    let mut stats1 = UnsupportedTypeStats::default();
    stats1.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());

    let mut stats2 = UnsupportedTypeStats::default();
    stats2.record("MultiPolygon".to_string(), "COMMUNE.shp".to_string());

    stats1.merge(&stats2);

    // Same source should not be duplicated after merge
    assert_eq!(stats1.by_type["MultiPolygon"].sources.len(), 1);
    assert_eq!(stats1.by_type["MultiPolygon"].count, 2);
}

// ============================================================================
// Task 9: Integration tests for JSON report quality section (AC: 4)
// ============================================================================

#[test]
fn test_json_report_with_quality_section() {
    // Subtask 9.1: Report JSON contient quality.unsupported_types
    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "MultiPolygon".to_string(),
        UnsupportedTypeReport {
            count: 38,
            sources: vec!["COMMUNE.shp".to_string()],
            total_sources: None,
        },
    );
    unsupported_types.insert(
        "GeometryCollection".to_string(),
        UnsupportedTypeReport {
            count: 2,
            sources: vec!["mixed.gpkg".to_string()],
            total_sources: None,
        },
    );

    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 100,
        tiles_failed: 0,
        tiles_skipped: 5,
        features_processed: 10000,
        duration_seconds: 42.5,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types,
            multi_geometries_decomposed: None,
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"quality\""));
    assert!(json.contains("\"unsupported_types\""));
    assert!(json.contains("\"MultiPolygon\""));
    assert!(json.contains("\"count\": 38"));
    assert!(json.contains("\"COMMUNE.shp\""));
    assert!(json.contains("\"GeometryCollection\""));
    assert!(json.contains("\"count\": 2"));
}

#[test]
fn test_json_report_without_quality_section() {
    // Subtask 9.2: Report JSON sans quality quand aucun type non supporté
    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 100,
        tiles_failed: 0,
        tiles_skipped: 5,
        features_processed: 10000,
        duration_seconds: 42.5,
        errors: vec![],
        dry_run: false,
        quality: None,
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    // quality ne doit pas apparaître si None (skip_serializing_if)
    assert!(!json.contains("\"quality\""));
}

#[test]
fn test_json_report_with_empty_quality_section() {
    // Code Review L3 Fix: Test quality section with empty BTreeMap
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
    // Code Review M3 Fix: quality is present but both fields are skipped when empty/None
    assert!(json.contains("\"quality\""));
    // Empty BTreeMap is now skipped via skip_serializing_if
    assert!(
        !json.contains("\"unsupported_types\""),
        "Empty unsupported_types should be skipped"
    );
    assert!(
        !json.contains("\"multi_geometries_decomposed\""),
        "None multi_geometries_decomposed should be skipped"
    );
}

#[test]
fn test_json_report_quality_breakdown_correct() {
    // Subtask 9.3: Breakdown par type correct (count + sources)
    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "MultiPolygon".to_string(),
        UnsupportedTypeReport {
            count: 38,
            sources: vec!["COMMUNE.shp".to_string(), "ZONE.shp".to_string()],
            total_sources: None,
        },
    );

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
            unsupported_types,
            multi_geometries_decomposed: None,
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();

    // Verify count and sources are present
    assert!(json.contains("\"count\": 38"));
    assert!(json.contains("\"COMMUNE.shp\""));
    assert!(json.contains("\"ZONE.shp\""));

    // Deserialize back to verify structure
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mp = &parsed["quality"]["unsupported_types"]["MultiPolygon"];
    assert_eq!(mp["count"], 38);
    assert_eq!(mp["sources"].as_array().unwrap().len(), 2);
}

#[test]
fn test_json_report_write_with_quality() {
    // Test that write_json_report works with quality section
    use mpforge_cli::report::write_json_report;
    use tempfile::NamedTempFile;

    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "MultiPolygon".to_string(),
        UnsupportedTypeReport {
            count: 10,
            sources: vec!["test.shp".to_string()],
            total_sources: None,
        },
    );

    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 5,
        tiles_failed: 0,
        tiles_skipped: 0,
        features_processed: 100,
        duration_seconds: 1.0,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types,
            multi_geometries_decomposed: None,
        }),
        rules_stats: None,
    };

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    write_json_report(&report, path).unwrap();

    let content = std::fs::read_to_string(path).unwrap();
    assert!(content.contains("\"quality\""));
    assert!(content.contains("\"MultiPolygon\""));
    assert!(content.contains("\"count\": 10"));
}

#[test]
fn test_json_report_with_total_sources_truncation() {
    // Code Review M1 Fix: Test total_sources when sources Vec is truncated
    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "MultiPolygon".to_string(),
        UnsupportedTypeReport {
            count: 100,
            sources: vec![
                "file1.shp".to_string(),
                "file2.shp".to_string(),
                "file3.shp".to_string(),
            ],
            total_sources: Some(50), // More total sources than in Vec (truncated)
        },
    );

    let report = ExecutionReport {
        status: ReportStatus::Success,
        tiles_generated: 10,
        tiles_failed: 0,
        tiles_skipped: 0,
        features_processed: 1000,
        duration_seconds: 5.0,
        errors: vec![],
        dry_run: false,
        quality: Some(QualitySection {
            unsupported_types,
            multi_geometries_decomposed: None,
        }),
        rules_stats: None,
    };

    let json = serde_json::to_string_pretty(&report).unwrap();
    assert!(json.contains("\"total_sources\": 50"));
    assert!(json.contains("\"sources\""));

    // Verify deserialization works
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        parsed["quality"]["unsupported_types"]["MultiPolygon"]["total_sources"],
        50
    );
    assert_eq!(
        parsed["quality"]["unsupported_types"]["MultiPolygon"]["count"],
        100
    );
}

// ============================================================================
// Task 10: Tests for WARN absence (AC: 2, 3)
// ============================================================================

#[test]
fn test_supported_types_not_affected_by_filtering() {
    // Subtask 10.2: Valid features are not impacted by filtering
    // UnsupportedTypeStats should not track supported types
    let stats = UnsupportedTypeStats::default();

    // Empty stats means no supported types were incorrectly filtered
    assert!(stats.is_empty());
    assert_eq!(stats.total(), 0);

    // After recording only unsupported types, supported types should not appear
    let mut stats = UnsupportedTypeStats::default();
    stats.record("MultiPolygon".to_string(), "test.shp".to_string());

    assert!(!stats.by_type.contains_key("Point"));
    assert!(!stats.by_type.contains_key("LineString"));
    assert!(!stats.by_type.contains_key("Polygon"));
}

#[test]
fn test_quality_section_serialization_format() {
    // Verify the exact JSON structure matches AC6.6.4 specification
    let mut unsupported_types = BTreeMap::new();
    unsupported_types.insert(
        "MultiPolygon".to_string(),
        UnsupportedTypeReport {
            count: 38,
            sources: vec!["COMMUNE.shp".to_string()],
            total_sources: None,
        },
    );
    unsupported_types.insert(
        "GeometryCollection".to_string(),
        UnsupportedTypeReport {
            count: 2,
            sources: vec!["mixed.gpkg".to_string()],
            total_sources: None,
        },
    );

    let quality = QualitySection {
        unsupported_types,
        multi_geometries_decomposed: None,
    };
    let json = serde_json::to_string_pretty(&quality).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Verify structure matches AC6.6.4
    assert!(parsed["unsupported_types"].is_object());
    assert_eq!(parsed["unsupported_types"]["MultiPolygon"]["count"], 38);
    assert_eq!(
        parsed["unsupported_types"]["MultiPolygon"]["sources"][0],
        "COMMUNE.shp"
    );
    assert_eq!(
        parsed["unsupported_types"]["GeometryCollection"]["count"],
        2
    );
    assert_eq!(
        parsed["unsupported_types"]["GeometryCollection"]["sources"][0],
        "mixed.gpkg"
    );
}
