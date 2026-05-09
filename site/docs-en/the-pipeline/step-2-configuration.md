# Step 2: Configuration

Before launching tiling, three configuration files must be prepared that describe **what** to process, **how** to map fields, and **what** metadata to embed in the map.

---

## Configuration file architecture

```
configs/
├── france-bdtopo.yaml         ← Main configuration (sources, grid, output)
├── bdtopo-mapping.yaml        ← Field mapping (source fields → Polish Map)
└── header_template.mp         ← Polish Map header template
```

These three files work together but are separated to enable reuse. The same mapping can serve multiple configurations (Northern France, Southern France, a region...).

## 1. Main configuration (YAML)

This is the central file that drives `mpforge`:

```yaml
# sources.yaml
version: 1

# --- Tiling grid ---
grid:
  cell_size: 0.15        # Cell size in degrees (~16.5 km)
  overlap: 0.005         # Slight overlap to avoid edge artifacts

# --- Data sources ---
inputs:
  # BDTOPO Shapefiles — multi-zone via brace expansion
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  - path: "${DATA_ROOT}/{${ZONES}}/HYDROGRAPHIE/SURFACE_HYDROGRAPHIQUE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  # Contour lines — wildcards + brace expansion
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

  # OSM POIs — regional data, filtered on communes of selected zones
  - path: "${OSM_DATA_ROOT}/gpkg/*-amenity-points.gpkg"
    layer_alias: "osm_amenity"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

# --- Output ---
output:
  directory: "${OUTPUT_DIR}/mp/"
  filename_pattern: "BDTOPO-{col:03}-{row:03}.mp"
  overwrite: true
  base_id: ${BASE_ID}

# --- Polish Map header ---
header:
  name: "BDTOPO-{col:03}-{row:03}"
  copyright: "2026 Allfab Studio - IGN BDTOPO 2025"
  levels: "5"
  level0: "24"
  level1: "22"
  level2: "20"
  level3: "18"
  level4: "16"
  routing: "Y"

# BDTOPO → Garmin types transformation rules
rules: pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml

# --- Error behavior ---
error_handling: "continue"
```

### Environment variables

All YAML fields accept the `${VAR}` syntax to inject environment variables. Variables are substituted **before** YAML parsing, which also works for numeric fields:

```yaml
inputs:
  - path: "${DATA_ROOT}/TRANSPORT/TRONCON_DE_ROUTE.shp"
  - path: "${CONTOURS_DATA_ROOT}/**/COURBE_*.shp"

output:
  directory: "${OUTPUT_DIR}/tiles/"
  base_id: ${BASE_ID}      # u32 — the variable must contain a number
```

```bash
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12
export CONTOURS_DATA_ROOT=./pipeline/data/contours
export OSM_DATA_ROOT=./pipeline/data/osm
export HIKING_TRAILS_DATA_ROOT=./pipeline/data/hiking-trails
export OUTPUT_DIR=./pipeline/output/2025/v2025.12/D038
export BASE_ID=38
export ZONES=D038

mpforge build --config config.yaml --jobs 8
```

!!! tip "Variable validation"
    Use `mpforge validate` to verify that all variables are properly defined before launching a long export. Unresolved variables are reported as warnings:
    ```
    ⚠ Unresolved environment variable: ${DATA_ROOT} (not set)
    ```

Only valid POSIX names are recognized: letters, digits and underscores, starting with a letter or underscore (e.g.: `DATA_ROOT`, `_MY_VAR`). Patterns like `${123}` or `${foo bar}` are ignored.

### Brace expansion (multi-zone)

In addition to classic wildcards (`*`, `?`, `**`), mpforge supports **brace expansion** in file paths. This allows targeting multiple subfolders without matching all the content of a directory:

```yaml
inputs:
  # Single department
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
  # With ZONES=D038 → resolved to: data/.../D038/TRANSPORT/TRONCON_DE_ROUTE.shp

  # Multi-department
  # With ZONES=D038,D069 → resolved to 2 entries:
  #   data/.../D038/TRANSPORT/TRONCON_DE_ROUTE.shp
  #   data/.../D069/TRANSPORT/TRONCON_DE_ROUTE.shp
```

The project's `sources.yaml` configuration file uses this syntax for all BDTOPO layers. The `build-garmin-map.sh` script handles setting the `ZONES`, `DATA_ROOT`, etc. variables automatically from its CLI parameters.

Brace expansion also works in `spatial_filter.source`: geometries from all matched files are automatically unioned into a single spatial filter.

### Grid parameters

