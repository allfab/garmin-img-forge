//! Integration tests for explicit SRS reprojection (Story 9.4)
//!
//! Tests the source_srs / target_srs configuration:
//! - AC1: Explicit source+target reprojection
//! - AC2: Default WGS84 target when target_srs omitted
//! - AC3: Backward compatibility (no source_srs/target_srs)
//! - AC4: Override SRS detected by GDAL
//! - AC5: Invalid SRS error (covered in config unit tests)

use gdal::spatial_ref::SpatialRef;
use gdal::vector::{FieldDefn, LayerAccess, OGRFieldType};
use gdal::DriverManager;
use mpforge::config::{Config, InputSource};
use mpforge::pipeline::reader::SourceReader;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test GeoPackage with a point in Lambert 93 (EPSG:2154).
/// The point (700000.0, 6600000.0) in Lambert 93 ≈ (3.0, 46.5) in WGS84.
fn create_lambert93_gpkg(dir: &std::path::Path) -> PathBuf {
    let gpkg_path = dir.join("lambert93_test.gpkg");
    let driver = DriverManager::get_driver_by_name("GPKG").unwrap();
    let mut ds = driver
        .create_vector_only(gpkg_path.to_str().unwrap())
        .unwrap();

    let mut srs = SpatialRef::from_epsg(2154).unwrap();
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let layer = ds
        .create_layer(gdal::vector::LayerOptions {
            name: "test_layer",
            srs: Some(&srs),
            ty: gdal::vector::OGRwkbGeometryType::wkbPoint,
            ..Default::default()
        })
        .unwrap();

    // Add a NAME field
    let field_defn = FieldDefn::new("NAME", OGRFieldType::OFTString).unwrap();
    field_defn.set_width(100);
    field_defn.add_to_layer(&layer).unwrap();

    // Add a point feature at (700000, 6600000) in Lambert 93
    let layer_defn = layer.defn();
    let mut feature = gdal::vector::Feature::new(layer_defn).unwrap();
    feature.set_field_string(0, "TestPoint").unwrap();
    let geom =
        gdal::vector::Geometry::from_wkt("POINT (700000.0 6600000.0)").unwrap();
    feature.set_geometry(geom).unwrap();
    feature.create(&layer).unwrap();

    gpkg_path
}

/// Create a test GeoPackage with a point in WGS84 (EPSG:4326).
/// The point (3.0, 46.5) — lon, lat.
fn create_wgs84_gpkg(dir: &std::path::Path) -> PathBuf {
    let gpkg_path = dir.join("wgs84_test.gpkg");
    let driver = DriverManager::get_driver_by_name("GPKG").unwrap();
    let mut ds = driver
        .create_vector_only(gpkg_path.to_str().unwrap())
        .unwrap();

    let mut srs = SpatialRef::from_epsg(4326).unwrap();
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
    let layer = ds
        .create_layer(gdal::vector::LayerOptions {
            name: "test_layer",
            srs: Some(&srs),
            ty: gdal::vector::OGRwkbGeometryType::wkbPoint,
            ..Default::default()
        })
        .unwrap();

    let field_defn = FieldDefn::new("NAME", OGRFieldType::OFTString).unwrap();
    field_defn.set_width(100);
    field_defn.add_to_layer(&layer).unwrap();

    let layer_defn = layer.defn();
    let mut feature = gdal::vector::Feature::new(layer_defn).unwrap();
    feature.set_field_string(0, "WGS84Point").unwrap();
    let geom = gdal::vector::Geometry::from_wkt("POINT (3.0 46.5)").unwrap();
    feature.set_geometry(geom).unwrap();
    feature.create(&layer).unwrap();

    gpkg_path
}

