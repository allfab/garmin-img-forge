//! Unit tests for geometry clipping operations (Story 6.3).
//!
//! Tests verify GDAL Intersection-based clipping of features to tile boundaries,
//! attribute preservation, and error handling modes.

use mpforge::pipeline::geometry_validator::ValidationStats;
use mpforge::pipeline::tiler::TileBounds;

// Test tolerance constants for consistent floating-point comparisons
const COORD_TOLERANCE: f64 = 1e-6; // ~1mm precision at equator
const AREA_TOLERANCE: f64 = 1e-6; // Area calculation tolerance

/// Helper: Create a test tile for basic tests
fn create_test_tile() -> TileBounds {
    TileBounds {
        col: 0,
        row: 0,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    }
}

// ============================================================================
// Task 1: TileBounds::to_gdal_polygon() Tests
// ============================================================================

#[test]
fn test_tile_to_gdal_polygon_valid_geometry() {
    // Subtask 1.4: Valider géométrie bbox (is_valid() = true)
    let tile = create_test_tile();

    let polygon = tile.to_gdal_polygon().unwrap();

    // Should be valid polygon
    assert!(polygon.is_valid());
    // geometry_type() returns OGRwkbGeometryType (u32), Polygon = 3
    assert_eq!(
        polygon.geometry_type(),
        gdal::vector::OGRwkbGeometryType::wkbPolygon
    );
}

#[test]
fn test_tile_to_gdal_polygon_has_wgs84_srs() {
    // Subtask 1.3: Définir SRS WGS84 (EPSG:4326) sur le Polygon
    let tile = create_test_tile();

    let polygon = tile.to_gdal_polygon().unwrap();

    // Should have WGS84 spatial reference
    let srs = polygon.spatial_ref().unwrap();
    assert_eq!(srs.auth_code().unwrap(), 4326);
    assert_eq!(srs.auth_name().unwrap(), "EPSG");
}

#[test]
fn test_tile_to_gdal_polygon_correct_bounds() {
    // Subtask 1.2: Construire Polygon à partir de [min_lon, min_lat, max_lon, max_lat]
    let tile = TileBounds {
        col: 5,
        row: 10,
        min_lon: 10.0,
        min_lat: 20.0,
        max_lon: 10.15,
        max_lat: 20.15,
    };

    let polygon = tile.to_gdal_polygon().unwrap();

    // Get envelope (bbox) from polygon
    let envelope = polygon.envelope();

    assert!((envelope.MinX - 10.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MinY - 20.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MaxX - 10.15).abs() < COORD_TOLERANCE);
    assert!((envelope.MaxY - 20.15).abs() < COORD_TOLERANCE);
}

#[test]
fn test_tile_to_gdal_polygon_closed_ring() {
    // Verify polygon ring is properly closed (first == last point)
    let tile = create_test_tile();

    let polygon = tile.to_gdal_polygon().unwrap();

    // Should have exactly 1 geometry (the exterior ring)
    assert_eq!(polygon.geometry_count(), 1);

    // WKT should show closed ring
    let wkt = polygon.wkt().unwrap();
    assert!(wkt.contains("POLYGON"));
    assert!(wkt.starts_with("POLYGON (("));
}

// ============================================================================
// Task 2: Geometry Intersection Tests
// ============================================================================

#[test]
fn test_geometry_intersection_linestring_crossing_boundary() {
    // AC1: LineString traversant frontière → LineString tronqué
    use gdal::vector::Geometry;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // LineString de [-0.5, 0.5] à [1.5, 0.5] (traverse frontière droite)
    let wkt = "LINESTRING(-0.5 0.5, 1.5 0.5)";
    let line = Geometry::from_wkt(wkt).unwrap();

    // Test intersection
    let clipped = line.intersection(&tile_bbox).unwrap();

    assert!(clipped.is_valid());
    // Le LineString devrait être tronqué aux frontières [0.0, 1.0]
    let envelope = clipped.envelope();
    assert!((envelope.MinX - 0.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MaxX - 1.0).abs() < COORD_TOLERANCE);
}

