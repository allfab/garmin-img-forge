#!/usr/bin/env python3
"""
Story 2.6 & 2.7: Python GDAL Bindings Tests (AC4, AC5, AC7)

Tests Python GDAL bindings for reading and writing Polish Map files.

Story 2.6:
- AC4: Python GDAL bindings work for writing
- AC5: Python GDAL bindings work for reading

Story 2.7:
- AC7: Round-trip with modification: read MP -> modify features -> write MP -> read MP

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_python_bindings.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing ogr_PolishMap.so
"""

import os
import sys
import tempfile

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

def test_ac4_write_polish_map():
    """AC4: Python GDAL bindings work for writing."""
    from osgeo import ogr

    print("  AC4: Python write test... ", end="")

    # Create temporary file
    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        # Get PolishMap driver
        driver = ogr.GetDriverByName('PolishMap')
        if driver is None:
            print("FAILED (driver not found)")
            return False

        # Create datasource
        ds = driver.CreateDataSource(temp_path)
        if ds is None:
            print("FAILED (CreateDataSource returned None)")
            return False

        # Get POI layer (layer 0)
        layer = ds.GetLayer(0)
        if layer is None:
            print("FAILED (GetLayer(0) returned None)")
            ds = None
            return False

        # Create feature
        feature = ogr.Feature(layer.GetLayerDefn())
        feature.SetField('Type', '0x2C00')
        feature.SetField('Label', 'Test POI')

        # Set Point geometry
        point = ogr.Geometry(ogr.wkbPoint)
        point.AddPoint(2.3522, 48.8566)
        feature.SetGeometry(point)

        # Write feature
        err = layer.CreateFeature(feature)
        if err != ogr.OGRERR_NONE:
            print(f"FAILED (CreateFeature returned {err})")
            ds = None
            return False

        # Close dataset (triggers flush)
        ds = None

        # Verify file was created
        if not os.path.exists(temp_path):
            print("FAILED (file not created)")
            return False

        # Verify file content
        with open(temp_path, 'r') as f:
            content = f.read()

        if '[POI]' not in content:
            print("FAILED ([POI] section not found)")
            return False

        if 'Test POI' not in content:
            print("FAILED (Label not found)")
            return False

        print("PASSED")
        return True

    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac5_read_polish_map():
    """AC5: Python GDAL bindings work for reading."""
    from osgeo import ogr

    print("  AC5: Python read test... ", end="")

    # Create temporary file with test data
    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        # Write test file
        with open(temp_path, 'w') as f:
            f.write("""[IMG ID]
Name=Test
CodePage=1252
[END]
[POI]
Type=0x2C00
Label=Restaurant
Data0=(48.8566,2.3522)
[END]
""")

        # Open with GDAL
        ds = ogr.Open(temp_path)
        if ds is None:
            print("FAILED (Open returned None)")
            return False

        # Check layer count
        layer_count = ds.GetLayerCount()
        if layer_count != 3:
            print(f"FAILED (expected 3 layers, got {layer_count})")
            ds = None
            return False

        # Get POI layer
        layer = ds.GetLayer(0)
        if layer is None:
            print("FAILED (GetLayer(0) returned None)")
            ds = None
            return False

        # Check layer name
        if layer.GetName() != 'POI':
            print(f"FAILED (expected 'POI', got '{layer.GetName()}')")
            ds = None
            return False

        # Read feature
        layer.ResetReading()
        feature = layer.GetNextFeature()
        if feature is None:
            print("FAILED (GetNextFeature returned None)")
            ds = None
            return False

        # Check attributes
        type_val = feature.GetField('Type')
        if type_val != '0x2C00':
            print(f"FAILED (expected Type='0x2C00', got '{type_val}')")
            ds = None
            return False

        label_val = feature.GetField('Label')
        if label_val != 'Restaurant':
            print(f"FAILED (expected Label='Restaurant', got '{label_val}')")
            ds = None
            return False

        # Check geometry
        geom = feature.GetGeometryRef()
        if geom is None:
            print("FAILED (GetGeometryRef returned None)")
            ds = None
            return False

        if geom.GetGeometryType() != ogr.wkbPoint:
            print(f"FAILED (expected wkbPoint, got {geom.GetGeometryType()})")
            ds = None
            return False

        # Close
        ds = None

        print("PASSED")
        return True

    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_roundtrip():
    """Test write then read (round-trip)."""
    from osgeo import ogr

    print("  Round-trip test... ", end="")

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        # Write
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(temp_path)

        layer = ds.GetLayer(0)  # POI
        feature = ogr.Feature(layer.GetLayerDefn())
        feature.SetField('Type', '0x4000')
        feature.SetField('Label', 'Roundtrip Test')

        point = ogr.Geometry(ogr.wkbPoint)
        point.AddPoint(2.348800, 48.853400)
        feature.SetGeometry(point)
        layer.CreateFeature(feature)

        ds = None

        # Read back
        ds = ogr.Open(temp_path)
        layer = ds.GetLayer(0)
        layer.ResetReading()
        feature = layer.GetNextFeature()

        if feature is None:
            print("FAILED (no feature after round-trip)")
            return False

        if feature.GetField('Label') != 'Roundtrip Test':
            print("FAILED (Label not preserved)")
            return False

        geom = feature.GetGeometryRef()
        x = geom.GetX()
        y = geom.GetY()

        # Check coordinate precision (6 decimals)
        if abs(x - 2.348800) > 1e-6 or abs(y - 48.853400) > 1e-6:
            print(f"FAILED (coordinates not preserved: {x}, {y})")
            return False

        ds = None

        print("PASSED")
        return True

    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac7_modify_and_persist():
    """AC7: Read MP -> modify features -> write MP -> read MP."""
    from osgeo import ogr

    print("  AC7: Modify and persist test... ", end="")

    # Create two temp files: original and modified
    fd1, original_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd1)
    fd2, modified_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd2)

    try:
        # Step 1: Create original MP file with multiple features
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(original_path)

        poi_layer = ds.GetLayer(0)

        # Create 3 features
        for i, (type_code, label) in enumerate([
            ('0x2C00', 'Original Restaurant'),
            ('0x4000', 'Original City'),
            ('0x6401', 'Original Gas Station')
        ]):
            feature = ogr.Feature(poi_layer.GetLayerDefn())
            feature.SetField('Type', type_code)
            feature.SetField('Label', label)
            point = ogr.Geometry(ogr.wkbPoint)
            point.AddPoint(2.35 + i * 0.01, 48.85 + i * 0.01)
            feature.SetGeometry(point)
            poi_layer.CreateFeature(feature)

        ds = None

        # Step 2: Read original, modify, write to new file
        ds_read = ogr.Open(original_path)
        if ds_read is None:
            print("FAILED (could not open original)")
            return False

        # Create new datasource for modified data
        ds_write = driver.CreateDataSource(modified_path)
        poi_layer_write = ds_write.GetLayer(0)

        # Read and modify features
        poi_layer_read = ds_read.GetLayer(0)
        poi_layer_read.ResetReading()

        modifications = [
            ('0x2C01', 'Modified Restaurant'),  # Changed type and label
            ('0x4001', 'Modified City'),
            ('0x6402', 'Modified Gas Station')
        ]

        feature_idx = 0
        while True:
            feat = poi_layer_read.GetNextFeature()
            if feat is None:
                break

            # Create new feature with modified attributes
            new_feat = ogr.Feature(poi_layer_write.GetLayerDefn())
            new_feat.SetField('Type', modifications[feature_idx][0])
            new_feat.SetField('Label', modifications[feature_idx][1])

            # Preserve geometry
            new_feat.SetGeometry(feat.GetGeometryRef().Clone())

            poi_layer_write.CreateFeature(new_feat)
            feature_idx += 1

        ds_read = None
        ds_write = None

        # Step 3: Read modified file and verify modifications
        ds_verify = ogr.Open(modified_path)
        if ds_verify is None:
            print("FAILED (could not open modified file)")
            return False

        poi_layer_verify = ds_verify.GetLayer(0)
        count = poi_layer_verify.GetFeatureCount()

        if count != 3:
            print(f"FAILED (expected 3 features, got {count})")
            return False

        poi_layer_verify.ResetReading()
        verified = 0
        while True:
            feat = poi_layer_verify.GetNextFeature()
            if feat is None:
                break

            label = feat.GetField('Label')
            if label is None or not label.startswith('Modified'):
                print(f"FAILED (expected modified label, got '{label}')")
                return False
            verified += 1

        ds_verify = None

        if verified != 3:
            print(f"FAILED (only verified {verified} features)")
            return False

        print("PASSED")
        return True

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        for path in [original_path, modified_path]:
            if os.path.exists(path):
                os.remove(path)

