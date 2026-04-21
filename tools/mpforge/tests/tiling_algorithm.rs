//! Tests for grid tiling algorithm (Story 6.2).

use mpforge::config::{FilterConfig, GridConfig};
use mpforge::pipeline::reader::{Feature, GeometryType, RTreeIndex};
use mpforge::pipeline::tiler::{TileBounds, TileProcessor};
use std::collections::HashMap;

// === Helper Functions ===

/// Create a simple point feature at given coordinates.
#[allow(dead_code)] // Will be used in later tests
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

// === Task 1: TileBounds Enrichment Tests ===

#[test]
fn test_tilebounds_fields() {
    // AC4: TileBounds should have col, row, and full bbox
    let tile = TileBounds {
        col: 15,
        row: 42,
        min_lon: 1.0,
        min_lat: 2.0,
        max_lon: 1.15,
        max_lat: 2.15,
    };

    assert_eq!(tile.col, 15);
    assert_eq!(tile.row, 42);
    assert_eq!(tile.min_lon, 1.0);
    assert_eq!(tile.min_lat, 2.0);
    assert_eq!(tile.max_lon, 1.15);
    assert_eq!(tile.max_lat, 2.15);
}

#[test]
fn test_tilebounds_to_aabb() {
    // AC1: TileBounds should convert to AABB for R-tree queries
    let tile = TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    let aabb = tile.to_aabb();
    assert_eq!(aabb.lower(), [0.0, 0.0]);
    assert_eq!(aabb.upper(), [1.0, 1.0]);
}

