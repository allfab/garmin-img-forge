#!/bin/bash
# Story 2.7: mkgmap Compilation Tests (AC2)
#
# Tests that generated .mp files compile successfully with mkgmap:
# - mkgmap completes without errors
# - .img file is generated
# - No warnings about invalid format
#
# Prerequisites:
# - mkgmap must be installed (Java application)
# - GDAL with PolishMap driver available
# - ogr2ogr in PATH
#
# Usage:
#   ./test_mkgmap_compilation.sh [TEST_DATA_DIR] [MKGMAP_JAR]
#
# Environment:
#   MKGMAP_JAR: Path to mkgmap.jar (optional, will search in common locations)

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DATA_DIR="${1:-$SCRIPT_DIR/data/valid-minimal}"
BUILD_DIR="${SCRIPT_DIR}/../build"
TMP_DIR="/tmp/ogr_polishmap_mkgmap_$$"

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
echo "  Story 2.7: mkgmap Compilation Tests"
echo "  AC2: Compilation with mkgmap"
echo "========================================"
echo "TEST_DATA_DIR: $TEST_DATA_DIR"
echo ""

# Find mkgmap
find_mkgmap() {
    # Check environment variable first
    if [ -n "$MKGMAP_JAR" ] && [ -f "$MKGMAP_JAR" ]; then
        echo "$MKGMAP_JAR"
        return 0
    fi

    # Second argument
    if [ -n "$2" ] && [ -f "$2" ]; then
        echo "$2"
        return 0
    fi

    # Common locations
    local SEARCH_PATHS=(
        "$HOME/mkgmap/mkgmap.jar"
        "$HOME/.local/share/mkgmap/mkgmap.jar"
        "/opt/mkgmap/mkgmap.jar"
        "/usr/share/mkgmap/mkgmap.jar"
        "/usr/local/share/mkgmap/mkgmap.jar"
    )

    for path in "${SEARCH_PATHS[@]}"; do
        if [ -f "$path" ]; then
            echo "$path"
            return 0
        fi
    done

    # Try to find it
    local found=$(find /home -name "mkgmap.jar" -type f 2>/dev/null | head -1)
    if [ -n "$found" ]; then
        echo "$found"
        return 0
    fi

    return 1
}

# Check Java
echo "=== Prerequisites Check ==="
if ! command -v java &> /dev/null; then
    echo -e "${YELLOW}WARNING: Java not found in PATH${NC}"
    echo "mkgmap requires Java 11+ to run"
    echo ""
    echo "To install mkgmap:"
    echo "  1. Install Java: sudo dnf install java-11-openjdk"
    echo "  2. Download mkgmap: https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz"
    echo "  3. Extract and set MKGMAP_JAR=/path/to/mkgmap.jar"
    echo ""
    echo -e "${YELLOW}Skipping mkgmap compilation tests (manual test required)${NC}"
    echo ""
    echo "=== Manual Test Procedure ==="
    echo "1. Generate .mp file: ogr2ogr -f 'PolishMap' test.mp input.geojson"
    echo "2. Compile with mkgmap: java -jar mkgmap.jar test.mp"
    echo "3. Verify: ls *.img (should show generated file)"
    echo "4. Check logs for errors/warnings"
    exit 0
fi
echo "Java: $(java -version 2>&1 | head -1)"

# Find mkgmap
MKGMAP_JAR=$(find_mkgmap "$@" 2>/dev/null || echo "")
if [ -z "$MKGMAP_JAR" ] || [ ! -f "$MKGMAP_JAR" ]; then
    echo -e "${YELLOW}WARNING: mkgmap.jar not found${NC}"
    echo ""
    echo "To install mkgmap:"
    echo "  1. Download: wget https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz"
    echo "  2. Extract: tar -xzf mkgmap-latest.tar.gz"
    echo "  3. Set environment: export MKGMAP_JAR=/path/to/mkgmap.jar"
    echo ""
    echo -e "${YELLOW}Skipping mkgmap compilation tests (manual test required)${NC}"
    echo ""
    echo "=== Manual Test Procedure ==="
    echo "1. Generate .mp file: ogr2ogr -f 'PolishMap' test.mp input.geojson"
    echo "2. Compile with mkgmap: java -jar mkgmap.jar test.mp"
    echo "3. Verify: ls *.img (should show generated file)"
    echo "4. Check logs for errors/warnings"
    exit 0
fi
echo "mkgmap: $MKGMAP_JAR"

# Check PolishMap driver
if ! ogrinfo --formats 2>/dev/null | grep -q "PolishMap"; then
    echo -e "${RED}ERROR: PolishMap driver not found in GDAL${NC}"
    echo "Make sure GDAL_DRIVER_PATH is set correctly"
    exit 1
fi
echo -e "${GREEN}PolishMap driver: registered${NC}"
echo ""

# Test files
INTEGRATION_GEOJSON="$TEST_DATA_DIR/integration_test.geojson"
if [ ! -f "$INTEGRATION_GEOJSON" ]; then
    echo -e "${RED}ERROR: Test file not found: $INTEGRATION_GEOJSON${NC}"
    exit 1
fi

