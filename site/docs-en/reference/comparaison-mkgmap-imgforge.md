# mkgmap / imgforge Comparison — IMG Size and Geometric Smoothing

This page analyzes why mkgmap r4924 and imgforge produce IMGs of different sizes and visually distinct geometries, from the same `.mp` tile. It is based on direct measurements of the `BDTOPO-001-004` tile (Vienne, D038) and the Java source code of mkgmap r4924 (`build/MapBuilder.java` L929-1354).

---

## Introduction — the question asked

For the same MP tile (`BDTOPO-001-004.mp`, 43 MB), mkgmap r4924 and imgforge produce IMGs of different sizes. The size ratio mentioned upstream ("6×") resulted from a non-rigorous comparison (different scopes: number of tiles, included sections). The instrumented measurement on the **same** tile `BDTOPO-001-004` gives a very different result, detailed below.

!!! note "Historical note"
    A first audit (commit `8acb0c2`, April 2026) had measured that the geometric RGN section of imgforge was **1.58× larger** than that of mkgmap for this same tile. Since then, the `EndLevel filtering` fix (commit `6478c47`) has reduced this section by **63%**. The current state (commit `975f432`) is documented in the following section.

---

## §1 — Reference measurements (current state)

### Measurement conditions

| Parameter | Value |
|---|---|
| Tile | `BDTOPO-001-004` (Vienne, D038, the densest) |
| `.mp` file | `pipeline/output/2026/v2026.03/D038/mp/BDTOPO-001-004.mp` (43 MB) |
| imgforge commit | `975f432` (HEAD, 2026-04-26) |
| mpforge profile | `generalize-profiles-local.yaml` (8 layers) |
| Measurement tool | `scripts/debug/bytes-per-level.py` |

### Method

For each IMG, the script extracts the TRE from the main sub-map, reads the `map levels` section and the `subdivisions` section, then aggregates the deltas `rgn_offset(i+1) − rgn_offset(i)` by level. This is the measurement used by the Garmin firmware to select data to render. It excludes extended sections (extended types, NET, NOD, RGN2-RGN5).

### Reproduction commands

```bash
# mkgmap measurement (IMG already available)
python3 scripts/debug/bytes-per-level.py tmp/mkgmap-vienne-build.img

# Single-tile imgforge build (sub-map 00380042)
mkdir -p /tmp/vienne-mp
cp pipeline/output/2026/v2026.03/D038/mp/BDTOPO-001-004.mp /tmp/vienne-mp/
imgforge build /tmp/vienne-mp --output /tmp/vienne-local.img

# imgforge measurement (analyze sub-map 00380042, not 00011855 which is the GMP container)
python3 scripts/debug/bytes-per-level.py /tmp/vienne-local.img  # reads the first sub-map
# Note: bytes-per-level.py reads the first alphabetical sub-map; for imgforge GMP,
# the tile sub-map (00380042) comes after the container (00011855). See §A.
```

### Comparison table

> Convention — column **mk÷if**: value > 1 means mkgmap has more bytes than imgforge at that level.

| n | bits (1) | mkgmap subdivs | mkgmap RGN | imgforge subdivs | imgforge RGN | mk÷if |
|---|----------|----------------|------------|------------------|--------------|-------|
| 6 | 16 (inh.)| 1              | 0          | 1                | 0            | —     |
| 5 | 18       | 95             | 129 713    | 4                | 8 200        | **15.8×** |
| 4 | 20       | 183            | 216 825    | 33               | 54 419       | **3.98×** |
| 3 | 21       | 188            | 242 083    | 41               | 68 354       | **3.54×** |
| 2 | 22       | 225            | 337 546    | 92               | 115 073      | **2.93×** |
| 1 | 23       | 226            | 381 806    | 101              | 143 296      | **2.66×** |
| 0 | 24       | 512            | 510 023    | 609              | 663 942      | 0.77× |
| **Σ** | | **1 430** | **1 817 996** | **881** | **1 053 284** | **1.73×** |

(1) "inh." for *inherited*: level 6 inherits its subdivision from level 5; no feature is emitted directly.

### Key observations

1. **imgforge is now 1.73× more compact than mkgmap** in geometric RGN data: 1 053 284 bytes vs 1 817 996 bytes. The `EndLevel filtering` fix (commit `6478c47`) eliminated erroneous emission at wide levels.

2. **The gap is greatest at wide levels**: at n=5, mkgmap stores 15.8× more data. But at n=0 the trend reverses: imgforge has 30% more data at maximum detail level.

