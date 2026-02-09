#!/bin/bash
GDAL_DRIVER_PATH=/nonexistent
export GDAL_DRIVER_PATH

TOTAL_PASSED=0
TOTAL_FAILED=0

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

for test in "${TESTS[@]}"; do
    OUTPUT=$(./build/$test 2>&1)
    PASSED=$(echo "$OUTPUT" | grep -o "Passed: [0-9]*" | grep -o "[0-9]*")
    FAILED=$(echo "$OUTPUT" | grep -o "Failed: [0-9]*" | grep -o "[0-9]*")
    
    if [ -n "$PASSED" ]; then
        TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
    fi
    if [ -n "$FAILED" ]; then
        TOTAL_FAILED=$((TOTAL_FAILED + FAILED))
    fi
done

echo "========================================="
echo "  TOTAL TEST COUNT"
echo "========================================="
echo "  ✓ Total Passed: $TOTAL_PASSED"
echo "  ✗ Total Failed: $TOTAL_FAILED"
echo "  📊 Total Tests: $((TOTAL_PASSED + TOTAL_FAILED))"
echo "========================================="
