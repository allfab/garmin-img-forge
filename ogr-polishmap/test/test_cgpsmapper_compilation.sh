#!/bin/bash
# Story 2.7: cGPSmapper Compilation Tests (AC3)
#
# Tests that generated .mp files compile successfully with cGPSmapper:
# - cGPSmapper completes without errors
# - .img file is generated
# - Valid for both compilers (NFR7)
#
# Prerequisites:
# - cGPSmapper must be installed (Windows application, can run via Wine on Linux)
# - GDAL with PolishMap driver available
# - ogr2ogr in PATH
#
# Usage:
#   ./test_cgpsmapper_compilation.sh [TEST_DATA_DIR] [CGPSMAPPER_PATH]
#
# Environment:
#   CGPSMAPPER_PATH: Path to cgpsmapper.exe (optional)
#
# NOTE: cGPSmapper is a Windows-only application. On Linux, it requires Wine.
#       If Wine is not available, this test provides manual testing instructions.

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DATA_DIR="${1:-$SCRIPT_DIR/data/valid-minimal}"
BUILD_DIR="${SCRIPT_DIR}/../build"
TMP_DIR="/tmp/ogr_polishmap_cgpsmapper_$$"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
TESTS_TOTAL=0
TESTS_PASSED=0
TESTS_FAILED=0

# Output file paths (initialized empty, set during tests)
CGPS_OUTPUT=""

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
echo "  Story 2.7: cGPSmapper Compilation Tests"
echo "  AC3: Compilation with cGPSmapper"
echo "========================================"
echo "TEST_DATA_DIR: $TEST_DATA_DIR"
echo ""

# Find cGPSmapper
find_cgpsmapper() {
    # Check environment variable first
    if [ -n "$CGPSMAPPER_PATH" ] && [ -f "$CGPSMAPPER_PATH" ]; then
        echo "$CGPSMAPPER_PATH"
        return 0
    fi

    # Second argument
    if [ -n "$2" ] && [ -f "$2" ]; then
        echo "$2"
        return 0
    fi

    # Common locations
    local SEARCH_PATHS=(
        "$HOME/cgpsmapper/cgpsmapper.exe"
        "$HOME/.local/share/cgpsmapper/cgpsmapper.exe"
        "/opt/cgpsmapper/cgpsmapper.exe"
        "/usr/local/share/cgpsmapper/cgpsmapper.exe"
        "$HOME/.wine/drive_c/Program Files/cGPSmapper/cgpsmapper.exe"
        "$HOME/.wine/drive_c/Program Files (x86)/cGPSmapper/cgpsmapper.exe"
    )

    for path in "${SEARCH_PATHS[@]}"; do
        if [ -f "$path" ]; then
            echo "$path"
            return 0
        fi
    done

    return 1
}

# Check Wine (required for cGPSmapper on Linux)
echo "=== Prerequisites Check ==="

# Determine platform
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]]; then
    PLATFORM="windows"
    echo "Platform: Windows"
elif command -v wine &> /dev/null; then
    PLATFORM="linux-wine"
    echo "Platform: Linux with Wine"
    echo "Wine: $(wine --version 2>&1 | head -1)"
else
    PLATFORM="linux"
    echo "Platform: Linux (Wine not available)"
fi

# Find cGPSmapper
CGPSMAPPER_PATH=$(find_cgpsmapper "$@" 2>/dev/null || echo "")

if [ -z "$CGPSMAPPER_PATH" ] || [ ! -f "$CGPSMAPPER_PATH" ]; then
    echo -e "${YELLOW}WARNING: cGPSmapper not found${NC}"
    echo ""
    echo "cGPSmapper is a Windows-only application."
    echo ""
    if [ "$PLATFORM" = "linux" ]; then
        echo "To run on Linux:"
        echo "  1. Install Wine: sudo dnf install wine"
        echo "  2. Download cGPSmapper from: http://www.cgpsmapper.com/"
        echo "  3. Set environment: export CGPSMAPPER_PATH=/path/to/cgpsmapper.exe"
    else
        echo "To install cGPSmapper:"
        echo "  1. Download from: http://www.cgpsmapper.com/"
        echo "  2. Extract and set CGPSMAPPER_PATH=/path/to/cgpsmapper.exe"
    fi
    echo ""
    echo -e "${YELLOW}Skipping cGPSmapper compilation tests (manual test required)${NC}"
    echo ""
    echo "========================================"
    echo "  Manual Test Procedure"
    echo "========================================"
    echo ""
    echo "On Windows:"
    echo "  1. Generate .mp file: ogr2ogr -f \"PolishMap\" test.mp input.geojson"
    echo "  2. Compile: cgpsmapper.exe test.mp"
    echo "  3. Verify: dir *.img (should show generated file)"
    echo "  4. Check output for errors"
    echo ""
    echo "On Linux (with Wine):"
    echo "  1. Generate .mp file: ogr2ogr -f \"PolishMap\" test.mp input.geojson"
    echo "  2. Compile: wine cgpsmapper.exe test.mp"
    echo "  3. Verify: ls *.img (should show generated file)"
    echo "  4. Check output for errors"
    echo ""
    echo "Expected results (NFR7):"
    echo "  - Exit code 0 (success)"
    echo "  - .img file generated"
    echo "  - No fatal errors in output"
    echo ""
    exit 0
