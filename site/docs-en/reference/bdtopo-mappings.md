# BD TOPO / Garmin Mappings

This page documents how IGN BD TOPO layers are transposed into Garmin types in the mpforge pipeline.

---

## BD TOPO themes used

### Transport

| BD TOPO layer | Garmin type | Description |
|---------------|-------------|-------------|
| `TRONCON_DE_ROUTE` (motorway) | `POLYLINE 0x0001` | Motorway |
| `TRONCON_DE_ROUTE` (national) | `POLYLINE 0x0002` | National road |
| `TRONCON_DE_ROUTE` (departmental) | `POLYLINE 0x0003` | Departmental road |
| `TRONCON_DE_ROUTE` (municipal) | `POLYLINE 0x0006` | Residential street |
| `TRONCON_DE_ROUTE` (track) | `POLYLINE 0x000A` | Unpaved track |
| `TRONCON_DE_ROUTE` (path) | `POLYLINE 0x000E` | Trail / footpath |
| `TRONCON_DE_VOIE_FERREE` | `POLYLINE 0x0014` | Railway |

### Hydrography

| BD TOPO layer | Garmin type | Description |
|---------------|-------------|-------------|
| `TRONCON_HYDROGRAPHIQUE` | `POLYLINE 0x001A` | Watercourse (line) |
| `SURFACE_HYDROGRAPHIQUE` | `POLYGON 0x0028` | Water body (surface) |

### Vegetation

| BD TOPO layer | Garmin type | Description |
|---------------|-------------|-------------|
| `ZONE_DE_VEGETATION` (forest) | `POLYGON 0x0050` | Forest / woodland |
| `ZONE_DE_VEGETATION` (orchard) | `POLYGON 0x0051` | Orchard / vineyard |
| `ZONE_DE_VEGETATION` (meadow) | `POLYGON 0x0052` | Meadow |

### Buildings

| BD TOPO layer | Garmin type | Description |
|---------------|-------------|-------------|
| `CONSTRUCTION_SURFACIQUE` | `POLYGON 0x0013` | Building footprint |

### Toponymy

| BD TOPO layer | Garmin type | Description |
|---------------|-------------|-------------|
| `LIEU_DIT_NON_HABITE` | `POI 0x6400+` | Locality, summit, pass |
| `COMMUNE` | `POI 0x0400+` | Municipal seat |

## Simplification profiles by layer

`mpforge` applies **multi-level simplification profiles** to BD TOPO layers via the catalog `pipeline/configs/ign-bdtopo/generalize-profiles.yaml`. Each feature can carry multiple geometries (`Data0=` detailed, `Data2=` for medium zoom), selected by `imgforge` at render time. Douglas-Peucker tolerances in WGS84 degrees.

| BD TOPO layer | Profile | `Data0` (detailed) | `Data2` (medium zoom) | Rationale |
|---|---|---|---|---|
| `BATIMENT` | **none** | raw (no DP) | — | Geometry preserved as delivered by IGN |
| `TRONCON_HYDROGRAPHIQUE` | mono-level | `simplify: 0.00005` (~5 m) | `simplify: 0.00020` (~22 m) | Detailed watercourses + medium zoom version |
| `ZONE_DE_VEGETATION` | mono-level + Chaikin | Chaikin 1× + `simplify: 0.00005` | `simplify: 0.00020` | Natural smoothing of contours |
| `TRONCON_DE_ROUTE` (Motorway, National) | dispatch `when: CL_ADMIN` | `simplify: 0.00001` (~1 m) | `simplify: 0.00008` | Maximum routing preservation |
| `TRONCON_DE_ROUTE` (Departmental) | dispatch `when: CL_ADMIN` | `simplify: 0.00003` | `simplify: 0.00010` | Detail/size balance |
| `TRONCON_DE_ROUTE` (Municipal, Other) | dispatch `when: CL_ADMIN` | `simplify: 0.00005` | `simplify: 0.00015` | Reasonable defaults |
| `TRONCON_DE_ROUTE` (Track, Path) | dispatch `when: CL_ADMIN` | `simplify: 0.00010` | `simplify: 0.00030` | More aggressive simplification |
| `TRONCON_DE_ROUTE` (other) | fallback `levels` default | `simplify: 0.00005` | `simplify: 0.00015` | Unknown CL_ADMIN |
| `COURBE` | mono-level | `simplify: 0.00008` | `simplify: 0.00025` | Contour lines |

**Constraints**: any routable layer (`TRONCON_DE_ROUTE`) **must** declare `n: 0` on each branch (routing guarantee). Tolerances `n: 0` for routable classes are capped at `≤ 0.00010°` (~11 m) to preserve connectivity at intersections.