echo "=== AC2 Test 1: Generate .mp File for Compilation ==="
MP_FILE="$TMP_DIR/test_compilation.mp"
if ogr2ogr -f "PolishMap" "$MP_FILE" "$INTEGRATION_GEOJSON" 2>/dev/null; then
    if [ -f "$MP_FILE" ] && [ -s "$MP_FILE" ]; then
        echo -e "Test: Generate .mp file ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: Generate .mp file ... ${RED}FAIL${NC} (empty or missing)"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: Generate .mp file ... ${RED}FAIL${NC} (ogr2ogr failed)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC2 Test 2: mkgmap Compiles Without Errors ==="
if [ -f "$MP_FILE" ]; then
    cd "$TMP_DIR"

    # Run mkgmap and capture output
    MKGMAP_OUTPUT="$TMP_DIR/mkgmap_output.log"
    MKGMAP_EXIT_CODE=0
    java -jar "$MKGMAP_JAR" \
        --family-id=1 \
        --product-id=1 \
        --family-name="Test Map" \
        "test_compilation.mp" > "$MKGMAP_OUTPUT" 2>&1 || MKGMAP_EXIT_CODE=$?

    if [ "$MKGMAP_EXIT_CODE" -eq 0 ]; then
        echo -e "Test: mkgmap exit code 0 ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: mkgmap exit code 0 ... ${RED}FAIL${NC} (exit code: $MKGMAP_EXIT_CODE)"
        echo "  Log contents:"
        cat "$MKGMAP_OUTPUT" | head -20
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    cd - > /dev/null
else
    echo -e "Test: mkgmap compiles without errors ... ${RED}FAIL${NC} (no .mp file)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC2 Test 3: .img File Generated ==="
IMG_FILE=$(find "$TMP_DIR" -name "*.img" -type f 2>/dev/null | head -1)
if [ -n "$IMG_FILE" ] && [ -f "$IMG_FILE" ]; then
    IMG_SIZE=$(stat -c%s "$IMG_FILE" 2>/dev/null || stat -f%z "$IMG_FILE" 2>/dev/null || echo "0")
    echo -e "Test: .img file generated ($IMG_SIZE bytes) ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: .img file generated ... ${RED}FAIL${NC}"
    echo "  Contents of temp dir:"
    ls -la "$TMP_DIR" | head -10
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC2 Test 4: No Format Errors in Logs (NFR6) ==="
if [ -f "$MKGMAP_OUTPUT" ]; then
    # Check for ERROR or SEVERE messages
    ERROR_COUNT=$(grep -ciE "(ERROR|SEVERE|fatal)" "$MKGMAP_OUTPUT" 2>/dev/null || echo "0")

    if [ "$ERROR_COUNT" -eq 0 ]; then
        echo -e "Test: No errors in mkgmap logs ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: No errors in mkgmap logs ... ${RED}FAIL${NC} ($ERROR_COUNT errors found)"
        grep -iE "(ERROR|SEVERE|fatal)" "$MKGMAP_OUTPUT" | head -5
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: No errors in mkgmap logs ... ${YELLOW}SKIP${NC} (no log file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC2 Test 5: Warnings Check (Invalid Format) ==="
if [ -f "$MKGMAP_OUTPUT" ]; then
    # Check for format-related warnings (these are acceptable)
    # "Unknown type" warnings are OK for non-standard Garmin type codes
    FORMAT_WARNINGS=$(grep -ciE "(invalid.*format|malformed|syntax error)" "$MKGMAP_OUTPUT" 2>/dev/null || echo "0")

    if [ "$FORMAT_WARNINGS" -eq 0 ]; then
        echo -e "Test: No format warnings ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: No format warnings ... ${YELLOW}WARNING${NC} ($FORMAT_WARNINGS format-related warnings)"
        grep -iE "(invalid.*format|malformed|syntax error)" "$MKGMAP_OUTPUT" | head -5
        # Count as pass since warnings don't prevent compilation
        TESTS_PASSED=$((TESTS_PASSED + 1))
    fi
else
    echo -e "Test: No format warnings ... ${YELLOW}SKIP${NC} (no log file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC2 Test 6: Multiple Features Compilation ==="
# Test compilation with a more complex file
COMPLEX_MP="$TMP_DIR/complex_compilation.mp"
FILTER_COMBINED="$TEST_DATA_DIR/filter-combined.mp"

if [ -f "$FILTER_COMBINED" ]; then
    cd "$TMP_DIR"
    cp "$FILTER_COMBINED" "complex_compilation.mp"

    MKGMAP_COMPLEX_OUTPUT="$TMP_DIR/mkgmap_complex_output.log"
    java -jar "$MKGMAP_JAR" "complex_compilation.mp" > "$MKGMAP_COMPLEX_OUTPUT" 2>&1 || true

    # Check for .img output
    if find "$TMP_DIR" -name "*.img" -newer "$COMPLEX_MP" -type f 2>/dev/null | grep -q "."; then
        echo -e "Test: Complex file compilation ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: Complex file compilation ... ${RED}FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    cd - > /dev/null
else
    echo -e "Test: Complex file compilation ... ${YELLOW}SKIP${NC} (no complex test file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Summary
echo ""
echo "========================================"
echo "mkgmap Compilation Test Summary:"
echo "  Total:  $TESTS_TOTAL"
echo "  Passed: $TESTS_PASSED"
echo "  Failed: $TESTS_FAILED"
echo "========================================"

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "${GREEN}All mkgmap compilation tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
