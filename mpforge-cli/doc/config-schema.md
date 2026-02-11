# Configuration File Schema

This document describes the YAML configuration format for `mpforge-cli`.

## Overview

The configuration file defines:
- Grid parameters for spatial tiling
- Input data sources (shapefiles, GeoPackage, PostGIS)
- Output directory and file naming
- Optional spatial filters
- Error handling behavior

## Schema

### Top Level

```yaml
version: <integer>          # Configuration version (default: 1)
grid: <GridConfig>          # Grid configuration (required)
inputs: [<InputSource>]     # List of input sources (required)
output: <OutputConfig>      # Output configuration (required)
filters: <FilterConfig>     # Optional filters
error_handling: <string>    # "continue" or "fail-fast" (default: "continue")
```

### GridConfig

Defines the spatial grid for tiling.

```yaml
grid:
  cell_size: <float>         # Cell size in degrees (required)
  overlap: <float>           # Overlap in degrees (default: 0.0)
  origin: [<float>, <float>] # Origin point [lon, lat] (optional)
```

**Example:**
```yaml
grid:
  cell_size: 0.15
  overlap: 0.01
  origin: [-5.0, 41.0]
```

### InputSource

Defines a single data source. Can be a file path or database connection.

**File-based source:**
```yaml
- path: <string>           # File path (supports wildcards)
  layers: [<string>]       # Optional: specific layers to read (see Layer Selection below)
```

**Database connection:**
```yaml
- connection: <string>     # GDAL connection string
  layer: <string>          # Single layer name (backward compatibility)
  layers: [<string>]       # Multiple layers (recommended)
```

#### Layer Selection

The `layer` (singular) and `layers` (plural) fields control which layers are loaded from multi-layer formats like GeoPackage or PostGIS:

| Field      | Type          | Behavior                                                                 |
|------------|---------------|--------------------------------------------------------------------------|
| `layers`   | List[String]  | **Recommended.** Load all specified layers. Supports multi-layer loading.|
| `layer`    | String        | **Deprecated.** Load a single layer. Use `layers` for consistency.       |
| _(none)_   | -             | **Default.** Load layer 0 (first layer in dataset).                      |

**Precedence:** If both `layer` and `layers` are specified, `layers` takes precedence.

**Examples:**
```yaml
# Shapefile with wildcard (single-layer format)
- path: "data/*.shp"

# GeoPackage with multiple layers (STORY 5.5)
- path: "data/bdtopo.gpkg"
  layers: ["buildings", "roads", "water"]

# GeoPackage with empty list (uses default layer 0 with warning)
- path: "data/poi.gpkg"
  layers: []

# GeoPackage without layers field (uses default layer 0, no warning)
- path: "data/single.gpkg"

# PostGIS with single layer (backward compatibility)
- connection: "PG:host=localhost dbname=gis user=postgres"
  layer: "roads"

# PostGIS with multiple layers (recommended)
- connection: "PG:host=localhost dbname=gis"
  layers: ["roads", "buildings", "poi"]
```

**Error Handling for Invalid Layers:**

If a specified layer does not exist in the dataset:

- **`error_handling: "continue"`** (default): Log a warning and continue loading other valid layers
- **`error_handling: "fail-fast"`**: Stop immediately with an error message

**Example with invalid layer:**
```yaml
inputs:
  - path: "data/bdtopo.gpkg"
    layers: ["buildings", "invalid_layer", "roads"]  # "invalid_layer" doesn't exist

error_handling: "continue"  # Loads "buildings" and "roads", skips "invalid_layer"
```

### OutputConfig

Defines output directory and file naming.

```yaml
output:
  directory: <string>         # Output directory path (required)
  filename_pattern: <string>  # Filename pattern (default: "{x}_{y}.mp")
```

**Filename pattern variables:**
- `{x}`: Tile X coordinate
- `{y}`: Tile Y coordinate

**Example:**
```yaml
output:
  directory: "tiles/"
  filename_pattern: "france_{x}_{y}.mp"
```

### FilterConfig

Optional spatial and attribute filters.

```yaml
filters:
  bbox: [<float>, <float>, <float>, <float>]  # [min_lon, min_lat, max_lon, max_lat]
```

**Example:**
```yaml
filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]  # Metropolitan France
```

## Complete Example

**Example 1: Mixed sources with multi-layer GeoPackage**

```yaml
version: 1

grid:
  cell_size: 0.15
  overlap: 0.01
  origin: [0.0, 0.0]

inputs:
  # Shapefile (single-layer format)
  - path: "data/buildings.shp"

  # GeoPackage with multiple layers (STORY 5.5)
  - path: "data/roads.gpkg"
    layers: ["primary", "secondary", "tertiary"]

  # PostGIS connection with multiple layers
  - connection: "PG:host=localhost dbname=gis"
    layers: ["poi", "parks"]

output:
  directory: "tiles/"
  filename_pattern: "{x}_{y}.mp"

filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]

error_handling: "continue"
```

