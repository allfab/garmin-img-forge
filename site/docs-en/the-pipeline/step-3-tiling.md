# Step 3: Tiling (mpforge)

This is the central step of the pipeline: `mpforge` reads geospatial data, slices it into spatial tiles and generates a Polish Map file (`.mp`) per tile.

---

## Via the build script (recommended)

The `build-garmin-map.sh` script orchestrates mpforge and imgforge in a single command:

```bash
# A single department
./scripts/build-garmin-map.sh --zones D038

# Multiple departments
./scripts/build-garmin-map.sh --zones D038,D069 --jobs 4

# Dry-run to verify paths and commands
./scripts/build-garmin-map.sh --zones D038,D069 --dry-run
```

The script:

- Auto-detects the year and version of BDTOPO data
- Exports environment variables (`DATA_ROOT`, `ZONES`, `OUTPUT_DIR`...) for mpforge
- Chains mpforge (tiling) then imgforge (compilation) automatically
- Manages multi-zone DEM (one `--dem` per department)

### `build-garmin-map.sh` options

#### Geography

| Option | Description | Default |
|--------|-------------|---------|
| `--zones ZONES` | Departments (required): `D038`, `D038,D069` | — |
| `--year YYYY` | BDTOPO year | auto-detected |
| `--version vYYYY.MM` | BDTOPO version | auto-detected |
| `--base-id N` | Garmin base ID (tile IDs = base × 10000 + seq) | first department code |

#### Data paths

| Option | Description | Default |
|--------|-------------|---------|
| `--data-dir DIR` | Data root (BDTOPO path = `{data-dir}/bdtopo/{year}/{version}`) | `./pipeline/data` |
| `--contours-dir DIR` | Contour lines root | `{data-dir}/contours` |
| `--dem-dir DIR` | DEM data root (BD ALTI) | `{data-dir}/dem` |
| `--osm-dir DIR` | OSM data root | `{data-dir}/osm` |
| `--hiking-trails-dir DIR` | GR trails root | `{data-dir}/hiking-trails` |
| `--output-base DIR` | Output directories base | `./pipeline/output` |
| `--config FILE` | Custom mpforge YAML config | `sources.yaml` |

The `--contours-dir`, `--dem-dir`, `--osm-dir` and `--hiking-trails-dir` options allow pointing to existing directories without having to follow the default directory structure. If omitted, they are derived from `--data-dir`.

#### mpforge

| Option | Description | Default |
|--------|-------------|---------|
| `--jobs N` | Parallel workers (common value for both phases) | `8` |
| `--mpforge-jobs N` | mpforge workers only (overrides `--jobs`) | `--jobs` value |
| `--disable-profiles` | Bypasses the external `generalize_profiles_path` catalog (inline `generalize:` remain active). Also accepts env var `MPFORGE_PROFILES=off`. | — |
| `--gdal-driver-path PATH` | Override `GDAL_DRIVER_PATH` to load a fresh `ogr-polishmap`. Auto-resolved (`~/.gdal/plugins/` → `tools/ogr-polishmap/build/`) if empty. | auto |

#### imgforge

| Option | Description | Default |
|--------|-------------|---------|
| `--imgforge-jobs N` | imgforge workers only (overrides `--jobs`) | `--jobs` value |
| `--family-id N` | Garmin Family ID (u16) | `1100` |
| `--product-id N` | Garmin Product ID (u16) | `1` |
| `--family-name STR` | Map name | `IGN-BDTOPO-{ZONES}-{VERSION}` |
| `--series-name STR` | Series name | `IGN-BDTOPO-MAP` |
| `--code-page N` | Encoding code page | `1252` |
| `--levels STR` | Zoom levels (decreasing) | `24,22,20,18,16` |
| `--typ FILE` | TYP styles file | `pipeline/resources/typfiles/I2023100.typ` |
| `--copyright STR` | Copyright message | auto |
| `--no-route` | Disable routing | — |
| `--no-dem` | Disable DEM (shaded relief) | — |

#### imgforge — geometry options (opt-in, recommended for large scopes)

These options propagate the corresponding imgforge flags; they change nothing if omitted. All values aligned with mkgmap defaults.

