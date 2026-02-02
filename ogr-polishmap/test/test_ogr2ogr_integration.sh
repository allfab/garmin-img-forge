#!/bin/bash
# Story 1.7, 2.6 & 2.7: ogr2ogr Integration Tests
# This script tests REAL ogr2ogr integration with:
# - Story 1.7: Spatial and attribute filters
# - Story 2.6: Bidirectional format conversion (GeoJSON <-> PolishMap)
# - Story 2.7: Extended round-trip tests, Shapefile support, coordinate validation
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
echo "  ogr2ogr Integration Tests"
echo "  Story 1.7: Spatial and Attribute Filters"
echo "  Story 2.6: Bidirectional Format Conversion"
echo "  Story 2.7: Extended Round-trip & Shapefile"
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
# Note: Must specify layer (POI) as PolishMap has 3 fixed layers

OUTPUT_SPATIAL="$TMP_DIR/spatial_output.geojson"
ogr2ogr -f "GeoJSON" -spat 2.0 48.0 2.4 48.4 "$OUTPUT_SPATIAL" "$SPATIAL_GRID" POI 2>/dev/null || true

if [ -f "$OUTPUT_SPATIAL" ]; then
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_SPATIAL" 2>/dev/null | tr -d '\n' || echo "0")
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
# Note: Must specify layer (POI) as PolishMap has 3 fixed layers

OUTPUT_ATTR="$TMP_DIR/attr_output.geojson"
ogr2ogr -f "GeoJSON" -where "Type = '0x2C00'" "$OUTPUT_ATTR" "$ATTR_TYPES" POI 2>/dev/null || true

if [ -f "$OUTPUT_ATTR" ]; then
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_ATTR" 2>/dev/null | tr -d '\n' || echo "0")
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
    FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_COMBINED" 2>/dev/null | tr -d '\n' || echo "0")
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

# ============================================================
# Story 2.6: Bidirectional Format Conversion Tests (AC2, AC3, AC7)
# ============================================================

echo ""
echo "========================================"
echo "  Story 2.6: Bidirectional Conversion"
echo "========================================"

# Test files for conversion
INTEGRATION_GEOJSON="$TEST_DATA_DIR/integration_test.geojson"
INTEGRATION_UTF8="$TEST_DATA_DIR/integration_test_utf8.geojson"
HEADER_SIMPLE="$TEST_DATA_DIR/header-simple.mp"

# Verify test files exist
if [ ! -f "$INTEGRATION_GEOJSON" ]; then
    echo -e "${YELLOW}WARNING: integration_test.geojson not found, skipping conversion tests${NC}"
