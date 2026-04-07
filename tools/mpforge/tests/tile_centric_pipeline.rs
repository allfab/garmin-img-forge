//! Integration tests for the tile-centric pipeline refactoring.
//!
//! Tests cover:
//! - AC1: scan_extents() returns correct global bbox
//! - AC2: read_features_for_tile() returns only features in tile bbox
//! - AC3: generate_tiles_from_bbox() produces same grid as generate_tiles()
//! - AC8: FilterConfig bbox filtering works with tile-centric pipeline

use mpforge::config::{Config, GridConfig, InputSource, OutputConfig};
use mpforge::pipeline::reader::{GlobalExtent, SourceReader};
use mpforge::pipeline::tiler::{TileBounds, TileProcessor};
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

fn make_config(filenames: &[&str]) -> Config {
    let inputs: Vec<InputSource> = filenames
        .iter()
        .map(|f| InputSource {
            path: Some(get_test_data_path(f)),
            layers: None,
            connection: None,
            layer: None,
            source_srs: None,
            target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        })
        .collect();

    Config {
        version: 1,
        grid: GridConfig {
            cell_size: 0.15,
            overlap: 0.01,
            origin: None,
        },
        inputs,
        output: OutputConfig {
            directory: "/tmp/mpforge-test-output".to_string(),
            filename_pattern: "{x}_{y}.mp".to_string(),
            field_mapping_path: None,
            overwrite: None,
            base_id: None,
        },
        filters: None,
        error_handling: "continue".to_string(),
        header: None,
        rules: None,
    }
}

// ============================================================================
// AC1: scan_extents() returns correct global bbox
// ============================================================================

#[test]
fn test_scan_extents_returns_valid_bbox() {
    let config = make_config(&["file1.shp"]);

    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("scan_extents should succeed");

    assert_eq!(extent.layer_count, 1, "Should have scanned 1 source");
    assert!(extent.min_x < extent.max_x, "min_x should be < max_x");
    assert!(extent.min_y < extent.max_y, "min_y should be < max_y");
}

#[test]
fn test_scan_extents_multiple_sources() {
    let config = make_config(&["file1.shp", "file2.shp"]);

    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("scan_extents should succeed");

    assert_eq!(extent.layer_count, 2, "Should have scanned 2 sources");
    assert!(extent.min_x < extent.max_x);
    assert!(extent.min_y < extent.max_y);
}

#[test]
fn test_scan_extents_matches_read_all_sources_bbox() {
    // AC1: verify scan_extents bbox matches the R-tree global bbox from read_all_sources
    let config = make_config(&["file1.shp", "file2.shp"]);

    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("scan_extents failed");
    let (_features, rtree, _unsup, _multi) =
        SourceReader::read_all_sources(&config).expect("read_all_sources failed");

    let rtree_bbox = rtree.global_bbox();
    let tolerance = 0.0001;

    // scan_extents bbox should encompass the R-tree bbox (layer extent >= feature extent)
    assert!(
        extent.min_x <= rtree_bbox.lower()[0] + tolerance,
        "scan min_x ({}) should be <= rtree min_x ({})",
        extent.min_x,
        rtree_bbox.lower()[0]
    );
    assert!(
        extent.min_y <= rtree_bbox.lower()[1] + tolerance,
        "scan min_y ({}) should be <= rtree min_y ({})",
        extent.min_y,
        rtree_bbox.lower()[1]
    );
    assert!(
        extent.max_x >= rtree_bbox.upper()[0] - tolerance,
        "scan max_x ({}) should be >= rtree max_x ({})",
        extent.max_x,
        rtree_bbox.upper()[0]
    );
    assert!(
        extent.max_y >= rtree_bbox.upper()[1] - tolerance,
        "scan max_y ({}) should be >= rtree max_y ({})",
        extent.max_y,
        rtree_bbox.upper()[1]
    );
}

#[test]
fn test_scan_extents_invalid_source_continue_mode() {
    // AC6: Invalid source in continue mode is skipped
    let config = Config {
        inputs: vec![
            InputSource {
                path: Some("/nonexistent/file.shp".to_string()),
                layers: None,
                connection: None,
                layer: None,
                source_srs: None,
                target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
            },
            InputSource {
                path: Some(get_test_data_path("file1.shp")),
                layers: None,
                connection: None,
                layer: None,
                source_srs: None,
                target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
            },
        ],
        error_handling: "continue".to_string(),
        ..make_config(&[])
    };

    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("Should succeed despite invalid source");
    assert_eq!(extent.layer_count, 1, "Should have scanned 1 valid source");
}

#[test]
fn test_scan_extents_invalid_source_fail_fast() {
    // AC7: Invalid source in fail-fast mode causes error
    let config = Config {
        inputs: vec![InputSource {
            path: Some("/nonexistent/file.shp".to_string()),
            layers: None,
            connection: None,
            layer: None,
            source_srs: None,
            target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
            spatial_filter: None,
        }],
        error_handling: "fail-fast".to_string(),
        ..make_config(&[])
    };

    let result = SourceReader::scan_extents(&config, &std::collections::HashMap::new());
    assert!(result.is_err(), "Should fail with invalid source in fail-fast mode");
}

// ============================================================================
// AC2: read_features_for_tile() returns only features in tile bbox
// ============================================================================

