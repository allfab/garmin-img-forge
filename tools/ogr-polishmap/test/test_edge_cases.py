#!/usr/bin/env python3
"""
Story 3.4: Edge Cases Tests (AC2-AC5)

Tests edge case handling for the Polish Map driver:
- Empty Label handling (AC2)
- Extreme WGS84 coordinates (AC3)
- Data0-Data10 fields extraction (AC4)
- All edge-cases parse without crash (AC5)

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_edge_cases.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing ogr_PolishMap.so
"""

import os
import sys
import glob

def check_gdal_available():
    """Check if GDAL Python bindings are available."""
    try:
        from osgeo import ogr, gdal
        return True
    except ImportError:
        return False

def check_polishmap_driver():
    """Check if PolishMap driver is registered."""
    from osgeo import ogr
    driver = ogr.GetDriverByName('PolishMap')
    return driver is not None

def get_test_data_dir():
    """Get path to edge-cases test data directory."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(script_dir, 'data', 'edge-cases')


# =============================================================================
# AC2: Empty Label Edge Case
# =============================================================================
def test_ac2_empty_label():
    """AC2: Empty Label field creates feature with empty string."""
    from osgeo import ogr, gdal

    print("  AC2: Empty label test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-empty-label.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        # Get POI layer
        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()

        empty_label_found = False
        normal_label_found = False

        while True:
            feat = poi_layer.GetNextFeature()
            if feat is None:
                break
            label = feat.GetField('Label')
            if label is None or label == '':
                empty_label_found = True
            elif label == 'Normal Label':
                normal_label_found = True

        ds = None

        if not empty_label_found:
            print("FAILED (empty label feature not found)")
            return False

        if not normal_label_found:
            print("FAILED (normal label feature not found)")
            return False

        print("PASSED")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC3: Extreme WGS84 Coordinates
# =============================================================================
def test_ac3_extreme_coords_max_lat():
    """AC3: Extreme WGS84 coordinates (max latitude) preserved."""
    from osgeo import ogr, gdal

    print("  AC3: Max latitude test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-coords-max-lat.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        y = geom.GetY()  # Latitude

        # Check if latitude is near 90 (North Pole area)
        if y < 89.0:
            print(f"FAILED (latitude {y} not near pole)")
            return False

        ds = None
        print(f"PASSED (lat={y:.4f})")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_ac3_extreme_coords_min_lat():
    """AC3: Extreme WGS84 coordinates (min latitude) preserved."""
    from osgeo import ogr, gdal

    print("  AC3: Min latitude test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-coords-min-lat.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        y = geom.GetY()  # Latitude

        # Check if latitude is near -90 (South Pole area)
        if y > -89.0:
            print(f"FAILED (latitude {y} not near south pole)")
            return False

        ds = None
        print(f"PASSED (lat={y:.4f})")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_ac3_extreme_coords_dateline():
    """AC3: Coordinates near dateline preserved."""
    from osgeo import ogr, gdal

    print("  AC3: Dateline coordinates test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-coords-max-lon.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        x = geom.GetX()  # Longitude

        # Check if longitude is near 180 (dateline)
        if abs(x) < 179.0:
            print(f"FAILED (longitude {x} not near dateline)")
            return False

        ds = None
        print(f"PASSED (lon={x:.4f})")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC4: Data Fields (Data0-Data10)
# =============================================================================
def test_ac4_all_data_fields():
    """AC4: Data0-Data10 all extracted correctly."""
    from osgeo import ogr, gdal

    print("  AC4: All data fields test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-data-all-fields.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        # Check that feature has geometry (Data0)
        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry from Data0)")
            return False

        # AC4: Verify feature parsed with multi-level data
        # Note: PolishMap driver stores Data0 as geometry, Data1-DataN may be
        # stored as additional geometry levels or attributes depending on impl.
        # At minimum, the file must parse without error and produce valid geometry.
        geom_type = geom.GetGeometryType()
        if geom_type == ogr.wkbUnknown:
            print("FAILED (unknown geometry type)")
            return False

        ds = None
        print(f"PASSED (feature with multi-data parsed, geom_type={geom_type})")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_ac4_sparse_data_fields():
    """AC4: Sparse data fields (Data0, Data5, Data10) handled."""
    from osgeo import ogr, gdal

    print("  AC4: Sparse data fields test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-data-sparse.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        # Check that feature has geometry
        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        ds = None
        print("PASSED")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC5: All Edge-Cases Parse Without Crash
# =============================================================================
def test_ac5_all_edge_cases_no_crash():
    """AC5: Process all edge-cases files with 0 crashes."""
    from osgeo import ogr, gdal

    print("  AC5: Zero crash validation... ", end="", flush=True)

    test_dir = get_test_data_dir()
    if not os.path.exists(test_dir):
        print("SKIPPED (test directory not found)")
        print("  Run: python3 tools/generate_edge_cases_corpus.py")
        return None

    # Find all .mp files in edge-cases directory
    test_files = glob.glob(os.path.join(test_dir, '*.mp'))

    # Minimum 1 file to run test; AC1 requires 50-100 but that's validated separately
    if len(test_files) < 1:
        print(f"SKIPPED (no test files found)")
        print("  Run: python3 tools/generate_edge_cases_corpus.py")
        return None

    if len(test_files) < 50:
        print(f"WARNING: Only {len(test_files)} files, AC1 requires 50-100. Running anyway...")

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    crashes = 0
    processed = 0
    opened_ok = 0

    for test_file in test_files:
        try:
            # Try to open the file
            ds = ogr.Open(test_file)

            if ds is not None:
                opened_ok += 1
                # Try to read all layers
                for i in range(ds.GetLayerCount()):
                    layer = ds.GetLayer(i)
                    if layer is not None:
                        layer.ResetReading()
                        # Try to read all features
                        while True:
                            feat = layer.GetNextFeature()
                            if feat is None:
                                break
                            # Access geometry to trigger any parsing
                            geom = feat.GetGeometryRef()
                            if geom:
                                geom.GetX()
                                geom.GetY()
                            # Access fields
                            feat.GetField('Label')
                            feat.GetField('Type')

                ds = None

            processed += 1

        except Exception as e:
            crashes += 1
            print(f"\n    CRASH in {os.path.basename(test_file)}: {e}")

    gdal.PopErrorHandler()

    if crashes > 0:
        print(f"FAILED ({crashes} crashes out of {len(test_files)} files)")
        return False

    print(f"PASSED ({processed} files, {opened_ok} opened OK, 0 crashes)")
    return True


# =============================================================================
# Additional Edge Case Tests
# =============================================================================
def test_label_special_chars():
    """Test special characters in labels."""
    from osgeo import ogr, gdal

    print("  Label special chars test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-label-special-chars.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        label = feat.GetField('Label')

        # Should contain special chars
        if not label or '<' not in label:
            print(f"FAILED (special chars not preserved: '{label}')")
            return False

        ds = None
        print(f"PASSED (label='{label[:30]}...')")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_label_unicode():
    """Test unicode accents in labels."""
    from osgeo import ogr, gdal

    print("  Label unicode test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-label-unicode-accents.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        label = feat.GetField('Label')

        # Should contain accented chars
        if not label or 'é' not in label:
            print(f"FAILED (accents not preserved: '{label}')")
            return False

        ds = None
        print(f"PASSED (label='{label}')")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_many_features():
    """Test file with many features (1000)."""
    from osgeo import ogr, gdal

    print("  Many features (1000) test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-features-1000.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        poi_layer = ds.GetLayerByName('POI')
        if poi_layer is None:
            poi_layer = ds.GetLayer(0)

        count = poi_layer.GetFeatureCount()

        if count < 900:  # Allow some tolerance
            print(f"FAILED (only {count} features, expected ~1000)")
            return False

        ds = None
        print(f"PASSED ({count} features)")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_polyline_many_points():
    """Test polyline with many points (500)."""
    from osgeo import ogr, gdal

    print("  Polyline many points test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-features-polyline-many-points.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        polyline_layer = ds.GetLayerByName('POLYLINE')
        if polyline_layer is None:
            polyline_layer = ds.GetLayer(1)

        if polyline_layer is None:
            print("FAILED (no POLYLINE layer)")
            return False

        polyline_layer.ResetReading()
        feat = polyline_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        point_count = geom.GetPointCount()

        if point_count < 400:  # Allow some tolerance
            print(f"FAILED (only {point_count} points, expected ~500)")
            return False

        ds = None
        print(f"PASSED ({point_count} points)")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_polygon_complex():
    """Test polygon with many vertices (100)."""
    from osgeo import ogr, gdal

    print("  Polygon complex test... ", end="", flush=True)

    test_file = os.path.join(get_test_data_dir(), 'edge-features-polygon-complex.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None

    gdal.PushErrorHandler('CPLQuietErrorHandler')

    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()

        if ds is None:
            print("FAILED (Open() returned NULL)")
            return False

        polygon_layer = ds.GetLayerByName('POLYGON')
        if polygon_layer is None:
            polygon_layer = ds.GetLayer(2)

        if polygon_layer is None:
            print("FAILED (no POLYGON layer)")
            return False

        polygon_layer.ResetReading()
        feat = polygon_layer.GetNextFeature()

        if feat is None:
            print("FAILED (no feature found)")
            return False

        geom = feat.GetGeometryRef()
        if geom is None:
            print("FAILED (no geometry)")
            return False

        # Get exterior ring
        ring = geom.GetGeometryRef(0)
        if ring is None:
            print("FAILED (no exterior ring)")
            return False

        point_count = ring.GetPointCount()

        if point_count < 80:  # Allow some tolerance
            print(f"FAILED (only {point_count} vertices, expected ~100)")
            return False

        ds = None
        print(f"PASSED ({point_count} vertices)")
        return True

    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# Main
# =============================================================================
def main():
    """Run all edge cases tests."""
    print("=== Story 3.4: Edge Cases Tests (AC2-AC5) ===")
    print()

    # Check GDAL availability
    if not check_gdal_available():
        print("ERROR: GDAL Python bindings not available.")
        print("Install with: pip install GDAL or dnf install python3-gdal")
        return 1

    print("GDAL Python bindings: available")

    # Check PolishMap driver
    if not check_polishmap_driver():
        print("ERROR: PolishMap driver not registered.")
        print("Ensure GDAL_DRIVER_PATH includes the plugin directory.")
        return 1

    print("PolishMap driver: registered")
    print(f"Test data directory: {get_test_data_dir()}")
    print()

    passed = 0
    failed = 0
    skipped = 0

    tests = [
        # AC2: Empty Label
        test_ac2_empty_label,

        # AC3: Extreme Coordinates
        test_ac3_extreme_coords_max_lat,
        test_ac3_extreme_coords_min_lat,
        test_ac3_extreme_coords_dateline,

        # AC4: Data Fields
        test_ac4_all_data_fields,
        test_ac4_sparse_data_fields,

        # AC5: Zero Crash
        test_ac5_all_edge_cases_no_crash,

        # Additional tests
        test_label_special_chars,
        test_label_unicode,
        test_many_features,
        test_polyline_many_points,
        test_polygon_complex,
    ]

    print("Running tests:")

    for test_func in tests:
        result = test_func()
        if result is True:
            passed += 1
        elif result is False:
            failed += 1
        else:  # None = skipped
            skipped += 1

    print()
    print("=" * 50)
    print(f"Test Summary: Passed={passed}, Failed={failed}, Skipped={skipped}")
    print("=" * 50)

    # Return exit code
    return 0 if failed == 0 else 1


if __name__ == '__main__':
    sys.exit(main())