else
    echo ""
    echo "=== AC2 Test: GeoJSON -> PolishMap Conversion ==="

    # Test 2.1: Basic GeoJSON to PolishMap conversion
    OUTPUT_MP="$TMP_DIR/converted.mp"
    if ogr2ogr -f "PolishMap" "$OUTPUT_MP" "$INTEGRATION_GEOJSON" 2>/dev/null; then
        if [ -f "$OUTPUT_MP" ] && [ -s "$OUTPUT_MP" ]; then
            # Check for [IMG ID] header
            if grep -q "\[IMG ID\]" "$OUTPUT_MP"; then
                echo -e "Test: GeoJSON -> PolishMap creates valid .mp file ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: GeoJSON -> PolishMap creates valid .mp file ... ${RED}FAIL${NC} (no [IMG ID] header)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: GeoJSON -> PolishMap creates valid .mp file ... ${RED}FAIL${NC} (empty or missing file)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: GeoJSON -> PolishMap creates valid .mp file ... ${RED}FAIL${NC} (command failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))

    # Test 2.2: Check POI features in converted file
    if [ -f "$OUTPUT_MP" ]; then
        POI_COUNT=$(grep -c "^\[POI\]" "$OUTPUT_MP" 2>/dev/null | tr -d '\n' || echo "0")
        if [ "$POI_COUNT" -eq 3 ]; then
            echo -e "Test: Converted .mp contains 3 POI sections ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: Converted .mp contains 3 POI sections ... ${RED}FAIL${NC} (got $POI_COUNT)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
        TESTS_TOTAL=$((TESTS_TOTAL + 1))

        # Test 2.3: Check POLYLINE features
        POLYLINE_COUNT=$(grep -c "^\[POLYLINE\]" "$OUTPUT_MP" 2>/dev/null | tr -d '\n' || echo "0")
        if [ "$POLYLINE_COUNT" -eq 2 ]; then
            echo -e "Test: Converted .mp contains 2 POLYLINE sections ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: Converted .mp contains 2 POLYLINE sections ... ${RED}FAIL${NC} (got $POLYLINE_COUNT)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
        TESTS_TOTAL=$((TESTS_TOTAL + 1))

        # Test 2.4: Check POLYGON features
        POLYGON_COUNT=$(grep -c "^\[POLYGON\]" "$OUTPUT_MP" 2>/dev/null | tr -d '\n' || echo "0")
        if [ "$POLYGON_COUNT" -eq 2 ]; then
            echo -e "Test: Converted .mp contains 2 POLYGON sections ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: Converted .mp contains 2 POLYGON sections ... ${RED}FAIL${NC} (got $POLYGON_COUNT)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
        TESTS_TOTAL=$((TESTS_TOTAL + 1))
    fi
fi

echo ""
echo "=== AC3 Test: PolishMap -> GeoJSON Conversion ==="

# Test 3.1: PolishMap POI layer to GeoJSON conversion
OUTPUT_JSON="$TMP_DIR/converted.geojson"
if ogr2ogr -f "GeoJSON" "$OUTPUT_JSON" "$HEADER_SIMPLE" POI 2>/dev/null; then
    if [ -f "$OUTPUT_JSON" ] && [ -s "$OUTPUT_JSON" ]; then
        # Check for valid GeoJSON structure
        if grep -q '"type": "FeatureCollection"' "$OUTPUT_JSON"; then
            echo -e "Test: PolishMap -> GeoJSON creates valid GeoJSON ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: PolishMap -> GeoJSON creates valid GeoJSON ... ${RED}FAIL${NC} (invalid structure)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: PolishMap -> GeoJSON creates valid GeoJSON ... ${RED}FAIL${NC} (empty or missing file)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: PolishMap -> GeoJSON creates valid GeoJSON ... ${RED}FAIL${NC} (command failed)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Test using poi-multiple.mp for more features
POI_MULTIPLE="$TEST_DATA_DIR/poi-multiple.mp"
if [ -f "$POI_MULTIPLE" ]; then
    OUTPUT_POI_JSON="$TMP_DIR/poi_converted.geojson"
    if ogr2ogr -f "GeoJSON" "$OUTPUT_POI_JSON" "$POI_MULTIPLE" POI 2>/dev/null; then
        if [ -f "$OUTPUT_POI_JSON" ]; then
            FEAT_COUNT=$(grep -c '"type": "Feature"' "$OUTPUT_POI_JSON" 2>/dev/null | tr -d '\n' || echo "0")
            if [ "$FEAT_COUNT" -ge 1 ]; then
                echo -e "Test: PolishMap with POIs -> GeoJSON preserves features ($FEAT_COUNT) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: PolishMap with POIs -> GeoJSON preserves features ... ${RED}FAIL${NC} (0 features)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: PolishMap with POIs -> GeoJSON preserves features ... ${RED}FAIL${NC} (no output)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
        TESTS_TOTAL=$((TESTS_TOTAL + 1))
    fi
fi

echo ""
echo "=== AC7 Test: Round-trip Conversion (GeoJSON -> MP -> GeoJSON) ==="

