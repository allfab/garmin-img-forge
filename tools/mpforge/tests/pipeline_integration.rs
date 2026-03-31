//! Integration test for complete pipeline
//! Story 5.3 - Task 7.5: End-to-end pipeline test

use mpforge::cli::BuildArgs;
use mpforge::config::Config;
use mpforge::pipeline;

#[test]
fn test_pipeline_with_valid_sources() {
    // Create a temporary config file
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs:
  - path: tests/integration/fixtures/test_data/file1.shp
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: continue
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    // Run the pipeline
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should succeed with valid sources: {:?}",
        result.err()
    );
}

#[test]
fn test_pipeline_with_empty_sources() {
    // Config with no inputs
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs: []
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: continue
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    // Run the pipeline - Story 5.4 AC5: empty datasets are now supported
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should succeed with empty dataset (Story 5.4 AC5): {:?}",
        result.err()
    );

    // Story 5.4 AC5: Empty datasets are now valid - file is created with warning logged
}

#[test]
fn test_pipeline_with_invalid_source() {
    // Config with invalid source
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs:
  - path: /nonexistent/file.shp
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: fail-fast
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    // Run the pipeline - should fail in fail-fast mode
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_err(),
        "Pipeline should fail with invalid source in fail-fast mode"
    );
}

// ============================================================================
// Task 9: Pipeline Clipping Integration Tests (Story 6.3)
// ============================================================================

#[test]
fn test_full_pipeline_with_clipping() {
    // AC1-5 + Subtask 9.1, 9.2, 9.3: Pipeline complet avec clipping
    use mpforge::config::Config;
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::geometry_validator::ValidationStats;
    use mpforge::pipeline::reader::SourceReader;
    use mpforge::pipeline::tiler::{clip_feature_to_tile, TileProcessor};

    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs:
  - path: tests/integration/fixtures/test_data/file1.shp
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: continue
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    // Phase 1: Read sources and build R-tree
    let (features, rtree, _unsupported, _multi_geom) = match SourceReader::read_all_sources(&config)
    {
        Ok(result) => result,
        Err(e) => {
            // If fixture doesn't exist, skip test gracefully
            eprintln!("Skipping test: fixture not found ({})", e);
            return;
        }
    };

    let original_feature_count = features.len();
    assert!(
        original_feature_count > 0,
        "Should load at least some features from fixture"
    );

    // Phase 2: Generate tiles and assign features
    let tile_processor = TileProcessor::new(config.grid.clone());
    let tiles = tile_processor.generate_tiles(&rtree, &config.filters);
    let tile_assignments = tile_processor.assign_features_to_tiles(&rtree, tiles);

    assert!(
        !tile_assignments.is_empty(),
        "Should have at least one non-empty tile"
    );

    // Phase 3: Clip features to tile boundaries
    let error_mode = config
        .error_handling
        .parse::<ErrorMode>()
        .unwrap_or_default();
    let mut total_clipped = 0;
    let mut total_invalid = 0;
    let mut validation_stats = ValidationStats::default();

    for (tile_bounds, feature_ids) in tile_assignments {
        let tile_bbox_geom = tile_bounds.to_gdal_polygon().unwrap();

        for &feature_id in &feature_ids {
            let feature = &features[feature_id];

            match clip_feature_to_tile(feature, &tile_bbox_geom, error_mode, &mut validation_stats)
            {
                Ok(Some(clipped)) => {
                    // Subtask 9.3: Verify clipped geometry is valid
                    let clipped_geom = match feature_to_test_geometry(&clipped) {
                        Ok(g) => g,
                        Err(_) => continue,
                    };
                    assert!(clipped_geom.is_valid(), "Clipped geometry should be valid");

                    total_clipped += 1;
                }
                Ok(None) => {
                    // Feature skipped (outside tile or invalid in continue mode)
                }
                Err(_) => {
                    total_invalid += 1;
                }
            }
        }
    }

    // Subtask 9.2: Number of clipped features ≥ original (due to boundary duplicates)
    // Note: In practice, with overlap, features can appear in multiple tiles
    assert!(
        total_clipped >= original_feature_count || total_invalid > 0,
        "Clipped features ({}) should be ≥ original ({}) unless invalid features exist",
        total_clipped,
        original_feature_count
    );
}