/// Helper to build a minimal Config for SRS tests.
fn make_config(input: InputSource) -> Config {
    Config {
        version: 1,
        grid: mpforge::config::GridConfig {
            cell_size: 10.0, // Large cell to cover all test data
            overlap: 0.0,
            origin: None,
        },
        inputs: vec![input],
        output: mpforge::config::OutputConfig {
            directory: "/tmp/mpforge_srs_test".to_string(),
            filename_pattern: "{col}_{row}.mp".to_string(),
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
// AC1: Reprojection explicite source+target
// ============================================================================

#[test]
fn test_srs_explicit_source_and_target_reprojection() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_lambert93_gpkg(temp_dir.path());

    let config = make_config(InputSource {
        path: Some(gpkg_path.to_str().unwrap().to_string()),
        connection: None,
        layer: None,
        layers: None,
        source_srs: Some("EPSG:2154".to_string()),
        target_srs: Some("EPSG:4326".to_string()),
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
    });

    // Scan extents should work with explicit SRS
    let extent = SourceReader::scan_extents(&config).unwrap();
    // Lambert 93 (700000, 6600000) ≈ WGS84 (3.0, 46.5) — check bounds are in WGS84 range
    assert!(
        extent.min_x > -10.0 && extent.min_x < 20.0,
        "min_x should be in WGS84 range, got: {}",
        extent.min_x
    );
    assert!(
        extent.min_y > 40.0 && extent.min_y < 55.0,
        "min_y should be in WGS84 range, got: {}",
        extent.min_y
    );

    // Read features should produce WGS84 coordinates
    // Use extent-based tile bounds to ensure spatial filter captures the feature
    let extent = SourceReader::scan_extents(&config).unwrap();
    let tile_bounds = mpforge::pipeline::tiler::TileBounds {
        min_lon: extent.min_x - 1.0,
        min_lat: extent.min_y - 1.0,
        max_lon: extent.max_x + 1.0,
        max_lat: extent.max_y + 1.0,
        col: 0,
        row: 0,
    };
    let (features, _, _) = SourceReader::read_features_for_tile(&config, &tile_bounds).unwrap();
    assert_eq!(features.len(), 1, "Should have exactly 1 feature");

    let feature = &features[0];
    let (lon, lat) = feature.geometry[0];
    // Lambert 93 (700000, 6600000) should be close to WGS84 (3.0, 46.5)
    // Code Review L1 Fix: Tightened tolerance from 0.5° (~50km) to 0.1° (~10km)
    assert!(
        (lon - 3.0).abs() < 0.1,
        "Longitude should be ~3.0 (WGS84), got: {}",
        lon
    );
    assert!(
        (lat - 46.5).abs() < 0.1,
        "Latitude should be ~46.5 (WGS84), got: {}",
        lat
    );
}

// ============================================================================
// AC2: Target par défaut WGS84
// ============================================================================

#[test]
fn test_srs_source_only_defaults_to_wgs84() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_lambert93_gpkg(temp_dir.path());

    let config = make_config(InputSource {
        path: Some(gpkg_path.to_str().unwrap().to_string()),
        connection: None,
        layer: None,
        layers: None,
        source_srs: Some("EPSG:2154".to_string()),
        target_srs: None, // Should default to WGS84
        attribute_filter: None,
        layer_alias: None,
            generalize: None,
    });

    let extent = SourceReader::scan_extents(&config).unwrap();
    let tile_bounds = mpforge::pipeline::tiler::TileBounds {
        min_lon: extent.min_x - 1.0,
        min_lat: extent.min_y - 1.0,
        max_lon: extent.max_x + 1.0,
        max_lat: extent.max_y + 1.0,
        col: 0,
        row: 0,
    };
    let (features, _, _) = SourceReader::read_features_for_tile(&config, &tile_bounds).unwrap();
    assert_eq!(features.len(), 1);

    let (lon, lat) = features[0].geometry[0];
    // Same result as AC1: should be WGS84 coords
    // Code Review L1 Fix: Tightened tolerance from 0.5° to 0.1°
    assert!(
        (lon - 3.0).abs() < 0.1,
        "Longitude should be ~3.0 (default WGS84), got: {}",
        lon
    );
    assert!(
        (lat - 46.5).abs() < 0.1,
        "Latitude should be ~46.5 (default WGS84), got: {}",
        lat
    );
}

// ============================================================================
// AC3: Backward compatibility (no source_srs/target_srs)
// ============================================================================

#[test]
fn test_srs_backward_compat_no_explicit_srs() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_wgs84_gpkg(temp_dir.path());

    let config = make_config(InputSource {
        path: Some(gpkg_path.to_str().unwrap().to_string()),
        connection: None,
        layer: None,
        layers: None,
        source_srs: None,
        target_srs: None,
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
    });

    let extent = SourceReader::scan_extents(&config).unwrap();
    let tile_bounds = mpforge::pipeline::tiler::TileBounds {
        min_lon: extent.min_x - 1.0,
        min_lat: extent.min_y - 1.0,
        max_lon: extent.max_x + 1.0,
        max_lat: extent.max_y + 1.0,
        col: 0,
        row: 0,
    };
    let (features, _, _) = SourceReader::read_features_for_tile(&config, &tile_bounds).unwrap();
    assert_eq!(features.len(), 1);

    let (lon, lat) = features[0].geometry[0];
    // WGS84 point (3.0, 46.5) should pass through unchanged
    assert!(
        (lon - 3.0).abs() < 0.01,
        "Longitude should be 3.0 (unchanged), got: {}",
        lon
    );
    assert!(
        (lat - 46.5).abs() < 0.01,
        "Latitude should be 46.5 (unchanged), got: {}",
        lat
    );
}