**Example 2: Real-world BDTOPO configuration (50 layers)**

```yaml
version: 1

grid:
  cell_size: 0.15
  overlap: 0.01

inputs:
  # BDTOPO Réunion - Single GeoPackage with ~50 layers
  - path: "data/bdtopo_reunion.gpkg"
    layers:
      # Buildings
      - "batiment"
      - "construction_lineaire"
      - "construction_ponctuelle"
      # Transportation
      - "route"
      - "troncon_de_route"
      - "noeud_routier"
      - "voie_ferree"
      - "troncon_de_voie_ferree"
      # Water features
      - "cours_d_eau"
      - "plan_d_eau"
      - "troncon_hydrographique"
      # Administrative
      - "commune"
      - "arrondissement"
      - "departement"
      # Land use
      - "zone_vegetation"
      - "terrain_sport"
      # ... (add other relevant layers)

output:
  directory: "tiles/"
  filename_pattern: "reunion_{x}_{y}.mp"

error_handling: "continue"  # Continue even if some layers are missing
```

## Validation Rules

The configuration file is validated both syntactically (YAML parsing) and semantically (business rules).

### Grid Validation

- **`cell_size`** must be **positive** (> 0.0)
  - ❌ Error: `cell_size: -0.15` or `cell_size: 0.0`
  - ✅ Valid: `cell_size: 0.15`

- **`overlap`** must be **non-negative** (>= 0.0)
  - ❌ Error: `overlap: -0.01`
  - ✅ Valid: `overlap: 0.0` or `overlap: 0.01`

### Input Validation

- **At least one input source** is required
  - ❌ Error: `inputs: []`
  - ✅ Valid: `inputs: [{ path: "data.shp" }]`

- Each input must have **either** `path` **or** `connection`, **not both or neither**
  - ❌ Error: `{ path: "data.shp", connection: "PG:..." }` (both)
  - ❌ Error: `{ layers: ["roads"] }` (neither)
  - ✅ Valid: `{ path: "data.shp" }`
  - ✅ Valid: `{ connection: "PG:host=localhost" }`

### Error Handling Validation

- **`error_handling`** must be `"continue"` or `"fail-fast"`
  - ❌ Error: `error_handling: "stop"`
  - ✅ Valid: `error_handling: "continue"`
  - ✅ Valid: `error_handling: "fail-fast"`

### Filters Validation

- If **`filters.bbox`** is specified, coordinates must form a valid bounding box:
  - **min_lon < max_lon** (bbox[0] < bbox[2])
  - **min_lat < max_lat** (bbox[1] < bbox[3])
  - ❌ Error: `bbox: [10.0, 41.0, -5.0, 51.5]` (min_lon > max_lon)
  - ❌ Error: `bbox: [-5.0, 51.5, 10.0, 41.0]` (min_lat > max_lat)
  - ✅ Valid: `bbox: [-5.0, 41.0, 10.0, 51.5]`

## Common Errors

### Invalid YAML Syntax

**Error:**
```
Failed to parse YAML config: config.yaml

Caused by:
    invalid type: found sequence, expected a map
```

**Cause:** Malformed YAML (missing colons, incorrect indentation, invalid syntax)

**Fix:** Validate your YAML syntax using a YAML validator

---

### Negative cell_size

**Error:**
```
Config validation failed for: config.yaml

Caused by:
    grid.cell_size must be positive, got: -0.15
```

**Fix:** Use a positive value for `cell_size`, typically between 0.10 and 0.20

---

### No Input Sources

**Error:**
```
Config validation failed for: config.yaml

Caused by:
    At least one input source is required
```

**Fix:** Add at least one entry in the `inputs` list

---

### Invalid PostGIS Connection

**Error:**
```
Failed to read config file: config.yaml

Caused by:
    No such file or directory (os error 2)
```

**Cause:** File path is incorrect or file doesn't exist

**Fix:** Check that the config file path is correct

---

### Wildcard No Match Warning

**Warning (non-fatal):**
```
WARN No files matched wildcard pattern pattern="data/*.xyz"
```

**Cause:** Wildcard pattern matched no files

**Behavior:** Processing continues, but this input source will be empty

**Fix:** Verify the wildcard pattern and ensure matching files exist

## PostGIS Connection Strings

PostGIS connections are detected automatically when the `connection` field:
- Starts with `PG:`, OR
- Contains `host=`

**Supported formats:**

```yaml
# OGR-style (recommended)
connection: "PG:host=localhost dbname=gis user=postgres password=secret"

# PostgreSQL-style
connection: "host=localhost dbname=gis user=postgres password=secret port=5432"

# With specific schema
connection: "PG:host=localhost dbname=gis schemas=public,osm"

# With SSL
connection: "PG:host=db.example.com dbname=gis sslmode=require"
```

