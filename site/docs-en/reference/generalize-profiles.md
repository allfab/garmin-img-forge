# Generalization Profiles Catalog

The `generalize-profiles.yaml` file is the central catalog for **multi-level geometric generalization** in mpforge. It declares, for each BD TOPO layer, how to simplify and smooth geometries at each Garmin map zoom level.

This file is referenced in `sources.yaml` via the directive:

```yaml
generalize_profiles_path: "../generalize-profiles.yaml"
```

---

## Why a profiles catalog?

The inline `generalize:` directive in `sources.yaml` produces a single simplified geometry (`Data0=`). The profiles catalog goes further: each feature carries **multiple geometries** according to zoom (`Data0=` detailed, `Data2=` simplified, etc.), which `imgforge` automatically selects for display.

```
Feature TRONCON_DE_ROUTE (motorway)
  └── Data0=  conservative VW geometry (max zoom)
  └── Data1=  medium VW
  └── Data2=  strong VW
  └── Data3=  ...
  └── Data4=
  └── Data5=
  └── Data6=  very aggressive VW (minimum zoom)
```

---

## File structure

```yaml
profiles:
  <SOURCE_LAYER>:          # GDAL layer name (e.g. TRONCON_DE_ROUTE)
    topology: true         # Optional — global topological simplification (absent = false)
    levels:                # Simple levels (without dispatch)
      - { n: 0, simplify: 0.00005 }
      - { n: 1, simplify: 0.00008 }
      ...
    when:                  # Conditional dispatch by attribute (optional)
      - field: CL_ADMIN
        values: [Autoroute, Nationale]
        levels:
          - { n: 0, simplify_vw: 0.000001 }
          ...
```

### Level keys (`levels[]`)

| Key | Type | Required | Description |
|-----|------|-------------|-------------|
| `n` | integer | yes | Level index in `MpHeader.levels` (0 = most detailed, 6 = coarsest) |
| `simplify` | float | no | Douglas-Peucker tolerance in WGS84 degrees |
| `simplify_vw` | float | no | Visvalingam-Whyatt triangular area threshold in WGS84² units (area of the triangle formed by 3 consecutive points — points whose area < threshold are removed). Typically used with `topology: true`. |
| `smooth` | string | no | Smoothing algorithm — only `"chaikin"` is supported |
| `iterations` | integer | no (if `smooth`) | Chaikin smoothing passes (bounded `[0, 5]`) |

!!! warning "Fail-fast constraints"
    On config load, mpforge validates:
    - `iterations ∈ [0, 5]`
    - `simplify ∈ [0, 0.001]` (≈ 0 to 110 m)
    - Any routable layer (`TRONCON_DE_ROUTE`) must declare `n: 0` in **each** `when` branch (routing requires strict `Data0=`)
    - The same `source_layer` cannot appear both inline in `generalize:` and in the catalog (conflict rejected)
    - `max(n)` of all profiles must be `< header.levels.len()` (otherwise `imgforge` silently drops `DataN` out of range)

### Tolerance reference

| Value | Approximate metric equivalent | Typical usage |
|--------|---------------------------|---------------|
| `0.00002` | ~2 m | Maximum zoom (Data0) — very conservative |
| `0.00005` | ~5 m | Detailed zoom |
| `0.00010` | ~11 m | Medium zoom |
| `0.00020` | ~22 m | Regional zoom |
| `0.00050` | ~55 m | National zoom |
| `0.00100` | ~110 m | Continental zoom (maximum allowed threshold) |

---

## Level contiguity — critical rule

The `n` values declared in a profile must form a **contiguous** sequence from `0` to `max(EndLevel)` of the rules using that profile. Skipping an index (e.g. `n=0` then `n=2` without `n=1`) creates a gap in the `Data0..DataN` sections of the `.mp` that desynchronizes the RGN index on sensitive Garmin firmware.

**mpforge automatically fills gaps** after `apply_profile` (via `fill_level_gaps` in `geometry_smoother.rs`): the writer always emits contiguous `DataN=`, even if the YAML omitted some.

---

## Topological simplification (`topology: true`)

