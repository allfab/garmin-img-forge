# imgforge — The Garmin Compiler

## The problem: an opaque binary format

The **Garmin IMG** format is a proprietary file system containing several sub-files (TRE, RGN, LBL, NET, NOD, DEM...) encoded in a binary format not publicly documented. Until now, two tools could produce it:

- **cGPSmapper** — proprietary, abandoned, Windows only
- **mkgmap** — open-source but written in Java, bulky, slow on large datasets

My goal: a **native Garmin IMG compiler in Rust**, dependency-free, capable of replacing mkgmap while adding modern features.

## The solution: imgforge

**imgforge** is a standalone Rust binary that compiles Polish Map files (`.mp`) into Garmin IMG files. It generates all the necessary sub-files:

| Sub-file | Role |
|----------|------|
| **TRE** | Spatial index, zoom levels |
| **RGN** | Geometries (points, lines, polygons) |
| **LBL** | Labels and encoding (ASCII, CP1252, UTF-8) |
| **NET** | Road network topology |
| **NOD** | Routing nodes (turn-by-turn) |
| **DEM** | Elevation data (hill shading, altitude profiles) |
| **TYP** | Custom symbology (colors, patterns, icons) |
| **TDB** | Map metadata |

## Available commands

| Command | Description |
|---------|-------------|
| `imgforge compile` | Compiles a single `.mp` file to `.img` |
| `imgforge build` | Assembles multiple `.mp` tiles into a complete `gmapsupp.img` |
| `imgforge typ compile` | Compiles a TYP text file (`.txt`) to binary (`.typ`) |
| `imgforge typ decompile` | Decompiles a TYP binary file (`.typ`) to text (`.txt`) |

### `compile`: a single tile

```bash
# Basic compilation
imgforge compile tile_0_0.mp

# With options
imgforge compile tile_0_0.mp \
    --output my_map.img \
    --description "BDTOPO Reunion" \
    --latin1 \
    --reduce-point-density 5.0 \
    --merge-lines
```

### `build`: complete map (gmapsupp)

```bash
# Assemble all tiles into a gmapsupp.img
imgforge build tiles/ \
    --output gmapsupp.img \
    --jobs 8 \
    --family-id 4136 \
    --product-id 1 \
    --family-name "BDTOPO France" \
    --series-name "IGN BDTOPO 2026" \
    --area-name "Metropolitan France" \
    --country-name "France" \
    --country-abbr "FRA" \
    --product-version 100 \
    --copyright-message "IGN BDTOPO 2026" \
    --latin1 \
    --levels "24,20,16" \
    --reduce-point-density 3.0 \
    --min-size-polygon 8 \
    --merge-lines \
    --typ-file bdtopo.typ \
    --dem ./srtm_hgt/ \
    --keep-going \
    --packaging legacy
```

The `build` command is the heart of the production pipeline. It:

1. Scans the directory to find all `.mp` files
2. Compiles each tile in parallel (rayon, N workers)
3. Assembles the compiled tiles into a single `gmapsupp.img`
4. Generates the companion TDB file
5. Optionally integrates the TYP file and DEM data

## Identity and Garmin metadata

The `build` command accepts options to identify the map in Garmin software (BaseCamp, MapInstall):

| Option | Description | Default |
|--------|-------------|---------|
| `--family-id <N>` | Family identifier (unique per map) | 1 |
| `--product-id <N>` | Product identifier | 1 |
| `--family-name <TEXT>` | Map family name | `Map` |
| `--series-name <TEXT>` | Series name (displayed in BaseCamp) | `imgforge` |
| `--area-name <TEXT>` | Geographic area covered | - |
| `--country-name <TEXT>` | Country name | - |
| `--country-abbr <TEXT>` | Country abbreviation (e.g.: `FRA`) | - |
| `--region-name <TEXT>` | Region name | - |
| `--region-abbr <TEXT>` | Region abbreviation | - |
| `--mapname <NAME>` | 8-digit numeric map identifier | - |
| `--product-version <N>` | Version (100 = v1.00) | 100 |
| `--copyright-message <TEXT>` | Copyright embedded in TRE and TDB | - |