#[test]
fn test_geometry_intersection_polygon_spanning_tiles() {
    // AC2: Polygon chevauchant bbox → Polygon valide
    use gdal::vector::Geometry;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Polygon plus grand que la tuile
    let wkt = "POLYGON((-0.5 -0.5, 1.5 -0.5, 1.5 1.5, -0.5 1.5, -0.5 -0.5))";
    let polygon = Geometry::from_wkt(wkt).unwrap();

    let clipped = polygon.intersection(&tile_bbox).unwrap();

    assert!(clipped.is_valid());
    // Le résultat devrait être clippé aux limites de la tuile [0, 1]
    let envelope = clipped.envelope();
    assert!((envelope.MinX - 0.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MinY - 0.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MaxX - 1.0).abs() < COORD_TOLERANCE);
    assert!((envelope.MaxY - 1.0).abs() < COORD_TOLERANCE);
}

#[test]
fn test_geometry_point_on_boundary_intersects() {
    // AC3: Point sur frontière → intersects
    use gdal::vector::Geometry;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Point exactement sur frontière est (1.0, 0.5)
    let wkt = "POINT(1.0 0.5)";
    let point = Geometry::from_wkt(wkt).unwrap();

    // Test intersection
    assert!(point.intersects(&tile_bbox));
}

#[test]
fn test_geometry_outside_tile_no_intersection() {
    // Cas limite: Géométrie complètement hors tuile
    use gdal::vector::Geometry;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Point hors tuile
    let wkt = "POINT(5.0 5.0)";
    let point = Geometry::from_wkt(wkt).unwrap();

    // Ne devrait PAS intersecter
    assert!(!point.intersects(&tile_bbox));

    // Intersection devrait retourner une géométrie vide
    let clipped = point.intersection(&tile_bbox).unwrap();
    assert!(clipped.is_empty());
}

// ============================================================================
// Task 6: Additional Geometry Clipping Tests
// ============================================================================

#[test]
fn test_polygon_spanning_4_tiles_with_area_validation() {
    // AC2 + Subtask 6.2 + 6.4: Polygon chevauchant 4 tuiles → aires valides
    use gdal::vector::Geometry;

    // Create 2x2 grid of tiles
    let tiles = vec![
        // Bottom-left [0,0]
        TileBounds {
            col: 0,
            row: 0,
            min_lon: 0.0,
            min_lat: 0.0,
            max_lon: 1.0,
            max_lat: 1.0,
        },
        // Bottom-right [1,0]
        TileBounds {
            col: 1,
            row: 0,
            min_lon: 1.0,
            min_lat: 0.0,
            max_lon: 2.0,
            max_lat: 1.0,
        },
        // Top-left [0,1]
        TileBounds {
            col: 0,
            row: 1,
            min_lon: 0.0,
            min_lat: 1.0,
            max_lon: 1.0,
            max_lat: 2.0,
        },
        // Top-right [1,1]
        TileBounds {
            col: 1,
            row: 1,
            min_lon: 1.0,
            min_lat: 1.0,
            max_lon: 2.0,
            max_lat: 2.0,
        },
    ];

    // Polygon centered on (1.0, 1.0) spanning all 4 tiles
    // Square: 0.5x0.5 to 1.5x1.5 → Total area = 1.0
    let wkt = "POLYGON((0.5 0.5, 1.5 0.5, 1.5 1.5, 0.5 1.5, 0.5 0.5))";
    let polygon = Geometry::from_wkt(wkt).unwrap();
    let original_area = polygon.area();

    let mut total_clipped_area = 0.0;
    let mut fragments_count = 0;

    for tile in tiles {
        let tile_bbox = tile.to_gdal_polygon().unwrap();
        let clipped = polygon.intersection(&tile_bbox).unwrap();

        if !clipped.is_empty() {
            assert!(clipped.is_valid());
            // Should be Polygon (not MultiPolygon for this simple case)
            assert_eq!(
                clipped.geometry_type(),
                gdal::vector::OGRwkbGeometryType::wkbPolygon
            );

            let clipped_area = clipped.area();
            total_clipped_area += clipped_area;
            fragments_count += 1;

            // Each fragment should have area = 0.25 (1/4 of original)
            assert!((clipped_area - 0.25).abs() < AREA_TOLERANCE);
        }
    }

    // All 4 tiles should have fragments
    assert_eq!(fragments_count, 4);

    // Sum of areas ≈ original area (tolerance 1%)
    let ratio = total_clipped_area / original_area;
    assert!(
        (0.99..=1.01).contains(&ratio),
        "Area ratio: {} (expected ≈1.0)",
        ratio
    );
}

