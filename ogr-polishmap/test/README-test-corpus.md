# Test Corpus Documentation

## Overview

This directory contains the test corpus for the PolishMap GDAL driver. The corpus is organized by test purpose and provides comprehensive coverage for parsing validation, round-trip testing, error recovery, and performance benchmarking.

## Directory Structure

```
test/data/
├── valid-minimal/       # 23 minimal valid .mp files (Smoke Tests)
├── valid-complex/       # 156 real-world complex .mp files (Integration Tests)
├── edge-cases/          # 60 edge-case .mp files (Boundary Testing) [Story 3.4]
├── error-recovery/      # 50 malformed .mp files (Error Handling Tests) [Story 3.4 extended]
└── performance/         # 5 large .mp files (Performance Benchmarks) [Story 3.4 extended]
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

### edge-cases/ (60 files) [Story 3.4]

Edge-case files for boundary condition testing. Generated using `tools/generate_edge_cases_corpus.py`.

**Categories:**
- **Label edge cases (15 files):** edge-empty-label, edge-label-very-long, edge-label-special-chars, edge-label-unicode-accents, edge-label-numbers-only, edge-label-spaces, edge-label-equals-sign, edge-label-brackets, edge-label-semicolon, edge-label-cp1252-extended, edge-label-mixed-encoding, edge-label-whitespace-only, edge-label-tab-chars, edge-label-german, edge-label-spanish
- **Coordinate edge cases (15 files):** edge-coords-max-lat, edge-coords-min-lat, edge-coords-max-lon, edge-coords-min-lon, edge-coords-antimeridian, edge-coords-extreme-precision, edge-coords-gps-precision, edge-coords-near-zero, edge-coords-negative, edge-coords-europe-bounds, edge-coords-france-extreme, edge-coords-integer, edge-coords-very-close, edge-coords-duplicate-points, edge-coords-world-corners
- **Data field edge cases (10 files):** edge-data-all-fields, edge-data-sparse, edge-data-out-of-order, edge-data-max-value, edge-data-zero, edge-data-many-coords, edge-data-many-vertices, edge-data-duplicate-index, edge-data-endlevel-variations, edge-data-mixed-types
- **Feature count edge cases (10 files):** edge-features-single, edge-features-100, edge-features-1000, edge-features-mixed-types, edge-features-same-location, edge-features-ordered, edge-features-random-order, edge-features-polyline-many-points, edge-features-polygon-complex, edge-features-all-types-each
- **Encoding edge cases (10 files):** edge-encoding-cp1252-all, edge-encoding-cp1252-french, edge-encoding-cp1252-german, edge-encoding-cp1252-spanish, edge-encoding-cp1252-symbols, edge-encoding-cp1252-math, edge-encoding-crlf, edge-encoding-lf, edge-encoding-mixed-eol, edge-encoding-whitespace

**AC Coverage:** AC1-AC5, NFR22

### error-recovery/ (50 files) [Story 3.4 extended]

Malformed files for error handling and graceful degradation testing.

**Original files (19):**
- Invalid format files: binary-file, invalid-format
- Missing required elements: missing-header, missing-label, missing-data0
- Invalid geometry: corrupt-geometry, polyline-one-point, polygon-two-points, polygon-open-ring
- Malformed syntax: malformed-key-value, invalid-section, nested-sections
- Edge cases: empty-file, truncated-file, whitespace-variations

**Extended files (31, Story 3.4):** Generated using `tools/generate_error_recovery_corpus.py`
- **Header errors (5):** err-header-duplicate, err-header-malformed-id, err-header-missing-name, err-header-invalid-codepage, err-header-empty-values
- **Section errors (8):** err-section-unknown-type, err-section-duplicate-poi, err-section-incomplete, err-section-end-only, err-section-nested-deep, err-section-empty, err-section-wrong-end, err-section-case-mismatch
- **Geometry errors (8):** err-geom-lat-over-90, err-geom-lon-over-180, err-geom-nan-coords, err-geom-inf-coords, err-geom-empty-data0, err-geom-single-parens, err-geom-no-comma, err-geom-extra-values
- **Attribute errors (5):** err-attr-type-invalid, err-attr-type-overflow, err-attr-endlevel-negative, err-attr-levels-invalid, err-attr-duplicate-key
- **Mixed errors (5):** err-mixed-partial-valid, err-mixed-progressive, err-mixed-recoverable-chain, err-mixed-critical-then-valid, err-mixed-valid-then-critical

**Three-Level Error Strategy Coverage:**
- Critical errors (NULL return): missing header, invalid format
- Recoverable errors (skip+continue): invalid sections, bad geometry
- Minor errors (default+log): missing optional fields

**AC Coverage:** AC6-AC9, NFR9

### performance/ (5 files) [Story 3.4 extended]

Large files for performance benchmarking.

- `perf-1mb.mp` - ~0.8 MB actual, ~4,000 features
- `perf-5mb.mp` - ~4 MB actual, ~21,000 features [Story 3.4]
- `perf-10mb.mp` - ~8 MB actual, ~42,000 features
- `perf-50mb.mp` - ~40 MB actual, ~214,000 features [Story 3.4]
- `perf-100mb.mp` - ~84 MB actual, ~428,000 features

**Note:** File sizes are approximate. The generator creates files with target feature counts that result in sizes close to but not exactly matching the named sizes.

**Generated using:** `tools/generate_perf_test_data.py` (Story 3.1, 3.4)

**AC Coverage:** AC10-AC12, NFR1, NFR3, NFR7

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
| NFR1 | 10 MB parsing < 2s | performance | PASS (AC11) |
| NFR3 | Memory < 2x file size | performance | PASS (AC12) |
| NFR7 | Linear scaling | performance | PASS (AC12) |
| NFR9 | No crashes (graceful errors) | error-recovery | PASS (50/50) |
| NFR20 | 500+ test files | All | **294 files** (23+156+60+50+5) |
| NFR21 | Smoke tests pass | valid-minimal | 23/23 pass |
| NFR22 | Edge cases handled | edge-cases | 60/60 pass |
| NFR23 | No regressions | valid-* | 179/179 pass (100%) |

**Note:** NFR20 requires 500+ test files. Current total is 294 files. Future stories may add additional files to reach the 500+ target (e.g., internationalization tests, additional real-world samples).

## Acceptance Criteria Mapping

### Story 3.3 (Valid Corpus)
| AC | Description | Test | Status |
|----|-------------|------|--------|
| AC1 | 10-20 valid-minimal files | test_ac1_* | PASS (23 files) |
| AC2 | POI smoke test | test_ac2_* | PASS |
| AC3 | All valid-minimal parse | test_ac3_* | PASS |
| AC4 | 100-200 valid-complex files | test_ac4_* | PASS (156 files) |
| AC5 | Complex file parsing | test_ac5_* | PASS |
| AC6 | Round-trip validation | test_ac6_* | PASS |
| AC7 | 100% regression prevention | test_ac7_* | PASS (179/179) |

### Story 3.4 (Edge Cases & Error Recovery)
| AC | Description | Test | Status |
|----|-------------|------|--------|
| AC1 | 50-100 edge-case files | test_edge_cases.py | PASS (60 files) |
| AC2 | Empty label handling | test_ac2_empty_label | PASS |
| AC3 | Extreme coordinates | test_ac3_extreme_coords_* | PASS |
| AC4 | Data field variations | test_ac4_all_data_fields | PASS |
| AC5 | Zero crashes on edge-cases | test_ac5_all_edge_cases_no_crash | PASS |
| AC6 | 50-100 error-recovery files | test_error_recovery.py | PASS (50 files) |
| AC7 | Missing header returns NULL | test_ac7_missing_header_returns_null | PASS |
| AC8 | Invalid section skip+continue | test_ac8_invalid_section_skip_continue | PASS |
| AC9 | Zero crashes on error files | test_ac9_zero_crashes_extended | PASS |
| AC10 | Performance benchmark files | test_ac10_benchmark_files_exist | PASS (5 files) |
| AC11 | 10 MB parsing < 2s | test_ac11_10mb_parsing_under_2_seconds | PASS |
| AC12 | Linear scaling + memory | test_ac12_linear_scaling_* | PASS |

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

## Generating New Corpus Files

### Edge-cases corpus [Story 3.4]
```bash
python3 tools/generate_edge_cases_corpus.py [output_dir]
```
Default output: `test/data/edge-cases/`

### Error-recovery corpus [Story 3.4]
```bash
python3 tools/generate_error_recovery_corpus.py [output_dir]
```
Default output: `test/data/error-recovery/`

## Maintenance

- **Story 3.3**: Initial corpus creation (2026-02-02)
- **Story 3.4**: Edge cases & error recovery expansion (2026-02-03)

When adding new test files:
1. Place in appropriate directory by purpose
2. Update this README with file description
3. Ensure tests in `test_valid_corpus.py` or relevant test file cover new files
4. Verify 100% pass rate with `pytest test/`