| Option | Description | Default |
|--------|-------------|---------|
| `--reduce-point-density F` | Douglas-Peucker epsilon for polylines (mkgmap reference: `4.0`) | — |
| `--simplify-polygons SPEC` | DP epsilon by resolution for polygons (example: `"24:12,18:10,16:8"`) | — |
| `--min-size-polygon N` | Filter polygons < N map units (mkgmap reference: `8`) | — |
| `--merge-lines` | Merge adjacent polylines (same type + label). Enabled by default in mkgmap — **activate whenever generating a quadrant or a half**, divides the polyline count by 2-3 and reduces imgforge memory peak. | — |
| `--packaging MODE` | Packaging format: `legacy` (6 FAT per tile) or `gmp` (1 `.GMP` per tile, Garmin NT format — validated on Alpha 100). See [imgforge — Packaging format](../the-project/imgforge.md#packaging-format---packaging). | `legacy` |

!!! tip "When to activate these options"
    For a single department, imgforge's defaults suffice.
    For a quadrant (≥ 20 departments), activate all 4 options: the IMG size drops by 15-25% and imgforge fits in RAM with fewer workers.

#### Build control

| Option | Description |
|--------|-------------|
| `--skip-existing` | Skips `.mp` tiles already present in phase 1 mpforge. **Also skips phase 2 imgforge** if the target `.img` already exists (publish-only mode, see [step 6](step-6-publishing.md#publishing-without-rebuilding)). |
| `--dry-run` | Simulate without executing |
| `-v`, `--verbose` | Verbose mode (`-vv` for very verbose) |
| `--version-info` | Script version |

### Complete example

```bash
export PROJ_DATA=/usr/share/proj
export OSM_CONFIG_FILE=./pipeline/configs/osm/osmconf.ini
export OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES
export OSM_MAX_TMPFILE_SIZE=1024

./scripts/build-garmin-map.sh \
  --zones D038 \
  --year 2025 \
  --version v2025.12 \
  --data-dir ./pipeline/data \
  --contours-dir ./pipeline/data/courbes \
  --dem-dir ./pipeline/data/bdaltiv2 \
  --output-base ./pipeline/output \
  --jobs 4 \
  -v
```

## Direct mpforge command

For fine control, mpforge can be called directly:

```bash
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12
export ZONES=D038
export OUTPUT_DIR=./pipeline/output/2025/v2025.12/D038
export BASE_ID=38

mpforge build --config pipeline/configs/ign-bdtopo/departement/sources.yaml --jobs 8
```

mpforge will:

1. Substitute variables `${DATA_ROOT}`, `${ZONES}`, etc. in the YAML
2. Expand brace patterns `{D038,D069}` to concrete paths
3. Resolve wildcards (`*`, `**`) via glob
4. Index features in a spatial R-tree
5. Compute the tiling grid according to `cell_size` and `overlap`
6. Distribute tiles across N parallel workers
7. For each tile: clip geometries, apply rules, export the `.mp`

### Spatial filtering (optional)

If large sources (contour lines, OSM...) are configured with a `spatial_filter`, mpforge pre-filters features by a reference geometry before tiling. In multi-zone mode, geometries from all matched files are automatically unioned:

```yaml
inputs:
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

## `cell_size` strategy by scope

The `grid.cell_size` parameter in the YAML config controls the mpforge tile size in degrees. **This is the most important lever to adapt when changing scale**. Contrary to natural intuition, the right value is not "the smallest possible for precision": imgforge's RGN splitter automatically subdivides large tiles internally. The real cost of small tiles is **the number of FAT entries in the gmapsupp.img** — which some GPS devices like the Garmin Alpha 100 load into RAM at boot, with a strict ceiling.

| Scope | Recommended `cell_size` | Tile size (~45°N) | Typical tiles | Config |
|-------|------------------------|-------------------|---------------|--------|
| **Department** (1 zone) | `0.30°` (current value of `sources.yaml`) | ~33 × 23 km (770 km²) | 3-10 | `sources.yaml` |
| **Region** (3-10 departments) | `0.30°` | ~33 × 23 km (770 km²) | 30-80 | `sources.yaml` |
| **Quadrant** (20-30 departments) | `0.45°` | ~50 × 35 km (1,750 km²) | 100-150 | dedicated `sources-france-XX.yaml` |
| **Half / Full France** | `0.60°` to `0.90°` | ~70 × 45 km (3,000+ km²) | 150-250 | dedicated `sources-france-XX.yaml` |

!!! note "Before 2026-04-16"
    Until commit [`e6fce3f`](https://github.com/allfab/garmin-img-forge/commit/e6fce3f), `sources.yaml` used `cell_size: 0.15°` (~16 km) inherited from initial tests on a single department. This value generated too many tiles for regional and quadrant scopes (see [the FRANCE-SE battle](../the-project/wins-and-pitfalls.md#the-france-se-quadrants--the-april-2026-battle)). If you are using an old clone, switch `cell_size` to `0.30°` before any build >= regional.

!!! warning "Garmin Alpha 100: FAT limit"
    The Alpha 100 crashes at boot if the gmapsupp.img contains too many FAT entries.
    Empirical rule: **target ≤ 250 tiles × 4-6 subfiles ≈ 1,000-1,500 FAT entries**.
    The mkgmap FRANCE-SUD reference (98 tiles, 3.19 GiB) works; a build
    with 973 tiles (same data, `cell_size: 0.15°`) systematically crashes.

In practice, quadrants share a common config file (`pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml`) that overrides `grid.cell_size` and lowers the `EndLevel` of bulky features (BATIMENT, ZONE_DE_VEGETATION) to lighten zoomed-out views. DOM territories each have their own `outre-mer/<slug>/sources.yaml` (native projection per territory).

## Output

```
output/tiles/
├── 000_000.mp
├── 000_001.mp
├── 001_000.mp
├── 001_001.mp
├── ...
└── 045_067.mp
```

Each `.mp` file is a complete Polish Map file, readable in a text editor:

```
[IMG ID]
Name=BDTOPO France
ID=0
Copyright=IGN 2026
Levels=4
Level0=24
Level1=21
Level2=18
Level3=15
[END]

[POLYLINE]
Type=0x0002
Label=National Road 7
Levels=0-2
Data0=(45.1234,5.6789),(45.1235,5.6790),(45.1240,5.6800)
[END]

[POLYGON]
Type=0x0050
Label=Chartreuse Forest
Data0=(45.35,5.78),(45.36,5.79),(45.35,5.80),(45.35,5.78)
[END]
```

## Useful options in production

### Preview without writing

```bash
# Dry-run: see how many tiles would be generated
mpforge build --config configs/france-bdtopo.yaml --dry-run
```

The pipeline runs normally (source reading, R-tree, clipping) but **no file is created**. Useful for validating the configuration before a long export.

### Resume an interrupted export

```bash
# If the export was interrupted (crash, timeout, Ctrl+C)
mpforge build --config configs/france-bdtopo.yaml --jobs 8 --skip-existing
```

Only missing tiles are generated. Tiles already present on disk are skipped.

### Estimate remaining tiles

```bash
# Combine dry-run and skip-existing
mpforge build --config configs/france-bdtopo.yaml --dry-run --skip-existing
```

### Generate a JSON report

```bash
mpforge build --config configs/france-bdtopo.yaml --jobs 8 --report report.json
```

The report contains export statistics:

```json
{
  "status": "success",
  "tiles_generated": 2047,
  "tiles_failed": 0,
  "tiles_skipped": 150,
  "features_processed": 1234567,
  "skipped_additional_geom": 0,
  "duration_seconds": 1845.3,
  "errors": []
}
```

!!! note "`skipped_additional_geom` field"
    Number of features dropped because at least one additional `Data<n>=` bucket failed to write (FFI error or invalid WKT). Does not appear when the value is `0` (mono-Data mode). See [mpforge — multi-level profiles](../the-project/mpforge.md#multi-level-profiles).

### Progressive verbosity

| Flag | Level | What appears |
|------|-------|-------------|
| _(none)_ | WARN | Progress bar + errors only |
| `-v` | INFO | Main steps, parallelization messages |
| `-vv` | DEBUG | GDAL logs, geometry repairs (disables progress bar) |
| `-vvv` | TRACE | Maximum verbosity (development) |

```bash
# Production — progress bar only
mpforge build --config configs/france-bdtopo.yaml

# INFO: main steps
mpforge build --config configs/france-bdtopo.yaml -v

# DEBUG: detailed GDAL logs (disables progress bar)
mpforge build --config configs/france-bdtopo.yaml -vv

# TRACE: maximum verbosity (development only)
mpforge build --config configs/france-bdtopo.yaml -vvv
```

!!! info "`RUST_LOG` filtering"
    GDAL/GEOS messages are emitted under the `gdal` target, mpforge messages under `mpforge`. This allows fine filtering without full `-vvv`:
    ```bash
    # Silence GDAL, keep mpforge in DEBUG
    RUST_LOG=mpforge=debug,gdal=warn mpforge build --config config.yaml -vv
    ```

!!! note "Informational messages in parallel mode"
    With `--jobs N` (N > 1) and `base_id` configured, mpforge emits **at `-v` only** an INFO message noting that tile IDs vary between two executions (non-deterministic sequential counter). This behavior is expected. For stable IDs, use `{col}_{row}` in `filename_pattern` and omit `base_id`.

## Parallelization

| Dataset size | Recommended threads | Approximate time |
|-------------|---------------------|-----------------|
| 1 department | 4 | ~5 min |
| 1 region | 4-8 | ~15-30 min |
| Full France | 8 | ~2-3h |

```bash
# Check number of available CPUs
nproc

# Adapt thread count
mpforge build --config configs/france-bdtopo.yaml --jobs $(nproc)
```

!!! warning "Memory consumption"
    Each worker opens its own GDAL datasets. With 8 threads and all of France in GeoPackage, expect 8-16 GB of RAM.

## Error handling

In `continue` mode (default), tiles with errors are logged but do not interrupt processing:

```
⚠️  Tile 012_045 failed: GDAL error: Invalid geometry
✅ Processing continues with remaining tiles...
```

In `fail-fast` mode, the first error stops everything:

```bash
mpforge build --config configs/france-bdtopo.yaml --fail-fast
```

## Tile verification

After tiling, you can verify the content of a tile with standard GDAL tools:

```bash
# Read tile metadata
ogrinfo -al output/tiles/015_042.mp

# Count features by layer
ogrinfo -al -so output/tiles/015_042.mp

# Convert to GeoJSON for visualization in QGIS
ogr2ogr -f "GeoJSON" tile_preview.geojson output/tiles/015_042.mp
```