fi

echo "cGPSmapper: $CGPSMAPPER_PATH"

# Check PolishMap driver
if ! ogrinfo --formats 2>/dev/null | grep -q "PolishMap"; then
    echo -e "${RED}ERROR: PolishMap driver not found in GDAL${NC}"
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

# Helper function to run cGPSmapper
run_cgpsmapper() {
    local mp_file="$1"
    local output_log="$2"

    if [ "$PLATFORM" = "windows" ]; then
        "$CGPSMAPPER_PATH" "$mp_file" > "$output_log" 2>&1
    else
        # Linux with Wine
        wine "$CGPSMAPPER_PATH" "$mp_file" > "$output_log" 2>&1
    fi
}

echo "=== AC3 Test 1: Generate .mp File for Compilation ==="
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
echo "=== AC3 Test 2: cGPSmapper Compiles Without Errors ==="
if [ -f "$MP_FILE" ]; then
    cd "$TMP_DIR"

    CGPS_OUTPUT="$TMP_DIR/cgpsmapper_output.log"
    CGPS_EXIT_CODE=0
    run_cgpsmapper "test_compilation.mp" "$CGPS_OUTPUT" || CGPS_EXIT_CODE=$?

    if [ "$CGPS_EXIT_CODE" -eq 0 ]; then
        echo -e "Test: cGPSmapper exit code 0 ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: cGPSmapper exit code 0 ... ${RED}FAIL${NC} (exit code: $CGPS_EXIT_CODE)"
        echo "  Log contents:"
        cat "$CGPS_OUTPUT" | head -20
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi

    cd - > /dev/null
else
    echo -e "Test: cGPSmapper compiles without errors ... ${RED}FAIL${NC} (no .mp file)"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC3 Test 3: .img File Generated ==="
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
echo "=== AC3 Test 4: No Fatal Errors in Logs (NFR7) ==="
if [ -f "$CGPS_OUTPUT" ]; then
    ERROR_COUNT=$(grep -ciE "(error|fatal|failed)" "$CGPS_OUTPUT" 2>/dev/null || echo "0")

    if [ "$ERROR_COUNT" -eq 0 ]; then
        echo -e "Test: No fatal errors in logs ... ${GREEN}PASS${NC}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "Test: No fatal errors in logs ... ${RED}FAIL${NC} ($ERROR_COUNT errors)"
        grep -iE "(error|fatal|failed)" "$CGPS_OUTPUT" | head -5
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
else
    echo -e "Test: No fatal errors in logs ... ${YELLOW}SKIP${NC} (no log file)"
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

echo ""
echo "=== AC3 Test 5: Valid for Both Compilers (NFR7) ==="
# This test checks that the same .mp file works with both mkgmap and cGPSmapper
# If cGPSmapper succeeded, the file is valid for cGPSmapper
# mkgmap compatibility is tested in test_mkgmap_compilation.sh
if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "Test: File valid for cGPSmapper ... ${GREEN}PASS${NC}"
    TESTS_PASSED=$((TESTS_PASSED + 1))
else
    echo -e "Test: File valid for cGPSmapper ... ${RED}FAIL${NC}"
    TESTS_FAILED=$((TESTS_FAILED + 1))
fi
TESTS_TOTAL=$((TESTS_TOTAL + 1))

# Summary
echo ""
echo "========================================"
echo "cGPSmapper Compilation Test Summary:"
echo "  Total:  $TESTS_TOTAL"
echo "  Passed: $TESTS_PASSED"
echo "  Failed: $TESTS_FAILED"
echo "========================================"

if [ "$TESTS_FAILED" -eq 0 ]; then
    echo -e "${GREEN}All cGPSmapper compilation tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
