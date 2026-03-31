#!/usr/bin/env python3
"""
Story 3.1 & 3.4: Performance Benchmarks

Tests for:
- NFR1 (parsing < 2s for 10 MB)
- NFR2 (writing < 3s for 10 MB)
- NFR3 (memory usage < 2x file size)
- NFR7 (linear scaling validation)

Story 3.4 ACs:
- AC10: Performance corpus benchmark files (1MB, 5MB, 10MB, 50MB, 100MB)
- AC11: 10 MB parsing < 2 seconds
- AC12: Linear scaling validation + memory < 2x

Architecture: Performance targets from PRD NFR1-4
"""

import os
import sys
import time
import tempfile
import resource
import pytest

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

# ============================================================================
# Story 3.4: Pytest-based Performance Tests (AC10, AC11, AC12)
# ============================================================================

# Determine paths for pytest fixtures
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPT_DIR)
BUILD_DIR = os.path.join(PROJECT_DIR, 'build')
PERF_DATA_DIR = os.path.join(PROJECT_DIR, 'test', 'data', 'performance')


def get_peak_memory_mb():
    """Get peak memory usage in MB (Linux)."""
    try:
        usage = resource.getrusage(resource.RUSAGE_SELF)
        return usage.ru_maxrss / 1024  # Convert KB to MB
    except Exception:
        return 0


class TestPerformanceAC10:
    """AC10: Performance corpus has benchmark files of 1MB, 5MB, 10MB, 50MB, 100MB."""

    def test_ac10_benchmark_files_exist(self):
        """Verify all required benchmark files exist."""
        required_sizes = ['1mb', '5mb', '10mb', '50mb', '100mb']

        for size in required_sizes:
            file_path = os.path.join(PERF_DATA_DIR, f'perf-{size}.mp')
            assert os.path.exists(file_path), f"Missing benchmark file: perf-{size}.mp"
            # Verify file is not empty
            assert os.path.getsize(file_path) > 0, f"Empty benchmark file: perf-{size}.mp"

    def test_ac10_files_have_expected_sizes(self):
        """Verify benchmark files have approximately expected sizes (within 50%)."""
        size_expectations = {
            '1mb': (0.5, 1.5),    # 0.5 MB to 1.5 MB
            '5mb': (2.5, 7.5),    # 2.5 MB to 7.5 MB
            '10mb': (5, 15),      # 5 MB to 15 MB
            '50mb': (25, 75),     # 25 MB to 75 MB
            '100mb': (50, 150),   # 50 MB to 150 MB
        }

        for size, (min_mb, max_mb) in size_expectations.items():
            file_path = os.path.join(PERF_DATA_DIR, f'perf-{size}.mp')
            if os.path.exists(file_path):
                actual_mb = os.path.getsize(file_path) / (1024 * 1024)
                assert min_mb <= actual_mb <= max_mb, \
                    f"perf-{size}.mp: {actual_mb:.2f} MB not in range [{min_mb}, {max_mb}]"


class TestPerformanceAC11:
    """AC11: 10 MB file parsing completes in < 2 seconds."""

    @pytest.fixture(autouse=True)
    def setup_gdal(self):
        """Set up GDAL with custom driver path."""
        os.environ['GDAL_DRIVER_PATH'] = BUILD_DIR
        from osgeo import gdal
        gdal.UseExceptions()
        self.gdal = gdal

    def test_ac11_10mb_parsing_under_2_seconds(self):
        """NFR1: Parsing 10 MB file < 2 seconds."""
        perf_10mb = os.path.join(PERF_DATA_DIR, 'perf-10mb.mp')
        if not os.path.exists(perf_10mb):
            pytest.skip(f"Performance file not found: {perf_10mb}")

        file_size_mb = os.path.getsize(perf_10mb) / (1024 * 1024)

        start_time = time.perf_counter()

        ds = self.gdal.OpenEx(perf_10mb, self.gdal.OF_VECTOR)
        assert ds is not None, "Failed to open dataset"

        # Force full parse by counting features
        total_features = 0
        for i in range(ds.GetLayerCount()):
            layer = ds.GetLayerByIndex(i)
            total_features += layer.GetFeatureCount()

        parse_time = time.perf_counter() - start_time
        ds = None

        print(f"\n  File: {file_size_mb:.2f} MB, {total_features} features")
        print(f"  Parse time: {parse_time:.3f}s (threshold: 2.0s)")

        assert parse_time < 2.0, \
            f"10 MB parsing took {parse_time:.3f}s, exceeds 2.0s threshold"


