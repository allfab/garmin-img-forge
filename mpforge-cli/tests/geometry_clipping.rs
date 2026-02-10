//! Unit tests for geometry clipping operations (Story 6.3).
//!
//! Tests verify GDAL Intersection-based clipping of features to tile boundaries,
//! attribute preservation, and error handling modes.

use mpforge_cli::pipeline::tiler::TileBounds;

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
    assert_eq!(polygon.geometry_type(), gdal::vector::OGRwkbGeometryType::wkbPolygon);
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

    assert!((envelope.MinX - 10.0).abs() < 1e-9);
    assert!((envelope.MinY - 20.0).abs() < 1e-9);
    assert!((envelope.MaxX - 10.15).abs() < 1e-9);
    assert!((envelope.MaxY - 20.15).abs() < 1e-9);
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
    assert!((envelope.MinX - 0.0).abs() < 1e-9);
    assert!((envelope.MaxX - 1.0).abs() < 1e-9);
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
    assert!((envelope.MinX - 0.0).abs() < 1e-6);
    assert!((envelope.MinY - 0.0).abs() < 1e-6);
    assert!((envelope.MaxX - 1.0).abs() < 1e-6);
    assert!((envelope.MaxY - 1.0).abs() < 1e-6);
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
