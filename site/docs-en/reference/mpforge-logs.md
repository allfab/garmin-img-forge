# mpforge Logs — Reading Guide

`mpforge` uses the **tracing** library (Rust) to emit structured messages. By default (without `-v`), only warnings (`WARN`) and errors (`ERROR`) are displayed. Each verbosity level unlocks an additional layer of detail.

## Verbosity levels

| Flag | Active level | Recommended usage |
|------|---------------|-----------------|
| *(none)* | `WARN` + `ERROR` | Production — only see problems |
| `-v` | + `INFO` | Phase-by-phase progress tracking |
| `-vv` | + `DEBUG` | Per-feature/tile diagnostics, no progress bar |
| `-vvv` | + `TRACE` | Fine debugging (geometries, rules, clipping) |

!!! tip "Filter by target"
    GDAL/GEOS messages are emitted under the `gdal` target, mpforge messages under `mpforge`. The `RUST_LOG` variable allows fine-grained filtering:
    ```bash
    # See DEBUG mpforge without GDAL noise
    RUST_LOG=mpforge=debug,gdal=warn mpforge build --config config.yaml -vv
    ```

---

## Messages by phase

### Phase 1a — Spatial filters

These messages appear with `-v` when inputs declare a `spatial_filter`.

| Message | Meaning |
|---------|---------------|
| `Building spatial filter geometry for source` | Building the geometric union of the spatial filter for source N (can take several seconds on a large COMMUNE shapefile) |
| `Spatial filter geometries pre-built` | Summary: N filters built, M unique (automatic deduplication by `(source, buffer)`) |

### Phase 1b — Extent analysis

| Message | Meaning |
|---------|---------------|
| `Phase 1b: Scanning source extents` | Scanning the extent of all sources (without loading features) |
| `Extent scan completed` | Scan complete; shows the number of layers and duration |
| `Grid generated` | Tile grid calculated; shows the number of tiles to process |
| `No input sources configured, nothing to process` | ⚠️ No source configured — pipeline ended without generating anything |
| `No tiles generated from extents, pipeline has nothing to process` | ⚠️ The grid is empty (null extent or `bbox` filter too restrictive) |

### Phase 1.5 — Topological pre-simplification

This phase only appears if layers with `topology: true` are declared in `generalize-profiles.yaml` (e.g. `COMMUNE`, `TRONCON_DE_ROUTE`).

| Message | Meaning |
|---------|---------------|
| `Phase 1.5: pré-simplification topologique globale` | Global read of all features from topological layers (without spatial filter) before tiling |
| `Phase 1.5: pré-simplification topologique terminée` | Summary: N features read, M simplified (with duration) |

!!! note "Why a global phase?"
    Topological layers share vertices at boundaries (e.g. adjacent municipalities). A tile-by-tile simplification would produce visible gaps. The global pre-simplification guarantees bit-exact boundaries in all tiles.

### Phase 2 — Tile processing

| Message | Meaning |
|---------|---------------|
| `Phase 2: Processing N tiles (tile-centric)` | Start of parallel/sequential processing of N tiles |
| `Pipeline parallèle : N workers rayon` | Parallel mode with N rayon workers (displayed with `-v` only) |
| `Pipeline séquentiel : 1 thread` | Sequential mode (debug) |
| `Multi-level generalization profiles resolved` | Profiles loaded from `generalize_profiles_path`; lists affected layers and maximum `Data` level |
| `Existing tile skipped` | Tile skipped because the `.mp` file already exists (`--skip-existing`) |

### End of pipeline

| Message | Meaning |
|---------|---------------|
| `Pipeline completed successfully` | All tiles processed without error |
| `Rapport JSON écrit avec succès` | JSON report successfully exported to the specified path |

---

## Common warnings

### mpforge warnings

