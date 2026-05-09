# Step 4: Compilation (imgforge)

This is the final software step of the pipeline: `imgforge` compiles all Polish Map tiles into a single `gmapsupp.img` file ready for the GPS.

---

## Via the build script (recommended)

If you use `build-garmin-map.sh`, compilation is automatic (step 2/2). The script passes all imgforge parameters:

```bash
# All-in-one: mpforge + imgforge
./scripts/build-garmin-map.sh --zones D038 --jobs 4

# Customize imgforge via the script
./scripts/build-garmin-map.sh --zones D038,D069 \
    --family-id 1100 --series-name "IGN-BDTOPO-MAP" \
    --levels "24,22,20,18,16" --no-dem

# Point to a custom DEM directory
./scripts/build-garmin-map.sh --zones D038 \
    --dem-dir ./pipeline/data/bdaltiv2 --jobs 4
```

The script automatically manages multi-zone DEM: for each zone in `--zones`, it passes a `--dem {dem-dir}/{zone}` to imgforge. If the DEM directory for a zone does not exist, a warning is displayed and the zone is skipped (compilation continues without DEM for that zone).

See [step 3 (tiling)](step-3-tiling.md#build-garmin-mapsh-options) for the complete reference of `build-garmin-map.sh` options.

## Direct imgforge command

```bash
imgforge build output/tiles/ --output output/gmapsupp.img --jobs 8
```

imgforge will:

1. Scan the directory to find all `.mp` files
2. Parse each file (header, POI, POLYLINE, POLYGON)
3. Compile each tile in parallel (TRE, RGN, LBL, NET, NOD, DEM)
4. Assemble all compiled tiles into a single `gmapsupp.img`
5. Generate the companion TDB file

## Complete production command

### Case 1 — Department (small scope, direct imgforge)

```bash
imgforge build ./pipeline/output/2025/v2025.12/D038/mp/ \
    --output ./pipeline/output/2025/v2025.12/D038/img/gmapsupp.img \
    --jobs 4 \
    --family-id 1100 --product-id 1 \
    --family-name "IGN-BDTOPO-D038-v2025.12" \
    --series-name "IGN-BDTOPO-MAP" \
    --code-page 1252 --lower-case \
    --levels "24,22,20,18,16" \
    --route \
    --typ-file pipeline/resources/typfiles/I2023100.typ \
    --copyright-message "©2026 Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap" \
    --dem ./pipeline/data/dem/D038/ \
    --dem-source-srs EPSG:2154 \
    --keep-going
```

Each option group is detailed lower on this page ([Identity](#map-identity), [Encoding](#encoding), [Geometric optimization](#geometric-optimization), [Symbology](#symbology), [DEM / Hill Shading](#dem--hill-shading)).

### Case 2 — FRANCE-SE quadrant (quadrant scope, 25 departments, via `build-garmin-map.sh`)

For large scopes, the wrapper script drives both phases (download + mpforge + imgforge + publishing). Example validated on Alpha 100 on April 16, 2026:

```bash
# 1. Download data (SHP + contours + OSM + DEM)
./scripts/download-data.sh \
    --region FRANCE-SE \
    --bdtopo-version v2026.03 \
    --format SHP \
    --with-contours --with-osm --with-dem

# 2. Build + publishing
./scripts/build-garmin-map.sh \
    --region FRANCE-SE \
    --base-id 940 \
    --year 2026 \
    --version v2026.03 \
    --data-dir ./pipeline/data \
    --contours-dir ./pipeline/data/contours \
    --dem-dir ./pipeline/data/dem \
    --osm-dir ./pipeline/data/osm \
    --hiking-trails-dir ./pipeline/data/hiking-trails \
    --output-base ./pipeline/output \
    --mpforge-jobs 4 \
    --imgforge-jobs 2 \
    --family-id 940 --product-id 1 \
    --family-name "IGN-BDTOPO-FRANCE-SE-v2026.03" \
    --series-name "IGN-BDTOPO-MAP" \
    --code-page 1252 \
    --levels "24,22,20,18,16" \
    --reduce-point-density 4.0 \
    --simplify-polygons "24:12,18:10,16:8" \
    --min-size-polygon 8 \
    --merge-lines \
    --typ pipeline/resources/typfiles/I2023100.typ \
    --copyright "©2026 Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap Contributors - Licence Ouverte Etalab 2.0" \
    --skip-existing \
    --publish \
    --publish-target local
```

!!! tip "What changes compared to the department case"
    - **Auto-resolved config**: `build-garmin-map.sh` detects the quadrant (`--region FRANCE-SE/SO/NE/NO`) and loads `pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml` with an adapted `cell_size: 0.45°` ([see cell_size strategy](step-3-tiling.md#cell_size-strategy-by-scope)). Add `--config <path>` to force a custom file.
    - **`--mpforge-jobs 4 --imgforge-jobs 2`**: phase 1 tiling with 4 workers, phase 2 compilation with 2 workers to avoid OOM killer on very dense areas (Marseille/Nice/Lyon).
    - **`--reduce-point-density 4.0 --simplify-polygons "24:12,18:10,16:8" --min-size-polygon 8`**: geometric simplification aligned with mkgmap defaults; essential once you exceed a few departments.
    - **`--merge-lines`**: merging of adjacent polylines (default in mkgmap). Significantly reduces IMG size and imgforge memory peak.
    - **`--skip-existing`**: already generated `.mp` tiles are reused. Bonus: if the target `.img` already exists, imgforge phase 2 is also skipped — useful for republishing without rebuilding.

!!! warning "`--hiking-trails-dir` data"
    The `download-data.sh` script does not automatically download GR trails; the `--hiking-trails-dir` flag of `build-garmin-map.sh` points to an optional directory that can be empty. If you don't have trails data, omit this flag or point it to an empty directory — the `france-quadrant/sources.yaml` config handles absence without error.

### Map identity

```bash
--family-id 1234              # Unique family identifier
--product-id 1                # Product identifier
--series-name "BDTOPO France" # Series name (displayed in BaseCamp)
--family-name "IGN BDTOPO"    # Family name
--area-name "Metro France"    # Geographic area covered
```

These metadata are written to the TDB file and are visible in Garmin software (BaseCamp, MapInstall).

### Encoding

```bash
--latin1                      # CP1252: all French accented characters
# or
--unicode                     # UTF-8: all Unicode characters
```

!!! tip "For France"
    `--latin1` suffices and produces more compact files. Use `--unicode` only if you are integrating multilingual data.

### Geometric optimization

```bash
--reduce-point-density 4.0    # Douglas-Peucker simplification (mkgmap default)
--min-size-polygon 8          # Filter micro-polygons
```

These options significantly reduce the final file size (sometimes -30 to -50%) by eliminating details invisible on a GPS screen.

imgforge also applies, by default, the mkgmap r4924 filter chain at each zoom level n>0: coordinate quantization (`RoundCoordsFilter`), sub-pixel feature rejection (`SizeFilter`) and post-quantization colinear removal (`RemoveObsoletePointsFilter`). These filters are transparent in production. To disable them (impact measurement, debug):

```bash
imgforge build tiles/ --no-round-coords --no-size-filter --no-remove-obsolete-points
```

### Symbology

```bash
--typ-file resources/bdtopo.typ  # Customize colors and icons
```

The TYP file defines the visual rendering: road colors, forest fill patterns, POI icons...

### DEM / Hill Shading

```bash
--dem ./pipeline/data/dem/D038/                    # BDAltiv2 (ASC, Lambert 93)
--dem-source-srs EPSG:2154

# Multi-zone: one --dem per department
--dem ./pipeline/data/dem/D038/ --dem ./pipeline/data/dem/D069/ --dem-source-srs EPSG:2154
```

Activates relief shading and altitude profiles on compatible GPS devices.

#### Controlling DEM resolution with `--dem-dists`

DEM can represent a very significant portion of the final file size. The `--dem-dists` parameter controls the density of elevation points encoded for each zoom level:

```bash
# Balanced profile (recommended) — good size/quality trade-off
--dem-dists 3,3,4,6,8,12,16,24,32

# Compact profile — lightweight file, sufficient for hiking
--dem-dists 4,6,8,12,16,24,32

# High-resolution profile — maximum detail, large file
--dem-dists 1,1,2,3,4,6,8,12,16
```

Each value corresponds to a zoom level (in `--levels` order). The larger the value, the fewer elevation points. If you provide fewer values than levels, the remaining ones are calculated by doubling the last value.

!!! warning "Size impact"
    Without `--dem-dists`, imgforge uses a high density by default, which can produce very large files (e.g.: 500+ MB for a single department). **Always specify this parameter in production.**

#### Interpolation

```bash
--dem-interpolation bilinear   # Fast, 4 points (default via auto)
--dem-interpolation bicubic    # Smooth, 16 points (Catmull-Rom)
```

`bicubic` is recommended with high-resolution data (BDAltiv2 25m) for smoother relief. `bilinear` suffices for SRTM data.

#### Complete example with optimized DEM

```bash
imgforge build ./pipeline/output/2025/v2025.12/D038/mp/ \
    --output ./pipeline/output/2025/v2025.12/D038/img/gmapsupp.img \
    --jobs 4 \
    --dem ./pipeline/data/dem/D038/ \
    --dem-source-srs EPSG:2154 \
    --dem-dists 3,3,4,6,8,12,16,24,32 \
    --dem-interpolation bicubic \
    --code-page 1252 --lower-case \
    --levels "24,22,20,18,16" \
    --typ-file pipeline/resources/typfiles/I2023100.typ \
    --keep-going \
    -vv
```

### Resilience

```bash
--keep-going                  # Continue if a tile fails
```

In production, some tiles may contain invalid geometries. `--keep-going` ignores them and continues compilation.

## Compile a single tile (debug)

To test or debug, compile an isolated tile:

```bash
imgforge compile output/tiles/015_042.mp \
    --output test.img \
    --description "Test tile Chartreuse" \
    --latin1 \
    -vv
```

The `-vv` (DEBUG) mode displays encoding details — useful for diagnosing issues.

## Compilation report

imgforge's standard output is a JSON report:

```json
{
  "tiles_compiled": 2047,
  "total_points": 152340,
  "total_polylines": 87210,
  "total_polygons": 34560,
  "errors": [],
  "duration_ms": 234000,
  "output_file": "gmapsupp.img",
  "output_size_bytes": 524288000
}
```

## Zoom levels

imgforge supports zoom level configuration via `--levels`:

```bash
# Simple format: list of resolutions (bits)
imgforge build tiles/ --levels "24,20,16"

# Explicit format: level:bits
imgforge build tiles/ --levels "0:24,1:20,2:16"
```

If not specified, imgforge uses the levels defined in the header of each `.mp` file.

Each level creates a set of subdivisions containing features whose `EndLevel` is greater than or equal to the level number. The more levels, the larger the file because features are duplicated.

| Configuration | Levels | Relative size |
|--------------|--------|---------------|
| `"24,18"` | 2 | Reference |
| `"24,20,16"` | 3 | +30-50% |
| `"24,22,20,18,16"` | 5 | +100-150% |
| `"24,23,22,21,20,19,18,17,16"` | 9 | +200-400% |

!!! tip "Recommendation"
    **3 levels** with jumps of 4+ bits (`"24,20,16"`) offer the best size/navigation trade-off. Consecutive levels (24→23→22) bring no perceptible visual difference on a Garmin GPS.

    See the [complete zoom levels reference](../reference/zoom-levels.md) to understand the correspondence with `EndLevel`.

## Routing control

!!! danger "Experimental routing"
    The road network is **routable on an experimental basis only**. Calculated routes are **indicative and non-prescriptive** — do not rely on them for navigation, regardless of the mode of transport.

```bash
# Full turn-by-turn navigation (NET + NOD)
imgforge build tiles/ --route

# Address search only (NET only, no navigation)
imgforge build tiles/ --net

# Consultation map only (no routing)
imgforge build tiles/ --no-route
```

By default, imgforge auto-detects: if roads with `RouteParam` are present, full routing is enabled.

## Packaging format (`--packaging`)

By default, imgforge generates 6 separate FAT files per tile (`TRE`, `RGN`, `LBL`, `NET`, `NOD`, `DEM`) — this is `legacy` mode, compatible with all Garmin firmware.

```bash
# Legacy mode (default)
imgforge build tiles/ --packaging legacy

# GMP mode — 1 .GMP file per tile (consolidated Garmin NT format)
imgforge build tiles/ --packaging gmp
```

| Mode | Files per tile | Compatibility |
|------|----------------|---------------|
| `legacy` | Up to 6 FAT files: `TRE` + `RGN` + `LBL` + (optional) `NET` + `NOD` + `DEM` | All Garmin firmware |
| `gmp` | A single `.GMP` (consolidated Garmin NT format) | NT firmware — validated on Alpha 100 |

The `gmp` mode significantly reduces the number of FAT entries in the `gmapsupp.img` (6 → 1 per tile), which lightens boot on modern NT firmware and corresponds to the format of commercial Garmin maps (Topo France v6 Pro, Topo Active...). See [IMG Format — GMP](../reference/garmin-img-format.md#gmp--consolidated-garmin-nt-format) for technical details and implementation pitfalls.

The `build-garmin-map.sh` script exposes this flag via `--packaging MODE`.

## Verifying the result

```bash
# File size
ls -lh output/gmapsupp.img

# Verify with mkgmap (optional, for comparison)
java -jar mkgmap.jar --check-roundabouts output/tiles/*.mp
```

The `gmapsupp.img` file is now ready for installation on the GPS.