| Parameter | Description | Recommended value |
|-----------|-------------|------------------|
| `cell_size` | Size of each tile in degrees | `0.15` (~16.5 km) |
| `overlap` | Overlap between adjacent tiles | `0.01` (~1.1 km) |
| `origin` | Grid southwest corner | `[-5.0, 41.0]` for France |

!!! tip "Choosing cell size"
    - **0.10**: Small tiles, more files, suitable for dense areas (Île-de-France)
    - **0.15**: Good compromise for all of France (~2000 tiles)
    - **0.25**: Large tiles, fewer files, suitable for rural areas

### Tile naming patterns

| Pattern | Result (col=15, row=42) | Description |
|---------|-------------------------|-------------|
| `{col}_{row}.mp` | `15_42.mp` | Simple |
| `{col:03}_{row:03}.mp` | `015_042.mp` | Zero-padded |
| `{seq:04}.mp` | `0157.mp` | Sequential |
| `tile_{col}_{row}.mp` | `tile_15_42.mp` | Custom prefix |

### Geometry generalization {#geometry-generalization}

For some layers, raw geometries (angular polygons, stepped polylines) benefit from being smoothed before export. mpforge provides a `generalize` directive per source that reproduces FME Generalizer (McMaster) type transformations.

```yaml
inputs:
  - path: "${DATA_ROOT}/LIEUX_NOMMES/ZONE_D_HABITATION.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    generalize:
      smooth: "chaikin"       # Algorithm: Chaikin corner-cutting
      iterations: 2           # Number of passes (each pass doubles vertices)
      simplify: 0.00005       # Douglas-Peucker after smoothing (WGS84 degrees, optional)
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `smooth` | string | — | Smoothing algorithm. Only `"chaikin"` is currently available |
| `iterations` | integer | 1 | Number of smoothing passes |
| `simplify` | float | — | Post-smoothing Douglas-Peucker tolerance (in WGS84 degrees) |

!!! tip "FME equivalence"
    **Chaikin corner-cutting** with `iterations: 2` produces a visual result close to FME's **McMaster sliding average** (neighbors=2, offset=25%). Combine with `simplify` to avoid vertex count explosion.

!!! note "Pipeline"
    Generalization is applied **after** clipping on tiles and **before** export to Polish Map. Points (POI) are not affected.

### Multi-level profiles

The inline `generalize:` above produces **a single** simplified geometry (`Data0=`). For richer maps, `mpforge` accepts an **external catalog** that declares **multi-level** profiles: each feature carries multiple geometries, from most detailed to coarsest, consumed by `imgforge` according to zoom.

Activation: one line at the root of `sources.yaml` pointing to an adjacent YAML file:

```yaml
generalize_profiles_path: "../generalize-profiles.yaml"
```

Catalog content — BD TOPO example (`pipeline/configs/ign-bdtopo/generalize-profiles.yaml`):

```yaml
profiles:
  # BATIMENT: intentionally absent → emitted as Data0 only, raw.
  # Preserves buildings as delivered by BD TOPO.

  TRONCON_HYDROGRAPHIQUE:
    levels:
      - { n: 0, simplify: 0.00005 }   # ~5 m: detailed watercourses
      - { n: 2, simplify: 0.00020 }   # ~22 m: mid-zoom

  TRONCON_DE_ROUTE:
    # Conditional dispatch by attribute: first match wins.
    when:
      - field: CL_ADMIN
        values: [Autoroute, Nationale]
        levels:
          - { n: 0, simplify: 0.00002 }   # ~2 m: max routing preservation
          - { n: 2, simplify: 0.00008 }
      - field: CL_ADMIN
        values: [Chemin, Sentier]
        levels:
          - { n: 0, simplify: 0.00010 }
          - { n: 2, simplify: 0.00030 }
    levels:                               # fallback if no when matches
      - { n: 0, simplify: 0.00005 }
      - { n: 2, simplify: 0.00015 }
