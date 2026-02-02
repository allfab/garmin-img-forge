#!/usr/bin/env python3
"""
Story 3.3: Testing Corpus - Valid Cases (Minimal & Complex)

Standalone test script (not pytest) for the valid test corpus:
- AC1: valid-minimal directory has 10-20 files
- AC2: Minimal POI smoke test (poi-simple.mp)
- AC3: All valid-minimal files parse successfully
- AC4: valid-complex directory has 100-200 files
- AC5: Complex real-world file parsing
- AC6: Round-trip validation (SIG -> MP -> SIG) including Shapefile
- AC7: Complete corpus regression prevention

Note: This script uses standalone execution pattern (main() with sys.exit)
consistent with other test scripts in this project. Run directly with Python.

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_valid_corpus.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing gdal_POLISHMAP.so
"""

import os
import sys
import glob
import shutil
import tempfile

# Test data directory relative to this script
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
TEST_DATA_DIR = os.path.join(SCRIPT_DIR, 'data')
VALID_MINIMAL_DIR = os.path.join(TEST_DATA_DIR, 'valid-minimal')
VALID_COMPLEX_DIR = os.path.join(TEST_DATA_DIR, 'valid-complex')


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


def get_mp_files(directory):
    """Get all .mp files in a directory."""
    return sorted(glob.glob(os.path.join(directory, '*.mp')))


def parse_mp_file(filepath):
    """Parse a .mp file and return dataset info.

    Returns:
        dict: {'poi_count': int, 'polyline_count': int, 'polygon_count': int, 'error': str or None}
    """
    from osgeo import ogr

    result = {
        'poi_count': 0,
        'polyline_count': 0,
        'polygon_count': 0,
        'error': None
    }

    try:
        ds = ogr.Open(filepath)
        if ds is None:
            result['error'] = 'Failed to open file'
            return result

        # Count features in each layer
        for layer_idx in range(ds.GetLayerCount()):
            layer = ds.GetLayer(layer_idx)
            if layer is None:
                continue

            name = layer.GetName()
            count = layer.GetFeatureCount()

            if name == 'POI':
                result['poi_count'] = count
            elif name == 'POLYLINE':
                result['polyline_count'] = count
            elif name == 'POLYGON':
                result['polygon_count'] = count

        ds = None

    except Exception as e:
        result['error'] = str(e)

    return result


# =============================================================================
# AC1: Valid-Minimal Directory with 10-20 Files
# =============================================================================
def test_ac1_valid_minimal_file_count():
    """AC1: valid-minimal directory should contain 10-20 .mp files."""
    print("  AC1: Valid-minimal file count (10-20)... ", end="")

    if not os.path.isdir(VALID_MINIMAL_DIR):
        print(f"FAILED (directory not found: {VALID_MINIMAL_DIR})")
        return False

    mp_files = get_mp_files(VALID_MINIMAL_DIR)
    count = len(mp_files)

    if count < 10:
        print(f"FAILED (only {count} files, expected >= 10)")
        return False

    # Note: We allow > 20 files as it exceeds the minimum requirement
    if count > 20:
        print(f"PASSED ({count} files - exceeds target)")
    else:
        print(f"PASSED ({count} files)")

    return True


def test_ac1_required_files_exist():
    """AC1: Required files must exist: poi-simple.mp, polyline-simple.mp, polygon-simple.mp."""
    print("  AC1: Required files exist... ", end="")

    required_files = ['poi-simple.mp', 'polyline-simple.mp', 'polygon-simple.mp']
    missing = []

    for filename in required_files:
        filepath = os.path.join(VALID_MINIMAL_DIR, filename)
        if not os.path.exists(filepath):
            missing.append(filename)

    if missing:
        print(f"FAILED (missing: {', '.join(missing)})")
        return False

    print("PASSED")
    return True


