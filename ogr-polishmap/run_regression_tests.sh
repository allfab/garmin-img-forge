#!/bin/bash
GDAL_DRIVER_PATH=/nonexistent
export GDAL_DRIVER_PATH

TESTS=(
    "test_driver_registration"
    "test_parser_and_open"
    "test_dataset_layers"
    "test_poi_layer"
    "test_polyline_layer"
    "test_polygon_layer"
    "test_filters"
    "test_create"
    "test_header"
    "test_poi_write"
    "test_polyline_write"
    "test_polygon_write"
    "test_driver_metadata"
    "test_createfield"
    "test_multigeom"
    "test_extended_attributes"
)

PASSED=0
FAILED=0
FAILED_TESTS=""

for test in "${TESTS[@]}"; do
    echo "Running $test..."
    if ./build/$test > /tmp/${test}_output.txt 2>&1; then
        PASSED=$((PASSED + 1))
        echo "  ✓ PASSED"
    else
        FAILED=$((FAILED + 1))
        FAILED_TESTS="$FAILED_TESTS\n  - $test"
        echo "  ✗ FAILED"
    fi
done

echo ""
echo "========================================="
echo "  REGRESSION TEST SUMMARY"
echo "========================================="
echo "  ✓ Passed: $PASSED"
echo "  ✗ Failed: $FAILED"
if [ $FAILED -gt 0 ]; then
    echo ""
    echo "Failed tests:$FAILED_TESTS"
fi
echo "========================================="

exit $FAILED
