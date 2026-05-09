# Zoom Levels and EndLevel

The Garmin IMG format organizes map data into **zoom levels**. Each level corresponds to a coordinate resolution and determines which features are visible when the user zooms in or out on their GPS. Configuring levels correctly is essential to produce a performant and legible map.

---

## Key concepts

### Resolution (bits)

The resolution of a level is expressed in **bits** (1 to 24). The higher the value, the more detailed the level:

| Bits | Precision/unit | Approx. GPS range | Typical usage |
|------|-----------------|-------------------|---------------|
| 24 | ≈ 2.4 m | 25 m – 350 m | Trails, buildings, maximum detail |
| 23 | ≈ 4.8 m | 50 m – 700 m | Neighbourhood, village |
| 22 | ≈ 9.5 m | 100 m – 1.5 km | Town, municipality |
| 21 | ≈ 19 m | 200 m – 3 km | Inter-municipal |
| 20 | ≈ 38 m | 400 m – 6 km | Small department |
| 18 | ≈ 152 m | 2 km – 23 km | District, department |
| 16 | ≈ 610 m | 6 km – 90 km | Region, large territory |
| 14 | ≈ 2.4 km | 25 km – 350 km | National view |

!!! info "Formula"
    1 map unit at N bits = 360 / 2^N degrees. At the latitude of France (~46°), 1 degree latitude ≈ 111 km. The "approx. GPS range" corresponds to the range of display scales where the Garmin firmware uses the N-bit level (10× to 150× the size of one map unit).

### Levels

Levels are numbered starting from **0** (the most detailed). Each level is associated with a resolution:

```
--levels "24,20,16"
```

Creates 3 levels:

| Level | Resolution | GPS zoom |
|-------|------------|----------|
| 0 | 24 bits | Most zoomed in (maximum detail) |
| 1 | 20 bits | Intermediate zoom |
| 2 | 16 bits | Most zoomed out (wide view) |

### EndLevel (in the .mp file)

Each feature (road, building, contour...) carries an `EndLevel` attribute that defines **up to which level it remains visible**:

```
[POLYLINE]
Type=0x01
EndLevel=2
Data0=(45.18,5.16),(45.19,5.17)
[END]
```

The rule is simple: **a feature with `EndLevel=N` is visible at levels 0 to N**.

---

## EndLevel / Levels correspondence

### With `--levels "24,20,16"` (3 levels)

| EndLevel | Visible at levels | Resolutions | Copies in the file |
|----------|-------------------|-------------|----------------------|
| 0 | 0 only | 24 | ×1 |
| 1 | 0, 1 | 24, 20 | ×2 |
| 2 | 0, 1, 2 | 24, 20, 16 | ×3 |

### With `--levels "24,22,20,18,16"` (5 levels)

| EndLevel | Visible at levels | Copies |
|----------|-------------------|--------|
| 0 | 0 | ×1 |
| 1 | 0, 1 | ×2 |
| 2 | 0, 1, 2 | ×3 |
| 3 | 0, 1, 2, 3 | ×4 |
| 4 | 0, 1, 2, 3, 4 | ×5 |

!!! warning "Impact on file size"
    Each additional copy increases the IMG file size. A feature with `EndLevel=7` in a 9-level configuration is written **8 times**. This is the primary lever for controlling output size.

---

## Recommendations

### Number of levels

| Levels | Usage | Size impact |
|---------|-------|---------------|
| 2 (`"24,18"`) | Simple map, minimum size | Reference |
| 3 (`"24,20,16"`) | Good size/navigation compromise | +30-50% |
| 5 (`"24,22,20,18,16"`) | Detailed navigation | +100-150% |
| 9 (`"24,23,...,16"`) | Theoretical maximum | +200-400% |

!!! tip "Recommendation for BD TOPO"
    **3 to 4 levels** with significant resolution jumps (4-6 bit gaps) offer the best compromise for a department.
    For a **quadrant** (FRANCE-SE, SO, NE, NO), the production configuration uses **7 levels `24/23/22/21/20/18/16`**: levels 23 and 21 densify the detailed zone for smooth panning without bloating wide zooms.

### Production 7-level configuration (France quadrants)

The `france-quadrant/` production configuration uses the header `24/23/22/21/20/18/16`. The table below shows the correspondence between index `n`, resolution, GPS range and visible categories:

| n | bits | Approx. GPS range | Production EndLevel | Visible categories |
|---|------|-------------------|--------------------:|---------------------|
| 0 | 24 | 25 m – 350 m | ≥ 0 (all) | Everything: buildings, trails, roads, contours… |
| 1 | 23 | 50 m – 700 m | ≥ 1 | Excluding features with EndLevel=0 (small local roads, buildings) |
| 2 | 22 | 100 m – 1.5 km | ≥ 2 | Same, excluding EndLevel≤1 |
| 3 | 21 | 200 m – 3 km | ≥ 3 | Primary roads, rail, hydrography, vegetation |
| 4 | 20 | 400 m – 6 km | ≥ 4 | Main roads (motorways → departmental), rail |
| 5 | 18 | 2 km – 23 km | = 6 only | Municipalities, residential areas, vegetation, toponymy |
| 6 | 16 | 6 km – 90 km | = 6 only | Municipalities, residential areas, vegetation, toponymy |