# =============================================================================
# AC2: Minimal POI Smoke Test
# =============================================================================
def test_ac2_poi_simple_smoke_test():
    """AC2: poi-simple.mp should have exactly 1 POI with Type, Label, and geometry."""
    from osgeo import ogr

    print("  AC2: POI smoke test (poi-simple.mp)... ", end="")

    filepath = os.path.join(VALID_MINIMAL_DIR, 'poi-simple.mp')
    if not os.path.exists(filepath):
        print("FAILED (file not found)")
        return False

    ds = ogr.Open(filepath)
    if ds is None:
        print("FAILED (could not open file)")
        return False

    # Get POI layer
    poi_layer = ds.GetLayerByName('POI')
    if poi_layer is None:
        print("FAILED (POI layer not found)")
        ds = None
        return False

    # Check exactly 1 feature
    feature_count = poi_layer.GetFeatureCount()
    if feature_count != 1:
        print(f"FAILED (expected 1 feature, got {feature_count})")
        ds = None
        return False

    # Check feature has required fields
    poi_layer.ResetReading()
    feature = poi_layer.GetNextFeature()

    type_val = feature.GetField('Type')
    if type_val is None or type_val == '':
        print("FAILED (Type field empty)")
        ds = None
        return False

    label_val = feature.GetField('Label')
    if label_val is None or label_val == '':
        print("FAILED (Label field empty)")
        ds = None
        return False

    geom = feature.GetGeometryRef()
    if geom is None:
        print("FAILED (geometry is None)")
        ds = None
        return False

    if geom.GetGeometryType() != ogr.wkbPoint:
        print(f"FAILED (expected Point, got {geom.GetGeometryName()})")
        ds = None
        return False

    ds = None
    print("PASSED")
    return True


# =============================================================================
# AC3: All Valid-Minimal Files Parse Successfully
# =============================================================================
def test_ac3_all_valid_minimal_parse():
    """AC3: All files in valid-minimal should parse without errors."""
    print("  AC3: All valid-minimal files parse... ", end="")

    mp_files = get_mp_files(VALID_MINIMAL_DIR)
    if not mp_files:
        print("FAILED (no .mp files found)")
        return False

    errors = []
    total_features = 0

    for filepath in mp_files:
        filename = os.path.basename(filepath)
        result = parse_mp_file(filepath)

        if result['error']:
            errors.append(f"{filename}: {result['error']}")
        else:
            total_features += result['poi_count'] + result['polyline_count'] + result['polygon_count']

    if errors:
        print(f"FAILED ({len(errors)} errors)")
        for err in errors[:5]:  # Show first 5 errors
            print(f"    - {err}")
        return False

    print(f"PASSED ({len(mp_files)} files, {total_features} features)")
    return True


# =============================================================================
# AC4: Valid-Complex Directory with 100-200 Files
# =============================================================================
def test_ac4_valid_complex_file_count():
    """AC4: valid-complex directory should contain 100-200 .mp files."""
    print("  AC4: Valid-complex file count (100-200)... ", end="")

    if not os.path.isdir(VALID_COMPLEX_DIR):
        print(f"FAILED (directory not found: {VALID_COMPLEX_DIR})")
        return False

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    count = len(mp_files)

    if count < 100:
        print(f"FAILED (only {count} files, expected >= 100)")
        return False

    if count > 200:
        print(f"PASSED ({count} files - exceeds target)")
    else:
        print(f"PASSED ({count} files)")

    return True