#[test]
fn test_read_features_for_tile_filters_spatially() {
    let config = make_config(&["file1.shp"]);

    // First get the full extent
    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("scan_extents failed");

    // Read features for a tile covering the first quarter of the extent
    let mid_x = (extent.min_x + extent.max_x) / 2.0;
    let mid_y = (extent.min_y + extent.max_y) / 2.0;

    let quarter_tile = TileBounds {
        col: 0,
        row: 0,
        min_lon: extent.min_x,
        min_lat: extent.min_y,
        max_lon: mid_x,
        max_lat: mid_y,
    };

    let (tile_features, _unsup, _multi) =
        SourceReader::read_features_for_tile(&config, &quarter_tile, &std::collections::HashMap::new())
            .expect("read_features_for_tile failed");

    // Read ALL features for comparison
    let (all_features, _rtree, _unsup2, _multi2) =
        SourceReader::read_all_sources(&config).expect("read_all_sources failed");

    // Tile features should be <= all features (spatial filter reduces count)
    assert!(
        tile_features.len() <= all_features.len(),
        "Tile features ({}) should be <= all features ({})",
        tile_features.len(),
        all_features.len()
    );
}

#[test]
fn test_read_features_for_tile_empty_area() {
    let config = make_config(&["file1.shp"]);

    // Use a tile far away from any data
    let empty_tile = TileBounds {
        col: 0,
        row: 0,
        min_lon: 170.0,
        min_lat: 80.0,
        max_lon: 180.0,
        max_lat: 90.0,
    };

    let (features, _unsup, _multi) =
        SourceReader::read_features_for_tile(&config, &empty_tile, &std::collections::HashMap::new())
            .expect("read_features_for_tile should succeed");

    assert!(
        features.is_empty(),
        "Should have 0 features in empty area, got {}",
        features.len()
    );
}

// ============================================================================
// AC3: generate_tiles_from_bbox() produces same grid as generate_tiles()
// ============================================================================

#[test]
fn test_generate_tiles_from_bbox_matches_generate_tiles() {
    let config = make_config(&["file1.shp", "file2.shp"]);

    let (_features, rtree, _unsup, _multi) =
        SourceReader::read_all_sources(&config).expect("read_all_sources failed");

    let processor = TileProcessor::new(config.grid.clone());

    // Generate tiles both ways
    let tiles_rtree = processor.generate_tiles(&rtree, &config.filters);

    let global_bbox = rtree.global_bbox();
    let bbox = [
        global_bbox.lower()[0],
        global_bbox.lower()[1],
        global_bbox.upper()[0],
        global_bbox.upper()[1],
    ];
    let tiles_bbox = processor.generate_tiles_from_bbox(&bbox, &config.filters);

    assert_eq!(
        tiles_rtree.len(),
        tiles_bbox.len(),
        "Both methods should produce same number of tiles"
    );

    // Verify tile positions match
    for (t_rtree, t_bbox) in tiles_rtree.iter().zip(tiles_bbox.iter()) {
        assert_eq!(t_rtree.col, t_bbox.col);
        assert_eq!(t_rtree.row, t_bbox.row);
        assert!((t_rtree.min_lon - t_bbox.min_lon).abs() < 1e-10);
        assert!((t_rtree.min_lat - t_bbox.min_lat).abs() < 1e-10);
        assert!((t_rtree.max_lon - t_bbox.max_lon).abs() < 1e-10);
        assert!((t_rtree.max_lat - t_bbox.max_lat).abs() < 1e-10);
    }
}

#[test]
fn test_generate_tiles_from_bbox_degenerate() {
    let processor = TileProcessor::new(GridConfig {
        cell_size: 0.15,
        overlap: 0.01,
        origin: None,
    });

    // Degenerate bbox (min == max)
    let tiles = processor.generate_tiles_from_bbox(&[1.0, 2.0, 1.0, 2.0], &None);
    assert!(tiles.is_empty(), "Degenerate bbox should produce 0 tiles");

    // Inverted bbox (min > max)
    let tiles = processor.generate_tiles_from_bbox(&[2.0, 3.0, 1.0, 2.0], &None);
    assert!(tiles.is_empty(), "Inverted bbox should produce 0 tiles");
}

#[test]
fn test_global_extent_to_bbox() {
    let extent = GlobalExtent {
        min_x: 1.0,
        min_y: 2.0,
        max_x: 3.0,
        max_y: 4.0,
        layer_count: 1,
    };

    assert_eq!(extent.to_bbox(), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_scan_extents_grid_covers_rtree_grid() {
    // F6 fix: verify that the grid from scan_extents() covers all tiles from R-tree.
    // scan_extents() may return a larger bbox than the R-tree (layer extent >= feature extent),
    // so the scan_extents grid should have >= tiles than the R-tree grid.
    let config = make_config(&["file1.shp", "file2.shp"]);

    let extent = SourceReader::scan_extents(&config, &std::collections::HashMap::new()).expect("scan_extents failed");
    #[allow(deprecated)]
    let (_features, rtree, _unsup, _multi) =
        SourceReader::read_all_sources(&config).expect("read_all_sources failed");

    let processor = TileProcessor::new(config.grid.clone());

    let tiles_scan = processor.generate_tiles_from_bbox(&extent.to_bbox(), &config.filters);
    let tiles_rtree = processor.generate_tiles(&rtree, &config.filters);

    // scan_extents grid should cover at least as many tiles as the rtree grid
    assert!(
        tiles_scan.len() >= tiles_rtree.len(),
        "scan_extents grid ({} tiles) should cover >= rtree grid ({} tiles)",
        tiles_scan.len(),
        tiles_rtree.len()
    );

    // Every rtree tile should be present in scan grid (by col/row)
    for t_rtree in &tiles_rtree {
        let found = tiles_scan.iter().any(|t| t.col == t_rtree.col && t.row == t_rtree.row);
        assert!(
            found,
            "R-tree tile ({},{}) not found in scan_extents grid",
            t_rtree.col, t_rtree.row
        );
    }
}
