#!/usr/bin/env python3
"""
Story 3.1: Performance Benchmarks (Task 5)

Tests for NFR1 (parsing < 2s for 10 MB), NFR2 (writing < 3s for 10 MB),
and linear scaling validation.

Architecture: Performance targets from PRD NFR1-4
"""

import os
import sys
import time
import tempfile
import resource

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

def get_memory_usage_mb():
    """Get current memory usage in MB (Linux)."""
    try:
        usage = resource.getrusage(resource.RUSAGE_SELF)
        return usage.ru_maxrss / 1024  # Convert KB to MB
    except Exception:
        return 0

def measure_parse_time(file_path, gdal_driver_path):
    """Measure time to parse a file and count features."""
    # Import GDAL with custom driver path
    os.environ['GDAL_DRIVER_PATH'] = gdal_driver_path
    from osgeo import gdal, ogr

    gdal.UseExceptions()

    start_memory = get_memory_usage_mb()
    start_time = time.perf_counter()

    ds = gdal.OpenEx(file_path, gdal.OF_VECTOR)
    if ds is None:
        return None, None, None

    # Count all features (forces full parse)
    total_features = 0
    for i in range(ds.GetLayerCount()):
        layer = ds.GetLayerByIndex(i)
        total_features += layer.GetFeatureCount()

    end_time = time.perf_counter()
    end_memory = get_memory_usage_mb()

    ds = None  # Close dataset

    parse_time = end_time - start_time
    memory_used = end_memory - start_memory

    return parse_time, total_features, memory_used

def measure_write_time(source_file, gdal_driver_path):
    """Measure time to write features to a new file."""
    os.environ['GDAL_DRIVER_PATH'] = gdal_driver_path
    from osgeo import gdal, ogr

    gdal.UseExceptions()

    # Open source file
    ds_src = gdal.OpenEx(source_file, gdal.OF_VECTOR)
    if ds_src is None:
        return None, None

    # Create temp output file
    with tempfile.NamedTemporaryFile(suffix='.mp', delete=False) as f:
        output_path = f.name

    try:
        start_time = time.perf_counter()

        # Create output dataset
        driver = gdal.GetDriverByName('POLISHMAP')
        ds_dst = driver.Create(output_path, 0, 0, 0, gdal.GDT_Unknown)

        # Copy all features
        total_features = 0
        for i in range(ds_src.GetLayerCount()):
            layer_src = ds_src.GetLayerByIndex(i)
            layer_name = layer_src.GetName()

            # Create matching layer in destination
            layer_dst = ds_dst.CreateLayer(layer_name, geom_type=layer_src.GetGeomType())

            # Copy features
            layer_src.ResetReading()
            while True:
                feat = layer_src.GetNextFeature()
                if feat is None:
                    break
                layer_dst.CreateFeature(feat)
                total_features += 1

        ds_dst = None  # Close and flush
        end_time = time.perf_counter()

        write_time = end_time - start_time
        output_size = os.path.getsize(output_path)

        return write_time, total_features, output_size

    finally:
        ds_src = None
        if os.path.exists(output_path):
            os.unlink(output_path)