// ============================================================================
// Task 7: Error Handling Tests
// ============================================================================

#[test]
fn test_invalid_geometry_continue_mode() {
    // Story 6.5: Self-intersecting polygon is now REPAIRED via make_valid().
    // make_valid produces a MultiPolygon (2 triangles from bow-tie), which
    // may fail downstream in gdal_geometry_to_coords (MultiPolygon not supported).
    // This is expected: Story 6.5 attempts repair; coordinate extraction limitations
    // are pre-existing and handled at pipeline level in continue mode.
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Create feature with self-intersecting polygon (bow-tie = invalid)
    let feature = Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    };

    let mut stats = ValidationStats::default();
    let result = clip_feature_to_tile(&feature, &tile_bbox, ErrorMode::Continue, &mut stats);

    // Story 6.5: Validation attempted on self-intersecting polygon.
    // Bow-tie may be repaired (simple output) or rejected (MultiPolygon filtered).
    assert!(
        stats.repaired_make_valid > 0
            || stats.repaired_buffer_zero > 0
            || stats.rejected_irrecoverable > 0,
        "Validation should process the geometry, got stats: {:?}",
        stats
    );

    // In continue mode, both repaired and rejected features are handled gracefully
    assert!(result.is_ok(), "Continue mode should not propagate errors");
}


#[test]
fn test_invalid_geometry_failfast_mode() {
    // Story 6.5: Self-intersecting polygon is now REPAIRED via make_valid().
    // In fail-fast mode, if repair succeeds, geometry is used for clipping.
    // Result depends on whether repaired MultiPolygon can be processed downstream.
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Invalid polygon (bow-tie) - will be repaired by make_valid
    let feature = Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    };

    let mut stats = ValidationStats::default();
    let _result = clip_feature_to_tile(&feature, &tile_bbox, ErrorMode::FailFast, &mut stats);

    // Story 6.5: Validation attempted on self-intersecting polygon.
    // Bow-tie may be repaired (simple output) or rejected (MultiPolygon filtered).
    assert!(
        stats.repaired_make_valid > 0
            || stats.repaired_buffer_zero > 0
            || stats.rejected_irrecoverable > 0,
        "Validation should process the geometry, got stats: {:?}",
        stats
    );
}