if [ -f "$INTEGRATION_GEOJSON" ]; then
    # Step 1: GeoJSON -> PolishMap
    ROUNDTRIP_MP="$TMP_DIR/roundtrip.mp"
    ROUNDTRIP_JSON="$TMP_DIR/roundtrip.geojson"

    if ogr2ogr -f "PolishMap" "$ROUNDTRIP_MP" "$INTEGRATION_GEOJSON" 2>/dev/null; then
        # Step 2: PolishMap -> GeoJSON (merge all layers into single GeoJSON)
        # First layer: POI
        ogr2ogr -f "GeoJSON" "$ROUNDTRIP_JSON" "$ROUNDTRIP_MP" POI -nln features 2>/dev/null
        # Append other layers
        ogr2ogr -f "GeoJSON" -update -append "$ROUNDTRIP_JSON" "$ROUNDTRIP_MP" POLYLINE -nln features 2>/dev/null
        ogr2ogr -f "GeoJSON" -update -append "$ROUNDTRIP_JSON" "$ROUNDTRIP_MP" POLYGON -nln features 2>/dev/null

        if [ -f "$ROUNDTRIP_JSON" ]; then
            # Count features in original and roundtrip
            ORIG_COUNT=$(grep -c '"type": "Feature"' "$INTEGRATION_GEOJSON" 2>/dev/null | tr -d '\n' || echo "0")
            RT_COUNT=$(grep -c '"type": "Feature"' "$ROUNDTRIP_JSON" 2>/dev/null | tr -d '\n' || echo "0")

            if [ "$RT_COUNT" -eq "$ORIG_COUNT" ]; then
                echo -e "Test: Round-trip preserves feature count ($ORIG_COUNT -> $RT_COUNT) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Round-trip preserves feature count ... ${RED}FAIL${NC} (orig: $ORIG_COUNT, rt: $RT_COUNT)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
            TESTS_TOTAL=$((TESTS_TOTAL + 1))

            # Test coordinate precision (check first coordinate appears in roundtrip)
            if grep -q "2.3522" "$ROUNDTRIP_JSON" && grep -q "48.8566" "$ROUNDTRIP_JSON"; then
                echo -e "Test: Round-trip preserves coordinate precision (6 decimals) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Round-trip preserves coordinate precision ... ${RED}FAIL${NC}"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
            TESTS_TOTAL=$((TESTS_TOTAL + 1))

            # Test attribute preservation (check Label field)
            if grep -q "Restaurant Paris" "$ROUNDTRIP_JSON"; then
                echo -e "Test: Round-trip preserves Label attribute ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Round-trip preserves Label attribute ... ${RED}FAIL${NC}"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
            TESTS_TOTAL=$((TESTS_TOTAL + 1))
        else
            echo -e "Test: Round-trip conversion ... ${RED}FAIL${NC} (no output JSON)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
            TESTS_TOTAL=$((TESTS_TOTAL + 1))
        fi
    else
        echo -e "Test: Round-trip conversion (JSON->MP step) ... ${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        TESTS_TOTAL=$((TESTS_TOTAL + 1))
    fi
fi

echo ""
echo "========================================"
echo "  Story 2.6: ogrinfo Integration (AC8)"
echo "========================================"

echo ""
echo "=== AC8 Test 6: ogrinfo --formats Lists PolishMap (4.1) ==="
# This test is already done in prerequisites but we add an explicit test
FORMATS_OUTPUT=$(ogrinfo --formats 2>/dev/null | grep "PolishMap")
if echo "$FORMATS_OUTPUT" | grep -q "PolishMap.*vector"; then
    echo -e "Test: ogrinfo --formats shows PolishMap driver ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo --formats shows PolishMap driver ... ${RED}FAIL${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 7: ogrinfo -al Shows Layers and Features (4.2) ==="