```

**Semantics**:

| Key | Role |
|---|---|
| `n` | level index in `MpHeader.levels` (`0` = most detailed = `Data0=`, `2` = `Data2=`, etc.) |
| `smooth` | `"chaikin"` or absent (optional) |
| `iterations` | Chaikin iterations, bound `[0, 5]` |
| `simplify` | Douglas-Peucker tolerance in WGS84 degrees, bound `[0, 0.001]` (≈ 110 m) |
| `when` | attribute-based dispatch (first match wins); `when.levels` replace the default `levels` |

**Fail-fast constraints at `load_config`**:

- Any routable layer (`TRONCON_DE_ROUTE`) **must** declare `n: 0` in every visible branch (default AND each `when`). Without this, routing on the `imgforge` side breaks (no `Data0=` = no NET/NOD arc).
- The same `source_layer` cannot appear both as inline `generalize:` **and** in the external catalog → conflict rejected.
- `max(n)` across all profiles must be `< header.levels.len()` (otherwise `imgforge` silently drops out-of-range `DataN`).
- `iterations` outside `[0, 5]` or `simplify` outside `[0, 0.001]` → explicit error at load time.

!!! tip "Strict opt-out"
    `mpforge build --disable-profiles` (or env var `MPFORGE_PROFILES=off`) bypasses **only** the external catalog. Inline `generalize:` directives remain active.

!!! note "Driver prerequisite"
    The multi-Data writer requires an up-to-date `ogr-polishmap` driver. The `build-garmin-map.sh` script auto-detects `~/.gdal/plugins/ogr_PolishMap.so` or `tools/ogr-polishmap/build/ogr_PolishMap.so` and exposes `GDAL_DRIVER_PATH` automatically. If `mpforge` is launched directly, verify that the system plugin is up to date (`ogrinfo --formats | grep Polish` to validate).

## 2. Field mapping

The field mapping translates column names from your source data to the standard fields of the Polish Map format:

```yaml
# bdtopo-mapping.yaml
field_mapping:
  # Main fields
  MP_TYPE: Type          # Garmin type code (e.g.: 0x4e00)
  NAME: Label            # Feature name

  # Location
  Country: CountryName   # Country (e.g.: "France~[0x1d]FRA")
  CityName: CityName     # City/commune
  Zip: Zip               # Postal code

  # Display parameters
  MPBITLEVEL: Levels     # Zoom levels (e.g.: "0-3")
  EndLevel: EndLevel     # Max level (0-9)
```

!!! warning "Where to place the field mapping"
    The path to the mapping file goes in `output.field_mapping_path` (not in `inputs`). This is a common mistake.

### Available Polish Map fields

| Category | Fields |
|----------|--------|
| **Core** | `Type`, `Label`, `EndLevel`, `Levels`, `Data0`-`Data9` (the `Label` field can be transformed via the [`label_case`](../the-project/mpforge.md#label-case-formatting-label_case) option in rules) |
| **Location** | `CityName`, `RegionName`, `CountryName`, `Zip` |
| **POI** | `SubType`, `Marine`, `City`, `StreetDesc`, `HouseNumber`, `PhoneNumber` |
| **Routing** | `DirIndicator`, `RouteParam` |

## 3. Header template

The header defines the metadata common to all tiles:

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
TreeSize=3000
RgnLimit=1024
Transparent=N
Marine=N
Preprocess=F
LBLcoding=9
SimplifyLevel=2
LeftSideTraffic=N
```

### Zoom levels

Levels (`Level0` to `Level3`) control at which zoom each object is visible:

| Level | Bits | Approximate zoom | Visible |
|-------|------|-----------------|---------|
| Level0 = 24 | 24 | Very detailed (~50m) | Everything |
| Level1 = 21 | 21 | Detailed (~500m) | Main roads, water bodies |
| Level2 = 18 | 18 | Medium (~5km) | Motorways, large cities |
| Level3 = 15 | 15 | Wide (~50km) | Metropolises, borders |

## Alternative configuration: all inline

If you don't want separate files, the header can be defined directly in the YAML:

```yaml
header:
  name: "BDTOPO Reunion"
  id: "0"
  copyright: "IGN 2026"
  levels: "4"
  level0: "24"
  level1: "21"
  level2: "18"
  level3: "15"
  tree_size: "3000"
  rgn_limit: "1024"
  lbl_coding: "9"
```

!!! info "Precedence"
    If `template` AND individual fields are both specified, the template takes precedence.

## OSM PBF source configuration

To integrate OpenStreetMap data, add PBF entries in the `inputs` section with `layers`, `layer_alias` and `attribute_filter`:

```yaml
inputs:
  # --- BD TOPO sources (multi-zone via brace expansion) ---
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  # --- OSM GPKG sources ---
  # Geofabrik PBFs are pre-converted to GPKG by download-data.sh (--with-osm)
  # The spatial_filter uses communes from ALL selected zones

  # Amenity POIs (restaurants, pharmacies, parking, etc.)
  - path: "${OSM_DATA_ROOT}/gpkg/*-amenity-points.gpkg"
    layer_alias: "osm_amenity"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

  # Shop POIs (bakeries, supermarkets, etc.)
  - path: "${OSM_DATA_ROOT}/gpkg/*-shop-points.gpkg"
    layer_alias: "osm_shop"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

### Key points

- **Glob path**: `**/*.osm.pbf` automatically includes all PBF files in the folder (multi-region)
- **`layer_alias`**: routes features to the correct ruleset in the categorization rules
- **`attribute_filter`**: GDAL filter applied before loading into memory
- **`spatial_filter`**: restricts features to the communal extent + buffer (recommended since Geofabrik PBFs cover entire regions)
- OSM data is natively in EPSG:4326 — no `source_srs`/`target_srs` needed
- Only `points` and `lines` layers are supported (not `multipolygons` — GDAL OSM driver limitation)
- Set `OSM_MAX_TMPFILE_SIZE=1024` to avoid the "Too many features accumulated" error on large PBFs
- Set `OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES` to suppress invalid geometry warnings
- Set `OSM_CONFIG_FILE=./pipeline/configs/osm/osmconf.ini` to use the project's custom `osmconf.ini`: it exposes the `amenity`, `shop`, `tourism`, `natural` tags as direct GDAL attributes (instead of grouping them in `other_tags`), allowing mpforge's rule engine to match on them. Without this variable, OSM POIs (refuges, springs, named summits, etc.) and `natural=ridge`/`cliff` linear features remain invisible in the final Garmin map.

## Validating the configuration

Before launching a tiling that may take several hours, verify the configuration with `mpforge validate`:

```bash
mpforge validate --config configs/france-bdtopo.yaml
```

Nine checks are performed in sequence:

| # | Check | What is verified |
|---|-------|-----------------|
| 1 | `yaml_syntax` | Valid YAML syntax, correct types (e.g.: `base_id` is indeed a number) |
| 2 | `semantic_validation` | Business rules: consistent grid, non-empty inputs, valid bbox, SRS, base_id in 1..9999, filename pattern, spatial_filter (buffer ≥ 0, non-empty source), generalize (iterations ≥ 1, simplify > 0, known algorithm) |
| 3 | `input_files` | Existence of each source file on disk (after wildcard resolution) |
| 4 | `rules_file` | Parsing and validation of the categorization rules file |
| 5 | `field_mapping` | Parsing of the GDAL field renaming file — **distinct from `garmin-rules.yaml`**: renames raw attribute *keys* before rules apply (e.g.: `NOM_COMMUN` → `NAME`). Useful when the data source changes its column names between editions. |
| 6 | `header_template` | Presence of a header template file, or direct values in the `header:` section |
| 7 | `spatial_filter` | Existence of spatial filtering source files (grouped by unique source) |
| 8 | `generalize` | External catalog (`generalize_profiles_path`) and/or per-input inline directives |
| 9 | `label_case` | label_case consistency in rules: warning if no ruleset rule sets `Label` |

Example output (BDTOPO D038 config without `field_mapping` or header template):

```
✓ yaml_syntax          — Parsed successfully
✓ semantic_validation  — All validations passed
✓ input_files          — 104 files found
✓ rules_file           — 28 rulesets, 351 rules total
- field_mapping        — Not configured (optional — renames raw GDAL attribute keys before applying garmin-rules.yaml)
✓ header_template      — Header configured (direct values, no template file)
✓ spatial_filter       — inputs #21-#103 (83): data/COMMUNE.shp (pattern)
✓ generalize           — catalog: ../generalize-profiles.yaml (8 profil(s), 84 niveau(x))
✓ label_case           — 20 ruleset(s): Voies ferrees: Title, Communes: Title, ...

Config valid. (7/10 checks passed)
```

Example with `field_mapping` configured:

```
✓ field_mapping        — 6 field mappings loaded
```

### JSON report

For CI/CD integration, export the result as JSON:

```bash
mpforge validate --config configs/france-bdtopo.yaml --report validation.json
```

### Diagnosing common errors

Undefined environment variables are reported:

```
  ⚠ Unresolved environment variable: ${DATA_ROOT} (not set)
```

A field with an incorrect type produces an explicit error:

```
✗ yaml_syntax — YAML syntax error: output.base_id: invalid type: string "${BASE_ID}", expected u32
```

!!! tip "Recommended workflow"
    1. Write/modify the configuration
    2. `mpforge validate --config config.yaml` to verify
    3. `mpforge build --config config.yaml --dry-run` to preview tiles
    4. `mpforge build --config config.yaml --jobs 8` to launch production

Exit code: `0` if the configuration is valid, `1` if invalid.