See [Step 2 — Multi-level profiles](../the-pipeline/step-2-configuration.md#geometry-generalization) for the YAML semantics.

## Categorization rules

The mappings between BD TOPO attributes and Garmin type codes are defined in the pipeline's YAML configuration files. The field mapping (`bdtopo-mapping.yaml`) bridges BD TOPO column names to standard Polish Map fields.

### Transposition example

A road in BD TOPO:

```
Layer: TRONCON_DE_ROUTE
IMPORTANCE attribute: 2
NATURE attribute: Route à 2 chaussées
Name: Route Nationale 7
```

Becomes in the `.mp` file (after field mapping):

```
[POLYLINE]
Type=0x0002
Label=Route Nationale 7
Levels=0-2
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(45.1234,5.6789),(45.1235,5.6790),...
[END]
```

## OSM / Garmin mappings

OpenStreetMap data is a complementary source to BD TOPO for POIs and natural features.

### Amenity (24 types)

| OSM tag | Garmin type | Description |
|---------|-------------|-------------|
| `amenity=bar` | `POI 0x15200` | Bar |
| `amenity=biergarten` | `POI 0x15201` | Beer garden |
| `amenity=cafe` | `POI 0x15202` | Café |
| `amenity=fast_food` | `POI 0x15203` | Fast food |
| `amenity=restaurant` / `food_court` | `POI 0x15204` | Restaurant |
| `amenity=ice_cream` | `POI 0x15205` | Ice cream shop |
| `amenity=pub` | `POI 0x15206` | Pub |
| `amenity=library` | `POI 0x15207` | Library |
| `amenity=bicycle_parking` | `POI 0x15208` | Bicycle parking |
| `amenity=bicycle_repair_station` | `POI 0x15209` | Bicycle repair station |
| `amenity=fuel` | `POI 0x1520a` | Fuel station |
| `amenity=charging_station` | `POI 0x1520b` | Charging station |
| `amenity=motorcycle_parking` | `POI 0x1520c` | Motorcycle parking |
| `amenity=parking` | `POI 0x1520d` | Parking |
| `amenity=taxi` | `POI 0x1520e` | Taxi stand |
| `amenity=clinic` / `doctors` | `POI 0x1520f` | Doctor / clinic |
| `amenity=dentist` | `POI 0x15210` | Dentist |
| `amenity=hospital` | `POI 0x15211` | Hospital |
| `amenity=pharmacy` | `POI 0x15212` | Pharmacy |
| `amenity=veterinary` | `POI 0x15213` | Veterinary |
| `amenity=shelter` | `POI 0x15214` | Shelter |
| `amenity=toilets` | `POI 0x15215` | Toilets |
| `amenity=*` (default) | `POI 0x15216` | Other amenity |

### Shop (53 types — selection)

| OSM tag | Garmin type | Description |
|---------|-------------|-------------|
| `shop=bakery` | `POI 0x15001` | Bakery |
| `shop=supermarket` | `POI 0x15006` | Supermarket |
| `shop=convenience` | `POI 0x15004` | Convenience store |
| `shop=butcher` | `POI 0x15007` | Butcher |
| `shop=hairdresser` | `POI 0x15002` | Hairdresser |
| `shop=pharmacy` | — | *(via amenity=pharmacy)* |
| `shop=*` (default) | `POI 0x1500e` | Other shop |

The complete 53 shop types are defined in `garmin-rules.yaml`. Types sharing the same Garmin code are grouped with the `in:` operator.

### Natural

| OSM tag | Geometry | Garmin type | Description |
|---------|-----------|-------------|-------------|
| `natural=ridge` | LINE | `POLYLINE 0x11a00` | Ridge |
| `natural=arete` | LINE | `POLYLINE 0x11a00` | Arete |
| `natural=cliff` | LINE | `POLYLINE 0x11a01` | Cliff |
| `natural=cave_entrance` | POINT | `POI 0x15301` | Cave entrance |
| `natural=rock` | POINT | `POI 0x06614` | Rock |
| `natural=sinkhole` | POINT | `POI 0x11509` | Sinkhole |
| `natural=cave` (default) | POINT | `POI 0x15300` | Cave |

### Tourism

| OSM tag | Garmin type | Description |
|---------|-------------|-------------|
| `tourism=viewpoint` | `POI 0x16` | Viewpoint (Scenic Area) |

## Layers not yet integrated

Some BD TOPO layers are not yet integrated in the pipeline:

| Layer | Reason |
|--------|--------|
| High-voltage power network | No appropriate standard Garmin type |
| Detailed regulated zones | Attribute complexity |
| Fine administrative boundaries | Redundancy with cadastral data |

!!! note "Contour lines and DEM: do not confuse"
    **Contour lines** (isolines at 10 m intervals) are **vector data** from IGN altimetric layers. They are integrated into the pipeline like any other data source via the mpforge YAML configuration.

    **DEM** (IGN BDAltiv2 or NASA SRTM) is a raster digital terrain model, used by imgforge (`--dem`) for **hill shading** and **elevation profiles** on the GPS. These are two complementary but distinct datasets.
