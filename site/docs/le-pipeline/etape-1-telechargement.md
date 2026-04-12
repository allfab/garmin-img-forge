# Étape 1 : Téléchargement des données

## Les données sources

La **BD TOPO IGN** est la base de données topographique de référence de l'IGN (Institut national de l'information géographique et forestière). Elle couvre l'ensemble du territoire français métropolitain et ultra-marin.

| Caractéristique | Valeur |
|----------------|--------|
| Précision | Métrique à décamétrique selon les thèmes |
| Formats disponibles | GeoPackage (`.gpkg`) ou Shapefile (`.shp`) |
| Projection | Lambert-93 (EPSG:2154) |
| Licence | Etalab 2.0 (ouverte et gratuite) |
| Mise à jour | Trimestrielle |
| Taille | ~40 Go pour la moitié sud de la France |

### Données complémentaires optionnelles

| Source | Usage | Licence |
|--------|-------|---------|
| **OpenStreetMap** | Sentiers de randonnée, commerces, équipements | ODbL |
| **SRTM 30m** (NASA) | Courbes de niveau, DEM/hill shading | Domaine public |
| **BDAltiv2** (IGN) | Altitude haute résolution France | Etalab 2.0 |

## Téléchargement automatisé

Le script `download-bdtopo.sh` automatise le téléchargement depuis le Géoportail IGN :

### Par département

```bash
# Télécharger un département (Isère) avec toutes les données complémentaires
./scripts/download-bdtopo.sh --zones D038 --with-contours --with-osm --with-dem

# Plusieurs départements
./scripts/download-bdtopo.sh --zones D038,D069 --with-contours --with-osm --with-dem
```

### Par région

```bash
# Auvergne-Rhône-Alpes
./scripts/download-bdtopo.sh --region ARA --with-contours --with-osm --with-dem
```

### France entière

```bash
./scripts/download-bdtopo.sh --region FXX --with-contours --with-osm --with-dem
```

### Cibler un millésime précis

Par défaut, le script télécharge **la dernière édition** publiée par l'IGN. Trois options permettent de figer un millésime antérieur (utile pour reproduire un build historique ou attendre qu'une édition soit intégralement publiée) :

```bash
# 1. Lister les millésimes disponibles pour une zone (ne télécharge rien)
./scripts/download-bdtopo.sh --zones D038 --list-editions

# 2. Résoudre via API la dernière édition d'un mois donné
./scripts/download-bdtopo.sh --zones D038 --bdtopo-version v2025.09

# 3. Forcer une date d'édition exacte
./scripts/download-bdtopo.sh --zones D038 --date 2025-09-15
```

| Option | Comportement |
|--------|--------------|
| `--list-editions` | Interroge l'API IGN, affiche les millésimes disponibles par zone au format `vYYYY.MM (date: YYYY-MM-DD)`, puis quitte. |
| `--bdtopo-version vYYYY.MM` | Résolution dynamique : le script interroge l'API, filtre les éditions du mois demandé et utilise la plus récente. |
| `--date YYYY-MM-DD` | Date exacte injectée dans le nom de dataset, sans passage par l'API de listing. |

!!! warning "Exclusivité"
    `--bdtopo-version` et `--date` ne peuvent pas être combinés. Utilisez l'un ou l'autre selon que vous connaissez ou non la date exacte de publication IGN.

!!! tip "Préparer un build reproductible"
    Commencez par `--list-editions` sur votre zone, notez la version cible (ex `v2025.09`), puis lancez votre pipeline avec `--bdtopo-version` pour garantir que toutes les zones pointent vers le même millésime.

## Organisation des données

Le script organise automatiquement les fichiers téléchargés :

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
│               └── ...  (même structure)
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

## Codes des zones

### Régions métropolitaines

| Code | Région |
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

### Départements

Codes `D001` à `D976` (numéro de département standard).

## Données d'élévation (DEM)

Pour le hill shading et les profils d'altitude sur le GPS, il faut des données d'élévation :

### SRTM (NASA) — recommandé pour débuter

```bash
# Télécharger les tuiles SRTM pour la France
# Depuis http://dwtkns.com/srtm30m/ (inscription NASA requise)
# Tuiles nécessaires : N42E000 à N51E010 environ
```