## Zoom levels

The `--levels` option defines the bit resolution of each zoom level:

```bash
# Simple format (bits per level, most detailed to widest)
imgforge build tiles/ --levels "24,20,16"

# Explicit format (level:bits)
imgforge build tiles/ --levels "0:24,1:20,2:16"
```

See the [complete zoom levels reference](../reference/zoom-levels.md) for EndLevel correspondence details, file size impact and recommendations.

Additional rendering options:

| Option | Description | Default |
|--------|-------------|---------|
| `--transparent` | Transparent overlay map | false |
| `--draw-priority <N>` | Display priority (overlay) | 25 |
| `--order-by-decreasing-area` | Sort polygons by decreasing area | false |
| `--lower-case` | Allow lowercase in labels (forces Format 9/10) | false |
| `--merge-lines` | Merges adjacent polylines of the same type and label | false |
| `--packaging <MODE>` | Sub-file packaging format: `legacy` (6 FAT files per tile) or `gmp` (1 `.GMP` per tile) | `legacy` |

#### Polyline merging (`--merge-lines`)

The `--merge-lines` option automatically merges adjacent polylines sharing the same Garmin type and label. On large scopes (quadrants, full France), it divides the number of polylines by 2 to 3 and significantly reduces the IMG file size:

```bash
imgforge build tiles/ --merge-lines
```

!!! tip "When to use it?"
    For a single department, the defaults suffice. For a quadrant (≥ 20 departments), enable `--merge-lines`: the IMG size drops by 15-25% and imgforge fits in RAM with fewer workers.

#### Packaging format (`--packaging`)

| Mode | Files generated per tile | Compatibility |
|------|--------------------------|---------------|
| `legacy` | Up to 6 FAT files: `TRE` + `RGN` + `LBL` + (optional) `NET` + `NOD` + `DEM` | All Garmin firmware |
| `gmp` | A single `.GMP` (consolidated Garmin NT format) | NT firmware — validated on Alpha 100 |

