//! Unit tests for R-tree spatial indexing.
//!
//! Tests cover:
//! - AC1: All features from all sources indexed by bbox
//! - AC2: All 3 geometry types correctly indexed
//! - AC3: Spatial queries return only intersecting features
//! - AC4: Duplicate features preserved (no deduplication)

use mpforge::pipeline::reader::{Feature, FeatureEnvelope, GeometryType, RTreeIndex};
use rstar::AABB;
use std::collections::HashMap;

// ============================================================================
// Test Helpers
// ============================================================================

/// Create a test Point feature at given coordinates.
fn create_point_feature(x: f64, y: f64) -> Feature {
    Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(x, y)],
        attributes: HashMap::new(),
        source_layer: None,
    }
}

/// Create a test LineString feature from two points.
fn create_line_feature(x1: f64, y1: f64, x2: f64, y2: f64) -> Feature {
    Feature {
        geometry_type: GeometryType::LineString,
        geometry: vec![(x1, y1), (x2, y2)],
        attributes: HashMap::new(),
        source_layer: None,
    }
}

/// Create a test Polygon feature (rectangle).
fn create_polygon_feature(x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> Feature {
    Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![
            (x_min, y_min),
            (x_max, y_min),
            (x_max, y_max),
            (x_min, y_max),
            (x_min, y_min), // Closing point
        ],
        attributes: HashMap::new(),
        source_layer: None,
    }
}

// ============================================================================
// Task 4: Tests unitaires R-tree construction (AC: 1, 2, 4)
// ============================================================================