Les fichiers HGT sont directement utilisables par imgforge (`--dem ./srtm_hgt/`).

### BDAltiv2 (IGN) — haute résolution France

Les fichiers ASC au format ESRI ASCII Grid (25 m), en projection Lambert 93, sont téléchargés automatiquement par `download-bdtopo.sh` avec `--with-dem` et stockés dans `pipeline/data/dem/{zone}/`. imgforge les utilise avec reprojection intégrée (`--dem ./pipeline/data/dem/D038/ --dem-source-srs EPSG:2154`). En multi-zones, le script `build-garmin-map.sh` passe un `--dem` par département.

## Données OSM (OpenStreetMap)

Les données OpenStreetMap complètent la BD TOPO avec des POIs (commerces, restaurants, pharmacies...) et des features naturelles (grottes, falaises, points de vue) non présents dans les données IGN.

### Téléchargement depuis Geofabrik

Le script `download-bdtopo.sh` gère aussi le téléchargement des fichiers `.osm.pbf` depuis [Geofabrik](https://download.geofabrik.de/europe/france.html) :

```bash
# BDTOPO + OSM pour Auvergne-Rhône-Alpes
./scripts/download-bdtopo.sh --region ARA --with-osm

# France entière (BDTOPO + 1 seul fichier OSM ~4.5 Go)
./scripts/download-bdtopo.sh --region FXX --with-osm

# Simuler sans télécharger
./scripts/download-bdtopo.sh --region ARA --with-osm --dry-run
```

!!! note "Régions Geofabrik"
    Geofabrik utilise les **anciennes régions françaises** (pré-2016). Le script gère automatiquement le mapping : `--region ARA` télécharge `auvergne-latest.osm.pbf` et `rhone-alpes-latest.osm.pbf`. Pour `--region FXX`, un seul fichier `france-latest.osm.pbf` est téléchargé.

### Organisation des données OSM

Les fichiers PBF Geofabrik sont automatiquement convertis en GPKG par `download-bdtopo.sh` (`--with-osm`), ce qui élimine les erreurs mémoire du driver GDAL OSM sur les gros PBF.

```
pipeline/data/osm/
├── auvergne-latest.osm.pbf           ← PBF source (conservé)
├── rhone-alpes-latest.osm.pbf
└── gpkg/                             ← GPKG extraits (utilisés par mpforge)
    ├── auvergne-latest-amenity-points.gpkg
    ├── auvergne-latest-shop-points.gpkg
    ├── auvergne-latest-natural-lines.gpkg
    ├── auvergne-latest-natural-points.gpkg
    ├── auvergne-latest-tourism-points.gpkg
    └── ...
```

Les GPKG sont directement utilisables par mpforge — pas de configuration OSM (`osmconf.ini`) nécessaire pour les GPKG.

## Courbes de niveau vectorielles

!!! note "DEM et courbes de niveau : ne pas confondre"
    Les **courbes de niveau** (isolignes au pas de 10 m) sont des **données vectorielles** issues des couches altimétriques de l'IGN. Elles sont intégrées au pipeline comme n'importe quelle source de données via la configuration YAML de mpforge. Le **DEM** (BDAltiv2, SRTM) est un modèle numérique de terrain en raster, utilisé par imgforge (`--dem`) pour l'**ombrage du relief** (hill shading) et les **profils d'altitude**. Ce sont deux données complémentaires mais distinctes.

Les courbes de niveau au pas de 10 m sont disponibles sous forme de données vectorielles (Shapefile) auprès de l'IGN. Elles sont téléchargées automatiquement par `download-bdtopo.sh` avec l'option `--with-contours` et stockées dans `pipeline/data/contours/{zone}/`.

```yaml
inputs:
  # Courbes de niveau — multi-zones via brace expansion
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

Le `spatial_filter` est important pour les courbes : il restreint le traitement aux communes des zones sélectionnées, évitant de charger des dalles de courbes inutiles.

Les courbes de niveau seront alors découpées en tuiles Polish Map et compilées dans la carte Garmin finale, indépendamment du DEM utilisé par imgforge pour le hill shading.
