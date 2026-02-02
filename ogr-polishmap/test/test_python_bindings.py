#!/usr/bin/env python3
"""
Story 2.6: Python GDAL Bindings Tests (AC4, AC5)

Tests Python GDAL bindings for reading and writing Polish Map files.

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_python_bindings.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing gdal_POLISHMAP.so
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

def main():
    """Run all Python binding tests."""
    print("=== Story 2.6: Python GDAL Bindings Tests (AC4, AC5) ===")
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

    print()
    print("=" * 40)
    print(f"Test Summary: Passed={passed}, Failed={failed}")
    print("=" * 40)

    return 0 if failed == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
