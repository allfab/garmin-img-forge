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

## Notes

- **Version:** Currently only version `1` is supported
- **Grid cell size:** Typically 0.10 to 0.20 degrees (~11-22 km)
- **Overlap:** Use small overlaps (0.01-0.02) to avoid edge artifacts
- **Error handling:**
  - `"continue"`: Log errors and continue processing (default)
  - `"fail-fast"`: Stop on first error
- **Input paths:** Relative to the config file location or absolute paths
- **Output directory:** Created automatically if it doesn't exist

## Story 5.1 Note

Configuration parsing and validation will be fully implemented in **Story 5.2**.
In Story 5.1, this documentation serves as the specification for the configuration format.
