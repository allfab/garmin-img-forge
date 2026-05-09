# Wins & Pitfalls

This project has been an intense learning journey. Here is an honest retrospective on what worked, what was difficult, and the lessons learned.

---

## Wins

### The GDAL/OGR driver works

The first major victory was getting GDAL to accept a format it had never seen before. The **ogr-polishmap** driver is 100% compliant with the 12 GDAL conventions, passes all tests, and integrates naturally into the ecosystem (ogr2ogr, QGIS, Python/GDAL).

This unblocked everything else: once GDAL knows how to write Polish Map, any tool in the GIS ecosystem can become a data source for Garmin maps.

### The mpforge static binary with embedded GDAL

Compiling a Rust binary that statically embeds GDAL 3.10.1, PROJ, GEOS, and the ogr-polishmap driver was a major technical challenge — but the result is spectacular: **a single executable file, zero dependencies**, that runs on any Linux distribution.

No more manually installing GDAL, no more incompatible version issues.

### imgforge replaces mkgmap

Writing a Garmin IMG compiler from scratch in Rust, capable of generating TRE, RGN, LBL, NET, NOD and DEM sub-files, was the most ambitious challenge of the project. The result: a single binary of a few MB that replaces a 40+ MB Java JAR, with significantly better performance thanks to native parallelization.

### Routing works

Generating the NET and NOD sub-files for turn-by-turn route calculation was meticulous reverse engineering work. Garmin routing is one of the most complex and least documented parts of the IMG format. After many iterations, maps produced by imgforge enable GPS navigation.

### DEM/Hill Shading

The integration of DEM (Digital Elevation Model) with native support for HGT (SRTM) and ASC (BDAltiv2 IGN) formats, built-in reprojection, and multi-level encoding, allows producing maps with relief shading and altitude profiles — directly on the GPS, without post-processing.

### Garmin Alpha 100 compatibility (April 2026)

One of the most technical battles of the project was making `gmapsupp.img` files produced by imgforge compatible with the **Garmin Alpha 100** — a field GPS with a particularly strict firmware regarding the binary structure of maps.

Maps compiled by imgforge worked perfectly in Garmin BaseCamp (PC software), but the physical GPS consistently displayed "no data" or completely ignored the file.

**The investigation methodology** was surgical:

1. Compilation of the same `.mp` tile with both tools (imgforge and mkgmap)
2. Byte-by-byte binary comparison of sub-files (TRE, RGN, LBL)
3. **Hybrid tests**: replacing sub-files between the two tools to isolate the failing component
4. Iterative tests on the physical GPS, cycle after cycle

The hybrid tests were key: by combining the TRE+RGN from mkgmap with the LBL from imgforge, the GPS worked. The reverse did not. The problem was therefore located in the **TRE+RGN** (spatial index + feature data) and not in the LBL (labels).

**10 corrections** were needed before obtaining a working file:

| Phase | Corrections |
|-------|-------------|
| **gmapsupp structure** | Sub-file ordering (MPS first), mandatory SRT sort descriptor, mandatory TYP, TDB forbidden in the container |
| **TRE (spatial index)** | Subdivision half-width (half-extent vs full), ext type sections always present, complete zoom levels even when empty, `is_last` flag per parent group |
| **RGN (data)** | **Missing 0x4B background polygon** in each subdivision, points incorrectly classified in indexed (0x20) section instead of regular (0x10) |

The last two corrections — the **0x4B background polygon** and **point classification** — solved the problem. mkgmap automatically adds a 0x4B type polygon covering each subdivision's area (this is the "map background"), and classifies normal points in the RGN regular section. imgforge was doing neither.

This investigation involved analyzing the mkgmap source code (~100,000 Java lines), cGPSmapper, and structural comparison of dozens of IMG files. The full details are documented in `docs/investigation-imgforge-alpha100.md` (internal documentation).

### The FRANCE-SE quadrants — the April 2026 battle

After the victory on departmental rendering, scaling up to the **quadrant** level (25 departments on `FRANCE-SE` — Auvergne-Rhône-Alpes, PACA, southern Occitanie, Corse) revealed two new blockers on the Alpha 100.

#### Bug 1 — The Alpha 100 crashes at boot on large quadrants

The first FRANCE-SE build (3.5 GB) literally **rebooted the device** at the moment of loading the map. No error, no message: hard reboot. Departmental builds (~170 MB) remained perfectly functional.

**The cause was not the file size** — the mkgmap FRANCE-SUD reference (complete southern half, 3.19 GiB) loaded without issue. It was the **number of FAT entries in the gmapsupp.img** that was the limiting factor:

| Metric | mkgmap FRANCE-SUD (OK) | imgforge FRANCE-SE (crashed) |
|---|---|---|
| Tiles | **98** | **702** |
| Sub-files per tile | 4 (TRE/RGN/LBL/DEM) | up to 6 (+NET+NOD depending on routing) |
| **FAT entries measured** *(parsed from actual gmapsupp.img)* | **~392** | **4,095** |