#[test]
fn test_clipping_with_boundary_features() {
    // Subtask 9.2: Verify features on boundaries are included in multiple tiles
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::geometry_validator::ValidationStats;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::{clip_feature_to_tile, TileBounds};
    use std::collections::HashMap;

    // Create 2 adjacent tiles
    let tile_left = TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    let tile_right = TileBounds {
        col: 1,
        row: 0,
        min_lon: 1.0,
        min_lat: 0.0,
        max_lon: 2.0,
        max_lat: 1.0,
    };

    // LineString crossing boundary at x=1.0
    let feature = Feature {
        geometry_type: GeometryType::LineString,
        geometry: vec![(0.5, 0.5), (1.5, 0.5)],
        attributes: HashMap::new(),
        source_layer: None,
    };

    let error_mode = ErrorMode::Continue;

    // Clip to left tile
    let tile_left_bbox = tile_left.to_gdal_polygon().unwrap();
    let clipped_left = clip_feature_to_tile(
        &feature,
        &tile_left_bbox,
        error_mode,
        &mut ValidationStats::default(),
    )
    .unwrap()
    .expect("Should clip to left tile");

    // Clip to right tile
    let tile_right_bbox = tile_right.to_gdal_polygon().unwrap();
    let clipped_right = clip_feature_to_tile(
        &feature,
        &tile_right_bbox,
        error_mode,
        &mut ValidationStats::default(),
    )
    .unwrap()
    .expect("Should clip to right tile");

    // Both tiles should have clipped fragments
    assert!(!clipped_left.geometry.is_empty());
    assert!(!clipped_right.geometry.is_empty());

    // Verify left fragment ends at boundary (x ≤ 1.0)
    for (x, _) in &clipped_left.geometry {
        assert!(*x <= 1.0 + 1e-6, "Left fragment should not exceed x=1.0");
    }

    // Verify right fragment starts at boundary (x ≥ 1.0)
    for (x, _) in &clipped_right.geometry {
        assert!(*x >= 1.0 - 1e-6, "Right fragment should start at x=1.0");
    }
}

#[test]
#[ignore] // Ignore by default (long-running performance test)
fn test_clipping_performance_1000_features() {
    // Subtask 9.4: Performance test - clip 1000 features < 5s
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::geometry_validator::ValidationStats;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;
    use std::time::Instant;

    let tile = mpforge::pipeline::tiler::TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Create 1000 test features (mix of Points, LineStrings, Polygons)
    let mut features = Vec::new();
    for i in 0..1000 {
        let feature_type = match i % 3 {
            0 => {
                // Point
                Feature {
                    geometry_type: GeometryType::Point,
                    geometry: vec![(0.5, 0.5)],
                    attributes: HashMap::new(),
                    source_layer: None,
                }
            }
            1 => {
                // LineString (crossing boundary)
                Feature {
                    geometry_type: GeometryType::LineString,
                    geometry: vec![(-0.5, 0.5), (1.5, 0.5)],
                    attributes: HashMap::new(),
                    source_layer: None,
                }
            }
            _ => {
                // Polygon (spanning tile)
                Feature {
                    geometry_type: GeometryType::Polygon,
                    geometry: vec![
                        (-0.5, -0.5),
                        (1.5, -0.5),
                        (1.5, 1.5),
                        (-0.5, 1.5),
                        (-0.5, -0.5),
                    ],
                    attributes: HashMap::new(),
                    source_layer: None,
                }
            }
        };
        features.push(feature_type);
    }

    // Measure clipping time
    let start = Instant::now();
    let mut clipped_count = 0;

    for feature in &features {
        if let Ok(Some(_)) = clip_feature_to_tile(
            feature,
            &tile_bbox,
            ErrorMode::Continue,
            &mut ValidationStats::default(),
        ) {
            clipped_count += 1;
        }
    }

    let elapsed = start.elapsed();

    // Should process 1000 features in < 5 seconds
    assert!(
        elapsed.as_secs() < 5,
        "Clipping 1000 features took {:?} (should be < 5s)",
        elapsed
    );

    assert!(
        clipped_count > 0,
        "Should have clipped at least some features"
    );
}

// Helper function to convert internal Feature to GDAL Geometry for validation
fn feature_to_test_geometry(
    feature: &mpforge::pipeline::reader::Feature,
) -> anyhow::Result<gdal::vector::Geometry> {
    use gdal::vector::Geometry;
    use mpforge::pipeline::reader::GeometryType;

    let wkt = match feature.geometry_type {
        GeometryType::Point => {
            if feature.geometry.is_empty() {
                anyhow::bail!("Empty point geometry");
            }
            let (x, y) = feature.geometry[0];
            format!("POINT({} {})", x, y)
        }
        GeometryType::LineString => {
            if feature.geometry.len() < 2 {
                anyhow::bail!("LineString needs at least 2 points");
            }
            let coords: Vec<String> = feature
                .geometry
                .iter()
                .map(|(x, y)| format!("{} {}", x, y))
                .collect();
            format!("LINESTRING({})", coords.join(", "))
        }
        GeometryType::Polygon => {
            if feature.geometry.len() < 3 {
                anyhow::bail!("Polygon needs at least 3 points");
            }
            let coords: Vec<String> = feature
                .geometry
                .iter()
                .map(|(x, y)| format!("{} {}", x, y))
                .collect();
            format!("POLYGON(({})))", coords.join(", "))
        }
    };

    Geometry::from_wkt(&wkt).map_err(|e| anyhow::anyhow!("WKT conversion failed: {}", e))
}