3. **mkgmap includes more features at wide levels**: 95 subdivisions at n=5 vs 4 for imgforge. This is not poor quality — it is a design choice: mkgmap keeps more features visible when zooming out but simplifies them aggressively via its filter chain. imgforge applies `EndLevel` filtering (features absent from levels above their EndLevel) but does not simplify the geometry of present features.

4. **The total single-tile IMG remains larger for imgforge** (4.9 MB vs 3.8 MB for mkgmap), despite smaller ordinal geometry. The difference comes from the GMP container overhead and extended sections: the declared imgforge RGN is 3.37 MB of which 2.32 MB are extended types/NET/NOD, vs 3.79 MB declared mkgmap of which 1.97 MB are extended. It is not the geometric section that explains the difference in total IMG size — it is the format overhead.

### Historical baseline (before EndLevel fix)

| n | mkgmap RGN | imgforge RGN (8acb0c2) | imgforge RGN (975f432) | Δ after fix |
|---|------------|------------------------|------------------------|-------------|
| 5 | 129 713    | 346 182                | 8 200                  | −97.6%      |
| 4 | 216 825    | 394 252                | 54 419                 | −86.2%      |
| 3 | 242 083    | 434 098                | 68 354                 | −84.3%      |
| 2 | 337 546    | 496 136                | 115 073                | −76.8%      |
| 1 | 381 806    | 548 017                | 143 296                | −73.9%      |
| 0 | 510 023    | 657 224                | 663 942                | +1.0%       |
| **Σ** | **1 817 996** | **2 875 909** | **1 053 284** | **−63.4%** |

Level 0 is almost identical before and after the fix: features with `EndLevel=0` were not affected by the bug. The improvement is entirely concentrated on levels n=1..5.

---

## §2 — The mkgmap filter chain

### Overview

mkgmap applies a filter chain per resolution, in two passes depending on the feature type. The reference source code is `MapBuilder.java` L929-1354.

**Preliminary gate (L929-930):**

```java
lines = lines.stream().filter(l -> l.getMinResolution() <= res).collect(Collectors.toList());
shapes = shapes.stream().filter(s -> s.getMinResolution() <= res).collect(Collectors.toList());
```

This gate is the equivalent of imgforge's `EndLevel filtering` (fix TD-1). A feature whose `MinResolution > res` is not simplified — it is not emitted at all.

### Polyline chain (L1248-1283)

For normal polylines (`res < 24`, all levels except n=0):

```
RoundCoordsFilter
→ SizeFilter(MIN_SIZE_LINE=1)
→ RemoveObsoletePointsFilter
→ DouglasPeuckerFilter(2.6 × (1 << shift))
→ RemoveEmpty → LineSplitterFilter → LinePreparerFilter → LineAddFilter
```

!!! note "Contour lines — different order"
    For contour lines (`isContourLine`) and overview features, mkgmap uses `keepParallelFilters` with a different order: **DP first**, then RoundCoords → SizeFilter → RemoveObsolete. This prevents RoundCoords from introducing false colinearities before simplification.

### Polygon chain (L1313-1335)

```
PolygonSplitterFilter
→ RoundCoordsFilter
→ RemoveObsoletePointsFilter
→ SizeFilter(min-size-polygon=8)
→ DouglasPeuckerFilter(2.6 × (1 << shift))
→ RemoveEmpty → LinePreparerFilter → ShapeAddFilter
```

### Default parameters

| CLI parameter | Default value | Effect |
|---|---|---|
| `reduce-point-density` | `2.6` | DP multiplier (coefficient) |
| `reduce-point-density-polygon` | `−1` (= same as lines) | DP multiplier for polygons |
| `min-size-polygon` | `8` | Min size (× shift) for polygons |
| `merge-lines` | *not enabled* | Merges segments of same type — requires `--merge-lines` |
| `MIN_SIZE_LINE` | `1` (Java constant) | Min size for polylines |

---

## §3 — Filter by filter

### RoundCoordsFilter

Quantizes each coordinate to the resolution grid `(1 << shift)` Garmin units. One Garmin unit ≈ 2.14 m (latitude, at French latitudes). **Disabled at res=24** (`enableLineCleanFilters` requires `res < 24`).

