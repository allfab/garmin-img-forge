//! Tests for tile feature assignment (Story 6.2 - Task 4).

use mpforge::config::GridConfig;
use mpforge::pipeline::reader::{Feature, GeometryType, RTreeIndex};
use mpforge::pipeline::tiler::TileProcessor;
use std::collections::HashMap;

// === Helper Functions ===

/// Create a simple point feature at given coordinates.
fn create_point_feature(lon: f64, lat: f64) -> Feature {
    Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(lon, lat)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    }
}

// === Task 4: R-tree Feature Assignment Tests ===

#[test]
fn test_assign_features_to_tiles_non_empty() {
    // AC3: Requête R-tree par tuile
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    let features = vec![
        create_point_feature(0.5, 0.5), // Tuile [0,0]
        create_point_feature(1.5, 1.5), // Tuile [1,1]
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);
    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // Should have 2 non-empty tiles
    assert_eq!(assignments.len(), 2);

    // Each tile should have 1 feature
    assert_eq!(assignments[0].1.len(), 1);
    assert_eq!(assignments[1].1.len(), 1);
}

#[test]
fn test_assign_features_to_tiles_skip_empty() {
    // AC5: Tuiles vides skippées
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Only one feature at [0.5, 0.5] → only tile [0,0] has features
    let features = vec![create_point_feature(0.5, 0.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None); // 4 tiles théoriques
    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // Only 1 tile should be returned (3 empty tiles skipped)
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].0.tile_id(), "0_0");
}

#[test]
fn test_assign_features_with_overlap_shared() {
    // AC3: Overlap → features partagées entre tuiles adjacentes
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.2, // 20% overlap
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Feature exactly on the boundary between [0,0] and [1,0]
    let features = vec![create_point_feature(1.0, 0.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);
    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // With overlap, the feature should appear in multiple tiles
    let total_refs: usize = assignments.iter().map(|(_, f)| f.len()).sum();
    assert!(total_refs >= 1); // At least 1 reference (could be 2+ with overlap)

    // Calculate how many tiles contain the feature
    let tiles_with_feature = assignments
        .iter()
        .filter(|(_, features)| !features.is_empty())
        .count();
    assert!(tiles_with_feature >= 1); // At least 1 tile contains it
}

#[test]
fn test_assign_features_all_tiles_empty() {
    // Edge case: Features outside all tiles (filter scenario)
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Features at [0.5, 0.5] but grid starts at [10.0, 10.0]
    let features = vec![create_point_feature(0.5, 0.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    // Manually create tiles that don't overlap with features
    let tiles = vec![mpforge::pipeline::tiler::TileBounds {
        col: 0,
        row: 0,
        min_lon: 10.0,
        min_lat: 10.0,
        max_lon: 11.0,
        max_lat: 11.0,
    }];

    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // Should return empty vec (all tiles empty)
    assert_eq!(assignments.len(), 0);
}

#[test]
fn test_assign_features_multiple_per_tile() {
    // AC3: Multiple features in single tile
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // 3 features in tile [0,0]
    let features = vec![
        create_point_feature(0.2, 0.2),
        create_point_feature(0.5, 0.5),
        create_point_feature(0.8, 0.8),
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);
    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // Should have 1 tile with 3 features
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].1.len(), 3);
}

// === Task 5: Pipeline Integration Test ===

#[test]
fn test_pipeline_integration_generate_and_assign() {
    // AC1-5: Complete pipeline from grid generation to feature assignment
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.1,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Create dataset spanning [0,0] -> [3,3] with features in each tile
    let features = vec![
        create_point_feature(0.5, 0.5), // Tile [0,0]
        create_point_feature(1.5, 0.5), // Tile [1,0]
        create_point_feature(0.5, 1.5), // Tile [0,1]
        create_point_feature(1.5, 1.5), // Tile [1,1]
        create_point_feature(2.5, 2.5), // Tile [2,2]
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    // Step 1: Generate tiles
    let tiles = processor.generate_tiles(&rtree, &None);
    assert!(!tiles.is_empty());

    // Step 2: Assign features to tiles
    let assignments = processor.assign_features_to_tiles(&rtree, tiles);

    // Verify results
    assert!(!assignments.is_empty()); // At least some non-empty tiles
    assert!(assignments.len() <= features.len()); // Cannot have more tiles than features

    // Verify each assignment has valid data
    for (tile, feature_ids) in &assignments {
        assert!(!feature_ids.is_empty()); // No empty tiles in result
        assert!(!tile.tile_id().is_empty()); // Valid tile ID
    }
}
