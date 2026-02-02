#!/bin/bash
# Story 3.1: Valgrind Memory Check Script (Task 6)
#
# Runs memory leak detection on PolishMap driver using Valgrind.
# Architecture: NFR-GDAL4 (Reference counting and memory management)
#
# Usage:
#   ./scripts/valgrind.sh [test_name]
#
# Examples:
#   ./scripts/valgrind.sh                    # Run all tests
#   ./scripts/valgrind.sh test_parser        # Run specific test

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/build"

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "============================================================"
echo "Story 3.1: Valgrind Memory Check (NFR-GDAL4)"
echo "============================================================"
echo ""

# Check Valgrind installation
if ! command -v valgrind &> /dev/null; then
    echo -e "${RED}ERROR: Valgrind not found${NC}"
    echo "Install with: sudo dnf install valgrind (Fedora) or sudo apt install valgrind (Ubuntu)"
    exit 1
fi

VALGRIND_VERSION=$(valgrind --version)
echo "Valgrind version: $VALGRIND_VERSION"
echo ""

# Ensure build is up to date
if [ ! -d "$BUILD_DIR" ]; then
    echo -e "${YELLOW}Build directory not found, running cmake...${NC}"
    mkdir -p "$BUILD_DIR"
    cd "$BUILD_DIR"
    cmake .. -DCMAKE_BUILD_TYPE=Debug
    make -j$(nproc)
    cd "$PROJECT_DIR"
fi

# Valgrind options
# --leak-check=full: Detailed leak info
# --show-leak-kinds=all: Show all leak types (definite, indirect, possible, reachable)
# --track-origins=yes: Track origin of uninitialized values
# --error-exitcode=1: Return error code on memory errors
VALGRIND_OPTS="--leak-check=full --show-leak-kinds=all --track-origins=yes --error-exitcode=1"

# Suppressions for known GDAL/system leaks (if needed)
SUPPRESSIONS=""
if [ -f "$SCRIPT_DIR/valgrind.supp" ]; then
    SUPPRESSIONS="--suppressions=$SCRIPT_DIR/valgrind.supp"
fi

cd "$BUILD_DIR"

# Run tests under Valgrind
TESTS_PASSED=0
TESTS_FAILED=0

run_test() {
    local test_name=$1
    local test_path="$BUILD_DIR/$test_name"

    if [ ! -x "$test_path" ]; then
        echo -e "${YELLOW}SKIP: $test_name (not found)${NC}"
        return
    fi

    echo -n "Testing $test_name... "

    # Run with Valgrind
    if valgrind $VALGRIND_OPTS $SUPPRESSIONS "$test_path" > /tmp/valgrind_$$.log 2>&1; then
        echo -e "${GREEN}PASSED${NC} (no leaks)"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        ((TESTS_FAILED++))
        # Show summary of errors
        echo "  Valgrind errors:"
        grep -E "(definitely lost|indirectly lost|possibly lost|ERROR SUMMARY)" /tmp/valgrind_$$.log | head -10
    fi

    rm -f /tmp/valgrind_$$.log
}

if [ -n "$1" ]; then
    # Run specific test
    run_test "$1"
else
    # Run all test executables
    echo "Running all tests under Valgrind..."
    echo ""

    # Core tests
    for test in test_parser_and_open test_header test_poi_layer test_polyline_layer \
                test_polygon_layer test_poi_write test_polyline_write test_polygon_write \
                test_create test_filters; do
        run_test "$test"
    done
fi

echo ""
echo "============================================================"
echo -e "Summary: ${GREEN}Passed: $TESTS_PASSED${NC}, ${RED}Failed: $TESTS_FAILED${NC}"
echo "============================================================"

if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi

exit 0
