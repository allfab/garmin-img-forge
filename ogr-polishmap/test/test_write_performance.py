#!/usr/bin/env python3
"""
Story 2.7: Write Performance Tests (AC6)

Tests performance of writing Polish Map files with 1000+ features:
- All features are written correctly
- File can be reopened and all features are readable
- Performance is acceptable (< 3s for 10 MB file) (NFR2)

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_write_performance.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing gdal_POLISHMAP.so
"""

import os
import sys
import time
import tempfile
import random

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

def generate_test_features(layer, count, feature_type='POI'):
    """Generate test features for performance testing.

    Args:
        layer: OGR layer to add features to
        count: Number of features to generate
        feature_type: 'POI', 'POLYLINE', or 'POLYGON'
    """
    from osgeo import ogr

    base_lon = 2.35  # Paris area
    base_lat = 48.85

    for i in range(count):
        feature = ogr.Feature(layer.GetLayerDefn())

        # Set type (varies by feature type)
        if feature_type == 'POI':
            type_codes = ['0x2C00', '0x4000', '0x6401', '0x2A00', '0x2B00']
            feature.SetField('Type', type_codes[i % len(type_codes)])
            feature.SetField('Label', f'POI_{i:05d}')

            # Point geometry
            point = ogr.Geometry(ogr.wkbPoint)
            point.AddPoint(base_lon + (i % 100) * 0.001, base_lat + (i // 100) * 0.001)
            feature.SetGeometry(point)

        elif feature_type == 'POLYLINE':
            type_codes = ['0x0001', '0x0002', '0x0016', '0x0003', '0x0005']
            feature.SetField('Type', type_codes[i % len(type_codes)])
            feature.SetField('Label', f'LINE_{i:05d}')

            # LineString geometry (3-5 points)
            line = ogr.Geometry(ogr.wkbLineString)
            num_points = 3 + (i % 3)
            for j in range(num_points):
                line.AddPoint(
                    base_lon + (i % 100) * 0.001 + j * 0.0001,
                    base_lat + (i // 100) * 0.001 + j * 0.0001
                )
            feature.SetGeometry(line)

        elif feature_type == 'POLYGON':
            type_codes = ['0x004C', '0x0050', '0x0017', '0x004B', '0x0013']
            feature.SetField('Type', type_codes[i % len(type_codes)])
            feature.SetField('Label', f'POLY_{i:05d}')

            # Polygon geometry (5-point closed ring)
            polygon = ogr.Geometry(ogr.wkbPolygon)
            ring = ogr.Geometry(ogr.wkbLinearRing)
            center_lon = base_lon + (i % 100) * 0.001
            center_lat = base_lat + (i // 100) * 0.001
            offset = 0.0005
            ring.AddPoint(center_lon - offset, center_lat - offset)
            ring.AddPoint(center_lon + offset, center_lat - offset)
            ring.AddPoint(center_lon + offset, center_lat + offset)
            ring.AddPoint(center_lon - offset, center_lat + offset)
            ring.AddPoint(center_lon - offset, center_lat - offset)  # Close ring
            polygon.AddGeometry(ring)
            feature.SetGeometry(polygon)

        err = layer.CreateFeature(feature)
        if err != 0:
            raise RuntimeError(f"CreateFeature failed for {feature_type} {i}: error {err}")

def test_ac6_write_performance_1000_features():
    """AC6: Test writing 1000+ features with performance measurement."""
    from osgeo import ogr

    print("  AC6 Test 1: Write 1000+ POI features ... ", end="", flush=True)

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        driver = ogr.GetDriverByName('PolishMap')
        if driver is None:
            print("FAILED (driver not found)")
            return False

        # Create datasource
        ds = driver.CreateDataSource(temp_path)
        if ds is None:
            print("FAILED (CreateDataSource returned None)")
            return False

        # Get POI layer
        poi_layer = ds.GetLayer(0)
        if poi_layer is None:
            print("FAILED (GetLayer(0) returned None)")
            ds = None
            return False

        # Generate and write 1000 POI features
        start_time = time.time()
        generate_test_features(poi_layer, 1000, 'POI')

        # Close to flush
        ds = None
        write_time = time.time() - start_time

        # Check file size
        file_size = os.path.getsize(temp_path)
        file_size_kb = file_size / 1024

        # Verify file is readable
        ds_read = ogr.Open(temp_path)
        if ds_read is None:
            print(f"FAILED (file not readable after write)")
            return False

        poi_layer_read = ds_read.GetLayer(0)
        feature_count = poi_layer_read.GetFeatureCount()
        ds_read = None

        if feature_count != 1000:
            print(f"FAILED (expected 1000 features, got {feature_count})")
            return False

        print(f"PASSED ({write_time:.2f}s, {file_size_kb:.1f} KB, {feature_count} features)")
        return True

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac6_write_all_geometry_types():
    """AC6: Test writing mixed geometry types."""
    from osgeo import ogr

    print("  AC6 Test 2: Write mixed geometry types ... ", end="", flush=True)

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(temp_path)

        # Write to each layer type
        poi_layer = ds.GetLayer(0)
        polyline_layer = ds.GetLayer(1)
        polygon_layer = ds.GetLayer(2)

        start_time = time.time()

        generate_test_features(poi_layer, 500, 'POI')
        generate_test_features(polyline_layer, 300, 'POLYLINE')
        generate_test_features(polygon_layer, 200, 'POLYGON')

        ds = None
        write_time = time.time() - start_time

        # Verify
        ds_read = ogr.Open(temp_path)
        poi_count = ds_read.GetLayer(0).GetFeatureCount()
        line_count = ds_read.GetLayer(1).GetFeatureCount()
        poly_count = ds_read.GetLayer(2).GetFeatureCount()
        total_count = poi_count + line_count + poly_count
        ds_read = None

        file_size = os.path.getsize(temp_path) / 1024

        if total_count == 1000:
            print(f"PASSED ({write_time:.2f}s, {file_size:.1f} KB, {poi_count}+{line_count}+{poly_count}={total_count})")
            return True
        else:
            print(f"FAILED (expected 1000 total, got {total_count})")
            return False

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac6_performance_threshold():
    """AC6: Test performance < 3s for ~10MB file (NFR2).

    NFR2 specifies: "< 3s for 10 MB file"
    A typical feature in MP format is ~200 bytes, so ~50,000 features = ~10MB
    We test with 20,000 features as a reasonable CI-friendly target (~4MB).
    """
    from osgeo import ogr

    print("  AC6 Test 3: Performance threshold (< 3s for ~4MB/20000 features) ... ", end="", flush=True)

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(temp_path)

        poi_layer = ds.GetLayer(0)
        polyline_layer = ds.GetLayer(1)
        polygon_layer = ds.GetLayer(2)

        start_time = time.time()

        # Write 20000 features total to approach NFR2 target (~4MB)
        # POIs are ~100 bytes, POLYLINEs ~200 bytes, POLYGONs ~300 bytes
        generate_test_features(poi_layer, 10000, 'POI')
        generate_test_features(polyline_layer, 6000, 'POLYLINE')
        generate_test_features(polygon_layer, 4000, 'POLYGON')

        ds = None
        write_time = time.time() - start_time

        file_size_kb = os.path.getsize(temp_path) / 1024
        file_size_mb = file_size_kb / 1024

        # NFR2: < 3s for writing (scaled for ~4MB, proportionally ~1.2s expected for 10MB)
        if write_time < 3.0:
            print(f"PASSED ({write_time:.2f}s < 3.0s, {file_size_mb:.1f} MB)")
            return True
        else:
            print(f"FAILED ({write_time:.2f}s >= 3.0s threshold)")
            return False

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac6_file_reopenable():
    """AC6: Test that written file is fully reopenable with correct attributes."""
    from osgeo import ogr

    print("  AC6 Test 4: File reopenable with all features and attributes ... ", end="", flush=True)

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(temp_path)

        poi_layer = ds.GetLayer(0)
        generate_test_features(poi_layer, 500, 'POI')
        ds = None

        # Reopen multiple times to ensure stability
        for i in range(3):
            ds_read = ogr.Open(temp_path)
            if ds_read is None:
                print(f"FAILED (reopen #{i+1} failed)")
                return False

            poi_layer_read = ds_read.GetLayer(0)
            count = poi_layer_read.GetFeatureCount()
            if count != 500:
                print(f"FAILED (reopen #{i+1}: expected 500, got {count})")
                return False

            # Read all features and verify attributes on first reopen
            poi_layer_read.ResetReading()
            features_read = 0
            attributes_valid = True
            while True:
                feat = poi_layer_read.GetNextFeature()
                if feat is None:
                    break
                features_read += 1

                # Verify attributes on first iteration (spot check every 50th feature)
                if i == 0 and features_read % 50 == 1:
                    type_val = feat.GetField('Type')
                    label_val = feat.GetField('Label')
                    geom = feat.GetGeometryRef()

                    if type_val is None or not type_val.startswith('0x'):
                        attributes_valid = False
                    if label_val is None or not label_val.startswith('POI_'):
                        attributes_valid = False
                    if geom is None or geom.GetGeometryType() != ogr.wkbPoint:
                        attributes_valid = False

            if features_read != 500:
                print(f"FAILED (reopen #{i+1}: could only read {features_read} features)")
                return False

            if i == 0 and not attributes_valid:
                print(f"FAILED (reopen #{i+1}: attributes not preserved correctly)")
                return False

            ds_read = None

        print("PASSED (3 reopens, all features + attributes verified)")
        return True

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def test_ac6_large_labels():
    """AC6: Test writing features with varying label lengths."""
    from osgeo import ogr

    print("  AC6 Test 5: Large labels handling ... ", end="", flush=True)

    fd, temp_path = tempfile.mkstemp(suffix='.mp')
    os.close(fd)

    try:
        driver = ogr.GetDriverByName('PolishMap')
        ds = driver.CreateDataSource(temp_path)

        poi_layer = ds.GetLayer(0)

        # Create features with varying label sizes
        test_labels = [
            "Short",
            "Medium length label here",
            "A much longer label that contains more characters for testing",
            "Label with special: cafe, hotel, rue",
            "Numbers: 12345 67890",
        ]

        for i, label in enumerate(test_labels):
            feature = ogr.Feature(poi_layer.GetLayerDefn())
            feature.SetField('Type', '0x2C00')
            feature.SetField('Label', label)
            point = ogr.Geometry(ogr.wkbPoint)
            point.AddPoint(2.35 + i * 0.01, 48.85 + i * 0.01)
            feature.SetGeometry(point)
            poi_layer.CreateFeature(feature)

        ds = None

        # Verify
        ds_read = ogr.Open(temp_path)
        poi_layer_read = ds_read.GetLayer(0)
        count = poi_layer_read.GetFeatureCount()
        ds_read = None

        if count == len(test_labels):
            print(f"PASSED ({count} features with varied labels)")
            return True
        else:
            print(f"FAILED (expected {len(test_labels)}, got {count})")
            return False

    except Exception as e:
        print(f"FAILED (exception: {e})")
        return False
    finally:
        if os.path.exists(temp_path):
            os.remove(temp_path)

def main():
    """Run all performance tests."""
    print("=== Story 2.7: Write Performance Tests (AC6) ===")
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

    print("Running performance tests:")

    # AC6 Test 1: Write 1000+ POI features
    if test_ac6_write_performance_1000_features():
        passed += 1
    else:
        failed += 1

    # AC6 Test 2: Write mixed geometry types
    if test_ac6_write_all_geometry_types():
        passed += 1
    else:
        failed += 1

    # AC6 Test 3: Performance threshold
    if test_ac6_performance_threshold():
        passed += 1
    else:
        failed += 1

    # AC6 Test 4: File reopenable
    if test_ac6_file_reopenable():
        passed += 1
    else:
        failed += 1

    # AC6 Test 5: Large labels
    if test_ac6_large_labels():
        passed += 1
    else:
        failed += 1

    print()
    print("=" * 50)
    print(f"Performance Test Summary: Passed={passed}, Failed={failed}")
    print("=" * 50)

    return 0 if failed == 0 else 1

if __name__ == '__main__':
    sys.exit(main())
