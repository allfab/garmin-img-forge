# Geometric Simplification Levels

The pipeline has **two independent simplification layers** that apply at different stages. Understanding their interaction allows you to precisely calibrate the size / geometric fidelity trade-off of the produced map.

---

## The two layers

| Layer | Tool | Active by default | Scope |
|--------|-------|:-----------------:|--------|
| Generalization profiles (`generalize-profiles-local.yaml`) | mpforge | Yes | Each feature receives multiple `Data0..Data6` geometries according to zoom; VW/DP + Chaikin algorithms |
| DP line / polygon filters + size filter | imgforge | No (opt-in) | Vertex reduction and micro-polygons at IMG encoding |
| Quantization + SizeFilter + RemoveObsoletePoints | imgforge | Yes | mkgmap r4924 filter chain applied to each subdivision at `n > 0` |

mpforge profiles and imgforge opt-in filters are **cumulative**: data exits the shapefiles, traverses the mpforge profiles (multi-Data simplification), then imgforge applies its own filter chain. The `--no-*` options in imgforge disable active default filters.

---

## The 4 levels — from least to most detailed

| # | mpforge profiles | imgforge — DP/size (opt-in) | imgforge — geom filters (default) | Use case |
|---|:-:|:-:|:-:|---|
| **1 — Quadrant (recommended)** | active | `min-size + merge` | active | **Quadrants, all of France** — mpforge profiles active, no double simplification |
| **2 — Standard** | active | none | active | **Production department** — recommended |
| **3 — Raw mpforge** | disabled | none | active | Measuring the contribution of profiles |
| **4 — Raw data** | disabled | none | disabled | Debug / measuring imgforge filter impact |

!!! warning "Double simplification — trap to avoid"
    The `--reduce-point-density` and `--simplify-polygons` options apply an **additional** DP in imgforge on data **already simplified** by mpforge (`generalize-profiles.yaml`). Combining both degrades geometric precision at detailed zooms (n=0..2, GPS 25–1500 m) without any real size gain.

    **Rule:** if mpforge profiles are active, do not use `--reduce-point-density` or `--simplify-polygons`. These options are only relevant without profiles (`--disable-profiles`).

!!! warning "Level 4 and Garmin hardware"
    Level 4 disables `--no-round-coords`, which produces an IMG with coordinates not quantized on the subdivision grid. Tolerated by QMapShack and QGIS, **potentially non-conformant for firmware rendering** (notably Alpha 100). Reserve for impact measurement and debug — do not use in production.

---

## Prerequisites — downloading data

Before any build, source data must be present in `pipeline/data/`. Use `download-data.sh`:

```bash
./scripts/download-data.sh \
    --zones D038 \
    --bdtopo-version v2026.03 \
    --format SHP \
    --with-contours \
    --with-osm \
    --with-dem
```

This populates `pipeline/data/bdtopo/2026/v2026.03/D038/`, `pipeline/data/contours/`, `pipeline/data/osm/` and `pipeline/data/dem/D038/` — the paths expected by `sources.yaml` via the environment variables below.

---

## Environment variables (standalone commands)

Before calling mpforge or imgforge directly (outside the script), export these variables — the `build-garmin-map.sh` script handles this automatically:

```bash
export DATA_ROOT="./pipeline/data/bdtopo/2026/v2026.03"
export CONTOURS_DATA_ROOT="./pipeline/data/contours"
export OSM_DATA_ROOT="./pipeline/data/osm"
export HIKING_TRAILS_DATA_ROOT="./pipeline/data/hiking-trails"
export OUTPUT_DIR="./pipeline/output/2026/v2026.03/D038"
export BASE_ID=38
export ZONES=D038
mkdir -p "$OUTPUT_DIR/mp" "$OUTPUT_DIR/img"
```

---

## Level 1 — Quadrant (recommended)

mpforge profiles active + imgforge filters `--min-size-polygon` and `--merge-lines`. Recommended for quadrants and all of France.

`--reduce-point-density` and `--simplify-polygons` are **excluded**: mpforge profiles already handle multi-level simplification; adding them would produce double simplification that degrades precision at detailed zooms (see box above).

=== "build-garmin-map.sh (recommended)"

    ```bash
    ./scripts/build-garmin-map.sh \
      --region FRANCE-SE \
      --config pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml \
      --levels "24,23,22,21,20,18,16" \
      --min-size-polygon 8 \
      --merge-lines
    ```

=== "mpforge (standalone)"

    ```bash
    # Profiles are active by default (generalize_profiles_path in sources.yaml)
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8
    ```

=== "imgforge (standalone)"

    ```bash
    imgforge build "$OUTPUT_DIR/mp" \
      --output "$OUTPUT_DIR/img/IGN-BDTOPO-D038-v2026.03.img" \
      --jobs 8 \
      --family-id 1100 --product-id 1 \
      --family-name "IGN-BDTOPO-D038-v2026.03" \
      --series-name "IGN-BDTOPO-MAP" \
      --code-page 1252 --lower-case \
      --levels "24,22,20,18,16" \
      --typ-file pipeline/resources/typfiles/I2023100.typ \
      --route \
      --dem ./pipeline/data/dem/D038 --dem-source-srs EPSG:2154 \
      --packaging legacy \
      --reduce-point-density 4.0 \
      --simplify-polygons "24:12,18:10,16:8" \
      --min-size-polygon 8 \
      --merge-lines
    ```

---

## Level 2 — Standard (production department)

mpforge profiles active, imgforge default filters. This is the reference configuration for a department.