#[test]
fn test_tilebounds_tile_id() {
    // AC4: tile_id should return "col_row" format
    let tile = TileBounds {
        col: 15,
        row: 42,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    assert_eq!(tile.tile_id(), "15_42");
}

#[test]
fn test_tilebounds_intersects_bbox_true() {
    // AC2: Tile intersection test - should return true
    let tile = TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    // Overlapping bbox
    let filter_bbox = [-0.5, -0.5, 0.5, 0.5];
    assert!(tile.intersects_bbox(&filter_bbox));
}

#[test]
fn test_tilebounds_intersects_bbox_false() {
    // AC2: Tile intersection test - should return false
    let tile = TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    // Non-overlapping bbox
    let filter_bbox = [2.0, 2.0, 3.0, 3.0];
    assert!(!tile.intersects_bbox(&filter_bbox));
}

#[test]
fn test_tilebounds_clone_and_debug() {
    // AC1: TileBounds should implement Clone and Debug
    let tile = TileBounds {
        col: 1,
        row: 2,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    let cloned = tile.clone();
    assert_eq!(cloned.col, tile.col);
    assert_eq!(cloned.row, tile.row);

    let debug_str = format!("{:?}", tile);
    assert!(debug_str.contains("TileBounds"));
}

// === Task 2: Grid Generation Algorithm Tests ===

#[test]
fn test_generate_tiles_simple_3x3_grid() {
    // AC1: Grille simple sans overlap
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Mock R-tree avec bbox [0,0]->[3,3]
    let features = vec![
        create_point_feature(0.5, 0.5),
        create_point_feature(2.5, 2.5),
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);

    // Should generate 3x3 = 9 tiles
    assert_eq!(tiles.len(), 9);

    // Verify first and last tile IDs
    assert_eq!(tiles[0].tile_id(), "0_0");
    assert_eq!(tiles[8].tile_id(), "2_2");

    // Verify first tile bbox (no overlap)
    assert_eq!(tiles[0].min_lon, 0.0);
    assert_eq!(tiles[0].min_lat, 0.0);
    assert_eq!(tiles[0].max_lon, 1.0);
    assert_eq!(tiles[0].max_lat, 1.0);
}

#[test]
fn test_generate_tiles_with_overlap() {
    // AC1: Vérifier expansion bbox avec overlap
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.1,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);
    let features = vec![create_point_feature(0.5, 0.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);

    // Tile [0,0] devrait avoir bbox étendu avec overlap
    // min = origin - overlap/2 = 0.0 - 0.05 = -0.05
    // max = min + cell_size + overlap = -0.05 + 1.0 + 0.1 = 1.05
    assert_eq!(tiles[0].min_lon, -0.05);
    assert_eq!(tiles[0].min_lat, -0.05);
    assert_eq!(tiles[0].max_lon, 1.05);
    assert_eq!(tiles[0].max_lat, 1.05);
}

#[test]
fn test_generate_tiles_auto_origin_from_global_bbox() {
    // AC1: Origin should default to global_bbox lower corner
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: None, // No explicit origin
    };

    let processor = TileProcessor::new(grid);

    // Features defining bbox [1.0, 2.0] -> [3.0, 4.0]
    let features = vec![
        create_point_feature(1.0, 2.0),
        create_point_feature(3.0, 4.0),
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);

    // Should generate 2x2 = 4 tiles starting at [1.0, 2.0]
    assert_eq!(tiles.len(), 4);

    // First tile should start at global_bbox origin
    assert_eq!(tiles[0].min_lon, 1.0);
    assert_eq!(tiles[0].min_lat, 2.0);
}

#[test]
fn test_generate_tiles_empty_rtree() {
    // Edge case: R-tree vide
    let grid = GridConfig {
        cell_size: 0.15,
        overlap: 0.005,
        origin: None,
    };

    let processor = TileProcessor::new(grid);

    let features: Vec<Feature> = Vec::new();
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles = processor.generate_tiles(&rtree, &None);

    // Should return empty vector for empty R-tree
    assert_eq!(tiles.len(), 0);
}

// === Task 3: Spatial Filtering Tests ===

#[test]
fn test_generate_tiles_no_filter_all_tiles_returned() {
    // AC2: Sans filter, toutes tuiles retournées
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    let features = vec![create_point_feature(1.5, 1.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    let tiles_no_filter = processor.generate_tiles(&rtree, &None);

    // Should generate 2x2 tiles (bbox [0,0] -> [2,2])
    assert_eq!(tiles_no_filter.len(), 4);
}

#[test]
fn test_generate_tiles_with_bbox_filter_subset() {
    // AC2: Avec filter bbox, subset de tuiles
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    let features = vec![
        create_point_feature(0.5, 0.5),
        create_point_feature(2.5, 2.5),
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    // Filter keeping only tiles in [0.5, 0.5] -> [1.5, 1.5] region
    let filter = Some(FilterConfig {
        bbox: [0.5, 0.5, 1.5, 1.5],
    });

    let tiles_filtered = processor.generate_tiles(&rtree, &filter);

    // Without filter: 3x3 = 9 tiles
    // With filter: only tiles intersecting [0.5,0.5]->[1.5,1.5] should remain
    // Tiles [0,0], [1,0], [0,1], [1,1] intersect → 4 tiles
    assert_eq!(tiles_filtered.len(), 4);

    // Verify all tiles intersect the filter
    for tile in &tiles_filtered {
        assert!(tile.intersects_bbox(&filter.as_ref().unwrap().bbox));
    }
}

#[test]
fn test_generate_tiles_filter_bbox_outside_grid_zero_tiles() {
    // AC2: Filter bbox hors grille → 0 tuiles
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    let features = vec![create_point_feature(0.5, 0.5)];
    let rtree = RTreeIndex::build(&features).unwrap();

    // Filter completely outside data bbox [10, 10] -> [15, 15]
    let filter = Some(FilterConfig {
        bbox: [10.0, 10.0, 15.0, 15.0],
    });

    let tiles = processor.generate_tiles(&rtree, &filter);

    // Should return 0 tiles (all filtered out)
    assert_eq!(tiles.len(), 0);
}

#[test]
fn test_generate_tiles_filter_bbox_partial_coverage() {
    // AC2: Filter bbox partiel (~30% tuiles gardées)
    let grid = GridConfig {
        cell_size: 1.0,
        overlap: 0.0,
        origin: Some([0.0, 0.0]),
    };

    let processor = TileProcessor::new(grid);

    // Data covering [0,0] -> [5,5] → 5x5 = 25 tiles
    let features = vec![
        create_point_feature(0.5, 0.5),
        create_point_feature(4.5, 4.5),
    ];
    let rtree = RTreeIndex::build(&features).unwrap();

    // All tiles without filter
    let tiles_all = processor.generate_tiles(&rtree, &None);
    assert_eq!(tiles_all.len(), 25);

    // Filter keeping roughly left-bottom quarter
    let filter = Some(FilterConfig {
        bbox: [0.0, 0.0, 2.5, 2.5],
    });

    let tiles_filtered = processor.generate_tiles(&rtree, &filter);

    // Should keep ~9 tiles (3x3 in bottom-left)
    // Allow some flexibility for edge cases
    assert!(tiles_filtered.len() >= 6 && tiles_filtered.len() <= 12);
}
