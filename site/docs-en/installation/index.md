# Prerequisites and Installation

Everything you need to set up the Garmin map production environment.

---

## Prerequisites

### Geographic data

| Source | Usage | Size | License |
|--------|-------|--------|---------|
| **BD TOPO IGN** | Vector data (roads, buildings, hydro, vegetation) | ~35 GB (all of France) | Etalab 2.0 (free) |
| **SRTM 30m** (NASA) | Elevation data for DEM/hill shading | ~2 GB (France) | Public domain |
| **BDAltiv2** (IGN) | High-resolution altitude (SRTM alternative) | ~5 GB (France) | Etalab 2.0 |
| **OpenStreetMap** (optional) | Supplementary data (trails, shops) | Variable | ODbL |

!!! info "BD TOPO IGN"
    BD TOPO has been freely accessible since January 1, 2021. Download from [IGN Géoportail](https://geoservices.ign.fr/bdtopo). The `download-data.sh` script automates the download.

!!! note "SRTM"
    SRTM 30m tiles can be downloaded from [dwtkns.com/srtm30m](http://dwtkns.com/srtm30m/) (NASA Earth Observation registration required).

### Operating system

| OS | Support |
|----|---------|
| **Linux** (Ubuntu, Debian, Fedora, Arch) | Recommended |
| **WSL2** (Windows Subsystem for Linux) | Supported |
| **macOS** | Not tested (should work) |
| **Native Windows** | ogr-polishmap only (via OSGeo4W) |

### Disk space

| Scenario | Required space |
|----------|-------------------|
| 1 department | ~2 GB |
| 1 region | ~5-10 GB |
| All of France | ~50 GB (data + tiles + output) |

### Software

#### Using pre-compiled binaries (easiest)

| Software | Version | Download | Usage |
|----------|---------|----------------|-------|
| **mpforge** (static binary) | v0.8.1 | [:material-download: tar.gz](https://github.com/allfab/garmin-img-forge/releases/download/mpforge-v0.8.1/mpforge-linux-amd64.tar.gz) · [:material-download: zip](https://github.com/allfab/garmin-img-forge/releases/download/mpforge-v0.8.1/mpforge-linux-amd64.zip) | Tiling — includes GDAL and ogr-polishmap |
| **imgforge** (static binary) | v0.8.2 | [:material-download: tar.gz](https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.8.2/imgforge-linux-amd64.tar.gz) · [:material-download: zip](https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.8.2/imgforge-linux-amd64.zip) | Garmin IMG compilation |

That's it! The pre-compiled mpforge binaries include GDAL, PROJ, GEOS and the ogr-polishmap driver. No system library installation required.

#### Compiling from sources

| Software | Version | Usage |
|----------|---------|-------|
| **Rust** | 1.70+ | Compiling mpforge and imgforge |
| **GDAL** | 3.6+ | Geospatial library (for mpforge) |
| **CMake** | 3.20+ | Building the ogr-polishmap driver |
| **GCC** | 13+ | C++ compilation of the driver |

### Target audience

This project is aimed at an **advanced GIS audience**: GIS administrators, developers, geomaticians. Handling geographic data (projections, formats, attributes) is an implicit prerequisite.