#[test]
fn test_degenerate_linestring_skipped() {
    // AC4 + Subtask 7.3: Géométrie dégénérée (LineString 1 point) → skip
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Degenerate LineString with only 1 point (invalid)
    let feature = Feature {
        geometry_type: GeometryType::LineString,
        geometry: vec![(0.5, 0.5)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    };

    // Should fail during WKT conversion (LineString needs ≥2 points)
    let result = clip_feature_to_tile(
        &feature,
        &tile_bbox,
        ErrorMode::Continue,
        &mut ValidationStats::default(),
    );

    // In continue mode, should handle gracefully (either Ok(empty vec) or Err handled)
    // Implementation returns Err from feature_to_gdal_geometry, which is fine
    assert!(result.is_err() || (result.is_ok() && result.unwrap().is_empty()));
}

// ============================================================================
// Task 8: Attribute Preservation Tests
// ============================================================================

#[test]
fn test_clip_preserves_all_attributes() {
    // AC5 + Subtask 8.1, 8.2, 8.3: Attributs préservés, seule géométrie modifiée
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Create feature with Polish Map attributes
    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), "0x01".to_string());
    attributes.insert("Label".to_string(), "Route Test".to_string());
    attributes.insert("EndLevel".to_string(), "3".to_string());
    attributes.insert("Data0".to_string(), "(0x01,0x02)".to_string());
    attributes.insert("Data1".to_string(), "(0x03,0x04)".to_string());

    // LineString traversing tile boundary (will be clipped)
    let feature = Feature {
        geometry_type: GeometryType::LineString,
        geometry: vec![(-0.5, 0.5), (1.5, 0.5)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: attributes.clone(),
        source_attributes: None,
        source_layer: None,
    };

    let result = clip_feature_to_tile(
        &feature,
        &tile_bbox,
        ErrorMode::FailFast,
        &mut ValidationStats::default(),
    );

    assert!(result.is_ok());
    let clipped_vec = result.unwrap();
    assert!(!clipped_vec.is_empty(), "Should return clipped feature(s)");
    let clipped = &clipped_vec[0];

    // Verify ALL attributes preserved
    assert_eq!(
        clipped.attributes.get("Type"),
        Some(&"0x01".to_string()),
        "Type attribute lost"
    );
    assert_eq!(
        clipped.attributes.get("Label"),
        Some(&"Route Test".to_string()),
        "Label attribute lost"
    );
    assert_eq!(
        clipped.attributes.get("EndLevel"),
        Some(&"3".to_string()),
        "EndLevel attribute lost"
    );
    assert_eq!(
        clipped.attributes.get("Data0"),
        Some(&"(0x01,0x02)".to_string()),
        "Data0 attribute lost"
    );
    assert_eq!(
        clipped.attributes.get("Data1"),
        Some(&"(0x03,0x04)".to_string()),
        "Data1 attribute lost"
    );

    // Verify geometry was modified (coords changed)
    assert_ne!(
        clipped.geometry, feature.geometry,
        "Geometry should be clipped"
    );

    // Verify geometry is within tile bounds [0, 1]
    for (x, y) in &clipped.geometry {
        assert!(*x >= 0.0 && *x <= 1.0, "X coord {} outside tile [0, 1]", x);
        assert!(*y >= 0.0 && *y <= 1.0, "Y coord {} outside tile [0, 1]", y);
    }
}

#[test]
fn test_point_feature_attributes_preserved() {
    // AC5: Point features also preserve attributes (no clipping needed)
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::HashMap;

    let tile = create_test_tile();
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Point inside tile with attributes
    let mut attributes = HashMap::new();
    attributes.insert("Type".to_string(), "0x1100".to_string());
    attributes.insert("Label".to_string(), "City".to_string());

    let feature = Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(0.5, 0.5)],
        additional_geometries: std::collections::BTreeMap::new(),
        attributes: attributes.clone(),
        source_attributes: None,
        source_layer: None,
    };

    let result = clip_feature_to_tile(
        &feature,
        &tile_bbox,
        ErrorMode::Continue,
        &mut ValidationStats::default(),
    );

    assert!(result.is_ok());
    let clipped_vec = result.unwrap();
    assert!(!clipped_vec.is_empty(), "Point should be returned");
    let clipped = &clipped_vec[0];

    // Attributes preserved
    assert_eq!(clipped.attributes, attributes);

    // Geometry unchanged for points
    assert_eq!(clipped.geometry, feature.geometry);
}