!!! note "Max road EndLevel = 4"
    In the quadrant configuration, roads (motorways, national, departmental) have `EndLevel: "4"`. They disappear at zooms n=5 and n=6 (6 km+). Only structural polygons (municipalities, forests, urban areas) and toponymy remain visible at wide zoom (`EndLevel: "6"`).

---

### EndLevel by feature category

The table below provides optimized `EndLevel` values for a 3-level configuration (`--levels "24,20,16"`):

| Category | Garmin type | EndLevel | Justification |
|-----------|-------------|----------|---------------|
| **Motorways** | 0x01 | 2 | Visible at all zooms |
| **National, departmental roads** | 0x04, 0x05 | 2 | Structural network |
| **Municipal roads** | 0x06, 0x07 | 1 | Visible at medium zoom |
| **Tracks, trails** | 0x0a, 0x16 | 0 | Detail only |
| **Main watercourses** | 0x1f | 2 | Landmarks at all zooms |
| **Streams** | 0x18 | 0 | Detail only |
| **Large water bodies** | 0x3c, 0x29 | 2 | Visible everywhere |
| **Small water bodies** | 0x40-0x44 | 0 | Detail only |
| **Buildings** | 0x13 | 0 | Detail only (all scopes) |
| **Forests** | 0x50 | 1 | Visible at medium zoom |
| **Master contours (25m)** | 0x22 | 1 | Visible at medium zoom |
| **Intermediate contours (10m)** | 0x21 | 0 | Detail only |

### Consistency between MP header Levels and `--levels`

The `.mp` files generated by mpforge contain a header with the zoom levels:

```ini
[IMG ID]
Levels=2
Level0=24
Level1=18
[END]
```

The `--levels` option of imgforge **replaces** these values. It is recommended to maintain consistency:

- If the header declares `Levels=2` with `Level0=24, Level1=18`, use `--levels "24,18"` or `--levels "24,20,16"` with adapted EndLevels
- EndLevels in features should **never exceed** the number of levels - 1. An `EndLevel=7` with only 3 levels has no more effect than `EndLevel=2`
- If you change the number of levels, **readjust the EndLevels** in the transformation rules

---

## Complete example

### 3-level configuration optimized for BD TOPO

**mpforge rules** (in `garmin-rules.yaml`):
```yaml
# Motorways: visible at all zooms
- match:
    CL_ADMIN: "Autoroute"
  set:
    Type: "0x01"
    EndLevel: "2"    # levels 0, 1, 2

# Trails: detail only
- match:
    NATURE: "Sentier"
  set:
    Type: "0x16"
    EndLevel: "0"    # level 0 only

# Master contours: medium zoom
- match:
    IMPORTANCE: "1"
  set:
    Type: "0x22"
    EndLevel: "1"    # levels 0, 1
```

**imgforge compilation**:
```bash
imgforge build tiles/ \
    --levels "24,20,16" \
    --output gmapsupp.img \
    --jobs 8
```

### Multi-Data: coupling level ↔ bucket

A feature can carry **multiple geometries** (`Data0=` very detailed, `Data2=` simplified for medium zoom, etc.). `imgforge` selects the appropriate bucket at render time. The `n` index of a `LevelSpec` in `generalize-profiles.yaml` corresponds directly to the **index** in `MpHeader.levels`:

| Index `n` | Header | Bucket emitted | Consumed by imgforge at |
|---|---|---|---|
| `0` | `Level0=24` | `Data0=` | very detailed zoom (`Level0`) |
| `2` | `Level2=20` | `Data2=` | medium zoom (`Level2`) |
| `4` | `Level4=16` | `Data4=` | coarse zoom (`Level4`) |

**Fail-fast constraint**: `max(n)` across all profiles must be `< header.levels.len()` — otherwise `imgforge` silently drops out-of-range buckets. `mpforge` validates at `load_config` and fails with an explicit message.

See [mpforge — multi-level profiles](../the-project/mpforge.md#multi-level-profiles) and [Step 2 — Multi-level profiles](../the-pipeline/step-2-configuration.md#geometry-generalization).

### Estimating size impact

For a mountainous department (Isère, 169 tiles):

| Configuration | Estimated size | Compilation time |
|--------------|----------------|-------------------|
| 9 levels, max EndLevel 7 | ~460 MB | ~35s |
| 3 levels, max EndLevel 2 | ~150-180 MB | ~15-20s |
| 2 levels, max EndLevel 1 | ~120-150 MB | ~10-15s |