// Code Review M3: AC3 backward compat with non-WGS84 auto-detect (no explicit SRS)
// is NOT tested here because the existing auto-detect code path has a known
// axis-order issue with GDAL 3.x (axes are swapped for non-WGS84 data).
// This is a pre-existing limitation, not introduced by Story 9.4.
// For Lambert 93 data, users should use source_srs: "EPSG:2154" explicitly.
// TODO: Add a non-WGS84 auto-detect test once the axis-order issue is resolved
// (likely by adding TraditionalGisOrder to the legacy path — see reader.rs M2 comment).

// ============================================================================
// AC4: Override SRS détecté
// ============================================================================

#[test]
fn test_srs_override_detected_srs() {
    let temp_dir = TempDir::new().unwrap();
    // Create WGS84 data but declare it as Lambert 93 via source_srs
    // This will produce incorrect coordinates, proving the override works
    let gpkg_path = create_wgs84_gpkg(temp_dir.path());

    let config = make_config(InputSource {
        path: Some(gpkg_path.to_str().unwrap().to_string()),
        connection: None,
        layer: None,
        layers: None,
        // Override: tell the system the data is in Lambert 93 (it's actually WGS84)
        source_srs: Some("EPSG:2154".to_string()),
        target_srs: Some("EPSG:4326".to_string()),
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
    });

    // Use scan_extents to get proper bounds for the spatial filter
    let extent = SourceReader::scan_extents(&config).unwrap();
    let tile_bounds = mpforge::pipeline::tiler::TileBounds {
        min_lon: extent.min_x - 1.0,
        min_lat: extent.min_y - 1.0,
        max_lon: extent.max_x + 1.0,
        max_lat: extent.max_y + 1.0,
        col: 0,
        row: 0,
    };
    let (features, _, _) = SourceReader::read_features_for_tile(&config, &tile_bounds).unwrap();
    assert_eq!(features.len(), 1);

    let (lon, lat) = features[0].geometry[0];
    // WGS84 point (3.0, 46.5) treated as Lambert93 → transformed → very different coords
    // The key assertion is that the coords are NOT (3.0, 46.5) anymore
    assert!(
        (lon - 3.0).abs() > 1.0 || (lat - 46.5).abs() > 1.0,
        "Coordinates should be different from original (override applied), got: ({}, {})",
        lon,
        lat
    );
}

// ============================================================================
// AC5: SRS invalide (config validation — unit test complement)
// ============================================================================

#[test]
fn test_srs_invalid_config_yaml_parsing() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:99999"
output:
  directory: "tiles/"
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("invalid source_srs"));
}

// ============================================================================
// AC5 bis: target_srs sans source_srs (warning, pas erreur)
// ============================================================================

#[test]
fn test_srs_target_without_source_not_error() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    target_srs: "EPSG:4326"
output:
  directory: "tiles/"
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    // Should validate OK (warning only)
    assert!(config.validate().is_ok());
}

// ============================================================================
// Scan extents with explicit SRS
// ============================================================================

#[test]
fn test_srs_scan_extents_explicit() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_lambert93_gpkg(temp_dir.path());

    let config = make_config(InputSource {
        path: Some(gpkg_path.to_str().unwrap().to_string()),
        connection: None,
        layer: None,
        layers: None,
        source_srs: Some("EPSG:2154".to_string()),
        target_srs: Some("EPSG:4326".to_string()),
                attribute_filter: None,
                layer_alias: None,
            generalize: None,
    });

    let extent = SourceReader::scan_extents(&config).unwrap();

    // Extent should be in WGS84 range (France ~42-51 lat, -5-10 lon)
    assert!(
        extent.min_x > -10.0 && extent.max_x < 20.0,
        "Extent X should be in WGS84 range, got: [{}, {}]",
        extent.min_x,
        extent.max_x
    );
    assert!(
        extent.min_y > 40.0 && extent.max_y < 55.0,
        "Extent Y should be in WGS84 range, got: [{}, {}]",
        extent.min_y,
        extent.max_y
    );
}
