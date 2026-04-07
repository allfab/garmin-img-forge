//! Integration tests for spatial filter geometry building.

use gdal::spatial_ref::SpatialRef;
use gdal::vector::{Geometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
use gdal::DriverManager;
use mpforge::pipeline::reader::SourceReader;
use tempfile::TempDir;

/// Create a test shapefile with 3 square polygons in EPSG:2154.
/// Squares are 1000m x 1000m at positions:
///   - (600000, 6200000) to (601000, 6201000)
///   - (602000, 6200000) to (603000, 6201000)
///   - (604000, 6200000) to (605000, 6201000)
fn create_test_shapefile(dir: &std::path::Path) -> String {
    let shp_path = dir.join("test_communes.shp");
    let path_str = shp_path.to_str().unwrap().to_string();

    let driver = DriverManager::get_driver_by_name("ESRI Shapefile").unwrap();
    let mut dataset = driver.create_vector_only(&path_str).unwrap();
    let srs = SpatialRef::from_epsg(2154).unwrap();
    let mut layer = dataset
        .create_layer(LayerOptions {
            name: "test_communes",
            srs: Some(&srs),
            ty: OGRwkbGeometryType::wkbPolygon,
            ..Default::default()
        })
        .unwrap();

    let squares = [
        (600_000.0, 6_200_000.0, 601_000.0, 6_201_000.0),
        (602_000.0, 6_200_000.0, 603_000.0, 6_201_000.0),
        (604_000.0, 6_200_000.0, 605_000.0, 6_201_000.0),
    ];

    for (min_x, min_y, max_x, max_y) in &squares {
        let wkt = format!(
            "POLYGON (({} {}, {} {}, {} {}, {} {}, {} {}))",
            min_x, min_y,
            max_x, min_y,
            max_x, max_y,
            min_x, max_y,
            min_x, min_y,
        );
        let geom = Geometry::from_wkt(&wkt).unwrap();
        layer.create_feature(geom).unwrap();
    }

    // Force flush
    drop(layer);
    drop(dataset);

    path_str
}

#[test]
fn test_build_spatial_filter_geometry_union_and_buffer() {
    let tmp = TempDir::new().unwrap();
    let shp_path = create_test_shapefile(tmp.path());

    let sf = SourceReader::build_spatial_filter_geometry(&shp_path, 100.0)
        .expect("build_spatial_filter_geometry should succeed");

    // Verify SRS was detected
    assert_eq!(sf.srs.as_deref(), Some("EPSG:2154"), "SRS should be detected as EPSG:2154");

    // Reconstruct geometry from WKB
    let geom = Geometry::from_wkb(&sf.wkb).expect("WKB deserialization should succeed");

    assert!(!geom.is_empty(), "Resulting geometry should not be empty");

    // Verify envelope is stored correctly
    let envelope = geom.envelope();
    assert!((sf.envelope[0] - envelope.MinX).abs() < 1.0, "Stored envelope MinX should match");
    assert!((sf.envelope[2] - envelope.MaxX).abs() < 1.0, "Stored envelope MaxX should match");

    // The union of 3 squares spans x: 600000..605000, y: 6200000..6201000
    // With a 100m buffer, the envelope should be approximately:
    //   x: 599900..605100, y: 6199900..6201100
    assert!(
        envelope.MinX < 600_000.0,
        "Buffer should extend left of first square: MinX={} should be < 600000",
        envelope.MinX
    );
    assert!(
        envelope.MaxX > 605_000.0,
        "Buffer should extend right of last square: MaxX={} should be > 605000",
        envelope.MaxX
    );
    assert!(
        envelope.MinY < 6_200_000.0,
        "Buffer should extend below squares: MinY={} should be < 6200000",
        envelope.MinY
    );
    assert!(
        envelope.MaxY > 6_201_000.0,
        "Buffer should extend above squares: MaxY={} should be > 6201000",
        envelope.MaxY
    );

    // Verify buffer is effective: envelope should be larger than union alone
    // Union envelope: 5000m x 1000m = 5_000_000 m²
    // With 100m buffer, area should be significantly larger
    let geom_area = geom.area();
    let union_area = 3.0 * 1000.0 * 1000.0; // 3 squares of 1km²
    assert!(
        geom_area > union_area,
        "Buffered area ({}) should be larger than union area ({})",
        geom_area,
        union_area
    );
}

#[test]
fn test_build_spatial_filter_geometry_no_buffer() {
    let tmp = TempDir::new().unwrap();
    let shp_path = create_test_shapefile(tmp.path());

    let sf = SourceReader::build_spatial_filter_geometry(&shp_path, 0.0)
        .expect("build_spatial_filter_geometry with 0 buffer should succeed");

    let geom = Geometry::from_wkb(&sf.wkb).expect("WKB deserialization should succeed");
    assert!(!geom.is_empty());

    // Without buffer, area should be close to 3 * 1km² = 3_000_000 m²
    let geom_area = geom.area();
    let expected = 3.0 * 1000.0 * 1000.0;
    let tolerance = expected * 0.01; // 1% tolerance
    assert!(
        (geom_area - expected).abs() < tolerance,
        "Without buffer, area ({}) should be close to {} (tolerance: {})",
        geom_area,
        expected,
        tolerance
    );
}

#[test]
fn test_build_spatial_filter_geometry_invalid_path() {
    let result = SourceReader::build_spatial_filter_geometry("/nonexistent/path.shp", 100.0);
    assert!(result.is_err(), "Should fail for nonexistent shapefile");
}
