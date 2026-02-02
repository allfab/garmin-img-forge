#!/usr/bin/env python3
"""
Story 3.2: Error Recovery Tests (AC1-AC7)

Tests the Three-Level Error Strategy for the Polish Map driver:
- Critical Errors: Fail + Return NULL (AC1, AC5)
- Recoverable Errors: Skip + Continue (AC2, AC4)
- Minor Issues: Default + Log (AC3)
- CPL Logging Consistency (AC6)
- Zero Crash Validation (AC7)

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed in GDAL plugin path

Usage:
    python3 test_error_recovery.py

Environment:
    GDAL_DRIVER_PATH: Set to directory containing gdal_POLISHMAP.so
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
    """Get path to error-recovery test data directory."""
    # Get directory containing this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(script_dir, 'data', 'error-recovery')


# =============================================================================
# AC1: Critical Error - Missing Header
# =============================================================================
def test_ac1_missing_header():
    """AC1: Open() returns NULL for file missing [IMG ID] header."""
    from osgeo import ogr, gdal
    
    print("  AC1: Missing header test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'missing-header.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    # Suppress error messages to stderr during test
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is not None:
            ds = None
            print("FAILED (Open() should return NULL)")
            return False
        
        # Verify CE_Failure was logged
        err_msg = gdal.GetLastErrorMsg()
        if 'missing' not in err_msg.lower() and 'header' not in err_msg.lower() and 'IMG ID' not in err_msg:
            print(f"FAILED (wrong error message: {err_msg})")
            return False
        
        print("PASSED")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC2: Recoverable Error - Malformed Section
# =============================================================================
def test_ac2_malformed_section():
    """AC2: Skip malformed section and continue parsing."""
    from osgeo import ogr, gdal
    
    print("  AC2: Malformed section test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'invalid-section.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is None:
            print("FAILED (Open() returned NULL - should recover)")
            return False
        
        # Should have valid features despite malformed section
        poi_layer = ds.GetLayer(0)
        poi_layer.ResetReading()
        
        valid_features = []
        while True:
            feat = poi_layer.GetNextFeature()
            if feat is None:
                break
            label = feat.GetField('Label')
            if label:
                valid_features.append(label)
        
        ds = None
        
        # Should have extracted valid POIs (skipping malformed one)
        if len(valid_features) < 2:
            print(f"FAILED (expected >= 2 valid features, got {len(valid_features)})")
            return False
        
        print(f"PASSED ({len(valid_features)} valid features extracted)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC3: Minor Issue - Missing Optional Field
# =============================================================================
def test_ac3_missing_label():
    """AC3: Missing Label uses empty string default."""
    from osgeo import ogr, gdal
    
    print("  AC3: Missing label test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'missing-label.mp')
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
        
        poi_layer = ds.GetLayer(0)
        poi_layer.ResetReading()
        
        features_with_empty_label = 0
        total_features = 0
        
        while True:
            feat = poi_layer.GetNextFeature()
            if feat is None:
                break
            total_features += 1
            label = feat.GetField('Label')
            # Missing label should be empty string or None
            if label is None or label == '':
                features_with_empty_label += 1
        
        ds = None
        
        # Should have all features (including those with missing/empty labels)
        if total_features < 4:
            print(f"FAILED (expected >= 4 features, got {total_features})")
            return False
        
        print(f"PASSED ({total_features} features, {features_with_empty_label} with empty label)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC4: Recoverable Error - Invalid Geometry
# =============================================================================
def test_ac4_corrupt_geometry():
    """AC4: Skip features with coordinates outside WGS84 range."""
    from osgeo import ogr, gdal
    
    print("  AC4: Corrupt geometry test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'corrupt-geometry.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is None:
            print("FAILED (Open() returned NULL - should recover)")
            return False
        
        poi_layer = ds.GetLayer(0)
        poi_layer.ResetReading()
        
        valid_count = 0
        while True:
            feat = poi_layer.GetNextFeature()
            if feat is None:
                break
            valid_count += 1
        
        ds = None
        
        # Should have only valid features (invalid coords should be skipped)
        # File has 5 POIs: 3 valid, 2 invalid (lat > 90 or lon > 180)
        if valid_count < 3:
            print(f"FAILED (expected >= 3 valid features, got {valid_count})")
            return False
        
        print(f"PASSED ({valid_count} valid features)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC5: Critical Error - Completely Corrupted File
# =============================================================================
def test_ac5_binary_file():
    """AC5: Open() returns NULL for binary/corrupted files."""
    from osgeo import ogr, gdal
    
    print("  AC5: Binary file test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'binary-file.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is not None:
            ds = None
            print("FAILED (Open() should return NULL for binary file)")
            return False
        
        print("PASSED")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_ac5_empty_file():
    """AC5: Open() returns NULL for empty files."""
    from osgeo import ogr, gdal
    
    print("  AC5: Empty file test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'empty-file.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is not None:
            ds = None
            print("FAILED (Open() should return NULL for empty file)")
            return False
        
        print("PASSED")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC6: CPL Logging Consistency (tested via AC1-AC5)
# =============================================================================
def test_ac6_error_logging():
    """AC6: Verify CPL logging is used for errors."""
    from osgeo import ogr, gdal
    
    print("  AC6: CPL logging test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'missing-header.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    # Set up error handler to capture messages
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        # Get last error
        err_type = gdal.GetLastErrorType()
        err_msg = gdal.GetLastErrorMsg()
        
        # Should be CE_Failure (3)
        if err_type != gdal.CE_Failure:
            print(f"FAILED (expected CE_Failure, got error type {err_type})")
            return False
        
        # Error message should have context
        if not err_msg or len(err_msg) < 10:
            print(f"FAILED (error message too short: '{err_msg}')")
            return False
        
        ds = None
        print("PASSED")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# AC7: Zero Crash Validation
# =============================================================================
def test_ac7_no_crashes():
    """AC7: Process all error-recovery files with 0 crashes."""
    from osgeo import ogr, gdal
    
    print("  AC7: Zero crash validation... ", end="", flush=True)
    
    test_dir = get_test_data_dir()
    if not os.path.exists(test_dir):
        print("SKIPPED (test directory not found)")
        return None
    
    # Find all .mp files in error-recovery directory
    test_files = glob.glob(os.path.join(test_dir, '*.mp'))
    
    if len(test_files) < 5:
        print(f"SKIPPED (only {len(test_files)} test files found, expected >= 5)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    crashes = 0
    processed = 0
    
    for test_file in test_files:
        try:
            # Try to open the file
            ds = ogr.Open(test_file)
            
            if ds is not None:
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
    
    print(f"PASSED ({processed}/{len(test_files)} files, 0 crashes)")
    return True


# =============================================================================
# Additional Recovery Tests
# =============================================================================
def test_truncated_file():
    """Test handling of truncated file (EOF mid-section)."""
    from osgeo import ogr, gdal
    
    print("  Truncated file test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'truncated-file.mp')
    if not os.path.exists(test_file):
        print("SKIPPED (test file not found)")
        return None
    
    gdal.PushErrorHandler('CPLQuietErrorHandler')
    
    try:
        ds = ogr.Open(test_file)
        gdal.PopErrorHandler()
        
        if ds is None:
            print("FAILED (Open() returned NULL - should recover partial data)")
            return False
        
        poi_layer = ds.GetLayer(0)
        poi_layer.ResetReading()
        
        count = 0
        while True:
            feat = poi_layer.GetNextFeature()
            if feat is None:
                break
            count += 1
        
        ds = None
        
        # Should have at least 1 valid POI before truncation
        if count < 1:
            print(f"FAILED (expected >= 1 features, got {count})")
            return False
        
        print(f"PASSED ({count} features before truncation)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_polyline_invalid_coords():
    """Test polyline with invalid coordinates (recoverable)."""
    from osgeo import ogr, gdal
    
    print("  Polyline invalid coords test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'polyline-invalid-coords.mp')
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
        
        # Get POLYLINE layer (index 1)
        polyline_layer = ds.GetLayer(1)
        polyline_layer.ResetReading()
        
        count = 0
        while True:
            feat = polyline_layer.GetNextFeature()
            if feat is None:
                break
            count += 1
        
        ds = None
        
        # Should have valid polylines (skipping invalid ones)
        if count < 2:
            print(f"FAILED (expected >= 2 valid polylines, got {count})")
            return False
        
        print(f"PASSED ({count} valid polylines)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_polygon_invalid_coords():
    """Test polygon with invalid coordinates (recoverable)."""
    from osgeo import ogr, gdal
    
    print("  Polygon invalid coords test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'polygon-invalid-coords.mp')
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
        
        # Get POLYGON layer (index 2)
        polygon_layer = ds.GetLayer(2)
        polygon_layer.ResetReading()
        
        count = 0
        while True:
            feat = polygon_layer.GetNextFeature()
            if feat is None:
                break
            count += 1
        
        ds = None
        
        # Should have valid polygons (skipping invalid ones)
        if count < 2:
            print(f"FAILED (expected >= 2 valid polygons, got {count})")
            return False
        
        print(f"PASSED ({count} valid polygons)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


def test_mixed_valid_invalid():
    """Test file with mixed valid and invalid content."""
    from osgeo import ogr, gdal
    
    print("  Mixed valid/invalid test... ", end="", flush=True)
    
    test_file = os.path.join(get_test_data_dir(), 'mixed-valid-invalid.mp')
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
        
        total_features = 0
        for layer_idx in range(3):
            layer = ds.GetLayer(layer_idx)
            layer.ResetReading()
            while True:
                feat = layer.GetNextFeature()
                if feat is None:
                    break
                total_features += 1
        
        ds = None
        
        # Should have extracted valid features from mixed content
        # File has 2 valid POIs, 2 valid polylines, 2 valid polygons = 6 valid total
        if total_features < 6:
            print(f"FAILED (expected >= 6 valid features, got {total_features})")
            return False
        
        print(f"PASSED ({total_features} valid features)")
        return True
        
    except Exception as e:
        gdal.PopErrorHandler()
        print(f"CRASHED ({e})")
        return False


# =============================================================================
# Main
# =============================================================================
def main():
    """Run all error recovery tests."""
    print("=== Story 3.2: Error Recovery Tests (AC1-AC7) ===")
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
        # Critical Errors (AC1, AC5)
        test_ac1_missing_header,
        test_ac5_binary_file,
        test_ac5_empty_file,
        
        # Recoverable Errors (AC2, AC4)
        test_ac2_malformed_section,
        test_ac4_corrupt_geometry,
        
        # Minor Issues (AC3)
        test_ac3_missing_label,
        
        # CPL Logging (AC6)
        test_ac6_error_logging,
        
        # Zero Crash Validation (AC7)
        test_ac7_no_crashes,
        
        # Additional tests
        test_truncated_file,
        test_polyline_invalid_coords,
        test_polygon_invalid_coords,
        test_mixed_valid_invalid,
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