**Note:** For PostGIS connections, specify either `layer` (single layer) or `layers` (multiple layers)

## Wildcard Patterns

File paths support glob-style wildcards:

- `*` matches any sequence of characters (except `/`)
- `?` matches any single character
- `**` matches any sequence including `/` (recursive)

**Examples:**

```yaml
# All shapefiles in data directory
- path: "data/*.shp"

# All GeoPackages recursively
- path: "data/**/*.gpkg"

# Specific pattern
- path: "data/france_*.shp"
```

**Behavior:**
- Files are resolved when the configuration is loaded
- A warning is logged if no files match the pattern
- Processing continues even if wildcards match no files

## CLI Options

### Parallel Processing

**Option:** `--jobs N` or `-j N`

**Description:** Configure parallel processing for tile export using N threads.

**Default:** `1` (sequential processing)

**Behavior:**
- **`--jobs 1`**: Sequential processing (debug mode, same as Epic 6 behavior)
- **`--jobs 2-8`**: Parallel processing (production mode, recommended for large datasets)
- **`--jobs > num_cpus`**: Warning logged; may degrade performance

**Examples:**

```bash
# Sequential export (default, debug mode)
mpforge-cli build --config config.yaml

# Parallel export with 4 threads (production mode)
mpforge-cli build --config config.yaml --jobs 4

# Parallel export with 8 threads (high-performance mode)
mpforge-cli build --config config.yaml -j 8
```

**Performance Notes:**

- **Small datasets (<50 tiles):** Use `--jobs 1` (parallel overhead not worth it)
- **Medium datasets (50-500 tiles):** Use `--jobs 2-4` (2× speedup expected)
- **Large datasets (>500 tiles):** Use `--jobs 4-8` (2-3× speedup expected)
- **CPU count:** Run `nproc` or `sysctl -n hw.ncpu` to check available CPUs
- **Recommendation:** Start with `--jobs 4` and adjust based on performance

**Thread Safety:**

The parallel implementation uses rayon for thread-safe data parallelism:
- Thread-safe error collection in `continue` mode
- Fail-fast mode interrupts all threads on first error
- Atomic counters for statistics aggregation
- Each tile creates its own GDAL dataset (no shared state)

**Story:** Epic 7 Story 7.1 - Parallélisation du Traitement des Tuiles (2026-02-11)

### Verbosity Levels

**Option:** `--verbose` or `-v` (can be repeated: `-v`, `-vv`, `-vvv`)

**Description:** Control logging verbosity for pipeline execution and troubleshooting.

**Default:** `0` (WARN level - shows only warnings and errors)

**Levels:**
- **No flag** (verbose=0): ERROR and WARN logs only, progress bar displayed
- **`-v`** (verbose=1): INFO logs + progress bar (shows major pipeline steps)
- **`-vv`** (verbose=2): DEBUG logs, progress bar disabled (detailed GDAL operations, per-tile stats)
- **`-vvv`** (verbose=3): TRACE logs, progress bar disabled (very verbose, all internal operations)

**Examples:**

```bash
# Production mode: progress bar only, minimal logging
mpforge-cli build --config config.yaml --jobs 4

# Monitoring mode: progress bar + major steps
mpforge-cli build --config config.yaml --jobs 4 -v

# Debug mode: detailed logs, no progress bar (troubleshooting)
mpforge-cli build --config config.yaml --jobs 4 -vv

# Trace mode: maximum verbosity (debugging internal issues)
mpforge-cli build --config config.yaml --jobs 4 -vvv
```

**Use Cases:**

- **Production pipelines:** No `-v` flag (progress bar + minimal logs)
- **CI/CD integration:** `-v` (INFO logs for monitoring pipeline progress)
- **Troubleshooting GDAL errors:** `-vv` (DEBUG logs show detailed GDAL operations)
- **Developer debugging:** `-vvv` (TRACE logs for internal debugging)

**Progress Bar Behavior:**

- **verbose=0 or verbose=1:** Progress bar displayed with ETA
- **verbose >= 2:** Progress bar disabled to avoid log pollution in debug mode

**Tip:** If a pipeline fails or produces unexpected results, re-run with `-vv` to get detailed troubleshooting logs.

**Story:** Epic 7 Story 7.2 - Progress Bar & Feedback Temps Réel (2026-02-11)

## Notes

- **Version:** Currently only version `1` is supported
- **Grid cell size:** Typically 0.10 to 0.20 degrees (~11-22 km)
- **Overlap:** Use small overlaps (0.01-0.02) to avoid edge artifacts
- **Error handling:**
  - `"continue"`: Log errors and continue processing (default)
  - `"fail-fast"`: Stop on first error