The `gmp` mode reduces the number of FAT entries from 6 to 1 per tile, which lightens directory parsing at startup on modern NT firmware. For a full France build (~1,500 tiles), this represents ~9,000 FAT entries in `legacy` vs ~1,500 in `gmp`. See [GMP — Consolidated Garmin NT format](../reference/garmin-img-format.md#gmp--consolidated-garmin-nt-format) for technical details.

## Label encoding

The Garmin format supports three label encodings, controlled by the `--latin1`, `--unicode` or `--code-page` options:

| Format | Encoding | Characters | Option |
|--------|----------|-----------|--------|
| Format 6 | ASCII 6-bit | A-Z, 0-9, space | (default without option) |
| Format 9 | CP1252/CP1250/CP1251 | Latin/Cyrillic accented characters | `--latin1` |
| Format 10 | UTF-8 | All Unicode characters | `--unicode` |

!!! tip "Recommendation"
    For French maps, use `--latin1` (CP1252) which covers all French accented characters while remaining compact. `--unicode` is useful for multilingual maps.

## Geometric optimization

imgforge offers options to reduce file sizes and improve GPS display performance:

### Douglas-Peucker simplification

```bash
# Simplify lines and polygons (threshold in map units)
imgforge build tiles/ --reduce-point-density 3.0
```

Reduces the number of points in geometries by eliminating points that do not significantly contribute to the shape. The higher the value, the more aggressive the simplification.

### Small polygon filtering

```bash
# Remove polygons with area < 8 map units²
imgforge build tiles/ --min-size-polygon 8
```

Eliminates micro-polygons invisible at GPS scale (small buildings, vegetation fragments...).

### Resolution-based simplification

In addition to `--reduce-point-density` (global threshold), `--simplify-polygons` allows a **different Douglas-Peucker threshold per resolution**:

```bash
# DP threshold adapted to each zoom level (resolution:threshold)
imgforge build tiles/ --simplify-polygons "24:12,18:10,16:8"
```

The lower the resolution (wide view), the more aggressive the simplification.

### mkgmap filter chain parity

imgforge implements the mkgmap r4924 geometric filter chain (`normalFilters`), applied at each zoom level n>0:

| Filter | Role | Opt-out flag |
|--------|------|--------------|
| `RoundCoordsFilter` | Quantizes coordinates to the `(1 << shift)` Garmin unit grid — eliminates sub-pixel points | `--no-round-coords` |
| `SizeFilter` | Rejects features whose bbox is too small to be visible at the current resolution | `--no-size-filter` |
| `RemoveObsoletePointsFilter` | Removes post-quantization duplicates, strict colinear points and spikes | `--no-remove-obsolete-points` |

These filters are **active by default**. The `--no-*` flags allow disabling them individually to measure their impact or reproduce a no-filtering baseline:

```bash
# Measure the impact of RoundCoordsFilter alone
imgforge build tiles/ --no-round-coords

# Baseline without the three filters (pre-mkgmap parity behavior)
imgforge build tiles/ --no-round-coords --no-size-filter --no-remove-obsolete-points
```

### Automatic splitting of large features

imgforge automatically splits features with more than **250 points** to avoid overflow in Garmin RGN encoding (variable-width delta):

- **Polylines**: split into segments of ≤250 points with 1 overlap point at junctions
- **Polygons**: split by recursive Sutherland-Hodgman clipping along the longest bounding box axis

This processing is **transparent** — no option to configure.

## Routing control

!!! danger "Experimental routing"
    The road network is **routable on an experimental basis only**. Calculated routes are **indicative and non-prescriptive** — do not rely on them for navigation, regardless of the mode of transport.

    The routable network is currently **hardcoded** based on BD TOPO data attributes. Dynamic configuration based on source routable attributes is not yet supported.

imgforge manages three routing modes:

| Mode | Option | Generates | Usage |
|------|--------|-----------|-------|
| Full | `--route` | NET + NOD | Turn-by-turn navigation |
| Search | `--net` | NET only | Address search without navigation |
| Disabled | `--no-route` | Nothing | Consultation map only |

By default, imgforge **auto-detects**: if roads with `RouteParam` are present in the data, full routing is enabled.

## DEM / Hill Shading

imgforge generates the Garmin DEM sub-file for relief shading and altitude profiles directly on the GPS:

```bash
# From HGT files (SRTM)
imgforge build tiles/ --dem ./srtm_hgt/

# From ASC files (BDAltiv2 IGN, Lambert 93)
imgforge build tiles/ --dem ./bdaltiv2/ --dem-source-srs EPSG:2154

# With DEM resolution control and bicubic interpolation
imgforge build tiles/ --dem ./bdaltiv2/ \
    --dem-source-srs EPSG:2154 \
    --dem-dists 3,3,4,6,8,12,16,24,32 \
    --dem-interpolation bicubic
```

### Supported elevation formats

| Format | Extension | Typical source |
|--------|-----------|---------------|
| HGT | `.hgt` | SRTM 1/3 arc-sec (NASA) |
| ASC | `.asc` | ESRI ASCII Grid (BDAltiv2 IGN) |

Reprojection is built-in via **proj4rs** (zero system dependency): Lambert 93, UTM, LAEA, Web Mercator and any proj4 string are supported.

### DEM options

| Option | Description | Default |
|--------|-------------|---------|
| `--dem <PATH,...>` | Elevation directories or files (`.hgt`, `.asc`) | - |
| `--dem-dists <DISTS>` | Distances between DEM points per zoom level | auto |
| `--dem-interpolation` | `auto`, `bilinear` or `bicubic` | `auto` |
| `--dem-source-srs` | Source SRS for ASC files (e.g.: `EPSG:2154`) | WGS84 |

### Controlling file size with `--dem-dists`

The `--dem-dists` parameter is the **main lever** for controlling the size of the generated file. It controls the density of elevation points encoded for each zoom level. The larger the value, the fewer elevation points in the final file.

Each value corresponds to a zoom level (in `--levels` order). If you provide fewer values than zoom levels, the remaining levels are calculated automatically by doubling the last value.

| Profile | `--dem-dists` | Result |
|---------|---------------|--------|
| High resolution | `1,1,2,3,4,6,8,12,16` | Large file, maximum detail |
| Balanced | `3,3,4,6,8,12,16,24,32` | Good size/quality compromise |
| Compact | `4,6,8,12,16,24,32` | Lightweight file, sufficient for hiking |

!!! warning "Size impact"
    Without `--dem-dists`, imgforge uses a high density by default at all zoom levels, which can produce very large files (e.g.: 500+ MB for a single department). Always specify this parameter in production.

### Interpolation

- **`bilinear`** — Uses 4 neighboring points. Fast, suitable for low-resolution data (SRTM 3 arc-sec).
- **`bicubic`** — Uses 16 points (Catmull-Rom). Produces smoother relief, ideal for high-resolution data (BDAltiv2 25m). Automatically falls back to `bilinear` at grid edges.
- **`auto`** — Bilinear by default (recommended).

## TYP Symbology

A `.typ` file customizes the visual rendering of the map on the GPS (colors, fill patterns, icons):

```bash
imgforge build tiles/ --typ-file bdtopo.typ
```

The TYP file is integrated directly into the final `gmapsupp.img`.

## Resilience

In production, some tiles may contain problematic data. The `--keep-going` option allows the compilation to continue despite errors:

```bash
imgforge build tiles/ --jobs 8 --keep-going
```

Tiles with errors are logged (warning) but do not prevent the generation of other tiles.

## Managing TYP files

The `imgforge typ` command allows converting TYP files between their text form (readable and editable) and their binary form (loaded by the GPS).

### Compile TYP text → binary

```bash
# From a UTF-8 or CP1252 text file (BOM auto-detection)
imgforge typ compile my-style.txt

# Specify encoding explicitly (CP1252 — TYPViewer output)
imgforge typ compile my-style.txt --encoding cp1252

# Choose the output file
imgforge typ compile my-style.txt --output my-style.typ
```

### Decompile TYP binary → text

```bash
# UTF-8 output by default (with BOM)
imgforge typ decompile bdtopo.typ

# CP1252 output for re-import into TYPViewer
imgforge typ decompile bdtopo.typ --encoding cp1252 --output bdtopo.txt
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--encoding <ENC>` | Encoding: `utf8`, `cp1252`, `auto` | `auto` (read) / `utf8` (write) |
| `--output <FILE>` | Output file | Swapped extension (`.txt` ↔ `.typ`) |

!!! warning "CP1252 encoding and TYPViewer files"
    Files produced by TYPViewer v4.6.5 are encoded in **Windows-1252 (CP1252)**. Use `--encoding cp1252` when compiling these files, or let the automatic detection (`auto`) handle the UTF-8 BOM.

    See [TYP file encoding](../reference/typ-styles.md) for details.

## Installation

### Pre-compiled binary

```bash
# Download and extract the archive
wget https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.5.1/imgforge-linux-amd64.tar.gz
tar xzf imgforge-linux-amd64.tar.gz

chmod +x imgforge
sudo mv imgforge /usr/local/bin/
imgforge --version
```

!!! info "Understanding `--version` output"
    The `-N-g<hash>` and `-dirty` suffixes have specific meanings — see the [Binary Versioning](../reference/binary-versioning.md) page for the complete version reading guide and the release workflow.

### Compilation from sources

```bash
# Prerequisites: Rust 1.70+ (no need for GDAL!)
cd tools/imgforge
cargo build --release
```

!!! success "Zero dependency"
    imgforge is a pure Rust binary — it depends neither on GDAL, nor on Java, nor on any system library. This is one of the major advantages over mkgmap.