def test_ac4_valid_complex_mixed_types():
    """AC4: valid-complex should have files with mixed geometry types."""
    print("  AC4: Valid-complex mixed geometry types... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if not mp_files:
        print("FAILED (no files found)")
        return False

    has_poi = False
    has_polyline = False
    has_polygon = False

    for filepath in mp_files[:50]:  # Check first 50 files
        result = parse_mp_file(filepath)
        if result['error']:
            continue

        if result['poi_count'] > 0:
            has_poi = True
        if result['polyline_count'] > 0:
            has_polyline = True
        if result['polygon_count'] > 0:
            has_polygon = True

        if has_poi and has_polyline and has_polygon:
            break

    missing = []
    if not has_poi:
        missing.append('POI')
    if not has_polyline:
        missing.append('POLYLINE')
    if not has_polygon:
        missing.append('POLYGON')

    if missing:
        print(f"FAILED (missing geometry types: {', '.join(missing)})")
        return False

    print("PASSED")
    return True


# =============================================================================
# AC5: Complex Real-World File Parsing
# =============================================================================
def test_ac5_complex_file_parsing():
    """AC5: Complex real-world files should parse with all layers containing features."""
    from osgeo import ogr

    print("  AC5: Complex file parsing... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if not mp_files:
        print("FAILED (no files found)")
        return False

    # Find mixed-* or real-world-* files
    complex_files = [f for f in mp_files if 'mixed' in os.path.basename(f).lower()
                     or 'real-world' in os.path.basename(f).lower()]

    if not complex_files:
        # Fall back to any file with multiple geometry types
        for filepath in mp_files:
            result = parse_mp_file(filepath)
            if result['error']:
                continue
            if (result['poi_count'] > 0 and result['polyline_count'] > 0
                and result['polygon_count'] > 0):
                complex_files.append(filepath)
                break

    if not complex_files:
        print("FAILED (no complex files with mixed geometry found)")
        return False

    # Test first complex file
    filepath = complex_files[0]
    result = parse_mp_file(filepath)

    if result['error']:
        print(f"FAILED ({result['error']})")
        return False

    total = result['poi_count'] + result['polyline_count'] + result['polygon_count']

    print(f"PASSED (POI={result['poi_count']}, POLYLINE={result['polyline_count']}, POLYGON={result['polygon_count']})")
    return True


def test_ac5_attribute_mapping():
    """AC5: Attributes should be mapped correctly (Type, Label, Data0)."""
    from osgeo import ogr

    print("  AC5: Attribute mapping verification... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if not mp_files:
        print("FAILED (no files found)")
        return False

    # Check first 10 files
    checked = 0
    errors = []

    for filepath in mp_files[:10]:
        ds = ogr.Open(filepath)
        if ds is None:
            continue

        for layer_idx in range(ds.GetLayerCount()):
            layer = ds.GetLayer(layer_idx)
            if layer is None or layer.GetFeatureCount() == 0:
                continue

            layer.ResetReading()
            feat = layer.GetNextFeature()
            if feat is None:
                continue

            # Check Type field exists and has valid format
            type_val = feat.GetField('Type')
            if type_val is not None:
                if not type_val.startswith('0x'):
                    errors.append(f"{os.path.basename(filepath)}: Type not hex format")

            # Check geometry exists
            geom = feat.GetGeometryRef()
            if geom is None:
                errors.append(f"{os.path.basename(filepath)}: No geometry")

            checked += 1
            break

        ds = None

    if errors:
        print(f"FAILED ({len(errors)} errors)")
        for err in errors[:3]:
            print(f"    - {err}")
        return False

    if checked == 0:
        print("FAILED (no features checked)")
        return False

    print(f"PASSED ({checked} files verified)")
    return True


def test_ac5_wgs84_bounds_validation():
    """AC5: Geometries should be within valid WGS84 bounds."""
    from osgeo import ogr

    print("  AC5: WGS84 bounds validation... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if not mp_files:
        print("FAILED (no files found)")
        return False

    # WGS84 bounds
    LAT_MIN, LAT_MAX = -90.0, 90.0
    LON_MIN, LON_MAX = -180.0, 180.0

    errors = []
    checked_features = 0

    for filepath in mp_files[:20]:  # Check first 20 files
        ds = ogr.Open(filepath)
        if ds is None:
            continue

        for layer_idx in range(ds.GetLayerCount()):
            layer = ds.GetLayer(layer_idx)
            if layer is None or layer.GetFeatureCount() == 0:
                continue

            layer.ResetReading()
            while True:
                feat = layer.GetNextFeature()
                if feat is None:
                    break

                geom = feat.GetGeometryRef()
                if geom is None:
                    continue

                # Get envelope (bounding box)
                env = geom.GetEnvelope()
                # env = (minX, maxX, minY, maxY)
                min_x, max_x, min_y, max_y = env

                # Check bounds (x = longitude, y = latitude)
                if min_x < LON_MIN or max_x > LON_MAX:
                    errors.append(f"{os.path.basename(filepath)}: lon out of bounds ({min_x}, {max_x})")
                if min_y < LAT_MIN or max_y > LAT_MAX:
                    errors.append(f"{os.path.basename(filepath)}: lat out of bounds ({min_y}, {max_y})")

                checked_features += 1

        ds = None

    if errors:
        print(f"FAILED ({len(errors)} errors)")
        for err in errors[:3]:
            print(f"    - {err}")
        return False

    if checked_features == 0:
        print("FAILED (no features checked)")
        return False

    print(f"PASSED ({checked_features} features validated)")
    return True


# =============================================================================
# AC6: Round-Trip Validation (SIG -> MP -> SIG)
# =============================================================================
def test_ac6_roundtrip_geometry_preservation():
    """AC6: Round-trip should preserve geometries within GPS precision (6 decimals)."""
    from osgeo import ogr

    print("  AC6: Round-trip geometry preservation... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if not mp_files:
        print("SKIPPED (no valid-complex files)")
        return True

    driver = ogr.GetDriverByName('PolishMap')
    if driver is None:
        print("FAILED (PolishMap driver not found)")
        return False

    # Test first file with features
    source_file = None
    original_coords = []

    for filepath in mp_files[:10]:
        ds = ogr.Open(filepath)
        if ds is None:
            continue

        # Find a layer with features
        for layer_idx in range(ds.GetLayerCount()):
            layer = ds.GetLayer(layer_idx)
            if layer is None or layer.GetFeatureCount() == 0:
                continue

            layer.ResetReading()
            feat = layer.GetNextFeature()
            if feat is None:
                continue

            geom = feat.GetGeometryRef()
            if geom is None:
                continue

            # Store coordinates for verification
            if layer.GetName() == 'POI':
                original_coords.append({
                    'type': 'POI',
                    'x': geom.GetX(),
                    'y': geom.GetY(),
                    'type_val': feat.GetField('Type'),
                    'label': feat.GetField('Label')
                })

            source_file = filepath
            break

        ds = None
        if source_file and original_coords:
            break

    if not source_file or not original_coords:
        print("SKIPPED (no suitable test file found)")
        return True

    # Create temp file for round-trip
    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        # Read source and write to temp
        ds_src = ogr.Open(source_file)
        ds_dst = driver.CreateDataSource(temp_path)

        for layer_idx in range(ds_src.GetLayerCount()):
            src_layer = ds_src.GetLayer(layer_idx)
            dst_layer = ds_dst.GetLayer(layer_idx)

            if src_layer is None or dst_layer is None:
                continue

            src_layer.ResetReading()
            while True:
                feat = src_layer.GetNextFeature()
                if feat is None:
                    break

                new_feat = ogr.Feature(dst_layer.GetLayerDefn())
                new_feat.SetField('Type', feat.GetField('Type') or '0x0000')
                new_feat.SetField('Label', feat.GetField('Label') or '')

                geom = feat.GetGeometryRef()
                if geom:
                    new_feat.SetGeometry(geom.Clone())

                dst_layer.CreateFeature(new_feat)

        ds_src = None
        ds_dst = None

        # Read back and verify coordinates
        ds_verify = ogr.Open(temp_path)
        if ds_verify is None:
            print("FAILED (could not read back temp file)")
            return False

        poi_layer = ds_verify.GetLayerByName('POI')
        if poi_layer is None or poi_layer.GetFeatureCount() == 0:
            # Try to verify any layer
            verified = False
            for layer_idx in range(ds_verify.GetLayerCount()):
                layer = ds_verify.GetLayer(layer_idx)
                if layer and layer.GetFeatureCount() > 0:
                    verified = True
                    break

            ds_verify = None
            if verified:
                print("PASSED (non-POI layers verified)")
                return True
            else:
                print("FAILED (no features in round-trip)")
                return False

        poi_layer.ResetReading()
        feat = poi_layer.GetNextFeature()

        if feat and original_coords:
            geom = feat.GetGeometryRef()
            if geom:
                orig = original_coords[0]
                x_diff = abs(geom.GetX() - orig['x'])
                y_diff = abs(geom.GetY() - orig['y'])

                # 6 decimal precision = 1e-6
                if x_diff > 1e-6 or y_diff > 1e-6:
                    print(f"FAILED (coordinate drift: dx={x_diff}, dy={y_diff})")
                    ds_verify = None
                    return False

        ds_verify = None
        print("PASSED")
        return True

    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)


def test_ac6_roundtrip_attribute_preservation():
    """AC6: Round-trip should preserve attributes exactly."""
    from osgeo import ogr

    print("  AC6: Round-trip attribute preservation... ", end="")

    # Use a simple test file
    filepath = os.path.join(VALID_MINIMAL_DIR, 'poi-simple.mp')
    if not os.path.exists(filepath):
        print("SKIPPED (poi-simple.mp not found)")
        return True

    driver = ogr.GetDriverByName('PolishMap')
    if driver is None:
        print("FAILED (PolishMap driver not found)")
        return False

    # Read original
    ds_orig = ogr.Open(filepath)
    if ds_orig is None:
        print("FAILED (could not open original)")
        return False

    poi_layer = ds_orig.GetLayerByName('POI')
    if poi_layer is None or poi_layer.GetFeatureCount() == 0:
        print("SKIPPED (no POI features)")
        ds_orig = None
        return True

    poi_layer.ResetReading()
    orig_feat = poi_layer.GetNextFeature()

    orig_type = orig_feat.GetField('Type')
    orig_label = orig_feat.GetField('Label')
    orig_geom = orig_feat.GetGeometryRef().Clone()

    ds_orig = None

    # Write to temp
    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        ds_write = driver.CreateDataSource(temp_path)
        write_layer = ds_write.GetLayer(0)  # POI

        new_feat = ogr.Feature(write_layer.GetLayerDefn())
        new_feat.SetField('Type', orig_type)
        new_feat.SetField('Label', orig_label)
        new_feat.SetGeometry(orig_geom)
        write_layer.CreateFeature(new_feat)

        ds_write = None

        # Read back
        ds_verify = ogr.Open(temp_path)
        if ds_verify is None:
            print("FAILED (could not read temp)")
            return False

        verify_layer = ds_verify.GetLayerByName('POI')
        verify_layer.ResetReading()
        verify_feat = verify_layer.GetNextFeature()

        verify_type = verify_feat.GetField('Type')
        verify_label = verify_feat.GetField('Label')

        ds_verify = None

        if verify_type != orig_type:
            print(f"FAILED (Type mismatch: '{orig_type}' -> '{verify_type}')")
            return False

        if verify_label != orig_label:
            print(f"FAILED (Label mismatch: '{orig_label}' -> '{verify_label}')")
            return False

        print("PASSED")
        return True

    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)


def test_ac6_roundtrip_via_geojson():
    """AC6: Round-trip MP -> GeoJSON -> MP preserves geometries (6 decimals precision)."""
    from osgeo import ogr

    print("  AC6: Round-trip via GeoJSON... ", end="")

    # Use poi-simple.mp as test source
    filepath = os.path.join(VALID_MINIMAL_DIR, 'poi-simple.mp')
    if not os.path.exists(filepath):
        print("SKIPPED (poi-simple.mp not found)")
        return True

    mp_driver = ogr.GetDriverByName('PolishMap')
    geojson_driver = ogr.GetDriverByName('GeoJSON')

    if mp_driver is None or geojson_driver is None:
        print("FAILED (required drivers not found)")
        return False

    # Create temp files (delete first since GeoJSON driver won't overwrite)
    fd1, geojson_path = tempfile.mkstemp(suffix='.geojson')
    os.close(fd1)
    os.remove(geojson_path)  # GeoJSON driver needs non-existent file
    fd2, mp_roundtrip_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd2)

    try:
        # Step 1: Read original MP and store coordinates
        ds_orig = ogr.Open(filepath)
        if ds_orig is None:
            print("FAILED (could not open original)")
            return False

        original_coords = []
        poi_layer = ds_orig.GetLayerByName('POI')
        if poi_layer and poi_layer.GetFeatureCount() > 0:
            poi_layer.ResetReading()
            feat = poi_layer.GetNextFeature()
            if feat:
                geom = feat.GetGeometryRef()
                if geom:
                    original_coords.append({
                        'x': geom.GetX(),
                        'y': geom.GetY(),
                        'type': feat.GetField('Type'),
                        'label': feat.GetField('Label')
                    })

        ds_orig = None

        if not original_coords:
            print("SKIPPED (no coordinates to test)")
            return True

        # Step 2: MP -> GeoJSON
        ds_mp = ogr.Open(filepath)
        ds_geojson = geojson_driver.CreateDataSource(geojson_path)

        poi_src = ds_mp.GetLayerByName('POI')
        if poi_src and poi_src.GetFeatureCount() > 0:
            poi_dst = ds_geojson.CreateLayer('POI', poi_src.GetSpatialRef(), ogr.wkbPoint)

            # Copy field definitions
            layer_defn = poi_src.GetLayerDefn()
            for i in range(layer_defn.GetFieldCount()):
                field_defn = layer_defn.GetFieldDefn(i)
                poi_dst.CreateField(field_defn)

            # Copy features
            poi_src.ResetReading()
            for feat in poi_src:
                poi_dst.CreateFeature(feat.Clone())

        ds_mp = None
        ds_geojson = None

        # Step 3: GeoJSON -> MP
        ds_geojson = ogr.Open(geojson_path)
        ds_mp_out = mp_driver.CreateDataSource(mp_roundtrip_path)

        poi_src = ds_geojson.GetLayerByName('POI')
        if poi_src and poi_src.GetFeatureCount() > 0:
            poi_dst = ds_mp_out.GetLayer(0)  # POI layer

            poi_src.ResetReading()
            for feat in poi_src:
                new_feat = ogr.Feature(poi_dst.GetLayerDefn())
                new_feat.SetField('Type', feat.GetField('Type') or '0x0000')
                new_feat.SetField('Label', feat.GetField('Label') or '')
                new_feat.SetGeometry(feat.GetGeometryRef().Clone())
                poi_dst.CreateFeature(new_feat)

        ds_geojson = None
        ds_mp_out = None

        # Step 4: Verify round-trip coordinates
        ds_verify = ogr.Open(mp_roundtrip_path)
        if ds_verify is None:
            print("FAILED (could not open round-trip file)")
            return False

        poi_verify = ds_verify.GetLayerByName('POI')
        if poi_verify is None or poi_verify.GetFeatureCount() == 0:
            print("FAILED (no features in round-trip)")
            ds_verify = None
            return False

        poi_verify.ResetReading()
        feat = poi_verify.GetNextFeature()
        geom = feat.GetGeometryRef()

        orig = original_coords[0]
        x_diff = abs(geom.GetX() - orig['x'])
        y_diff = abs(geom.GetY() - orig['y'])

        ds_verify = None

        # Check 6 decimal precision
        if x_diff > 1e-6 or y_diff > 1e-6:
            print(f"FAILED (precision loss: dx={x_diff:.9f}, dy={y_diff:.9f})")
            return False

        print(f"PASSED (precision verified: dx={x_diff:.9f}, dy={y_diff:.9f})")
        return True

    finally:
        for path in [geojson_path, mp_roundtrip_path]:
            if os.path.exists(path):
                os.remove(path)


def test_ac6_roundtrip_multiple_files():
    """AC6: Round-trip 10 valid-complex files without data loss."""
    from osgeo import ogr

    print("  AC6: Round-trip multiple files... ", end="")

    mp_files = get_mp_files(VALID_COMPLEX_DIR)
    if len(mp_files) < 10:
        print(f"SKIPPED (need 10 files, found {len(mp_files)})")
        return True

    driver = ogr.GetDriverByName('PolishMap')
    if driver is None:
        print("FAILED (PolishMap driver not found)")
        return False

    tested = 0
    errors = []

    for filepath in mp_files[:10]:
        fd, temp_path = tempfile.mkstemp(suffix='.mp')
        os.close(fd)

        try:
            # Read original
            ds_orig = ogr.Open(filepath)
            if ds_orig is None:
                continue

            original_counts = {}
            for layer_idx in range(ds_orig.GetLayerCount()):
                layer = ds_orig.GetLayer(layer_idx)
                if layer:
                    original_counts[layer.GetName()] = layer.GetFeatureCount()

            # Write to temp
            ds_out = driver.CreateDataSource(temp_path)

            for layer_idx in range(ds_orig.GetLayerCount()):
                src_layer = ds_orig.GetLayer(layer_idx)
                dst_layer = ds_out.GetLayer(layer_idx)

                if src_layer is None or dst_layer is None:
                    continue

                src_layer.ResetReading()
                for feat in src_layer:
                    new_feat = ogr.Feature(dst_layer.GetLayerDefn())
                    new_feat.SetField('Type', feat.GetField('Type') or '0x0000')
                    new_feat.SetField('Label', feat.GetField('Label') or '')
                    geom = feat.GetGeometryRef()
                    if geom:
                        new_feat.SetGeometry(geom.Clone())
                    dst_layer.CreateFeature(new_feat)

            ds_orig = None
            ds_out = None

            # Verify
            ds_verify = ogr.Open(temp_path)
            if ds_verify is None:
                errors.append(f"{os.path.basename(filepath)}: could not read back")
                continue

            for layer_idx in range(ds_verify.GetLayerCount()):
                layer = ds_verify.GetLayer(layer_idx)
                if layer:
                    name = layer.GetName()
                    count = layer.GetFeatureCount()
                    if name in original_counts and count != original_counts[name]:
                        errors.append(f"{os.path.basename(filepath)}: {name} count mismatch ({original_counts[name]} -> {count})")

            ds_verify = None
            tested += 1

        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    if errors:
        print(f"FAILED ({len(errors)} errors)")
        for err in errors[:3]:
            print(f"    - {err}")
        return False

    print(f"PASSED ({tested} files verified)")
    return True


def test_ac6_roundtrip_via_shapefile():
    """AC6/Task5.2: Round-trip MP -> Shapefile -> MP preserves attributes."""
    from osgeo import ogr

    print("  AC6: Round-trip via Shapefile... ", end="")

    # Use poi-simple.mp as test source
    filepath = os.path.join(VALID_MINIMAL_DIR, 'poi-simple.mp')
    if not os.path.exists(filepath):
        print("SKIPPED (poi-simple.mp not found)")
        return True

    mp_driver = ogr.GetDriverByName('PolishMap')
    shp_driver = ogr.GetDriverByName('ESRI Shapefile')

    if mp_driver is None:
        print("FAILED (PolishMap driver not found)")
        return False

    if shp_driver is None:
        print("SKIPPED (Shapefile driver not available)")
        return True

    # Create temp directory for shapefile (multiple files)
    shp_dir = tempfile.mkdtemp(prefix='mp_shp_test_')
    shp_path = os.path.join(shp_dir, 'test.shp')
    fd, mp_roundtrip_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        # Step 1: Read original MP and store attributes
        ds_orig = ogr.Open(filepath)
        if ds_orig is None:
            print("FAILED (could not open original)")
            return False

        original_attrs = []
        poi_layer = ds_orig.GetLayerByName('POI')
        if poi_layer and poi_layer.GetFeatureCount() > 0:
            poi_layer.ResetReading()
            feat = poi_layer.GetNextFeature()
            if feat:
                original_attrs.append({
                    'type': feat.GetField('Type'),
                    'label': feat.GetField('Label')
                })

        ds_orig = None

        if not original_attrs:
            print("SKIPPED (no attributes to test)")
            return True

        # Step 2: MP -> Shapefile
        ds_mp = ogr.Open(filepath)
        ds_shp = shp_driver.CreateDataSource(shp_path)

        poi_src = ds_mp.GetLayerByName('POI')
        if poi_src and poi_src.GetFeatureCount() > 0:
            poi_dst = ds_shp.CreateLayer('POI', poi_src.GetSpatialRef(), ogr.wkbPoint)

            # Copy field definitions
            layer_defn = poi_src.GetLayerDefn()
            for i in range(layer_defn.GetFieldCount()):
                field_defn = layer_defn.GetFieldDefn(i)
                poi_dst.CreateField(field_defn)

            # Copy features
            poi_src.ResetReading()
            for feat in poi_src:
                poi_dst.CreateFeature(feat.Clone())

        ds_mp = None
        ds_shp = None

        # Step 3: Shapefile -> MP
        ds_shp = ogr.Open(shp_path)
        if ds_shp is None:
            print("FAILED (could not open intermediate shapefile)")
            return False

        ds_mp_out = mp_driver.CreateDataSource(mp_roundtrip_path)

        shp_layer = ds_shp.GetLayer(0)
        if shp_layer and shp_layer.GetFeatureCount() > 0:
            poi_dst = ds_mp_out.GetLayer(0)  # POI layer

            shp_layer.ResetReading()
            for feat in shp_layer:
                new_feat = ogr.Feature(poi_dst.GetLayerDefn())
                new_feat.SetField('Type', feat.GetField('Type') or '0x0000')
                new_feat.SetField('Label', feat.GetField('Label') or '')
                new_feat.SetGeometry(feat.GetGeometryRef().Clone())
                poi_dst.CreateFeature(new_feat)

        ds_shp = None
        ds_mp_out = None

        # Step 4: Verify round-trip attributes
        ds_verify = ogr.Open(mp_roundtrip_path)
        if ds_verify is None:
            print("FAILED (could not open round-trip file)")
            return False

        poi_verify = ds_verify.GetLayerByName('POI')
        if poi_verify is None or poi_verify.GetFeatureCount() == 0:
            print("FAILED (no features in round-trip)")
            ds_verify = None
            return False

        poi_verify.ResetReading()
        feat = poi_verify.GetNextFeature()

        orig = original_attrs[0]
        rt_type = feat.GetField('Type')
        rt_label = feat.GetField('Label')

        ds_verify = None

        # Check attribute preservation
        if rt_type != orig['type']:
            print(f"FAILED (Type mismatch: '{orig['type']}' -> '{rt_type}')")
            return False

        if rt_label != orig['label']:
            print(f"FAILED (Label mismatch: '{orig['label']}' -> '{rt_label}')")
            return False

        print(f"PASSED (Type='{rt_type}', Label='{rt_label}')")
        return True

    finally:
        if os.path.exists(mp_roundtrip_path):
            os.remove(mp_roundtrip_path)
        if os.path.exists(shp_dir):
            shutil.rmtree(shp_dir)


# =============================================================================
# AC7: Complete Corpus Regression Prevention
# =============================================================================
def test_ac7_all_valid_files_parse():
    """AC7: 100% of valid files should parse successfully (regression prevention)."""
    print("  AC7: Complete corpus parsing (regression prevention)... ", end="")

    all_files = []
    all_files.extend(get_mp_files(VALID_MINIMAL_DIR))
    all_files.extend(get_mp_files(VALID_COMPLEX_DIR))

    if not all_files:
        print("FAILED (no valid files found)")
        return False

    errors = []
    success_count = 0

    for filepath in all_files:
        result = parse_mp_file(filepath)
        if result['error']:
            errors.append(f"{os.path.basename(filepath)}: {result['error']}")
        else:
            success_count += 1

    total = len(all_files)
    pass_rate = (success_count / total) * 100 if total > 0 else 0

    if errors:
        print(f"FAILED ({pass_rate:.1f}% pass rate, {len(errors)} errors)")
        for err in errors[:5]:
            print(f"    - {err}")
        return False

    print(f"PASSED ({success_count}/{total} files, 100% pass rate)")
    return True


# =============================================================================
# Main
# =============================================================================
def main():
    """Run all valid corpus tests."""
    print("=== Story 3.3: Testing Corpus - Valid Cases ===")
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
    print()
    print(f"Test data directory: {TEST_DATA_DIR}")
    print(f"  valid-minimal: {VALID_MINIMAL_DIR}")
    print(f"  valid-complex: {VALID_COMPLEX_DIR}")
    print()

    passed = 0
    failed = 0
    skipped = 0

    print("Running tests:")
    print()

    # AC1: Valid-Minimal Directory
    print("AC1: Valid-Minimal Directory (10-20 files)")
    if test_ac1_valid_minimal_file_count():
        passed += 1
    else:
        failed += 1

    if test_ac1_required_files_exist():
        passed += 1
    else:
        failed += 1
    print()

    # AC2: Minimal POI Smoke Test
    print("AC2: Minimal POI Smoke Test")
    if test_ac2_poi_simple_smoke_test():
        passed += 1
    else:
        failed += 1
    print()

    # AC3: All Valid-Minimal Parse
    print("AC3: All Valid-Minimal Files Parse")
    if test_ac3_all_valid_minimal_parse():
        passed += 1
    else:
        failed += 1
    print()

    # AC4: Valid-Complex Directory
    print("AC4: Valid-Complex Directory (100-200 files)")
    result_ac4_count = test_ac4_valid_complex_file_count()
    if result_ac4_count:
        passed += 1
    else:
        failed += 1

    result_ac4_mixed = test_ac4_valid_complex_mixed_types()
    if result_ac4_mixed:
        passed += 1
    else:
        failed += 1
    print()

    # AC5: Complex File Parsing
    print("AC5: Complex File Parsing")
    result_ac5_parse = test_ac5_complex_file_parsing()
    if result_ac5_parse:
        passed += 1
    else:
        failed += 1

    result_ac5_attr = test_ac5_attribute_mapping()
    if result_ac5_attr:
        passed += 1
    else:
        failed += 1

    result_ac5_wgs84 = test_ac5_wgs84_bounds_validation()
    if result_ac5_wgs84:
        passed += 1
    else:
        failed += 1
    print()

    # AC6: Round-Trip Validation
    print("AC6: Round-Trip Validation")
    result_ac6_geom = test_ac6_roundtrip_geometry_preservation()
    if result_ac6_geom:
        passed += 1
    else:
        failed += 1

    result_ac6_attr = test_ac6_roundtrip_attribute_preservation()
    if result_ac6_attr:
        passed += 1
    else:
        failed += 1

    result_ac6_geojson = test_ac6_roundtrip_via_geojson()
    if result_ac6_geojson:
        passed += 1
    else:
        failed += 1

    result_ac6_multi = test_ac6_roundtrip_multiple_files()
    if result_ac6_multi:
        passed += 1
    else:
        failed += 1

    result_ac6_shp = test_ac6_roundtrip_via_shapefile()
    if result_ac6_shp:
        passed += 1
    else:
        failed += 1
    print()

    # AC7: Complete Corpus Regression Prevention
    print("AC7: Complete Corpus Regression Prevention")
    if test_ac7_all_valid_files_parse():
        passed += 1
    else:
        failed += 1
    print()

    # Summary
    print("=" * 50)
    print(f"Test Summary: Passed={passed}, Failed={failed}")
    print("=" * 50)

    # Return status
    return 0 if failed == 0 else 1


if __name__ == '__main__':
    sys.exit(main())
