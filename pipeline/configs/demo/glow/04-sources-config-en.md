# sources.yaml — Grid, layer and header

## Tiling grid

```yaml
grid:
  cell_size: 0.225   # ~25 km per tile
  overlap: 0.005     # Overlap to avoid edge artifacts
```

mpforge splits the department into independent square tiles.
Each tile becomes a **Polish Map** file (`.mp`).

## Declaring an input layer

```yaml
- path: "./pipeline/data/IGN-BDTOPO/2026/v2026.03/D038/TRANSPORT/TRONCON_DE_ROUTE.shp"
  source_srs: "EPSG:2154"   # Lambert-93 (IGN projection)
  target_srs: "EPSG:4326"   # WGS84 (required by Garmin GPS)
  dedup_by_field: ID        # Removes duplicates on IGN identifier
```

## Polish Map header — 7 zoom levels

```yaml
header:
  levels: "7"
  level0: "24"   #   ~1 m/px — maximum zoom (pedestrian navigation)
  level1: "23"   #   ~2 m/px
  level2: "22"   #   ~5 m/px
  level3: "21"   #  ~10 m/px
  level4: "20"   #  ~20 m/px
  level5: "18"   #  ~80 m/px
  level6: "16"   # ~320 m/px — wide view (area overview)
  routing: "Y"   # Routing enabled
```