The Alpha 100 firmware loads the file allocation table into RAM at boot. At 4,095 entries, available memory is exceeded and the device reboots.

**Fix**: increase the mpforge tile size from `cell_size: 0.15°` (~16 km, 193 km²) to `0.45°` (~50 km, 1,750 km²). FRANCE-SE then dropped to **136 tiles** or ~550 FAT entries — close to the mkgmap reference. The map now loads without issue.

The conceptual novelty: `cell_size` does not impact render quality (imgforge's RGN splitter automatically subdivides large tiles internally), only the gmapsupp filesystem slicing. For any new quadrant, targeting ≤ 250 tiles is the rule — `0.45°` for a quadrant, `0.30°` for a regional or a department.

#### Bug 2 — Geometric artifacts on dense communes

After the Bug 1 fix, the map loaded... but **entire communes were missing in blocks** (Marseille, Nice, Lyon) in QmapShack and Alpha 100. Build logs then displayed **thousands of warnings** `Subdivision X RGN size Y exceeds MAX_RGN_SIZE 65528`, with some subdivisions at **252 KiB — four times the Garmin 64 KiB limit**.

The cause: a single-line constant in `tools/imgforge/src/img/splitter.rs`:

```rust
// Step 2: Recursive splitting until all areas fit limits
add_areas_to_list(initial, 8)  // max_depth = 8
```

With `cell_size: 0.45°` (1,750 km²/tile) and dense urban areas, the splitter abandoned at depth 8 without having sufficiently subdivided urban zones. The remaining subdivisions were then written as-is, too large for the Garmin format to encode → corrupted data → missing communes.

**The trap**: careful reading of the mkgmap source code revealed that **mkgmap imposes no depth limit**. In `MapSplitter.java`, the `addAreasToList(areas, alist, 0)` function is initiated with `depth=0` (L113) and calls itself with `depth+1` (L186) without ever testing a ceiling — the `depth` parameter is only used as visual log padding (L140-141). The real stopping conditions are the reached size, minimum dimension, and inability to split a single feature.

`max_depth=8` was therefore a silent deviation by imgforge from mkgmap, not a faithful implementation. The fix was to pass `usize::MAX`:

```rust
add_areas_to_list(initial, usize::MAX)
```

After recompilation, zero `MAX_RGN_SIZE` warnings in logs — all subdivisions fit under 64 KiB. All communes render correctly on Alpha 100 and QmapShack.

**Lesson**: any hardcoded ceiling in imgforge that has no explicit equivalent in mkgmap is suspect by default. Line-by-line comparative analysis remains the correct method.

#### Consequence — Memory OOM during build

The Bug 2 fix unblocked geometric quality, but with `usize::MAX` depth, the splitter exhausts RAM in very dense areas: each subdivision clones its features (points/lines/polygons), and with 4 imgforge workers in parallel on Marseille/Lyon tiles, peak memory exceeds the available 32 GB → OOM killer (`exit 137`).

**Immediate workaround**: `--imgforge-jobs 2 --merge-lines`. `--merge-lines` merges adjacent polylines (mkgmap default option, never activated on imgforge until now) — significant reduction in the number of polylines in memory. With 2 jobs instead of 4, the FRANCE-SE build fits in RAM.

**Clean documented solution**: a splitter refactor tech-spec (move-not-clone + drop parent) will eventually allow returning to 4 jobs. To be implemented in a dedicated iteration.

Reference commits: [`e6fce3f`](https://github.com/allfab/garmin-img-forge/commit/e6fce3f) (cell_size), [`7cef948`](https://github.com/allfab/garmin-img-forge/commit/7cef948) (splitter max_depth), [`7e4a8f2`](https://github.com/allfab/garmin-img-forge/commit/7e4a8f2) (`--skip-existing` publish-only).

### GMP packaging (consolidated Garmin NT format) — April 2026

All modern commercial Garmin maps (Topo France v6 Pro, Topo Active...) use the **GMP** format: instead of 6 separate FAT files per tile (`TRE/RGN/LBL/NET/NOD/DEM`), a single `.GMP` file encapsulates all of them. On a full France build (~1,500 tiles), this represents ~9,000 FAT entries in `legacy` mode vs ~1,500 in `gmp` mode — an 83% reduction.

The implementation of `GmpWriter` was more complex than expected. The container format itself is relatively simple (61-byte header + 179-byte copyright + concatenated blobs with offset relocation), but the Alpha 100 firmware imposes constraints on the **internal content** of the TRE embedded in the GMP — constraints that official Garmin maps implicitly satisfy and are documented nowhere.

Validation required **5 hardware test cycles** (GC1-GC5) and a DEM relocation bug discovered only in production build with real altimetry data. The final root cause: a TRE with NT extension (`hlen=309`) and empty sections inside a GMP is rejected by the Alpha 100 firmware — the standard TRE (`hlen=188`) works perfectly.

**Result**: `--packaging gmp` has been functional in production since April 25, 2026, validated on Alpha 100 with IGN BD TOPO D038 data (routing + BDAltiv2 altimetry).

---

## Pitfalls and difficulties

### The Garmin IMG format is not documented

The IMG format is proprietary and Garmin does not publish a specification. All imgforge development work relied on reverse engineering: analyzing existing IMG files byte by byte, studying the mkgmap source code (Java, 100,000+ lines), and testing empirically on physical GPS devices.

Some sub-files (NOD in particular) have extremely complex encoding structures with bitstream compression formats, signed deltas and plateaus — decoding then re-encoding these structures required many iterations.

### Multi-geometries and the Polish Map format

The Polish Map format only supports simple geometries (Point, LineString, Polygon). However, BD TOPO contains MultiPolygon, MultiLineString, etc. The ogr-polishmap driver automatically decomposes multi-geometries, but this step can generate a large number of additional features and requires particular attention to geometric quality.

### Character encoding

The transition from UTF-8 (source data) to CP1252 (Polish Map format by default) then to Garmin encoding formats (Format 6/9/10) is a bug trap. Special characters, accents, non-Latin characters... each stage in the chain can corrupt labels if encoding is not handled correctly.

### Polish Map format limits

- Maximum 1024 points per polyline — long rivers or roads must be split
- Coordinates in WGS84 decimal degrees only — data in local projection must be reprojected
- No native support for Bézier curves or arcs
- CP1252 encoding by default — characters outside the Latin-1 set require UTF-8

### BD TOPO data size

~40 GB of vector data for the southern half of France is massive. The first mpforge prototypes took hours. Adding parallelization (rayon), spatial indexing (R-tree), and the `--skip-existing` option was necessary to make the pipeline viable in production.

### GmpWriter implementation — undocumented firmware constraints

Implementation of `GmpWriter` to produce the Garmin NT GMP format. The container itself is not difficult to implement (spec partially available in `tmp/gimgtools/garmin_struct.h`). The difficulty came entirely from **Alpha 100 firmware constraints** on internal content, revealed by iterative hardware tests.

**Obstacle 1 — The NT extension of TRE (`hlen=309`)**

Official Garmin maps have a TRE with `hlen=309` inside the GMP, and their 121 NT extension bytes contain valid data. Our TRE produced these 121 bytes all at zero (absent sections). The Alpha 100 firmware has different behaviors depending on the `hlen` value:

- `hlen=309` + `tre10_rec_size=0` → **crash** (division by zero: `count = size / rec_size`)
- `hlen=309` + `tre10_rec_size=1` + rest at zero → **invisible tile** (empty NT sections invalidate the record)
- `hlen=188` (standard TRE) → ✅ **visible and functional tile**

Five hardware test cycles were needed to converge on this conclusion: the Alpha 100 firmware prefers a standard TRE inside a GMP rather than an NT TRE with empty sections. Substituting an official Garmin GMP (GC1) first confirmed that the container format was correct — the problem came from the TRE content produced by `GmpWriter`.

**Obstacle 2 — DEM relocation (`relocate_dem`)**

The internal offsets of standalone blobs must be relocated to GMP-absolute. For DEM, each 60-byte section-header contains two fields to patch (`data_offset` at +32 and `data_offset2` at +36). The first implementation patched the wrong positions (+20 and +24, i.e. `tiles_lon-1` and `tiles_lat-1`).

This bug remained invisible throughout the synthetic test phase, as integration tests used `dem: None`. It only appeared in production build with real BDAltiv2 data: `tiles_lon-1` went from 1 to ~1290, the firmware attempted to allocate a table of 1290 DEM descriptors and rejected the file.

**Lesson**: for any binary format with optional sections, integration tests must cover all subtypes — including NET, NOD and DEM. A test with `dem: None` does not validate `relocate_dem`.

---

## Lessons learned

1. **Starting with the GDAL driver** was the right choice. By integrating into the existing ecosystem rather than reinventing everything, I immediately benefited from GDAL's full power.

2. **The Polish Map intermediate format** is essential for debugging. Being able to inspect text `.mp` files before binary compilation saved hundreds of hours of debugging.

3. **Rust** proved to be an excellent choice: near-C performance, memory safety, library ecosystem (rayon, clap, serde), and above all the ability to produce dependency-free static binaries.

4. **Declarative YAML configuration** makes the pipeline accessible to non-developers. You describe *what you want*, not *how to do it*.

5. **Reverse engineering is a marathon**, not a sprint. One must accept not understanding certain structures for weeks, then having a flash of understanding by comparing two hexadecimal files.

6. **Testing on the target hardware** is irreplaceable. A file that works on BaseCamp can silently fail on a physical GPS. The Garmin Alpha 100 firmware imposes undocumented constraints (mandatory background polygon, strict RGN structure) that only a device test can reveal.

7. **Hybrid tests** (mixing sub-files from two sources) are a devastatingly effective debugging technique for binary formats. By replacing one component at a time, the culprit is isolated in a few iterations instead of searching through hundreds of thousands of bytes.
