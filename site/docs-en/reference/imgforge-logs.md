# imgforge Logs — Reading Guide

`imgforge` uses the **tracing** library (Rust) to emit structured messages. By default (without `-v`), only warnings (`WARN`) and errors (`ERROR`) are displayed — console output is limited to the progress bar and the final summary. Each verbosity level unlocks an additional layer of detail.

## Verbosity levels

| Flag | Active level | Recommended usage |
|------|---------------|-----------------|
| *(none)* | `WARN` + `ERROR` | Production — progress bar + summary only |
| `-v` | + `INFO` | Tile-by-tile tracking, routing messages |
| `-vv` | + `DEBUG` | Encoding diagnostics, bar disabled |
| `-vvv` | + `TRACE` | Fine debugging (bitstream, subdivisions) |

In production, imgforge prints no log messages as long as there are no warnings or errors. The progress bar displays during tile compilation, followed by the structured summary.

---

## Production console output

Without `-v`, imgforge successively displays the progress bar then the summary:

```
[████████████████████████████████████████] 55/55 tiles (100%) — ETA: 0s

✅ Compilation complete — Status: SUCCESS
╔════════════════════════════════════════════════════════╗
║ EXECUTION SUMMARY                                      ║
╠════════════════════════════════════════════════════════╣
║ Tiles compiled:        55                              ║
║ Tiles failed:           0                              ║
║ Points:            182340                              ║
║ Polylines:          94710                              ║
║ Polygons:           31820                              ║
║ IMG size:           50.0 MB                            ║
║ Total duration:      8.4 sec                           ║
╚════════════════════════════════════════════════════════╝
   Output file: gmapsupp.img

💡 Tip: Use -vv for detailed debug logs
```

---

## Messages by level

### INFO level (`-v`)

These messages appear only with `-v`.

| Message | Meaning |
|---------|---------------|
| `Compilation de N tuile(s) .mp` | Number of `.mp` files detected in the input directory |
| `Tuile compilée` | A tile has been compiled successfully (with point/polyline/polygon counters) |
| `--route/--net specified but no RoadID found in .mp data — Routing inactif dans cette tuile : aucun tronçon routable (RoadID inexistant)` | Routing was requested (`--route`) but the `.mp` data contains no `RoadID` — the tile is compiled without NET/NOD. Expected behavior with BD TOPO. |
| `JSON report written` | JSON report written to the path specified by `--report` |
| `Barre de progression désactivée (verbose >= 2)` | In `-vv` mode, the progress bar is disabled to avoid interfering with detailed logs |

### DEBUG level (`-vv`)

With `-vv`, imgforge displays internal processing details:

| Message | Meaning |
|---------|---------------|
| `File is not UTF-8, using CP1252 fallback` | The `.mp` file is not UTF-8 encoded — CP1252 fallback applied (standard BD TOPO) |

---

## Common warnings (`WARN`)

These messages always appear, regardless of verbosity level.

| Message | Cause | Action |
|---------|-------|--------|
| `DEM generation failed: <reason>` | Unable to generate elevation data for this tile | Verify that DEM files cover the tile extent and that the SRS is correct |
| `DEM loading failed: <reason>` | Error loading elevation sources | Check `--dem` paths and existence of HGT/ASC files |
| `N tiles compiled, N errors` | Some tiles failed in `--keep-going` mode | Inspect error messages for the affected tiles |
| `Ignoring malformed level entry: '<value>'` | A value in `--levels` is not a valid integer | Correct the syntax: `"24,20,16"` or `"0:24,1:20,2:16"` |

---

## JSON report (`--report`)

With `--report build-report.json`, imgforge writes a structured JSON file in addition to the console summary:

```bash
imgforge build tiles/ --output gmapsupp.img --jobs 8 --report build-report.json
```

```json
{
  "tiles_compiled": 55,
  "tiles_failed": 0,
  "total_points": 182340,
  "total_polylines": 94710,
  "total_polygons": 31820,
  "duration_ms": 8420,
  "duration_seconds": 8.42,
  "output_file": "gmapsupp.img",
  "img_size_bytes": 52428800
}
```

| Field | Description |
|-------|-------------|
| `tiles_compiled` | Successfully compiled tiles |
| `tiles_failed` | Tiles with errors (non-zero = problem) |
| `total_points` | Total POIs compiled (all tiles) |
| `total_polylines` | Total polylines compiled |
| `total_polygons` | Total polygons compiled |
| `duration_ms` | Execution duration in milliseconds |
| `duration_seconds` | Execution duration in seconds (float) |
| `output_file` | Path of the produced IMG file |
| `img_size_bytes` | IMG file size in bytes |

### Reading in a shell script

```bash
TILES=$(jq '.tiles_compiled' build-report.json)
FAILED=$(jq '.tiles_failed' build-report.json)
DURATION=$(jq '.duration_seconds' build-report.json)
SIZE=$(jq '.img_size_bytes' build-report.json)

echo "Tiles: ${TILES} (${FAILED} failure(s))"
echo "Duration: ${DURATION}s"
echo "Size: $((SIZE / 1048576)) MB"
```

---

## Usage with `build-garmin-map.sh`

The `scripts/build-garmin-map.sh` script automatically passes `--report` to imgforge and reads the metrics from the JSON report at the end of the pipeline to display them in the global summary.
