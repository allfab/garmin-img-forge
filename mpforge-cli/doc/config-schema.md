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
  layers: [<string>]       # Optional: specific layers to read
```

**Database connection:**
```yaml
- connection: <string>     # GDAL connection string
  layer: <string>          # Layer name (required for connections)
  layers: [<string>]       # Alternative: multiple layers
```

**Examples:**
```yaml
# Shapefile with wildcard
- path: "data/*.shp"

# GeoPackage with specific layers
- path: "data/poi.gpkg"
  layers: ["restaurants", "hotels"]

# PostGIS connection
- connection: "PG:host=localhost dbname=gis user=postgres"
  layer: "roads"
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

```yaml
version: 1

grid:
  cell_size: 0.15
  overlap: 0.01
  origin: [0.0, 0.0]

inputs:
  - path: "data/buildings.shp"
  - path: "data/roads.gpkg"
    layers: ["primary", "secondary"]
  - connection: "PG:host=localhost dbname=gis"
    layer: "poi"

output:
  directory: "tiles/"
  filename_pattern: "{x}_{y}.mp"

filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]

error_handling: "continue"
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

## Implementation Note

Configuration parsing and validation implemented in **Story 5.2** (2026-02-10).