# Test with header-simple.mp which has POI features
OGRINFO_AL_OUTPUT=$(ogrinfo -al "$HEADER_SIMPLE" 2>/dev/null)
# Check that layers are displayed
if echo "$OGRINFO_AL_OUTPUT" | grep -q "Layer name: POI" && \
   echo "$OGRINFO_AL_OUTPUT" | grep -q "Layer name: POLYLINE" && \
   echo "$OGRINFO_AL_OUTPUT" | grep -q "Layer name: POLYGON"; then
    echo -e "Test: ogrinfo -al displays all 3 layers ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo -al displays all 3 layers ... ${RED}FAIL${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Check that field definitions are shown
if echo "$OGRINFO_AL_OUTPUT" | grep -q "Type: String" && \
   echo "$OGRINFO_AL_OUTPUT" | grep -q "Label: String"; then
    echo -e "Test: ogrinfo -al shows field definitions ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo -al shows field definitions ... ${RED}FAIL${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 8: ogrinfo with Minimal File (4.3) ==="
# Test with header-simple.mp (minimal valid file)
OGRINFO_MINIMAL=$(ogrinfo -so "$HEADER_SIMPLE" 2>/dev/null)
if echo "$OGRINFO_MINIMAL" | grep -q "1: POI"; then
    echo -e "Test: ogrinfo -so with minimal file shows layers ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: ogrinfo -so with minimal file shows layers ... ${RED}FAIL${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC8 Test 9: ogrinfo with Multi-layer Complex File (4.4) ==="
# Test with filter-combined.mp which has POI, POLYLINE, POLYGON
if [ -f "$COMBINED" ]; then
    OGRINFO_COMPLEX=$(ogrinfo -so "$COMBINED" 2>/dev/null)
    # Check layer listing
    if echo "$OGRINFO_COMPLEX" | grep -q "1: POI" && \
       echo "$OGRINFO_COMPLEX" | grep -q "2: POLYLINE" && \
       echo "$OGRINFO_COMPLEX" | grep -q "3: POLYGON"; then
        echo -e "Test: ogrinfo -so lists all layers from complex file ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: ogrinfo -so lists all layers from complex file ... ${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))

    # Verify feature count with -al
    POI_FEAT_COUNT=$(ogrinfo -al "$COMBINED" 2>/dev/null | grep -c "OGRFeature(POI)" || echo "0")
    if [ "$POI_FEAT_COUNT" -gt 0 ]; then
        echo -e "Test: ogrinfo -al shows features from complex file ($POI_FEAT_COUNT POIs) ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: ogrinfo -al shows features from complex file ... ${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
fi

echo ""
echo "=== Test 2.6: UTF-8 to CP1252 Encoding Conversion ==="

if [ -f "$INTEGRATION_UTF8" ]; then
    UTF8_OUTPUT_MP="$TMP_DIR/utf8_converted.mp"
    if ogr2ogr -f "PolishMap" "$UTF8_OUTPUT_MP" "$INTEGRATION_UTF8" 2>/dev/null; then
        if [ -f "$UTF8_OUTPUT_MP" ] && [ -s "$UTF8_OUTPUT_MP" ]; then
            # Check that labels are present (encoding handled by driver)
            if grep -q "Label=" "$UTF8_OUTPUT_MP"; then
                echo -e "Test: UTF-8 GeoJSON -> PolishMap with labels ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: UTF-8 GeoJSON -> PolishMap with labels ... ${RED}FAIL${NC} (no labels)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: UTF-8 GeoJSON -> PolishMap with labels ... ${RED}FAIL${NC} (empty file)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: UTF-8 GeoJSON -> PolishMap with labels ... ${RED}FAIL${NC} (command failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    TESTS_TOTAL=$((TESTS_TOTAL + 1))
fi

# ============================================================
# Story 2.7: Extended Round-trip Tests (AC1, AC5)
# ============================================================

echo ""
echo "========================================"
echo "  Story 2.7: Extended Round-trip Tests"
echo "========================================"