=== "build-garmin-map.sh (recommended)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8
    ```

=== "imgforge (standalone)"

    ```bash
    imgforge build "$OUTPUT_DIR/mp" \
      --output "$OUTPUT_DIR/img/IGN-BDTOPO-D038-v2026.03.img" \
      --jobs 8 \
      --family-id 1100 --product-id 1 \
      --family-name "IGN-BDTOPO-D038-v2026.03" \
      --series-name "IGN-BDTOPO-MAP" \
      --code-page 1252 --lower-case \
      --levels "24,22,20,18,16" \
      --typ-file pipeline/resources/typfiles/I2023100.typ \
      --route \
      --dem ./pipeline/data/dem/D038 --dem-source-srs EPSG:2154 \
      --packaging legacy
    ```

---

## Level 3 — Raw mpforge geometries

Profiles disabled, imgforge defaults. Allows measuring the contribution of generalization profiles on map size and smoothness.

=== "build-garmin-map.sh (recommended)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038 \
      --disable-profiles
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8 \
      --disable-profiles
    ```

=== "imgforge (standalone)"

    ```bash
    imgforge build "$OUTPUT_DIR/mp" \
      --output "$OUTPUT_DIR/img/IGN-BDTOPO-D038-v2026.03.img" \
      --jobs 8 \
      --family-id 1100 --product-id 1 \
      --family-name "IGN-BDTOPO-D038-v2026.03" \
      --series-name "IGN-BDTOPO-MAP" \
      --code-page 1252 --lower-case \
      --levels "24,22,20,18,16" \
      --typ-file pipeline/resources/typfiles/I2023100.typ \
      --route \
      --dem ./pipeline/data/dem/D038 --dem-source-srs EPSG:2154 \
      --packaging legacy
    ```

!!! note "Targeted bypass via environment variable"
    `MPFORGE_PROFILES=off mpforge build --config …` is equivalent to `--disable-profiles`. Useful for CI scripts that do not want to modify arguments.

---

## Level 4 — Complete raw data

Profiles disabled + all default imgforge filters disabled. Reserved for impact measurement and debug.

=== "build-garmin-map.sh (recommended)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038 \
      --disable-profiles \
      --no-round-coords \
      --no-size-filter \
      --no-remove-obsolete-points
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8 \
      --disable-profiles
    ```

=== "imgforge (standalone)"

    ```bash
    imgforge build "$OUTPUT_DIR/mp" \
      --output "$OUTPUT_DIR/img/IGN-BDTOPO-D038-v2026.03.img" \
      --jobs 8 \
      --family-id 1100 --product-id 1 \
      --family-name "IGN-BDTOPO-D038-v2026.03" \
      --series-name "IGN-BDTOPO-MAP" \
      --code-page 1252 --lower-case \
      --levels "24,22,20,18,16" \
      --typ-file pipeline/resources/typfiles/I2023100.typ \
      --route \
      --dem ./pipeline/data/dem/D038 --dem-source-srs EPSG:2154 \
      --packaging legacy \
      --no-round-coords \
      --no-size-filter \
      --no-remove-obsolete-points
    ```

---

## Simplification options reference

### mpforge options

| Option | Description |
|--------|-------------|
| _(default)_ | `generalize-profiles-local.yaml` profiles active — each feature receives `Data0..Data6` according to its VW/DP tolerances |
| `--disable-profiles` | Bypasses the external catalog; inline `generalize:` directives in `sources.yaml` remain active |
| `MPFORGE_PROFILES=off` | Environment variable equivalent of `--disable-profiles` |

### imgforge opt-in options (additional simplification)

| Option | mkgmap reference | Description |
|--------|:-----------------:|-------------|
| `--reduce-point-density 4.0` | `4.0` | Douglas-Peucker on polylines (epsilon in map units) |
| `--simplify-polygons "24:12,18:10,16:8"` | — | DP on polygons by resolution (bits:epsilon) |
| `--min-size-polygon 8` | `8` | Filters polygons < N map units (eliminates micro-surfaces) |
| `--merge-lines` | enabled | Merges adjacent polylines of the same type and label |

!!! tip "When to enable opt-in options"
    For a **department**, the defaults are sufficient (standard level 2).
    For a **quadrant** (≥ 20 departments), enable only `--min-size-polygon 8` and `--merge-lines`. Do not use `--reduce-point-density` or `--simplify-polygons` if mpforge profiles are active (double simplification — see box above).

### imgforge default filters (opt-out)

These filters reproduce the mkgmap r4924 chain — they apply to each subdivision at `n > 0`.

| Option | Description |
|--------|-------------|
| `--no-round-coords` | Disables coordinate quantization on the subdivision grid (`RoundCoordsFilter`) |
| `--no-size-filter` | Disables rejection of sub-pixel features (`SizeFilter`) |
| `--no-remove-obsolete-points` | Disables removal of colinear/spike points after quantization (`RemoveObsoletePointsFilter`) |

---

## Going further

- [Generalization profiles](generalize-profiles.md) — YAML structure, VW/DP algorithms, conditional dispatch, production BDTOPO profiles
- [mkgmap/imgforge comparison](comparaison-mkgmap-imgforge.md) — RGN bytes per level measurements and filter chain analysis
- [Step 3 — Tiling (mpforge)](../the-pipeline/step-3-tiling.md) — complete reference for `build-garmin-map.sh` options
- [Step 4 — Compilation (imgforge)](../the-pipeline/step-4-compilation.md) — geometric optimization and DEM