- **Input paths:** Relative to current working directory or absolute paths
- **Output directory:** Created automatically if it doesn't exist
- **Validation:** Configuration is validated at load time; errors provide clear messages indicating the problem
- **Parallel processing:** Use `--jobs N` to enable multi-threaded tile export (Story 7.1)

## JSON Execution Report (Story 7.3)

The CLI can generate a structured JSON report for CI/CD integration and automation.

### Usage

```bash
mpforge-cli build --config config.yaml --report report.json
```

The `--report` option is **optional**. If not specified, only the console summary is displayed.

### JSON Schema

When `--report` is specified, a JSON file is created with the following schema:

```json
{
  "status": "success",           // "success" | "failure"
  "tiles_generated": 2047,        // Number of successfully exported tiles
  "tiles_failed": 3,              // Number of failed tiles
  "tiles_skipped": 150,           // Number of skipped empty tiles
  "features_processed": 1234567,  // Total features exported across all tiles
  "duration_seconds": 1845.3,     // Total execution time (float for precision)
  "errors": [                     // Detailed error information (empty if success)
    {
      "tile": "12_45",            // Tile identifier
      "error": "GDAL error: Invalid geometry at feature 12345"  // Complete error message
    }
  ]
}
```

### Field Descriptions

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Overall execution status: `"success"` if no failures, `"failure"` otherwise |
| `tiles_generated` | integer | Number of tiles successfully exported to `.mp` files |
| `tiles_failed` | integer | Number of tiles that failed to export (0 if success) |
| `tiles_skipped` | integer | Number of tiles skipped because they were empty (no features) |
| `features_processed` | integer | Total number of features (points, linestrings, polygons) exported across all tiles |
| `duration_seconds` | float | Total pipeline execution duration in seconds (includes reading, tiling, and writing) |
| `errors` | array | Array of error objects with `tile` (ID) and `error` (message) fields. Empty if `status == "success"` |

### Exit Codes

The CLI returns appropriate exit codes for CI/CD integration:

- **Exit code 0:** Success (all tiles exported, no errors)
- **Exit code 1:** Failure (one or more tiles failed to export)

**Example CI/CD usage:**

```bash
#!/bin/bash
# Production BDTOPO pipeline with JSON report
mpforge-cli build --config bdtopo.yaml --jobs 8 --report report.json

# Exit code determines CI/CD success/failure
if [ $? -eq 0 ]; then
  echo "Pipeline succeeded - archiving report"
  cp report.json /archive/$(date +%Y%m%d)-report.json
else
  echo "Pipeline failed - triggering alerts"
  # Parse JSON for error details
  jq '.errors[] | "\(.tile): \(.error)"' report.json
  exit 1
fi
```

### Parsing JSON Reports

**Using jq (command-line JSON processor):**

```bash
# Extract status
jq '.status' report.json

# Count failures
jq '.tiles_failed' report.json

# List all errors
jq '.errors[] | "\(.tile): \(.error)"' report.json

# Calculate success rate
jq '(.tiles_generated / (.tiles_generated + .tiles_failed) * 100) | floor' report.json
```

**Using Python:**

```python
import json

with open('report.json') as f:
    report = json.load(f)

print(f"Status: {report['status']}")
print(f"Success rate: {report['tiles_generated']}/{report['tiles_generated'] + report['tiles_failed']}")

if report['status'] == 'failure':
    print(f"\nErrors ({len(report['errors'])}):")
    for err in report['errors']:
        print(f"  - Tile {err['tile']}: {err['error']}")
```

### Behavior Notes

- **Report generation:** JSON report is written **after** pipeline completion, even if errors occurred
- **Write errors:** If JSON write fails (e.g., permission denied), a warning is logged but the pipeline does not crash
- **Console summary:** Always displayed regardless of `--report` option
- **Error truncation:** If more than 5 errors, console shows only top 5 (full list in JSON report)
- **Pretty formatting:** JSON is formatted with indentation for human readability

**Story:** Epic 7 Story 7.3 - Rapport d'Exécution & Intégration CI/CD (2026-02-11)

## Implementation Notes

- **Configuration parsing and validation:** Story 5.2 (2026-02-10)
- **Multi-layer GeoPackage support:** Story 5.5 (2026-02-10)
  - Fixed bug where only first layer (`layers[0]`) was loaded
  - Now correctly iterates over all specified layers
  - Supports `error_handling` mode for invalid layer names
  - Backward compatible with `layer` (singular) field
- **JSON execution report:** Story 7.3 (2026-02-11)
  - Optional `--report` CLI flag for CI/CD integration
  - Structured JSON schema with status, counts, duration, and detailed errors
  - Exit code 0 (success) / 1 (failure) for automation
  - French i18n console summary with top 5 errors display
