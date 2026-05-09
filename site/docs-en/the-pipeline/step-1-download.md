# Step 1: Downloading Data

## Source data

The **IGN BD TOPO** is the reference topographic database of the IGN (Institut national de l'information géographique et forestière). It covers the entire French metropolitan and overseas territory.

| Characteristic | Value |
|----------------|-------|
| Precision | Metric to decametric depending on themes |
| Available formats | GeoPackage (`.gpkg`) or Shapefile (`.shp`) |
| Projection | Lambert-93 (EPSG:2154) |
| License | Etalab 2.0 (open and free) |
| Update | Quarterly |
| Size | ~40 GB for the southern half of France |

### Optional complementary data

| Source | Usage | License |
|--------|-------|---------|
| **OpenStreetMap** | Hiking trails, shops, amenities | ODbL |
| **SRTM 30m** (NASA) | Contour lines, DEM/hill shading | Public domain |
| **BDAltiv2** (IGN) | High-resolution altitude France | Etalab 2.0 |

## Automated download

The `download-data.sh` script automates downloading from the IGN Géoportail:

### By department

```bash
# Download a department (Isère) with all complementary data
./scripts/download-data.sh --zones D038 --with-contours --with-osm --with-dem

# Multiple departments
./scripts/download-data.sh --zones D038,D069 --with-contours --with-osm --with-dem
```

### By region

```bash
# Auvergne-Rhône-Alpes
./scripts/download-data.sh --region ARA --with-contours --with-osm --with-dem
```

### Full France

```bash
./scripts/download-data.sh --region FXX --with-contours --with-osm --with-dem
```

### Targeting a specific edition

By default, the script downloads **the latest edition** published by the IGN. Three options allow pinning an earlier edition (useful for reproducing a historical build or waiting for an edition to be fully published):

```bash
# 1. List available editions for an area (downloads nothing)
./scripts/download-data.sh --zones D038 --list-editions

# 2. Resolve via API the latest edition for a given month
./scripts/download-data.sh --zones D038 --bdtopo-version v2025.09

# 3. Force an exact edition date
./scripts/download-data.sh --zones D038 --date 2025-09-15
```

| Option | Behavior |
|--------|----------|
| `--list-editions` | Queries the IGN API, displays available editions per zone in format `vYYYY.MM (date: YYYY-MM-DD)`, then exits. |
| `--bdtopo-version vYYYY.MM` | Dynamic resolution: the script queries the API, filters editions for the requested month and uses the most recent. |
| `--date YYYY-MM-DD` | Exact date injected into the dataset name, without going through the listing API. |

!!! warning "Exclusivity"
    `--bdtopo-version` and `--date` cannot be combined. Use one or the other depending on whether you know the exact IGN publication date.

!!! tip "Preparing a reproducible build"
    Start with `--list-editions` on your area, note the target version (e.g. `v2025.09`), then launch your pipeline with `--bdtopo-version` to ensure all zones point to the same edition.

## Data organization

The script automatically organizes downloaded files:

```
pipeline/data/
├── bdtopo/
│   └── 2025/
│       └── v2025.12/
│           ├── D038/
│           │   ├── ADMINISTRATIF/
│           │   ├── BATI/
│           │   ├── HYDROGRAPHIE/
│           │   ├── LIEUX_NOMMES/
│           │   ├── OCCUPATION_DU_SOL/
│           │   ├── SERVICES_ET_ACTIVITES/
│           │   ├── TRANSPORT/
│           │   │   ├── TRONCON_DE_ROUTE.shp
│           │   │   ├── TRONCON_DE_VOIE_FERREE.shp
│           │   │   └── ...
│           │   └── ZONES_REGLEMENTEES/
│           └── D069/
│               └── ...  (same structure)
├── contours/
│   ├── D038/
│   │   ├── COURBE_0800_6480.shp
│   │   └── ...
│   └── D069/
├── dem/
│   ├── D038/
│   │   ├── BDALTIV2_25M_*.asc
│   │   └── ...
│   └── D069/
├── osm/
│   ├── auvergne-latest.osm.pbf
│   ├── rhone-alpes-latest.osm.pbf
│   └── gpkg/
│       ├── auvergne-latest-amenity-points.gpkg
│       ├── rhone-alpes-latest-shop-points.gpkg
│       └── ...
└── hiking-trails/
    └── FRANCE-GR.shp
```

## Zone codes

### Metropolitan regions

| Code | Region |
|------|--------|
| R11 | Île-de-France |
| R24 | Centre-Val de Loire |
| R27 | Bourgogne-Franche-Comté |
| R28 | Normandie |
| R32 | Hauts-de-France |
| R44 | Grand Est |
| R52 | Pays de la Loire |
| R53 | Bretagne |
| R75 | Nouvelle-Aquitaine |
| R76 | Occitanie |
| R84 | Auvergne-Rhône-Alpes |
| R93 | Provence-Alpes-Côte d'Azur |
| R94 | Corse |

### Departments

Codes `D001` to `D976` (standard department number).

## Elevation data (DEM)

For hill shading and altitude profiles on the GPS, elevation data is required:

### SRTM (NASA) — recommended for beginners

```bash
# Download SRTM tiles for France
# From http://dwtkns.com/srtm30m/ (NASA registration required)
# Required tiles: approximately N42E000 to N51E010
```

HGT files are directly usable by imgforge (`--dem ./srtm_hgt/`).

### BDAltiv2 (IGN) — high-resolution France

ASC files in ESRI ASCII Grid format (25 m), in Lambert 93 projection, are automatically downloaded by `download-data.sh` with `--with-dem` and stored in `pipeline/data/dem/{zone}/`. imgforge uses them with built-in reprojection (`--dem ./pipeline/data/dem/D038/ --dem-source-srs EPSG:2154`). For multi-zone, the `build-garmin-map.sh` script passes one `--dem` per department.

## OSM data (OpenStreetMap)

OpenStreetMap data complements BD TOPO with POIs (shops, restaurants, pharmacies...) and natural features (caves, cliffs, viewpoints) not present in IGN data.

### Downloading from Geofabrik

The `download-data.sh` script also handles downloading `.osm.pbf` files from [Geofabrik](https://download.geofabrik.de/europe/france.html):

```bash
# BDTOPO + OSM for Auvergne-Rhône-Alpes
./scripts/download-data.sh --region ARA --with-osm

# Full France (BDTOPO + single OSM file ~4.5 GB)
./scripts/download-data.sh --region FXX --with-osm

# Simulate without downloading
./scripts/download-data.sh --region ARA --with-osm --dry-run
```

!!! note "Geofabrik regions"
    Geofabrik uses the **old French regions** (pre-2016). The script automatically handles the mapping: `--region ARA` downloads `auvergne-latest.osm.pbf` and `rhone-alpes-latest.osm.pbf`. For `--region FXX`, a single `france-latest.osm.pbf` file is downloaded.

### OSM data organization

Geofabrik PBF files are automatically converted to GPKG by `download-data.sh` (`--with-osm`), which eliminates GDAL OSM driver memory errors on large PBFs.

```
pipeline/data/osm/
├── auvergne-latest.osm.pbf           ← Source PBF (kept)
├── rhone-alpes-latest.osm.pbf
└── gpkg/                             ← Extracted GPKG (used by mpforge)
    ├── auvergne-latest-amenity-points.gpkg
    ├── auvergne-latest-shop-points.gpkg
    ├── auvergne-latest-natural-lines.gpkg
    ├── auvergne-latest-natural-points.gpkg
    ├── auvergne-latest-tourism-points.gpkg
    └── ...
```

GPKG files are directly usable by mpforge — no OSM configuration (`osmconf.ini`) needed for GPKG.

## Vector contour lines

!!! note "DEM and contour lines: do not confuse"
    **Contour lines** (isolines at 10 m intervals) are **vector data** from IGN altimetric layers. They are integrated into the pipeline like any data source via mpforge's YAML configuration. **DEM** (BDAltiv2, SRTM) is a raster digital terrain model, used by imgforge (`--dem`) for **relief shading** (hill shading) and **altitude profiles**. These are two complementary but distinct data types.

10 m interval contour lines are available as vector data (Shapefile) from the IGN. They are automatically downloaded by `download-data.sh` with the `--with-contours` option and stored in `pipeline/data/contours/{zone}/`.

```yaml
inputs:
  # Contour lines — multi-zone via brace expansion
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

The `spatial_filter` is important for contours: it restricts processing to the communes of the selected zones, avoiding loading unnecessary contour tiles.

Contour lines will then be sliced into Polish Map tiles and compiled into the final Garmin map, independently of the DEM used by imgforge for hill shading.