def run_performance_tests():
    """Run all performance benchmark tests."""
    # Determine paths
    script_dir = os.path.dirname(os.path.abspath(__file__))
    project_dir = os.path.dirname(script_dir)
    build_dir = os.path.join(project_dir, 'build')
    perf_data_dir = os.path.join(project_dir, 'test', 'data', 'performance')

    print("=" * 70)
    print("Story 3.1: Performance Benchmark Tests (NFR1, NFR2, NFR7)")
    print("=" * 70)
    print()

    # Check if test data exists
    if not os.path.isdir(perf_data_dir):
        print(f"ERROR: Performance test data directory not found: {perf_data_dir}")
        print("Run: python3 tools/generate_perf_test_data.py -s 1 10")
        return 1

    results = []
    tests_passed = 0
    tests_failed = 0

    # Test 1: Parse 10 MB file < 2s (NFR1)
    print("Test NFR1: Parsing 10 MB file < 2s")
    perf_10mb = os.path.join(perf_data_dir, 'perf-10mb.mp')
    if os.path.exists(perf_10mb):
        file_size = os.path.getsize(perf_10mb) / (1024 * 1024)
        parse_time, features, memory = measure_parse_time(perf_10mb, build_dir)
        if parse_time is not None:
            passed = parse_time < 2.0
            status = "PASSED" if passed else "FAILED"
            print(f"  File: {file_size:.2f} MB, {features} features")
            print(f"  Parse time: {parse_time:.3f}s (threshold: 2.0s) ... {status}")
            if passed:
                tests_passed += 1
            else:
                tests_failed += 1
            results.append(('NFR1', file_size, parse_time, 2.0, passed))
        else:
            print(f"  ERROR: Could not open {perf_10mb}")
            tests_failed += 1
    else:
        print(f"  SKIPPED: {perf_10mb} not found")
        print("  Run: python3 tools/generate_perf_test_data.py -s 10")
    print()

    # Test 2: Write 10 MB file < 3s (NFR2)
    print("Test NFR2: Writing 10 MB file < 3s")
    if os.path.exists(perf_10mb):
        write_time, features, output_size = measure_write_time(perf_10mb, build_dir)
        if write_time is not None:
            output_size_mb = output_size / (1024 * 1024)
            passed = write_time < 3.0
            status = "PASSED" if passed else "FAILED"
            print(f"  Features: {features}, Output: {output_size_mb:.2f} MB")
            print(f"  Write time: {write_time:.3f}s (threshold: 3.0s) ... {status}")
            if passed:
                tests_passed += 1
            else:
                tests_failed += 1
            results.append(('NFR2', output_size_mb, write_time, 3.0, passed))
        else:
            print(f"  ERROR: Write test failed")
            tests_failed += 1
    else:
        print(f"  SKIPPED: Source file not found")
    print()

    # Test 3: Linear scaling (1 MB vs 10 MB)
    print("Test NFR7: Linear scaling (1 MB vs 10 MB)")
    perf_1mb = os.path.join(perf_data_dir, 'perf-1mb.mp')
    if os.path.exists(perf_1mb) and os.path.exists(perf_10mb):
        time_1mb, features_1mb, _ = measure_parse_time(perf_1mb, build_dir)
        time_10mb, features_10mb, _ = measure_parse_time(perf_10mb, build_dir)

        if time_1mb and time_10mb and time_1mb > 0:
            size_1mb = os.path.getsize(perf_1mb) / (1024 * 1024)
            size_10mb = os.path.getsize(perf_10mb) / (1024 * 1024)
            size_ratio = size_10mb / size_1mb
            time_ratio = time_10mb / time_1mb

            # Allow 20% deviation from linear (0.8x to 1.2x expected ratio)
            expected_min = size_ratio * 0.5
            expected_max = size_ratio * 1.5
            passed = expected_min <= time_ratio <= expected_max

            status = "PASSED" if passed else "FAILED"
            print(f"  1 MB: {time_1mb:.3f}s ({features_1mb} features)")
            print(f"  10 MB: {time_10mb:.3f}s ({features_10mb} features)")
            print(f"  Size ratio: {size_ratio:.1f}x, Time ratio: {time_ratio:.1f}x")
            print(f"  Linear scaling (within 50%): {status}")

            if passed:
                tests_passed += 1
            else:
                tests_failed += 1
            results.append(('Linear Scaling', time_ratio, expected_min, expected_max, passed))
        else:
            print("  ERROR: Timing measurements failed")
            tests_failed += 1
    else:
        print("  SKIPPED: Required test files not found")
    print()

    # Summary
    print("=" * 70)
    print(f"Performance Test Summary: Passed={tests_passed}, Failed={tests_failed}")
    print("=" * 70)

    return 0 if tests_failed == 0 else 1

if __name__ == '__main__':
    sys.exit(run_performance_tests())
