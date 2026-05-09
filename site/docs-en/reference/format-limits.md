# Format Limits

Every format has its constraints. Knowing them allows you to anticipate problems and configure the pipeline accordingly.

---

## Polish Map format (.mp)

| Constraint | Detail |
|-----------|--------|
| **Simple geometries only** | Point, LineString, Polygon. No MultiPolygon or GeometryCollection. |
| **WGS84 coordinates** | Latitude/longitude in decimal degrees (EPSG:4326). Data in local projections must be reprojected. |
| **Max 1024 points per polyline** | Longer lines must be split. |
| **CP1252 encoding** | Default. UTF-8 is possible via `CodePage=65001` but less common. |
| **Text format** | Verbose: a large `.mp` file can reach several hundred MB. |
| **No topology** | Each feature is independent. Topological relationships (road network) are reconstructed by the compiler. |
| **`Data0=` to `Data9=` per feature** | The MP spec allows up to 10 geometries per polyline/polygon (zoom levels). POI remains single-geometry. |

### Multi-geometry fields (Data1..Data9)

`mpforge` + `ogr-polishmap` produce **multi-Data** `.mp` files: each POLYLINE / POLYGON can carry multiple geometries (detailed → simplified). `imgforge` selects the appropriate bucket based on zoom.

```
[POLYLINE]
Type=0x11002
Label=A7
Data0=(45.268551,4.807629),(45.268266,4.806821),...  # max detail, Data0
Data2=(45.268551,4.807629),(45.268334,4.805908),...  # simplified, medium zoom
[END]
```

POI remains single-geometry (MP spec §4.4.3.1). See [mpforge — multi-level profiles](../the-project/mpforge.md#multi-level-profiles) for activation.

### Workaround: multi-geometries

If your data contains MultiPolygon, decompose them before import:

```bash
# With ogr2ogr
ogr2ogr -f "ESRI Shapefile" output.shp input.shp -explodecollections

# With mpforge / ogr-polishmap
# → Decomposition is automatic at write time
```

The ogr-polishmap driver automatically decomposes multi-geometries during writing. mpforge silently filters unsupported types and displays a summary at the end of processing.

## Garmin IMG format

| Constraint | Detail |
|-----------|--------|
| **Max size ~4 GB** | Limit of the internal FAT filesystem in the IMG format. |
| **Fixed resolution per level** | Rendering is not sub-pixel vector. Each zoom level has a fixed coordinate resolution (defined by the `Level` field). |
| **Limited routing** | Only polylines with the `RouteParam` attribute are routable. Garmin routing is not as flexible as desktop software. |
| **Label encoding** | Format 6 (ASCII) only supports A-Z, 0-9. For French accents, Format 9 (CP1252) or Format 10 (UTF-8) is required. |
| **No incremental update** | To modify the map, the entire `gmapsupp.img` must be recompiled. |
| **Subdivisions** | Each tile is split into size-limited subdivisions. Too many features per tile can generate too many subdivisions. |

### Impact on configuration

These limits directly influence configuration choices:

- **`cell_size: 0.15`** — Produces reasonably sized tiles (a few MB each)
- **`--reduce-point-density 3.0`** — Reduces size by simplifying geometries
- **`--min-size-polygon 8`** — Eliminates invisible micro-polygons
- **`--latin1`** — Enables Format 9 for French accents

## Format comparison

| Criterion | Polish Map (.mp) | Garmin IMG (.img) |
|---------|-----------------|-------------------|
| Type | Text (INI) | Binary |
| Human-readable | Yes (text editor) | No |
| Size | Verbose | Compact |
| Editable | Yes | No |
| Usable on GPS | No | Yes |
| Multi-zoom levels | Yes (`Data0..Data9=` buckets) | Yes (native) |
| Routing | Attributes only | Full topology |

The Polish Map format is a **working format** — you inspect it, correct it, validate it. The Garmin IMG format is a **distribution format** — optimized for display and navigation on an embedded device.
