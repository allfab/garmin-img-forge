//! Tests for multi-geometry decomposition functionality (Story 6.7).
//!
//! Tests cover:
//! - MultiGeometryStats accumulation and reporting
//! - Multi-geometry decomposition (MultiPoint, MultiLineString, MultiPolygon)
//! - GeometryCollection filtering
//! - Simple geometry non-regression

use mpforge_cli::pipeline::reader::MultiGeometryStats;

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
