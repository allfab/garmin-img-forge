# Test Corpus Documentation

## Overview

This directory contains the test corpus for the PolishMap GDAL driver. The corpus is organized by test purpose and provides comprehensive coverage for parsing validation, round-trip testing, error recovery, and performance benchmarking.

## Directory Structure

```
test/data/
├── valid-minimal/       # 23 minimal valid .mp files (Smoke Tests)
├── valid-complex/       # 156 real-world complex .mp files (Integration Tests)
├── error-recovery/      # 19 malformed .mp files (Error Handling Tests)
└── performance/         # 3 large .mp files (Performance Benchmarks)
```

## Corpus Details

### valid-minimal/ (23 files)

Minimal valid files for smoke testing. Each file contains 1-5 features with minimal required attributes.

**Required files:**
- `poi-simple.mp` - Single POI with Type, Label, Data0
- `polyline-simple.mp` - Single POLYLINE with 2 points
- `polygon-simple.mp` - Single POLYGON with closed ring

**Coverage:**
- POI variations: simple, multiple, no-label, mixed-sections, with-encoding
- POLYLINE variations: simple, multiple, many-points, no-parens, spaces
- POLYGON variations: simple, multiple, many-points, mixed-layers, cp1252
- Header variations: simple, full, cp1252
- Filter test files: attribute-types, combined, spatial-grid

**AC Coverage:** AC1, AC2, AC3, NFR21 (Smoke Tests)

### valid-complex/ (156 files)

Real-world complex files for integration testing. 155 files generated using `tools/generate_valid_complex_corpus.py` + 1 pre-existing file (`mixed-all-types.mp`).

**File categories:**
- `poi-varied-*.mp` (30 files) - POI-only with varied types (0x0001-0x6403), labels, positions
- `polyline-varied-*.mp` (30 files) - POLYLINE-only with 2-100 points, varied road types
- `polygon-varied-*.mp` (30 files) - POLYGON-only with 4-50 points, varied zone types
- `mixed-varied-*.mp` (50 files) - Mixed geometry types (POI + POLYLINE + POLYGON)
- `real-world-*.mp` (15 files) - Complex files with 100+ features each
- `mixed-all-types.mp` (1 file) - Pre-existing integration test file

**Geographic coverage:**
- France metropolitan (41.0-51.5 lat, -5.0-10.0 lon)
- Europe (35.0-60.0 lat, -10.0-30.0 lon)
- World samples (-60.0-80.0 lat, -180.0-180.0 lon)

**AC Coverage:** AC4, AC5, AC6

### error-recovery/ (19 files)

Malformed files for error handling and graceful degradation testing.

**Categories:**
- Invalid format files: binary-file, invalid-format
- Missing required elements: missing-header, missing-label, missing-data0
- Invalid geometry: corrupt-geometry, polyline-one-point, polygon-two-points, polygon-open-ring
- Malformed syntax: malformed-key-value, invalid-section, nested-sections
- Edge cases: empty-file, truncated-file, whitespace-variations

**Behavior:** Driver should handle gracefully without crashes (Story 3.2)

### performance/ (3 files)

Large files for performance benchmarking.

- `perf-1mb.mp` - ~4,000 features (~0.5s parse time)
- `perf-10mb.mp` - ~42,000 features (~5s parse time)
- `perf-100mb.mp` - ~428,000 features (~50s parse time)

**Generated using:** `tools/generate_perf_test_data.py` (Story 3.1)

## Running Tests

### All corpus tests
```bash
export GDAL_DRIVER_PATH="/path/to/build"
python3 test/test_valid_corpus.py
```

### Individual test suites
```bash
# Valid corpus tests (AC1-AC7)
python3 test/test_valid_corpus.py

# Error recovery tests (Story 3.2)
python3 test/test_error_recovery.py

# Performance tests (Story 3.1)
python3 test/test_performance_parsing.py
python3 test/test_write_performance.py

# Python bindings tests (Story 2.6, 2.7)
python3 test/test_python_bindings.py
```

## Generating New Corpus Files

### valid-complex corpus
```bash
python3 tools/generate_valid_complex_corpus.py [output_dir]
```

Default output: `test/data/valid-complex/`

### Performance test files
```bash
python3 tools/generate_perf_test_data.py [output_dir]
```

## NFR Coverage

| NFR | Description | Corpus | Coverage |
|-----|-------------|--------|----------|
| NFR20 | 500+ test files | All | 201 files (23+156+19+3) - **Partial** (Story 3.4 adds edge-cases) |
| NFR21 | Smoke tests pass | valid-minimal | 23/23 pass |
| NFR23 | No regressions | valid-* | 179/179 pass (100%) |

**Note:** NFR20 requires 500+ test files. Current total is 201 files. Story 3.4 (Edge Cases) will add 50-100 additional files to approach the 500+ target.

## Acceptance Criteria Mapping

| AC | Description | Test | Status |
|----|-------------|------|--------|
| AC1 | 10-20 valid-minimal files | test_ac1_* | PASS (23 files) |
| AC2 | POI smoke test | test_ac2_* | PASS |
| AC3 | All valid-minimal parse | test_ac3_* | PASS |
| AC4 | 100-200 valid-complex files | test_ac4_* | PASS (156 files) |
| AC5 | Complex file parsing | test_ac5_* | PASS |
| AC6 | Round-trip validation | test_ac6_* | PASS |
| AC7 | 100% regression prevention | test_ac7_* | PASS (179/179) |

## File Format Reference

### Minimal valid .mp structure
```
[IMG ID]
Name=Map Name
ID=12345678
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x2C00
Label=Restaurant Name
Data0=(48.8566,2.3522)
[END]

[POLYLINE]
Type=0x0001
Label=Route Name
Data0=(48.8566,2.3522),(48.8577,2.3533)
[END]

[POLYGON]
Type=0x0003
Label=Zone Name
Data0=(48.8566,2.3522),(48.8577,2.3522),(48.8577,2.3533),(48.8566,2.3533),(48.8566,2.3522)
[END]
```

### Supported attributes
- `Type` (required): Hex code 0xXXXX
- `Label` (optional): Text description
- `Data0` (required): Coordinates (lat,lon) or sequence
- `Data1-DataN` (optional): Additional coordinates
- `EndLevel` (optional): Max zoom level
- `Levels` (optional): Detail levels

## Maintenance

- **Story 3.3**: Initial corpus creation (2026-02-02)
- **Story 3.4**: Edge cases corpus expansion (planned)

When adding new test files:
1. Place in appropriate directory by purpose
2. Update this README with file description
3. Ensure tests in `test_valid_corpus.py` cover new files
4. Verify 100% pass rate with `python3 test/test_valid_corpus.py`