| n | res | shift | Cell size | Effect |
|---|-----|-------|-------------------|-------|
| 0 | 24  | —     | *filter disabled (res=24)* | None |
| 1 | 23  | 1     | 2 units ≈ 4 m    | Micro-jitter < 4 m merged |
| 2 | 22  | 2     | 4 units ≈ 9 m    | — |
| 3 | 21  | 3     | 8 units ≈ 17 m   | Curves < 17 m merged |
| 4 | 20  | 4     | 16 units ≈ 34 m  | — |
| 5 | 18  | 6     | 64 units ≈ 137 m | All detail < 137 m disappears |
| 6 | 16  | 8     | 256 units ≈ 549 m | Only large inflections |

**Special contour line mode**: for each intermediate point, mkgmap tests the **4 corners of the cell** and chooses the one that minimizes the sum of distances to the segment before and after (`calcDistortion`). This "best-fit" mode produces a trace naturally aligned on the grid, visually smoother than naive rounding.

!!! success "Implemented in imgforge"
    imgforge applies `RoundCoordsFilter` by default at levels n>0 (gated on `shift > 0`). Coordinates are quantized to the grid `(1 << shift)` Garmin units before RGN encoding, eliminating sub-pixel points. Disableable with `--no-round-coords` to measure isolated impact or compare with a non-quantized baseline.

### SizeFilter

Removes features whose bounding box is too small to be visible at the current resolution. **Disabled at res=24.**

```
maxDimension < minSize × (1 << shift)
```

| Feature | minSize | shift=6 (n=5) | threshold ≈ |
|---|---|---|---|
| Polyline | `MIN_SIZE_LINE = 1` | 1 × 64 = 64 units | ~137 m |
| Polygon | `min-size-polygon = 8` (default) | 8 × 64 = 512 units | ~1 096 m |

Example: a section of `CONSTRUCTION_LINEAIRE` (wall, hedge) 20 m long disappears from n=3 (shift=3, threshold = 8 units ≈ 17 m for lines). This reduces the RGN at wide levels without affecting maximum zoom.

!!! success "Implemented in imgforge"
    imgforge applies `SizeFilter(MIN_SIZE_LINE=1)` by default at levels n>0. Polylines and polygons whose max-dim bounding box is below the threshold `1 × (1 << shift)` are eliminated before RGN encoding. Disableable with `--no-size-filter`.

### DouglasPeuckerFilter

The maximum error is scaled exponentially by level:

```java
// DouglasPeuckerFilter.java L43
maxErrorDistance = filterDistance * (1 << config.getShift());
// with filterDistance = 2.6 (default) and shift = 24 - res
// Disabled at res=24 (enableLineCleanFilters requires res < 24)
```

| n | res | shift | DP error (units) | DP error (meters) | Local mpforge profile (municipal roads) |
|---|-----|-------|---------------------|---------------------|------------------------------------------|
| 0 | 24  | —     | *filter disabled*   | —                   | simplify_vw 0.000003° ≈ 0.33 m |
| 1 | 23  | 1     | 5.2                 | ~11 m               | 0.000005° ≈ 0.56 m |
| 2 | 22  | 2     | 10.4                | ~22 m               | 0.000010° ≈ 1.1 m |
| 3 | 21  | 3     | 20.8                | ~45 m               | 0.000018° ≈ 2 m |
| 4 | 20  | 4     | 41.6                | ~89 m               | 0.000070° ≈ 7.8 m |
| 5 | 18  | 6     | 166.4               | ~356 m              | 0.000130° ≈ 14 m |
| 6 | 16  | 8     | 665.6               | ~1 426 m            | 0.000300° ≈ 33 m |

**mkgmap is 10 to 40× more aggressive** than our profile at wide zoom levels (n=4..6). At n=6, mkgmap tolerates an error of ~1.4 km — only the large inflections of the road network survive.

### RemoveObsoletePointsFilter

Eliminates after each filter:

- Duplicate points (identical coordinates after rounding)
- Strictly colinear points (`STRICTLY_STRAIGHT`)
- Spikes (abrupt direction reversals)

Applied after `RoundCoordsFilter` for polylines and polygons. Post-quantization cleanup is crucial: `RoundCoordsFilter` may project two distinct coordinates to the same rounded value, producing duplicates that `RemoveObsolete` immediately eliminates.

!!! success "Implemented in imgforge"
    imgforge applies `RemoveObsoletePointsFilter` by default at levels n>0, after `RoundCoordsFilter`. Post-quantization duplicates, strictly colinear points and spikes are removed before RGN encoding. Disableable with `--no-remove-obsolete-points`.

### SmoothingFilter — dead code

```java
// SmoothingFilter.java — never instantiated in MapBuilder r4924
// stepsize = 5 << shift — sliding average of adjacent point groups
```

