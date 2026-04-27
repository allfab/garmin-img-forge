# Prérequis et Installation

Tout ce qu'il faut pour mettre en place l'environnement de production de cartes Garmin.

---

## Prérequis

### Données géographiques

| Source | Usage | Taille | Licence |
|--------|-------|--------|---------|
| **BD TOPO IGN** | Données vectorielles (routes, bâtiments, hydro, végétation) | ~35 Go (France entière) | Etalab 2.0 (gratuite) |
| **SRTM 30m** (NASA) | Données d'élévation pour DEM/hill shading | ~2 Go (France) | Domaine public |
| **BDAltiv2** (IGN) | Altitude haute résolution (alternative au SRTM) | ~5 Go (France) | Etalab 2.0 |
| **OpenStreetMap** (optionnel) | Données complémentaires (sentiers, commerces) | Variable | ODbL |

!!! info "BD TOPO IGN"
    La BD TOPO est librement accessible depuis le 1er janvier 2021. Téléchargement depuis le [Géoportail IGN](https://geoservices.ign.fr/bdtopo). Le script `download-data.sh` automatise le téléchargement.

!!! note "SRTM"
    Les tuiles SRTM 30m sont téléchargeables depuis [dwtkns.com/srtm30m](http://dwtkns.com/srtm30m/) (inscription NASA Earth Observation requise).

### Système d'exploitation

| OS | Support |
|----|---------|
| **Linux** (Ubuntu, Debian, Fedora, Arch) | Recommandé |
| **WSL2** (Windows Subsystem for Linux) | Supporté |
| **macOS** | Non testé (devrait fonctionner) |
| **Windows natif** | ogr-polishmap uniquement (via OSGeo4W) |

### Espace disque

| Scénario | Espace nécessaire |
|----------|-------------------|
| 1 département | ~2 Go |
| 1 région | ~5-10 Go |
| France entière | ~50 Go (données + tuiles + output) |

### Logiciels

#### Utilisation avec binaires pré-compilés (le plus simple)

| Logiciel | Version | Téléchargement | Usage |
|----------|---------|----------------|-------|
| **mpforge** (binaire statique) | v0.5.0 | [:material-download: tar.gz](https://github.com/allfab/garmin-img-forge/releases/download/mpforge-v0.5.0/mpforge-linux-amd64.tar.gz) · [:material-download: zip](https://github.com/allfab/garmin-img-forge/releases/download/mpforge-v0.5.0/mpforge-linux-amd64.zip) | Tuilage — inclut GDAL et ogr-polishmap |
| **imgforge** (binaire statique) | v0.5.1 | [:material-download: tar.gz](https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.5.1/imgforge-linux-amd64.tar.gz) · [:material-download: zip](https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.5.1/imgforge-linux-amd64.zip) | Compilation Garmin IMG |

C'est tout ! Les binaires pré-compilés de mpforge embarquent GDAL, PROJ, GEOS et le driver ogr-polishmap. Aucune installation de bibliothèque système n'est nécessaire.

#### Compilation depuis les sources

| Logiciel | Version | Usage |
|----------|---------|-------|
| **Rust** | 1.70+ | Compilation de mpforge et imgforge |
| **GDAL** | 3.6+ | Bibliothèque géospatiale (pour mpforge) |
| **CMake** | 3.20+ | Build du driver ogr-polishmap |
| **GCC** | 13+ | Compilation C++ du driver |

### Public visé

Ce projet s'adresse à un public **averti en géomatique** : administrateurs SIG, développeurs, géomaticiens. La manipulation de données géographiques (projections, formats, attributs) est un pré-requis implicite.
