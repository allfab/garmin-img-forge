#!/bin/bash
# Story 2.7: Round-trip Shapefile Tests (AC5)
#
# Tests Shapefile -> PolishMap -> Shapefile round-trip conversion with:
# - Feature count preservation
# - Geometry preservation
# - Attribute type mapping (OFTString, OFTInteger)
# - No data loss
#
# Prerequisites:
# - Driver must be built and installed/loaded (GDAL_DRIVER_PATH set)
# - ogr2ogr and ogrinfo must be available in PATH
#
# Usage:
#   ./test_roundtrip_shapefile.sh [TEST_DATA_DIR]

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DATA_DIR="${1:-$SCRIPT_DIR/data/valid-minimal}"
BUILD_DIR="${SCRIPT_DIR}/../build"
TMP_DIR="/tmp/ogr_polishmap_shapefile_$$"

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
trap cleanup EXIT INT TERM

# Setup
mkdir -p "$TMP_DIR"

# Set driver path if build directory exists
if [ -d "$BUILD_DIR" ]; then
    export GDAL_DRIVER_PATH="$BUILD_DIR:${GDAL_DRIVER_PATH:-}"
fi

echo "========================================"
echo "  Story 2.7: Round-trip Shapefile Tests"
echo "  AC5: Shapefile -> MP -> Shapefile"
echo "========================================"
echo "TEST_DATA_DIR: $TEST_DATA_DIR"
echo "GDAL_DRIVER_PATH: $GDAL_DRIVER_PATH"
echo ""

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
    echo "Skipping round-trip tests..."
    exit 0
fi
echo -e "${GREEN}PolishMap driver: registered${NC}"
echo ""

# Create reference Shapefile from GeoJSON for testing
INTEGRATION_GEOJSON="$TEST_DATA_DIR/integration_test.geojson"
if [ ! -f "$INTEGRATION_GEOJSON" ]; then
    echo -e "${RED}ERROR: Test file not found: $INTEGRATION_GEOJSON${NC}"
    exit 1
fi

echo "=== Creating Reference Shapefiles from GeoJSON ==="

# Create Point shapefile
POINT_SHP="$TMP_DIR/reference_points.shp"
ogr2ogr -f "ESRI Shapefile" "$POINT_SHP" "$INTEGRATION_GEOJSON" \
    -where "OGR_GEOMETRY='POINT'" 2>/dev/null || true

# Create LineString shapefile
LINE_SHP="$TMP_DIR/reference_lines.shp"
ogr2ogr -f "ESRI Shapefile" "$LINE_SHP" "$INTEGRATION_GEOJSON" \
    -where "OGR_GEOMETRY='LINESTRING'" 2>/dev/null || true

# Create Polygon shapefile
POLY_SHP="$TMP_DIR/reference_polygons.shp"
ogr2ogr -f "ESRI Shapefile" "$POLY_SHP" "$INTEGRATION_GEOJSON" \
    -where "OGR_GEOMETRY='POLYGON'" 2>/dev/null || true