`SmoothingFilter` is present in the mkgmap r4924 sources but **never instantiated** in `MapBuilder.java`. It is a historical artifact. The visual smoothing effect of mkgmap comes from `RoundCoordsFilter` (grid quantization, best-fit contour line mode) and `DouglasPeuckerFilter` (elimination of micro-deviations), not from this filter.

The `smooth: chaikin` smoothing of imgforge/mpforge (curve subdivision iterations) has no equivalent in mkgmap r4924 — both tools smooth via orthogonal mechanisms.

### LineMergeFilter — advanced option

`LineMergeFilter` is instantiated in `MapBuilder.java` L933 only if the `--merge-lines` option is passed to mkgmap. **Not enabled by default.** This filter merges segments of the same type that are adjacent to reduce the number of distinct features. It is not included in the standard chain documented here.

---

## §4 — Comparison with our pipeline

### What mpforge/imgforge implements

| mkgmap mechanism | mpforge/imgforge equivalent | Coverage |
|---|---|---|
| Gate `minResolution <= res` | `filter_features_for_level` (`writer.rs`) | ✅ Fix TD-1 commit `6478c47` |
| `DouglasPeuckerFilter` | `simplify` / `simplify_vw` in YAML profile | ⚠️ 8 layers only, tolerances 10-40× below mkgmap at wide levels |
| `RoundCoordsFilter` | `round_coords` (`filters/round_coords.rs`) — active by default n>0, `--no-round-coords` | ✅ |
| `SizeFilter` lines/polygons | `passes_size_filter` (`filters/size.rs`) — active by default n>0, `--no-size-filter` | ✅ |
| `RemoveObsoletePointsFilter` | `remove_obsolete_points` (`filters/remove_obsolete_points.rs`) — active by default n>0, `--no-remove-obsolete-points` | ✅ |
| `smooth: chaikin` smoothing | `geometry_smoother.rs` | ⚠️ A few polygon layers only, no scaling by resolution |
| `LineMergeFilter` | Absent (non-standard mkgmap option) | — |
| `SmoothingFilter` | N/A (dead mkgmap code) | — |

### Layer coverage by generalize-profiles-local.yaml

Out of **124 863 features** in `BDTOPO-001-004.mp` (Vienne), the local profile covers **8 layers** representing **34% of features**. The remaining 66% traverse the pipeline without per-level geometric simplification.

**Covered layers (profile with simplification):**

| Layer | Features | Max EndLevel | Notes |
|---|---|---|---|
| `TRONCON_DE_ROUTE` | 24 571 | 6 | VW algorithm, dispatch by `CL_ADMIN` |
| `ZONE_DE_VEGETATION` | 12 263 | 6 | Chaikin + DP |
| `TRONCON_HYDROGRAPHIQUE` | 3 205 | 6 | DP |
| `CONSTRUCTION_LINEAIRE` | 1 038 | 4 | DP — bridges, walls, hedges |
| `SURFACE_HYDROGRAPHIQUE` | 531 | 6 | Chaikin + DP |
| `COURBE` | 767 | 4 | DP — contour lines |
| `ZONE_D_HABITATION` | 116 | 6 | Chaikin + DP |
| `COMMUNE` | 55 | 6 | VW, topology |
| **Covered subtotal** | **42 546** | | **34.1%** |

**Main uncovered layers:**

| Layer | Features | EndLevel | Type | Impact |
|---|---|---|---|---|
| `BATIMENT` | 66 161 | 0 | Polygon | Intentionally excluded (see note) |
| `FRANCE_GR` | 4 480 | 4 | Polyline | Already simplified inline (34K pts→393 pts at levels 1-4) |
| `osm_amenity` | 3 545 | var. | POI | Points — no complex geometry |
| `TOPONYMIE` | 2 641 | var. | POI | Points |
| `LIGNE_OROGRAPHIQUE` | 2 529 | **0** | Polyline | EndLevel=0 only → profile without effect |
| `ZONE_D_ACTIVITE_OU_D_INTERET` | 840 | var. | Polygon | Economic zones |
| `osm_shop` | 710 | var. | POI | Points |
| `PYLONE` | 564 | var. | POI | Points |
| `TERRAIN_DE_SPORT` | 262 | var. | Polygon | |
| `TRONCON_DE_VOIE_FERREE` | 225 | **4** | Polyline | Identical geometry at levels 0-4 — profile candidate |
| `DETAIL_HYDROGRAPHIQUE` | 123 | var. | Polygon/Polyline | |
| Other | 1 312 | — | — | CIMETIERE, LIGNE_ELECTRIQUE, etc. |
| **Uncovered subtotal** | **82 317** | | | **65.9%** |