#[test]
fn test_rtree_build_with_mixed_geometry_types() {
    // Subtask 4.1: Test build_index avec 3 types géométriques
    // AC1, AC2: R-tree indexe tous les types géométriques

    let features = vec![
        create_point_feature(0.0, 0.0),             // Point
        create_line_feature(1.0, 1.0, 2.0, 2.0),    // LineString
        create_polygon_feature(3.0, 3.0, 4.0, 4.0), // Polygon
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    // Verify all features are indexed
    assert_eq!(
        rtree.tree_size(),
        3,
        "R-tree should contain exactly 3 features"
    );

    // Verify global bbox encompasses all features (0,0 to 4,4)
    let global_bbox = rtree.global_bbox();
    let lower = global_bbox.lower();
    let upper = global_bbox.upper();

    assert!(
        lower[0] <= 0.0 && lower[1] <= 0.0,
        "Global bbox min should be at or before (0,0)"
    );
    assert!(
        upper[0] >= 4.0 && upper[1] >= 4.0,
        "Global bbox max should be at or after (4,4)"
    );
}

#[test]
fn test_rtree_index_size_equals_input_count() {
    // Subtask 4.2: Test index size = input features count
    // AC1: All features indexed

    let features = vec![
        create_point_feature(0.0, 0.0),
        create_point_feature(1.0, 1.0),
        create_point_feature(2.0, 2.0),
        create_point_feature(3.0, 3.0),
        create_point_feature(4.0, 4.0),
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    assert_eq!(
        rtree.tree_size(),
        5,
        "R-tree size should match input feature count"
    );
}

#[test]
fn test_rtree_preserves_duplicates() {
    // Subtask 4.3: Test duplicates préservées (même géométrie, 2 features distinctes)
    // AC4: Duplicate features preserved (no deduplication)

    let features = vec![
        create_point_feature(1.0, 1.0),
        create_point_feature(1.0, 1.0), // Exact duplicate geometry
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    assert_eq!(
        rtree.tree_size(),
        2,
        "R-tree should index both features even if geometries are identical"
    );
}

#[test]
fn test_rtree_global_bbox_calculation() {
    // Subtask 4.4: Test bbox global calculé correctement
    // AC1: Global bbox encompasses all features

    let features = vec![
        create_point_feature(-10.0, -5.0), // Min corner
        create_point_feature(20.0, 15.0),  // Max corner
        create_point_feature(0.0, 0.0),    // Middle
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let global_bbox = rtree.global_bbox();
    let lower = global_bbox.lower();
    let upper = global_bbox.upper();

    assert_eq!(lower[0], -10.0, "Global bbox min X should be -10.0");
    assert_eq!(lower[1], -5.0, "Global bbox min Y should be -5.0");
    assert_eq!(upper[0], 20.0, "Global bbox max X should be 20.0");
    assert_eq!(upper[1], 15.0, "Global bbox max Y should be 15.0");
}

#[test]
fn test_rtree_build_empty_features() {
    // Edge case: Build R-tree from empty feature vector
    let features: Vec<Feature> = vec![];

    let result = RTreeIndex::build(&features);

    // Should succeed with empty tree
    assert!(
        result.is_ok(),
        "Building R-tree from empty features should succeed"
    );

    let rtree = result.unwrap();
    assert_eq!(rtree.tree_size(), 0, "Empty R-tree should have size 0");
}

// ============================================================================
// Task 5: Tests unitaires requêtes spatiales (AC: 3)
// ============================================================================

#[test]
fn test_rtree_query_intersecting_features() {
    // Subtask 5.1: Test query_bbox retourne features intersectant
    // AC3: Spatial query returns only intersecting features

    let features = vec![
        create_point_feature(0.5, 0.5), // Inside tile [0,0,1,1]
        create_point_feature(5.0, 5.0), // Outside tile
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let tile_bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let candidates = rtree.query_intersecting(&tile_bbox);

    assert_eq!(
        candidates.len(),
        1,
        "Query should return exactly 1 feature inside tile"
    );
    assert_eq!(
        candidates[0], 0,
        "Query should return feature ID 0 (first feature)"
    );
}

#[test]
fn test_rtree_query_excludes_outside_features() {
    // Subtask 5.2: Test query_bbox exclut features hors zone
    // AC3: Features outside bbox are excluded

    let features = vec![
        create_point_feature(10.0, 10.0), // Far outside
        create_point_feature(20.0, 20.0), // Even further
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let tile_bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let candidates = rtree.query_intersecting(&tile_bbox);

    assert_eq!(
        candidates.len(),
        0,
        "Query should return no features when all are outside bbox"
    );
}

#[test]
fn test_rtree_query_empty_result() {
    // Subtask 5.3: Test requête bbox vide retourne 0 features
    // Edge case: No features intersect query bbox

    let features = vec![create_point_feature(10.0, 10.0)];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let tile_bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let candidates = rtree.query_intersecting(&tile_bbox);

    assert_eq!(
        candidates.len(),
        0,
        "Query should return empty result when no features intersect"
    );
}

#[test]
fn test_rtree_query_all_features() {
    // Subtask 5.4: Test requête bbox couvrant tout retourne toutes features
    // AC3: Large bbox returns all features

    let features = vec![
        create_point_feature(0.0, 0.0),
        create_point_feature(5.0, 5.0),
        create_point_feature(10.0, 10.0),
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let large_bbox = AABB::from_corners([-100.0, -100.0], [100.0, 100.0]);
    let candidates = rtree.query_intersecting(&large_bbox);

    assert_eq!(
        candidates.len(),
        3,
        "Query with large bbox should return all features"
    );

    // Verify all feature IDs are present (order doesn't matter)
    assert!(candidates.contains(&0), "Should contain feature ID 0");
    assert!(candidates.contains(&1), "Should contain feature ID 1");
    assert!(candidates.contains(&2), "Should contain feature ID 2");
}

#[test]
fn test_rtree_query_with_linestring() {
    // Test query with LineString geometry (bbox covers line segment)
    // AC2, AC3: All geometry types correctly indexed and queryable

    let features = vec![
        create_line_feature(0.0, 0.0, 2.0, 2.0), // Line crossing tile [0,0,1,1]
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let tile_bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let candidates = rtree.query_intersecting(&tile_bbox);

    assert_eq!(
        candidates.len(),
        1,
        "Query should return LineString that intersects tile bbox"
    );
}

#[test]
fn test_rtree_query_with_polygon() {
    // Test query with Polygon geometry (bbox covers polygon)
    // AC2, AC3: All geometry types correctly indexed and queryable

    let features = vec![
        create_polygon_feature(0.5, 0.5, 1.5, 1.5), // Polygon overlapping tile [0,0,1,1]
    ];

    let rtree = RTreeIndex::build(&features).expect("Failed to build R-tree");

    let tile_bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let candidates = rtree.query_intersecting(&tile_bbox);

    assert_eq!(
        candidates.len(),
        1,
        "Query should return Polygon that intersects tile bbox"
    );
}

// ============================================================================
// FeatureEnvelope Tests
// ============================================================================

#[test]
fn test_feature_envelope_creation() {
    // Test FeatureEnvelope struct creation
    let bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let envelope = FeatureEnvelope {
        feature_id: 42,
        bbox,
    };

    assert_eq!(envelope.feature_id, 42);
    assert_eq!(envelope.bbox, bbox);
}

#[test]
fn test_feature_envelope_rtree_object_trait() {
    // Test that FeatureEnvelope implements RTreeObject trait correctly
    use rstar::RTreeObject;

    let bbox = AABB::from_corners([0.0, 0.0], [1.0, 1.0]);
    let envelope = FeatureEnvelope {
        feature_id: 0,
        bbox,
    };

    // Call envelope() method from RTreeObject trait
    let returned_bbox = envelope.envelope();

    assert_eq!(
        returned_bbox, bbox,
        "envelope() should return the stored bbox"
    );
}