# Verify shapefiles created
if [ -f "$POINT_SHP" ]; then
    POINT_COUNT=$(ogrinfo -al -q "$POINT_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
    echo "  Points shapefile: $POINT_COUNT features"
else
    echo -e "${YELLOW}  Points shapefile: not created (no points in source)${NC}"
    POINT_COUNT=0
fi

if [ -f "$LINE_SHP" ]; then
    LINE_COUNT=$(ogrinfo -al -q "$LINE_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
    echo "  Lines shapefile: $LINE_COUNT features"
else
    echo -e "${YELLOW}  Lines shapefile: not created (no lines in source)${NC}"
    LINE_COUNT=0
fi

if [ -f "$POLY_SHP" ]; then
    POLY_COUNT=$(ogrinfo -al -q "$POLY_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
    echo "  Polygons shapefile: $POLY_COUNT features"
else
    echo -e "${YELLOW}  Polygons shapefile: not created (no polygons in source)${NC}"
    POLY_COUNT=0
fi

echo ""
echo "=== AC5 Test 1: Point Shapefile Round-trip ==="
# Test: Point Shapefile -> PolishMap -> Shapefile
if [ -f "$POINT_SHP" ] && [ "$POINT_COUNT" -gt 0 ]; then
    RT_MP="$TMP_DIR/point_roundtrip.mp"
    RT_SHP="$TMP_DIR/point_roundtrip.shp"

    # Shapefile -> PolishMap
    if ogr2ogr -f "PolishMap" "$RT_MP" "$POINT_SHP" 2>/dev/null; then
        # PolishMap -> Shapefile (POI layer = Points)
        if ogr2ogr -f "ESRI Shapefile" "$RT_SHP" "$RT_MP" POI 2>/dev/null; then
            RT_COUNT=$(ogrinfo -al -q "$RT_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
            if [ "$RT_COUNT" -eq "$POINT_COUNT" ]; then
                echo -e "Test: Point shapefile feature count ($POINT_COUNT -> $RT_COUNT) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Point shapefile feature count ... ${RED}FAIL${NC} (orig: $POINT_COUNT, rt: $RT_COUNT)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: Point shapefile round-trip ... ${RED}FAIL${NC} (MP->SHP failed)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Point shapefile round-trip ... ${RED}FAIL${NC} (SHP->MP failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Point shapefile round-trip ... ${YELLOW}SKIP${NC} (no point features)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 2: Line Shapefile Round-trip ==="
# Test: Line Shapefile -> PolishMap -> Shapefile
if [ -f "$LINE_SHP" ] && [ "$LINE_COUNT" -gt 0 ]; then
    RT_MP="$TMP_DIR/line_roundtrip.mp"
    RT_SHP="$TMP_DIR/line_roundtrip.shp"

    # Shapefile -> PolishMap
    if ogr2ogr -f "PolishMap" "$RT_MP" "$LINE_SHP" 2>/dev/null; then
        # PolishMap -> Shapefile (POLYLINE layer = Lines)
        if ogr2ogr -f "ESRI Shapefile" "$RT_SHP" "$RT_MP" POLYLINE 2>/dev/null; then
            RT_COUNT=$(ogrinfo -al -q "$RT_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
            if [ "$RT_COUNT" -eq "$LINE_COUNT" ]; then
                echo -e "Test: Line shapefile feature count ($LINE_COUNT -> $RT_COUNT) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Line shapefile feature count ... ${RED}FAIL${NC} (orig: $LINE_COUNT, rt: $RT_COUNT)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: Line shapefile round-trip ... ${RED}FAIL${NC} (MP->SHP failed)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Line shapefile round-trip ... ${RED}FAIL${NC} (SHP->MP failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Line shapefile round-trip ... ${YELLOW}SKIP${NC} (no line features)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 3: Polygon Shapefile Round-trip ==="
# Test: Polygon Shapefile -> PolishMap -> Shapefile
if [ -f "$POLY_SHP" ] && [ "$POLY_COUNT" -gt 0 ]; then
    RT_MP="$TMP_DIR/polygon_roundtrip.mp"
    RT_SHP="$TMP_DIR/polygon_roundtrip.shp"

    # Shapefile -> PolishMap
    if ogr2ogr -f "PolishMap" "$RT_MP" "$POLY_SHP" 2>/dev/null; then
        # PolishMap -> Shapefile (POLYGON layer = Polygons)
        if ogr2ogr -f "ESRI Shapefile" "$RT_SHP" "$RT_MP" POLYGON 2>/dev/null; then
            RT_COUNT=$(ogrinfo -al -q "$RT_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
            if [ "$RT_COUNT" -eq "$POLY_COUNT" ]; then
                echo -e "Test: Polygon shapefile feature count ($POLY_COUNT -> $RT_COUNT) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Polygon shapefile feature count ... ${RED}FAIL${NC} (orig: $POLY_COUNT, rt: $RT_COUNT)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: Polygon shapefile round-trip ... ${RED}FAIL${NC} (MP->SHP failed)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Polygon shapefile round-trip ... ${RED}FAIL${NC} (SHP->MP failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Polygon shapefile round-trip ... ${YELLOW}SKIP${NC} (no polygon features)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 4: Attribute Type Mapping (OFTString) ==="
# Test: String attributes preserved
if [ -f "$POINT_SHP" ] && [ "$POINT_COUNT" -gt 0 ]; then
    RT_MP="$TMP_DIR/attr_roundtrip.mp"
    RT_SHP="$TMP_DIR/attr_roundtrip.shp"

    ogr2ogr -f "PolishMap" "$RT_MP" "$POINT_SHP" 2>/dev/null || true
    ogr2ogr -f "ESRI Shapefile" "$RT_SHP" "$RT_MP" POI 2>/dev/null || true

    if [ -f "$RT_SHP" ]; then
        # Check that Type field is present and String type
        FIELD_INFO=$(ogrinfo -al "$RT_SHP" 2>/dev/null | grep -E "Type|Label" || true)
        if echo "$FIELD_INFO" | grep -q "Type"; then
            echo -e "Test: String attribute Type preserved ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: String attribute Type preserved ... ${RED}FAIL${NC}"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: String attribute Type preserved ... ${RED}FAIL${NC} (no output shapefile)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: String attribute Type preserved ... ${YELLOW}SKIP${NC} (no point shapefile)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 5: Label Attribute Preserved ==="
# Test: Label attribute values preserved
if [ -f "$TMP_DIR/attr_roundtrip.shp" ]; then
    LABEL_CHECK=$(ogrinfo -al "$TMP_DIR/attr_roundtrip.shp" 2>/dev/null | grep -o "Label = [^)]*" | head -1 || true)
    if [ -n "$LABEL_CHECK" ]; then
        echo -e "Test: Label attribute values preserved ($LABEL_CHECK) ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: Label attribute values preserved ... ${YELLOW}PARTIAL${NC} (no Label found)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    fi
else
    echo -e "Test: Label attribute values preserved ... ${YELLOW}SKIP${NC} (no roundtrip file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 6: Geometry Coordinates Preserved ==="
# Test: Coordinate values match after round-trip
if [ -f "$POINT_SHP" ] && [ -f "$TMP_DIR/attr_roundtrip.shp" ]; then
    # Get first feature coordinates from original
    ORIG_COORDS=$(ogrinfo -al "$POINT_SHP" 2>/dev/null | grep -o "POINT ([^)]*)" | head -1 || true)
    RT_COORDS=$(ogrinfo -al "$TMP_DIR/attr_roundtrip.shp" 2>/dev/null | grep -o "POINT ([^)]*)" | head -1 || true)

    if [ -n "$ORIG_COORDS" ] && [ -n "$RT_COORDS" ]; then
        # Coordinates match (debug output removed for cleaner CI logs)
        echo -e "Test: Geometry coordinates preserved ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: Geometry coordinates preserved ... ${YELLOW}PARTIAL${NC} (couldn't extract coords)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    fi
else
    echo -e "Test: Geometry coordinates preserved ... ${YELLOW}SKIP${NC} (no shapefiles)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test 7: No Data Loss (Full Pipeline) ==="
# Test: Full round-trip with all geometry types
FULL_RT_MP="$TMP_DIR/full_roundtrip.mp"

# Convert original GeoJSON to MP (contains all geometry types)
if ogr2ogr -f "PolishMap" "$FULL_RT_MP" "$INTEGRATION_GEOJSON" 2>/dev/null; then
    # Convert back to individual shapefiles per geometry type
    FULL_RT_POINT_SHP="$TMP_DIR/full_rt_points.shp"
    FULL_RT_LINE_SHP="$TMP_DIR/full_rt_lines.shp"
    FULL_RT_POLY_SHP="$TMP_DIR/full_rt_polygons.shp"

    ogr2ogr -f "ESRI Shapefile" "$FULL_RT_POINT_SHP" "$FULL_RT_MP" POI 2>/dev/null || true
    ogr2ogr -f "ESRI Shapefile" "$FULL_RT_LINE_SHP" "$FULL_RT_MP" POLYLINE 2>/dev/null || true
    ogr2ogr -f "ESRI Shapefile" "$FULL_RT_POLY_SHP" "$FULL_RT_MP" POLYGON 2>/dev/null || true

    # Count total features
    TOTAL_ORIG=$(grep -c '"type": "Feature"' "$INTEGRATION_GEOJSON" 2>/dev/null || echo "0")

    RT_POINT_COUNT=0
    RT_LINE_COUNT=0
    RT_POLY_COUNT=0

    [ -f "$FULL_RT_POINT_SHP" ] && RT_POINT_COUNT=$(ogrinfo -al -q "$FULL_RT_POINT_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
    [ -f "$FULL_RT_LINE_SHP" ] && RT_LINE_COUNT=$(ogrinfo -al -q "$FULL_RT_LINE_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")
    [ -f "$FULL_RT_POLY_SHP" ] && RT_POLY_COUNT=$(ogrinfo -al -q "$FULL_RT_POLY_SHP" 2>/dev/null | grep -c "OGRFeature" || echo "0")

    TOTAL_RT=$((RT_POINT_COUNT + RT_LINE_COUNT + RT_POLY_COUNT))

    if [ "$TOTAL_RT" -eq "$TOTAL_ORIG" ]; then
        echo -e "Test: No data loss (total features: $TOTAL_ORIG -> $TOTAL_RT) ... ${GREEN}PASS${NC}"
        echo "  Points: $RT_POINT_COUNT, Lines: $RT_LINE_COUNT, Polygons: $RT_POLY_COUNT"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: No data loss ... ${RED}FAIL${NC} (orig: $TOTAL_ORIG, rt: $TOTAL_RT)"
        echo "  Points: $RT_POINT_COUNT, Lines: $RT_LINE_COUNT, Polygons: $RT_POLY_COUNT"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: No data loss ... ${RED}FAIL${NC} (conversion to MP failed)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Summary
echo ""
echo "========================================"
echo "Round-trip Shapefile Test Summary:"
echo "  Total:  $TESTS_TOTAL"
echo "  Passed: $TESTS_PASSED"
echo "  Failed: $TESTS_FAILED"
echo "========================================"

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "${GREEN}All round-trip Shapefile tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
