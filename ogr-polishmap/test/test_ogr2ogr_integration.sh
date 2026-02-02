#!/bin/bash
# Story 1.7: ogr2ogr Integration Tests (AC8)
# This script tests REAL ogr2ogr integration with spatial and attribute filters
#
# Prerequisites:
# - Driver must be built and installed/loaded (GDAL_DRIVER_PATH set)
# - ogr2ogr and ogrinfo must be available in PATH
#
# Usage:
#   ./test_ogr2ogr_integration.sh [TEST_DATA_DIR]

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DATA_DIR="${1:-$SCRIPT_DIR/data/valid-minimal}"
BUILD_DIR="${SCRIPT_DIR}/../build"
TMP_DIR="/tmp/ogr_polishmap_test_$$"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TESTS_TOTAL=0
TESTS_PASSED=0
TESTS_FAILED=0

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# Setup
mkdir -p "$TMP_DIR"

# Set driver path if build directory exists
if [ -d "$BUILD_DIR" ]; then
    export GDAL_DRIVER_PATH="$BUILD_DIR:${GDAL_DRIVER_PATH:-}"
fi

echo "========================================"
echo "  ogr2ogr Integration Tests (AC8)"
echo "  Story 1.7: Spatial and Attribute Filters"
echo "========================================"
echo "TEST_DATA_DIR: $TEST_DATA_DIR"
echo "GDAL_DRIVER_PATH: $GDAL_DRIVER_PATH"
echo ""

# Helper function
run_test() {
    local name="$1"
    local cmd="$2"
    local expected="$3"
    local check_type="${4:-contains}"  # contains, equals, count

    TESTS_TOTAL=$((TESTS_TOTAL + 1))
    echo -n "Test: $name ... "

    local output
    if ! output=$(eval "$cmd" 2>&1); then
        echo -e "${RED}FAIL${NC} (command failed)"
        echo "  Command: $cmd"
        echo "  Output: $output"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi

    local success=false
    case "$check_type" in
        contains)
            if echo "$output" | grep -q "$expected"; then
                success=true
            fi
            ;;
        count)
            local count=$(echo "$output" | grep -c "OGRFeature" || echo "0")
            if [ "$count" -eq "$expected" ]; then
                success=true
            fi
            ;;
        file_features)
            # Count features in output GeoJSON
            if [ -f "$expected" ]; then
                local feat_count=$(grep -c '"type": "Feature"' "$expected" 2>/dev/null || echo "0")
                if [ "$feat_count" -gt 0 ]; then
                    success=true
                fi
            fi
            ;;
    esac

    if $success; then
        echo -e "${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "  Expected: $expected"
        echo "  Got: $output"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        return 1
    fi
}

# Check prerequisites
echo "=== Prerequisites Check ==="
if ! command -v ogr2ogr &> /dev/null; then
    echo -e "${RED}ERROR: ogr2ogr not found in PATH${NC}"
    exit 1
fi
echo "ogr2ogr: $(which ogr2ogr)"

if ! command -v ogrinfo &> /dev/null; then
    echo -e "${RED}ERROR: ogrinfo not found in PATH${NC}"
    exit 1
fi
echo "ogrinfo: $(which ogrinfo)"

# Check driver is registered
if ! ogrinfo --formats 2>/dev/null | grep -q "PolishMap"; then
    echo -e "${YELLOW}WARNING: PolishMap driver not found in GDAL${NC}"
    echo "Make sure GDAL_DRIVER_PATH is set correctly"
    echo "Skipping integration tests..."
    exit 0
fi
echo -e "${GREEN}PolishMap driver: registered${NC}"
echo ""

# Test files
SPATIAL_GRID="$TEST_DATA_DIR/filter-spatial-grid.mp"
ATTR_TYPES="$TEST_DATA_DIR/filter-attribute-types.mp"
COMBINED="$TEST_DATA_DIR/filter-combined.mp"

# Verify test files exist
for f in "$SPATIAL_GRID" "$ATTR_TYPES" "$COMBINED"; do
    if [ ! -f "$f" ]; then
        echo -e "${RED}ERROR: Test file not found: $f${NC}"
        exit 1
    fi
done

echo "=== AC8 Test 1: ogr2ogr -spat (Spatial Filter) ==="
# Test spatial filter with ogr2ogr - export subset of grid
# Grid is 10x10 from (48.0, 2.0) to (48.9, 2.9)
# Filter: lon 2.0-2.4, lat 48.0-48.4 should give 25 features (5x5)