def test_ac7_roundtrip_all_geometry_types():
    """AC7: Round-trip with all geometry types (POI, POLYLINE, POLYGON)."""
    from osgeo import ogr

    print("  AC7: Multi-geometry round-trip... ", end="")

    fd1, original_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd1)
    fd2, modified_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd2)

    try:
        # Step 1: Create original with all geometry types
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(original_path)

        # POI
        poi_layer = ds.GetLayer(0)
        feat = ogr.Feature(poi_layer.GetLayerDefn())
        feat.SetField('Type', '0x2C00')
        feat.SetField('Label', 'Test POI')
        point = ogr.Geometry(ogr.wkbPoint)
        point.AddPoint(2.35, 48.85)
        feat.SetGeometry(point)
        poi_layer.CreateFeature(feat)

        # POLYLINE
        line_layer = ds.GetLayer(1)
        feat = ogr.Feature(line_layer.GetLayerDefn())
        feat.SetField('Type', '0x0001')
        feat.SetField('Label', 'Test Road')
        line = ogr.Geometry(ogr.wkbLineString)
        line.AddPoint(2.35, 48.85)
        line.AddPoint(2.36, 48.86)
        line.AddPoint(2.37, 48.87)
        feat.SetGeometry(line)
        line_layer.CreateFeature(feat)

        # POLYGON
        poly_layer = ds.GetLayer(2)
        feat = ogr.Feature(poly_layer.GetLayerDefn())
        feat.SetField('Type', '0x004C')
        feat.SetField('Label', 'Test Forest')
        polygon = ogr.Geometry(ogr.wkbPolygon)
        ring = ogr.Geometry(ogr.wkbLinearRing)
        ring.AddPoint(2.40, 48.80)
        ring.AddPoint(2.42, 48.80)
        ring.AddPoint(2.42, 48.82)
        ring.AddPoint(2.40, 48.82)
        ring.AddPoint(2.40, 48.80)
        polygon.AddGeometry(ring)
        feat.SetGeometry(polygon)
        poly_layer.CreateFeature(feat)

        ds = None

        # Step 2: Read, modify labels, write to new file
        ds_read = ogr.Open(original_path)
        ds_write = driver.CreateDataSource(modified_path)

        for layer_idx in range(3):
            src_layer = ds_read.GetLayer(layer_idx)
            dst_layer = ds_write.GetLayer(layer_idx)

            src_layer.ResetReading()
            while True:
                feat = src_layer.GetNextFeature()
                if feat is None:
                    break

                new_feat = ogr.Feature(dst_layer.GetLayerDefn())
                new_feat.SetField('Type', feat.GetField('Type'))

                old_label = feat.GetField('Label') or ''
                new_feat.SetField('Label', f'RT-{old_label}')

                new_feat.SetGeometry(feat.GetGeometryRef().Clone())
                dst_layer.CreateFeature(new_feat)

        ds_read = None
        ds_write = None

        # Step 3: Verify
        ds_verify = ogr.Open(modified_path)

        total_features = 0
        for layer_idx in range(3):
            layer = ds_verify.GetLayer(layer_idx)
            count = layer.GetFeatureCount()
            total_features += count

            layer.ResetReading()
            while True:
                feat = layer.GetNextFeature()
                if feat is None:
                    break
                label = feat.GetField('Label')
                if label is None or not label.startswith('RT-'):
                    print(f"FAILED (layer {layer_idx}: label not modified)")
                    return False

        ds_verify = None

        if total_features != 3:
            print(f"FAILED (expected 3 total features, got {total_features})")
            return False

        print("PASSED")
        return True

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        for path in [original_path, modified_path]:
            if os.path.exists(path):
                os.remove(path)

def main():
    """Run all Python binding tests."""
    print("=== Story 2.6 & 2.7: Python GDAL Bindings Tests (AC4, AC5, AC7) ===")
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

    passed = 0
    failed = 0

    print("Running tests:")

    # AC4: Write test
    if test_ac4_write_polish_map():
        passed += 1
    else:
        failed += 1

    # AC5: Read test
    if test_ac5_read_polish_map():
        passed += 1
    else:
        failed += 1

    # Round-trip test
    if test_roundtrip():
        passed += 1
    else:
        failed += 1

    # AC7: Modify and persist test (Story 2.7)
    if test_ac7_modify_and_persist():
        passed += 1
    else:
        failed += 1

    # AC7: Multi-geometry round-trip test (Story 2.7)
    if test_ac7_roundtrip_all_geometry_types():
        passed += 1
    else:
        failed += 1

    print()
    print("=" * 40)
    print(f"Test Summary: Passed={passed}, Failed={failed}")
    print("=" * 40)

    return 0 if failed == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