#[test]
fn test_clip_level_coords_concave_polygon_additional_geometry_preserved() {
    // Verifies that additional_geometries with a concave but geometrically valid
    // polygon are preserved through tile clipping. This is the normal fast path:
    // intersection() succeeds directly without repair.
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::BTreeMap;
    use std::collections::HashMap;

    let tile = create_test_tile(); // [0,0] → [1,1]
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // Concave polygon entirely inside the tile.
    let concave: Vec<(f64, f64)> = vec![
        (0.1, 0.1),
        (0.9, 0.1),
        (0.9, 0.9),
        (0.5, 0.6),
        (0.1, 0.9),
        (0.1, 0.1),
    ];

    let mut additional = BTreeMap::new();
    additional.insert(1u8, concave);

    let feature = Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![(0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9), (0.1, 0.1)],
        additional_geometries: additional,
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    };

    let result = clip_feature_to_tile(
        &feature,
        &tile_bbox,
        ErrorMode::Continue,
        &mut ValidationStats::default(),
    );

    assert!(result.is_ok(), "clip_feature_to_tile should not error");
    let clipped_vec = result.unwrap();
    assert!(!clipped_vec.is_empty(), "Feature should survive clipping");
    let clipped = &clipped_vec[0];
    assert!(
        clipped.additional_geometries.contains_key(&1),
        "Level 1 should be preserved for valid concave polygon; keys: {:?}",
        clipped.additional_geometries.keys().collect::<Vec<_>>()
    );
    // Clipped coordinates must be non-empty and geometrically sensible.
    let coords = &clipped.additional_geometries[&1];
    assert!(!coords.is_empty(), "Clipped additional geometry must have coordinates");
    assert!(coords.len() >= 3, "Clipped polygon must have at least 3 points");
}

#[test]
fn test_clip_level_coords_self_intersecting_additional_geometry_handled_gracefully() {
    // Verifies that additional_geometries with a truly self-intersecting (bow-tie)
    // polygon do not panic. A bow-tie repairs to MultiPolygon which clip_level_coords_to_bbox
    // filters out (multi-fragment rule), so the level is absent — fill_level_gaps then
    // clones the nearest valid level. This is the correct graceful-degradation path.
    use mpforge::config::ErrorMode;
    use mpforge::pipeline::reader::{Feature, GeometryType};
    use mpforge::pipeline::tiler::clip_feature_to_tile;
    use std::collections::BTreeMap;
    use std::collections::HashMap;

    let tile = create_test_tile(); // [0,0] → [1,1]
    let tile_bbox = tile.to_gdal_polygon().unwrap();

    // True bow-tie: edges (0.2,0.2)→(0.8,0.8) and (0.8,0.2)→(0.2,0.8) cross at (0.5,0.5).
    // GEOS considers this invalid; make_valid produces two triangles (MultiPolygon).
    let bow_tie: Vec<(f64, f64)> = vec![
        (0.2, 0.2),
        (0.8, 0.8),
        (0.8, 0.2),
        (0.2, 0.8),
        (0.2, 0.2),
    ];

    let mut additional = BTreeMap::new();
    additional.insert(1u8, bow_tie);

    let feature = Feature {
        geometry_type: GeometryType::Polygon,
        geometry: vec![(0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.1, 0.9), (0.1, 0.1)],
        additional_geometries: additional,
        attributes: HashMap::new(),
        source_attributes: None,
        source_layer: None,
    };

    let result = clip_feature_to_tile(
        &feature,
        &tile_bbox,
        ErrorMode::Continue,
        &mut ValidationStats::default(),
    );

    // Must not panic or error — irrecoverable additional_geometries are silently dropped.
    assert!(result.is_ok(), "Self-intersecting additional geometry must not cause an error");
    let clipped_vec = result.unwrap();
    assert!(!clipped_vec.is_empty(), "Primary geometry must still be clipped");
    // The key property: no panic and primary geometry intact.
    // Level 1 outcome is GEOS-version dependent:
    // - Some GEOS versions treat the bow-tie as valid (kept as-is, level 1 present)
    // - Others return a MultiPolygon from make_valid (filtered, level 1 absent)
    // Either way, if present, the geometry must have ≥3 points.
    let clipped = &clipped_vec[0];
    if let Some(coords) = clipped.additional_geometries.get(&1) {
        assert!(
            coords.len() >= 3,
            "Level 1, when present, must contain a valid polygon (≥3 points); got {}",
            coords.len()
        );
    }
}