| Message | Cause | Action |
|---------|-------|--------|
| `WARNING: --jobs exceeds available CPUs, may degrade performance` | `--jobs` > number of physical CPUs | Reduce `--jobs` to `nproc` or less |
| `All tiles share the same fixed ID 'N'` | `output.base_id` absent and multiple tiles have the same fixed ID | Add `base_id` in config or use `{col}_{row}` in `filename_pattern` |
| `Le pattern {seq} produit des noms non-déterministes en mode parallèle` | `{seq}` in `filename_pattern` + `--jobs > 1` | Use `{col}_{row}` for reproducible names |
| `base_id génère les IDs de tuiles via un compteur séquentiel non-déterministe en mode parallèle` | `base_id` configured + `--jobs > 1` | Expected behavior in parallel; stable IDs in sequential mode |
| `Invalid error_handling mode in config, defaulting to 'continue'` | Unknown value in `error_handling` | Use `"continue"` or `"fail-fast"` |
| `No features to export, dataset will be empty` | Empty tile after clipping | Normal for tiles at data borders |
| `Feature rejected during validation: <reason>` | Invalid geometry rejected after repair attempt | Inspect source data (often a digitization artifact) |
| `Intersection produced invalid geometry` | GDAL clipping produced an invalid geometry | Often benign; the feature is skipped for this tile |
| `Skipping POLYGON feature with less than 4 points` | Polygon too small to be valid (unclosed ring) | Filter upstream or ignore |

### GDAL warnings (target: `gdal`)

These messages come from the underlying GDAL/GEOS engine, not directly from mpforge.

| Prefix | Typical cause |
|---------|---------------|
| `WARN gdal: ...` | GDAL/GEOS warning (e.g. self-intersecting geometry, unrecognized SRS) |
| `ERROR gdal: ...` | GDAL error (e.g. corrupted file, unsupported driver) |

GDAL warnings are often benign and correspond to clippings at tile edges. To silence them in production:
```bash
RUST_LOG=gdal=error mpforge build --config config.yaml -v
```

---

## Useful DEBUG messages

With `-vv`, mpforge displays feature-by-feature detail:

| Message | Meaning |
|---------|---------------|
| `Tile has no features, skipping` | Tile entirely empty after R-tree query |
| `Feature outside tile, skipping` | Feature outside tile bounds (normal) |
| `Intersection empty, skipping` | Feature/tile intersection empty (feature at border) |
| `Point geometry, no clipping needed` | POI — no clipping needed |
| `Using repaired geometry for clipping` | Invalid geometry automatically repaired before clipping |
| `Repaired invalid additional_geometry before tile clip` | Multi-Data geometry repaired |
| `MultiPoint: extracted all sub-points` | Multi-geometry decomposed into primitives |

---

## JSON execution report

With `--report report.json`, mpforge writes a structured JSON file:

```json
{
  "status": "success",
  "tiles_generated": 2047,
  "tiles_failed": 0,
  "tiles_skipped": 150,
  "features_processed": 1234567,
  "duration_seconds": 1845.3,
  "errors": [],
  "quality": {
    "unsupported_types": {
      "MultiPolygon": { "count": 12, "sources": ["SURFACE_HYDROGRAPHIQUE"] }
    },
    "multi_geometries_decomposed": {
      "MultiPoint": 45
    }
  }
}
```

| Field | Description |
|-------|-------------|
| `status` | `"success"` or `"failure"` |
| `tiles_generated` | Successfully exported tiles |
| `tiles_failed` | Tiles with errors (non-zero → `status: "failure"`) |
| `tiles_skipped` | Empty or skipped tiles (`--skip-existing`) |
| `features_processed` | Total features processed (all tiles) |
| `duration_seconds` | Total execution duration in seconds (float) |
| `skipped_additional_geom` | Features whose additional `Data<n>=` failed (multi-Data mode only, omitted if 0) |
| `dry_run` | `true` if `--dry-run` (omitted if `false`) |
| `quality.unsupported_types` | Unsupported geometry types (counters + sources) |
| `quality.multi_geometries_decomposed` | Multi-geometries decomposed into primitives (counters) |
| `errors` | Error details per tile: `{ "tile": "003_012", "error": "..." }` |