!!! note "BATIMENT — intentional exclusion"
    The 66 161 buildings have EndLevel=0: they only appear at maximum detail level (n=0, res=24) where geometric filters are disabled. Adding them to the profile would have no effect on the RGN at wide levels, and building simplification produces non-orthogonal angles visible on GPS.

!!! note "LIGNE_OROGRAPHIQUE — useless profile"
    The 2 529 features all have EndLevel=0 (embankments, levees, quarries). They only emit at n=0 where DP is disabled. Adding these layers to the profiles catalog would bring no RGN gain.

---

## §5 — Behavior according to profiles configuration

| Configuration | mpforge behavior | MP size | Estimated RGN impact |
|---|---|---|---|
| `generalize_profiles_path: generalize-profiles-local.yaml` | DP/VW on 8 layers, Data0..DataN according to EndLevel | ~43 MB (production) | Reference (1 053 284 bytes) |
| `generalize_profiles_path: generalize-profiles-no-simplify.yaml` | Profile loaded, levels n=0..6 without `simplify` → identical raw geometry at all levels | Larger | Larger than local profile: same EndLevel but without point reduction per level, so more bytes per DataN |
| `generalize_profiles_path` key absent | No catalog → features with `Data0=` only | Smaller (1 DataN per feature) | Minimal |
| `mpforge build --disable-profiles` | Empties the external catalog, keeps per-input `generalize:` inline | Intermediate | Only inline `generalize:` apply |

---

## §6 — Prioritized recommendations

Implementation candidates ranked by gain/complexity ratio, based on §1 measurements.

Levels n=1..5 represent **389 342 bytes** (37% of total RGN) — this is the range where mkgmap filters have the most effect. Level n=0 (663 942 bytes, 63%) is unaffected by mkgmap filters (`res < 24`).

| Candidate | Estimated gain (% total RGN) | Status |
|---|---|---|
| **Increase DP/VW tolerances at levels n=4..6** (align with mkgmap scaling × 10-40) | ~3-5% — n=1..5 levels covered at 34%, 20-40% reduction on this fraction | ✅ Implemented (`generalize-profiles*.yaml`) |
| **Extend profile to `TRONCON_DE_VOIE_FERREE`** (EndLevel=4, identical geometry at 5 levels — 1534 points × 4 levels without simplification ≈ 24 KB) | ~2% on levels n=1..4 | ✅ Implemented (`generalize-profiles*.yaml`) |
| **`RoundCoordsFilter` in imgforge** | ~4-7% — eliminates sub-pixel points on all features at levels n=1..5 | ✅ Implemented — active by default, `--no-round-coords` to disable |
| **`SizeFilter` (polylines + polygons) in imgforge** | ~2-6% — removes features too small at current zoom | ✅ Implemented — active by default, `--no-size-filter` to disable |
| **`RemoveObsoletePointsFilter` in imgforge** | ~1-3% — post-rounding colinear cleanup | ✅ Implemented — active by default, `--no-remove-obsolete-points` to disable |

### Summary

All 5 candidates are now implemented. The estimated total combined gain remains on the order of **12-21% of total RGN** (1 053 284 → ~830-925 KB), assuming approximate independence of effects. The main effect of mkgmap visually (geometric smoothing at wide zooms) comes mostly from `RoundCoordsFilter` + scaled DP.

The three imgforge filters (`RoundCoordsFilter`, `SizeFilter`, `RemoveObsoletePointsFilter`) are active by default at all levels n>0. They can be disabled individually to measure their isolated impact or reproduce a baseline behavior without filters:

```bash
# Disable only RoundCoordsFilter (measure sub-pixel impact)
imgforge build tiles/ --no-round-coords

# Disable all three filters (baseline without post-parsing filtering)
imgforge build tiles/ --no-round-coords --no-size-filter --no-remove-obsolete-points
```

!!! success "DataN mkgmap anomaly — resolved"
    The difference of 95 subdivisions (mkgmap) vs 4 (imgforge) at n=5 is **not** due to feature promotion beyond their EndLevel. Both tools include exactly the same 4 795 features at n=5 (those with `EndLevel=6`). The gap comes from the spatial splitter granularity — see §7.

---

## §7 — Feature selection by level: is imgforge correct?

### Initial hypothesis