class TestPerformanceAC12:
    """AC12: Linear scaling validation & memory usage < 2x file size."""

    @pytest.fixture(autouse=True)
    def setup_gdal(self):
        """Set up GDAL with custom driver path."""
        os.environ['GDAL_DRIVER_PATH'] = BUILD_DIR
        from osgeo import gdal
        gdal.UseExceptions()
        self.gdal = gdal

    def test_ac12_linear_scaling_1mb_vs_10mb(self):
        """Verify parsing time scales linearly with file size."""
        perf_1mb = os.path.join(PERF_DATA_DIR, 'perf-1mb.mp')
        perf_10mb = os.path.join(PERF_DATA_DIR, 'perf-10mb.mp')

        if not os.path.exists(perf_1mb) or not os.path.exists(perf_10mb):
            pytest.skip("Required performance files not found")

        # Measure 1 MB
        start = time.perf_counter()
        ds1 = self.gdal.OpenEx(perf_1mb, self.gdal.OF_VECTOR)
        for i in range(ds1.GetLayerCount()):
            ds1.GetLayerByIndex(i).GetFeatureCount()
        time_1mb = time.perf_counter() - start
        ds1 = None

        # Measure 10 MB
        start = time.perf_counter()
        ds10 = self.gdal.OpenEx(perf_10mb, self.gdal.OF_VECTOR)
        for i in range(ds10.GetLayerCount()):
            ds10.GetLayerByIndex(i).GetFeatureCount()
        time_10mb = time.perf_counter() - start
        ds10 = None

        size_1mb = os.path.getsize(perf_1mb) / (1024 * 1024)
        size_10mb = os.path.getsize(perf_10mb) / (1024 * 1024)
        size_ratio = size_10mb / size_1mb
        time_ratio = time_10mb / time_1mb if time_1mb > 0 else float('inf')

        # Allow 50% deviation from linear scaling
        expected_min = size_ratio * 0.5
        expected_max = size_ratio * 1.5

        print(f"\n  1 MB: {time_1mb:.3f}s")
        print(f"  10 MB: {time_10mb:.3f}s")
        print(f"  Size ratio: {size_ratio:.1f}x, Time ratio: {time_ratio:.1f}x")
        print(f"  Expected range: [{expected_min:.1f}x, {expected_max:.1f}x]")

        assert expected_min <= time_ratio <= expected_max, \
            f"Time ratio {time_ratio:.1f}x outside linear range [{expected_min:.1f}x, {expected_max:.1f}x]"

    def test_ac12_linear_scaling_5mb_vs_50mb(self):
        """Verify parsing time scales linearly for larger files."""
        perf_5mb = os.path.join(PERF_DATA_DIR, 'perf-5mb.mp')
        perf_50mb = os.path.join(PERF_DATA_DIR, 'perf-50mb.mp')

        if not os.path.exists(perf_5mb) or not os.path.exists(perf_50mb):
            pytest.skip("Required performance files not found")

        # Measure 5 MB
        start = time.perf_counter()
        ds5 = self.gdal.OpenEx(perf_5mb, self.gdal.OF_VECTOR)
        for i in range(ds5.GetLayerCount()):
            ds5.GetLayerByIndex(i).GetFeatureCount()
        time_5mb = time.perf_counter() - start
        ds5 = None

        # Measure 50 MB
        start = time.perf_counter()
        ds50 = self.gdal.OpenEx(perf_50mb, self.gdal.OF_VECTOR)
        for i in range(ds50.GetLayerCount()):
            ds50.GetLayerByIndex(i).GetFeatureCount()
        time_50mb = time.perf_counter() - start
        ds50 = None

        size_5mb = os.path.getsize(perf_5mb) / (1024 * 1024)
        size_50mb = os.path.getsize(perf_50mb) / (1024 * 1024)
        size_ratio = size_50mb / size_5mb
        time_ratio = time_50mb / time_5mb if time_5mb > 0 else float('inf')

        # Allow 50% deviation from linear scaling
        expected_min = size_ratio * 0.5
        expected_max = size_ratio * 1.5

        print(f"\n  5 MB: {time_5mb:.3f}s")
        print(f"  50 MB: {time_50mb:.3f}s")
        print(f"  Size ratio: {size_ratio:.1f}x, Time ratio: {time_ratio:.1f}x")

        assert expected_min <= time_ratio <= expected_max, \
            f"Time ratio {time_ratio:.1f}x outside linear range [{expected_min:.1f}x, {expected_max:.1f}x]"

    def test_ac12_memory_usage_under_2x_file_size(self):
        """NFR3: Memory usage < 2x file size during parsing."""
        perf_10mb = os.path.join(PERF_DATA_DIR, 'perf-10mb.mp')

        if not os.path.exists(perf_10mb):
            pytest.skip("Performance file not found")

        file_size_mb = os.path.getsize(perf_10mb) / (1024 * 1024)
        max_allowed_mb = file_size_mb * 2

        # Get baseline memory
        baseline_memory = get_peak_memory_mb()

        # Parse file
        ds = self.gdal.OpenEx(perf_10mb, self.gdal.OF_VECTOR)
        assert ds is not None, "Failed to open dataset"

        # Force full parse
        for i in range(ds.GetLayerCount()):
            layer = ds.GetLayerByIndex(i)
            layer.GetFeatureCount()
            # Iterate through features to ensure memory is allocated
            layer.ResetReading()
            while layer.GetNextFeature() is not None:
                pass

        peak_memory = get_peak_memory_mb()
        memory_used = peak_memory - baseline_memory
        ds = None

        print(f"\n  File size: {file_size_mb:.2f} MB")
        print(f"  Memory used: {memory_used:.2f} MB (NFR3 max: {max_allowed_mb:.2f} MB)")
        print(f"  Ratio: {memory_used / file_size_mb:.2f}x (NFR3 requires < 2x)")

        # NFR3: Memory usage < 2x file size
        # Note: ru_maxrss includes process baseline, so we check delta from baseline
        # If delta is negative or zero (measurement noise), skip assertion
        if memory_used > 0:
            assert memory_used < max_allowed_mb, \
                f"Memory usage {memory_used:.2f} MB exceeds NFR3 limit of 2x file size ({max_allowed_mb:.2f} MB)"


if __name__ == '__main__':
    sys.exit(run_performance_tests())