Layers whose features share vertices at boundaries (adjacent municipalities, road intersections) use `topology: true` together with `simplify_vw`. The VW algorithm is preferred over DP (`simplify`) for these layers because its constraint on shared vertices is more compatible with topology, but `simplify_vw` can be used on any layer — it is not a technical constraint.

```yaml
COMMUNE:
  topology: true
  levels:
    - { n: 0, simplify_vw: 0.00003 }
    - { n: 1, simplify_vw: 0.00007 }
    ...
```

**Why?** A tile-by-tile simplification would produce visual gaps at 4-tile crossings (yellow background between grey municipalities). mpforge runs a **global pre-simplification** (Phase 1.5) on all features before tiling, guaranteeing bit-exact boundaries in all adjacent tiles.

The Visvalingam-Whyatt algorithm (`simplify_vw`) is topologically constrained: it preserves vertices shared between neighboring features.

!!! warning "Memory usage at large scale"
    Phase 1.5 loads the **entire shared-vertex graph of all data** into RAM before any parallelization. This behavior is independent of `--mpforge-jobs`.

    On a department (~40 tiles), the topological graph easily fits in memory. On a **France quadrant** (~25 departments, 1000+ tiles), it can exceed 40 GB and trigger the OOM killer (exit code 137) even with 32 GB RAM + ZRAM.

    **Solution**: use a bifurcated catalog without `topology: true` for large-scale scopes. See [Bifurcated catalogs by scope](#bifurcated-catalogs-by-scope) below.

---

## Conditional dispatch (`when`)

For layers with heterogeneous characteristics (e.g. `TRONCON_DE_ROUTE` mixing motorways and footpaths), attribute-based dispatch allows different tolerances depending on a field value:

```yaml
TRONCON_DE_ROUTE:
  topology: true
  when:
    - field: CL_ADMIN
      values: [Autoroute, Nationale]
      levels:
        - { n: 0, simplify_vw: 0.000001 }
        - { n: 1, simplify_vw: 0.000002 }
        - { n: 2, simplify_vw: 0.000004 }
        - { n: 3, simplify_vw: 0.000008 }
        - { n: 4, simplify_vw: 0.000015 }
        - { n: 5, simplify_vw: 0.000030 }
        - { n: 6, simplify_vw: 0.000080 }
    - field: CL_ADMIN
      values: [Départementale]
      levels:
        - { n: 0, simplify_vw: 0.000003 }
        - { n: 1, simplify_vw: 0.000006 }
        - { n: 2, simplify_vw: 0.000010 }
        - { n: 3, simplify_vw: 0.000020 }
        - { n: 4, simplify_vw: 0.000040 }
        - { n: 5, simplify_vw: 0.000080 }
        - { n: 6, simplify_vw: 0.000200 }
    - field: CL_ADMIN
      values: [Communale, "Sans objet"]
      levels:
        - { n: 0, simplify_vw: 0.000005 }
        - { n: 1, simplify_vw: 0.000010 }
        - { n: 2, simplify_vw: 0.000018 }
        - { n: 3, simplify_vw: 0.000035 }
        - { n: 4, simplify_vw: 0.000070 }
        - { n: 5, simplify_vw: 0.000130 }
        - { n: 6, simplify_vw: 0.000300 }
    - field: CL_ADMIN
      values: [Chemin, Sentier]
      levels:
        - { n: 0, simplify_vw: 0.000010 }
        - { n: 1, simplify_vw: 0.000020 }
        - { n: 2, simplify_vw: 0.000035 }
        - { n: 3, simplify_vw: 0.000070 }
        - { n: 4, simplify_vw: 0.000130 }
        - { n: 5, simplify_vw: 0.000250 }
        - { n: 6, simplify_vw: 0.000550 }
  levels:
    # Default branch (features not matching any when branch above)
    - { n: 0, simplify_vw: 0.000005 }
    - { n: 1, simplify_vw: 0.000010 }
    - { n: 2, simplify_vw: 0.000018 }
    - { n: 3, simplify_vw: 0.000035 }
    - { n: 4, simplify_vw: 0.000070 }
    - { n: 5, simplify_vw: 0.000130 }
    - { n: 6, simplify_vw: 0.000300 }
```

Resolution follows **first-match-wins**: the first `when` branch whose `field` value is in the `values` list is applied. Any feature whose attribute matches no `when` branch falls into the root `levels` branch (default branch). **Each branch must declare all levels `n=0..6`** — gaps are filled by `fill_level_gaps` but produce a discontinuous stepped simplification.

The production profiles table below indicates 5 branches for `TRONCON_DE_ROUTE` (4 `when` + 1 default branch).

---

## Bifurcated catalogs by scope {#bifurcated-catalogs-by-scope}

The project maintains **two distinct catalogs** depending on the geographic scope of the build:

| File | Scope | `topology` roads/municipalities | When to use |
|---------|-------|:--------------------------:|-----------------|
| `pipeline/configs/ign-bdtopo/generalize-profiles.yaml` | `departement/`, `outre-mer/` | ✅ `true` | Build of one or a few departments |
| `pipeline/configs/ign-bdtopo/france-quadrant/generalize-profiles.yaml` | `france-quadrant/` | ❌ absent (= `false`) | FRANCE-SE/SO/NE/NO quadrants (~25 depts.) |

The simplification values (n=0..6) are **identical** between the two catalogs. Only `topology` differs. The `france-quadrant` catalog is referenced by its local `sources.yaml` via a direct relative path:

```yaml
# pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml
generalize_profiles_path: "generalize-profiles.yaml"   # local catalog
```

```yaml
# pipeline/configs/ign-bdtopo/departement/sources.yaml
generalize_profiles_path: "../generalize-profiles.yaml" # shared catalog
```

!!! note "No visual regression"
    Quadrant builds use `--no-route` (no route calculation). Topological continuity at tile boundaries is therefore unnecessary: any micro-offsets of vertices at tile junctions are invisible to the eye and have no impact on disabled routing.

---

## Production BDTOPO profiles

Both catalogs cover 9 layers for the 7-level header `24/23/22/21/20/18/16`:

| Layer | Algorithm | Dispatch | `topology` (dept.) | `topology` (quadrant) |
|--------|------------|----------|:-----------------:|:---------------------:|
| `TRONCON_DE_ROUTE` | `simplify_vw` | By `CL_ADMIN` (5 branches) | ✅ | ❌ |
| `COMMUNE` | `simplify_vw` | No | ✅ | ❌ |
| `TRONCON_HYDROGRAPHIQUE` | `simplify` (DP) | No | — | — |
| `SURFACE_HYDROGRAPHIQUE` | Chaikin + `simplify` (DP) | No | — | — |
| `ZONE_DE_VEGETATION` | Chaikin + `simplify` (DP) | No | — | — |
| `ZONE_D_HABITATION` | Chaikin + `simplify` (DP) | No | — | — |
| `COURBE` | `simplify` (DP) | No | — | — |
| `CONSTRUCTION_LINEAIRE` | `simplify` (DP) | No | — | — |
| `TRONCON_DE_VOIE_FERREE` | `simplify` (DP) | No | — | — |

!!! note "BATIMENT intentionally absent"
    Buildings are excluded from the catalog: they must remain intact (raw `Data0=` geometry only). Any simplification of buildings produces absurd angles visible on the GPS.

---

## Opting out of the catalog

To disable the external catalog without modifying the YAML (useful for debugging or comparing with a baseline):

```bash
# Via CLI
mpforge build --config config.yaml --disable-profiles

# Via environment variable
MPFORGE_PROFILES=off mpforge build --config config.yaml
```

Only the `generalize_profiles_path` catalog is disabled. Inline `generalize:` directives in `sources.yaml` remain active.

---

## Going further

The [mkgmap/imgforge comparison](comparaison-mkgmap-imgforge.md) page analyzes the mkgmap r4924 filter chain
(`RoundCoordsFilter`, `SizeFilter`, `DouglasPeuckerFilter`) by resolution, measures RGN bytes
per level (mkgmap vs imgforge) on the reference BDTOPO-001-004 tile, and lists prioritized recommendations
for reducing IMG size and aligning geometric smoothing.