The difference of 95 subdivisions (mkgmap) vs 4 (imgforge) at n=5, with a byte gap of 15.8×, had raised the question: does mkgmap include features at n=5 that imgforge would wrongly exclude? The hypothesis was that mkgmap reads `DataN` sections cumulatively (`minResolution = min(resolution of all DataN read)`) and thus promotes features beyond their `EndLevel`.

### Verification — mpforge never emits DataN beyond EndLevel

The distribution of the production MP (`BDTOPO-001-004.mp`, 124 863 features) is perfectly clean:

| EndLevel | max DataN | Features |
|---|---|---|
| 0 | 0 | 99 728 |
| 2 | 2 | 7 810 |
| 4 | 4 | 4 854 |
| 6 | 6 | 4 795 |

mpforge never emits `DataN` beyond `EndLevel`. The hypothesis of promotion by cumulative reading cannot therefore occur: mkgmap does not see `Data5=` for a feature with `EndLevel=4`.

### Verification — same features at n=5

According to imgforge's `feature_visible_at_level` logic (`writer.rs`):

- Feature visible at n=5 if `EndLevel ≥ 5` AND there exists `DataN(k ≤ 5)`
- Result on the MP: **4 795 features** (exactly those with `EndLevel=6`)

mkgmap applies the same rule via its gate `l.getMinResolution() <= res` (`MapBuilder.java` L929), where `minResolution` is derived from `EndLevel`. Both tools include the same features at n=5.

### Explanation of the byte gap

The difference of 129 713 bytes (mkgmap) vs 8 200 bytes (imgforge) at n=5 for the same 4 795 features comes from **two combined factors**:

1. **Splitter granularity**: mkgmap creates 95 small subdivisions (~50 features/subdiv) where imgforge creates 4 large subdivisions (~1 200 features/subdiv). Each subdivision carries a fixed structural overhead in the RGN — 95 × overhead >> 4 × overhead.

2. **Filter chain**: the 4 795 features pass through `RoundCoordsFilter` + `DouglasPeuckerFilter` (error 166 units ≈ 356 m at n=5) in mkgmap, reducing each polyline to a few points. In imgforge, simplification at n=5 depends on the YAML profile (14 m for municipal roads, absent for features outside the profile).

### Conclusion

The imgforge implementation is **correct and semantically faithful** to mkgmap on feature selection. The size difference at n=5 is entirely explained by the splitter granularity (more structural overhead on the mkgmap side) and the absence of `RoundCoords` + `SizeFilter` filters on the imgforge side.

mkgmap's fine granularity (small subdivisions) may offer an advantage on the Garmin firmware side (less data to load per query), but this is a splitter optimization distinct from the subject of this page.

---

## Appendix A — Manual extraction of the imgforge sub-map

The `bytes-per-level.py` script reads the **first** alphabetical sub-map finding a TRE. For an imgforge IMG in GMP format, two sub-maps have a TRE:

| Sub-map | Sections | Role |
|---|---|---|
| `00011855` | LBL, RGN, TRE | GMP container (2 levels, ~24 bytes RGN) |
| `00380042` | LBL, NET, NOD, RGN, TRE | BDTOPO-001-004 tile (7 levels, 1 053 284 bytes) |

The script chooses `00011855` (alphabetical order) — the result displayed by default is the empty container. To analyze the actual tile, modify the script to target `00380042` explicitly, or add a `--submap <name>` argument.

---

## Appendix B — Reference sources

| File | Lines | Content |
|---|---|---|
| `tmp/mkgmap/src/.../build/MapBuilder.java` | L929-930, L1248-1283, L1313-1354 | Complete filter chain |
| `tmp/mkgmap/src/.../filters/RoundCoordsFilter.java` | — | Quantization + best-fit contour lines |
| `tmp/mkgmap/src/.../filters/DouglasPeuckerFilter.java` | L43 | Scaling `filterDistance * (1 << shift)` |
| `tmp/mkgmap/src/.../filters/SizeFilter.java` | — | Bounding box filtering |
| `tmp/mkgmap/src/.../filters/RemoveObsoletePointsFilter.java` | — | Post-RoundCoords cleanup |
| `tmp/mkgmap/src/.../filters/SmoothingFilter.java` | — | Dead code (historical) |
| `scripts/debug/bytes-per-level.py` | — | RGN bytes per level measurement from TRE |
| `docs/implementation-artifacts/audit-mkgmap-r4924-wide-zoom.md` | §1 | Historical baseline commit `8acb0c2` |
| `pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml` | — | EndLevel per BDTOPO layer |