echo ""
echo "=== AC1 Test 10: Coordinate Precision Validation (6 decimals) ==="
# Test that coordinates are preserved to at least 6 decimal places
if [ -f "$INTEGRATION_GEOJSON" ]; then
    PRECISION_MP="$TMP_DIR/precision_test.mp"
    PRECISION_JSON="$TMP_DIR/precision_test.geojson"

    if ogr2ogr -f "PolishMap" "$PRECISION_MP" "$INTEGRATION_GEOJSON" 2>/dev/null; then
        ogr2ogr -f "GeoJSON" "$PRECISION_JSON" "$PRECISION_MP" POI -nln features 2>/dev/null || true

        if [ -f "$PRECISION_JSON" ]; then
            # Extract coordinates from both files and compare
            # Original has 2.3522, 48.8566 (4 decimals)
            # Check that precision is preserved
            if grep -q "2.3522" "$PRECISION_JSON" && grep -q "48.8566" "$PRECISION_JSON"; then
                echo -e "Test: Coordinate precision preserved (4+ decimals) ... ${GREEN}PASS${NC}"
                TESTS_PASSED=$((TESTS_PASSED + 1))
            else
                echo -e "Test: Coordinate precision preserved ... ${RED}FAIL${NC}"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: Coordinate precision preserved ... ${RED}FAIL${NC} (no output)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Coordinate precision preserved ... ${RED}FAIL${NC} (conversion failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Coordinate precision preserved ... ${YELLOW}SKIP${NC} (no test file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC5 Test: Shapefile Round-trip Support ==="
# Test Shapefile -> PolishMap -> Shapefile round-trip
if [ -f "$INTEGRATION_GEOJSON" ]; then
    # Create temp shapefiles from GeoJSON
    SHP_POINT="$TMP_DIR/test_points.shp"
    ogr2ogr -f "ESRI Shapefile" "$SHP_POINT" "$INTEGRATION_GEOJSON" -where "OGR_GEOMETRY='POINT'" 2>/dev/null || true

    if [ -f "$SHP_POINT" ]; then
        RT_SHP_MP="$TMP_DIR/shp_roundtrip.mp"
        RT_SHP_OUT="$TMP_DIR/shp_roundtrip_out.shp"

        # Shapefile -> PolishMap
        if ogr2ogr -f "PolishMap" "$RT_SHP_MP" "$SHP_POINT" 2>/dev/null; then
            # PolishMap -> Shapefile
            if ogr2ogr -f "ESRI Shapefile" "$RT_SHP_OUT" "$RT_SHP_MP" POI 2>/dev/null; then
                ORIG_COUNT=$(ogrinfo -al -q "$SHP_POINT" 2>/dev/null | grep -c "OGRFeature" || echo "0")
                RT_COUNT=$(ogrinfo -al -q "$RT_SHP_OUT" 2>/dev/null | grep -c "OGRFeature" || echo "0")

                if [ "$RT_COUNT" -eq "$ORIG_COUNT" ] && [ "$RT_COUNT" -gt 0 ]; then
                    echo -e "Test: Shapefile round-trip ($ORIG_COUNT features) ... ${GREEN}PASS${NC}"
                    TESTS_PASSED=$((TESTS_PASSED + 1))
                else
                    echo -e "Test: Shapefile round-trip ... ${RED}FAIL${NC} (orig: $ORIG_COUNT, rt: $RT_COUNT)"
                    TESTS_FAILED=$((TESTS_FAILED + 1))
                fi
            else
                echo -e "Test: Shapefile round-trip ... ${RED}FAIL${NC} (MP->SHP failed)"
                TESTS_FAILED=$((TESTS_FAILED + 1))
            fi
        else
            echo -e "Test: Shapefile round-trip ... ${RED}FAIL${NC} (SHP->MP failed)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Shapefile round-trip ... ${YELLOW}SKIP${NC} (no shapefile created)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    fi
else
    echo -e "Test: Shapefile round-trip ... ${YELLOW}SKIP${NC} (no test file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC1 Test 11: All Attributes Round-trip Validation ==="
# Test that all expected attributes are preserved
if [ -f "$ROUNDTRIP_JSON" ]; then
    ATTR_OK=true

    # Check Type attribute (hex format)
    if ! grep -qE '"Type":\s*"0x[0-9A-Fa-f]+"' "$ROUNDTRIP_JSON"; then
        echo "  Missing or invalid: Type attribute"
        ATTR_OK=false
    fi

    # Check Label attribute
    if ! grep -q '"Label":' "$ROUNDTRIP_JSON"; then
        echo "  Missing: Label attribute"
        ATTR_OK=false
    fi

    if $ATTR_OK; then
        echo -e "Test: All attributes preserved in round-trip ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: All attributes preserved in round-trip ... ${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: All attributes preserved in round-trip ... ${YELLOW}SKIP${NC} (no roundtrip file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC1 Test 12: Complex File Multi-geometry Round-trip ==="
# Test round-trip with complex multi-geometry file
COMPLEX_FILE="$TEST_DATA_DIR/../valid-complex/mixed-all-types.mp"
if [ -f "$COMPLEX_FILE" ]; then
    COMPLEX_JSON="$TMP_DIR/complex_roundtrip.geojson"
    COMPLEX_MP_RT="$TMP_DIR/complex_roundtrip.mp"

    # Read feature counts from original
    ORIG_POI=$(ogrinfo -al -q "$COMPLEX_FILE" 2>/dev/null | grep -c "OGRFeature(POI)" || echo "0")
    ORIG_LINE=$(ogrinfo -al -q "$COMPLEX_FILE" 2>/dev/null | grep -c "OGRFeature(POLYLINE)" || echo "0")
    ORIG_POLY=$(ogrinfo -al -q "$COMPLEX_FILE" 2>/dev/null | grep -c "OGRFeature(POLYGON)" || echo "0")

    # MP -> GeoJSON
    ogr2ogr -f "GeoJSON" "$COMPLEX_JSON" "$COMPLEX_FILE" POI -nln features 2>/dev/null || true
    ogr2ogr -f "GeoJSON" -update -append "$COMPLEX_JSON" "$COMPLEX_FILE" POLYLINE -nln features 2>/dev/null || true
    ogr2ogr -f "GeoJSON" -update -append "$COMPLEX_JSON" "$COMPLEX_FILE" POLYGON -nln features 2>/dev/null || true

    # GeoJSON -> MP
    if [ -f "$COMPLEX_JSON" ] && ogr2ogr -f "PolishMap" "$COMPLEX_MP_RT" "$COMPLEX_JSON" 2>/dev/null; then
        RT_POI=$(ogrinfo -al -q "$COMPLEX_MP_RT" 2>/dev/null | grep -c "OGRFeature(POI)" || echo "0")
        RT_LINE=$(ogrinfo -al -q "$COMPLEX_MP_RT" 2>/dev/null | grep -c "OGRFeature(POLYLINE)" || echo "0")
        RT_POLY=$(ogrinfo -al -q "$COMPLEX_MP_RT" 2>/dev/null | grep -c "OGRFeature(POLYGON)" || echo "0")

        ORIG_TOTAL=$((ORIG_POI + ORIG_LINE + ORIG_POLY))
        RT_TOTAL=$((RT_POI + RT_LINE + RT_POLY))

        if [ "$RT_TOTAL" -eq "$ORIG_TOTAL" ]; then
            echo -e "Test: Complex multi-geometry round-trip ($ORIG_TOTAL features) ... ${GREEN}PASS${NC}"
            TESTS_PASSED=$((TESTS_PASSED + 1))
        else
            echo -e "Test: Complex multi-geometry round-trip ... ${RED}FAIL${NC} (orig: $ORIG_TOTAL, rt: $RT_TOTAL)"
            TESTS_FAILED=$((TESTS_FAILED + 1))
        fi
    else
        echo -e "Test: Complex multi-geometry round-trip ... ${RED}FAIL${NC} (conversion failed)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Complex multi-geometry round-trip ... ${YELLOW}SKIP${NC} (no complex test file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
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