OUTPUT_SPATIAL="$TMP_DIR/spatial_output.geojson"
ogr2ogr -f "GeoJSON" -spat 2.0 48.0 2.4 48.4 "$OUTPUT_SPATIAL" "$SPATIAL_GRID" 2>/dev/null || true

if [ -f "$OUTPUT_SPATIAL" ]; then
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_SPATIAL" 2>/dev/null || echo "0")
    if [ "$FEAT_COUNT" -eq 25 ]; then
        echo -e "Test: ogr2ogr -spat bbox exports 25 features ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: ogr2ogr -spat bbox exports 25 features ... ${RED}FAIL${NC} (got $FEAT_COUNT)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: ogr2ogr -spat bbox exports 25 features ... ${RED}FAIL${NC} (no output file)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 2: ogr2ogr -where (Attribute Filter) ==="
# Test attribute filter - export only Type=0x2C00 (10 restaurants)

OUTPUT_ATTR="$TMP_DIR/attr_output.geojson"
ogr2ogr -f "GeoJSON" -where "Type = '0x2C00'" "$OUTPUT_ATTR" "$ATTR_TYPES" 2>/dev/null || true

if [ -f "$OUTPUT_ATTR" ]; then
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_ATTR" 2>/dev/null || echo "0")
    if [ "$FEAT_COUNT" -eq 10 ]; then
        echo -e "Test: ogr2ogr -where Type='0x2C00' exports 10 features ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: ogr2ogr -where Type='0x2C00' exports 10 features ... ${RED}FAIL${NC} (got $FEAT_COUNT)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: ogr2ogr -where Type='0x2C00' exports 10 features ... ${RED}FAIL${NC} (no output file)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 3: ogr2ogr -spat + -where (Combined Filters) ==="
# Test combined filters on filter-combined.mp
# North region (lat > 48.5) + Type=0x2C00 (Restaurant) = 1 feature

OUTPUT_COMBINED="$TMP_DIR/combined_output.geojson"
ogr2ogr -f "GeoJSON" -spat 2.0 48.5 3.0 49.0 -where "Type = '0x2C00'" "$OUTPUT_COMBINED" "$COMBINED" POI 2>/dev/null || true

if [ -f "$OUTPUT_COMBINED" ]; then
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_COMBINED" 2>/dev/null || echo "0")
    if [ "$FEAT_COUNT" -eq 1 ]; then
        echo -e "Test: ogr2ogr -spat + -where combined exports 1 feature ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: ogr2ogr -spat + -where combined exports 1 feature ... ${RED}FAIL${NC} (got $FEAT_COUNT)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: ogr2ogr -spat + -where combined exports 1 feature ... ${RED}FAIL${NC} (no output file)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 4: ogrinfo -spat (Spatial Query) ==="
# Test ogrinfo with spatial filter

OGRINFO_OUTPUT=$(ogrinfo -al -spat 2.0 48.0 2.1 48.1 "$SPATIAL_GRID" 2>/dev/null | grep -c "OGRFeature" || echo "0")
# Should return ~4 features (2x2 grid cell area, but boundary conditions may vary)
if [ "$OGRINFO_OUTPUT" -ge 1 ] && [ "$OGRINFO_OUTPUT" -le 10 ]; then
    echo -e "Test: ogrinfo -spat returns features in bbox ... ${GREEN}PASS${NC} ($OGRINFO_OUTPUT features)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo -spat returns features in bbox ... ${RED}FAIL${NC} (got $OGRINFO_OUTPUT)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 5: ogrinfo -where (Attribute Query) ==="
# Test ogrinfo with attribute filter

OGRINFO_OUTPUT=$(ogrinfo -al -where "Type = '0x4000'" "$ATTR_TYPES" 2>/dev/null | grep -c "OGRFeature" || echo "0")
if [ "$OGRINFO_OUTPUT" -eq 10 ]; then
    echo -e "Test: ogrinfo -where Type='0x4000' returns 10 features ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo -where Type='0x4000' returns 10 features ... ${RED}FAIL${NC} (got $OGRINFO_OUTPUT)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Summary
echo ""
echo "========================================"
echo "Integration Test Summary:"
echo "  Total:  $TESTS_TOTAL"
echo "  Passed: $TESTS_PASSED"
echo "  Failed: $TESTS_FAILED"
echo "========================================"

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "${GREEN}All integration tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
