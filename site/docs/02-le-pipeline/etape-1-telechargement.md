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
# Télécharger un département (Isère)
./scripts/download-bdtopo.sh --zones D038 --data-root ./data/bdtopo

# Plusieurs départements
./scripts/download-bdtopo.sh --zones D038,D073,D074 --data-root ./data/bdtopo
```

### Par région

```bash
# Auvergne-Rhône-Alpes
./scripts/download-bdtopo.sh --zones R84 --data-root ./data/bdtopo
```

### France entière

```bash
./scripts/download-bdtopo.sh --zones FRANCE --data-root ./data/bdtopo
```

## Organisation des données

Le script organise automatiquement les fichiers téléchargés :

```
data/bdtopo/
└── 2026/
    └── v3.0/
        ├── D038/
        │   ├── BDTOPO_3-0_TOUSTHEMES_SHP_LAMB93_D038/
        │   │   ├── TRANSPORT/
        │   │   │   ├── TRONCON_DE_ROUTE.shp
        │   │   │   ├── TRONCON_DE_VOIE_FERREE.shp
        │   │   │   └── ...
        │   │   ├── HYDROGRAPHIE/
        │   │   ├── VEGETATION/
        │   │   ├── BATI/
        │   │   └── ...
        │   └── BDTOPO_3-0_TOUSTHEMES_GPKG_LAMB93_D038/
        │       └── BDTOPO.gpkg
        └── D073/
            └── ...
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

Les fichiers ASC au format ESRI ASCII Grid, en projection Lambert 93, sont supportés nativement par imgforge avec reprojection intégrée (`--dem ./bdaltiv2/ --dem-source-srs EPSG:2154`).

## Courbes de niveau vectorielles

!!! note "DEM et courbes de niveau : ne pas confondre"
    Les **courbes de niveau** (isolignes au pas de 10 m) sont des **données vectorielles** issues des couches altimétriques de l'IGN. Elles sont intégrées au pipeline comme n'importe quelle source de données via la configuration YAML de mpforge. Le **DEM** (BDAltiv2, SRTM) est un modèle numérique de terrain en raster, utilisé par imgforge (`--dem`) pour l'**ombrage du relief** (hill shading) et les **profils d'altitude**. Ce sont deux données complémentaires mais distinctes.

Les courbes de niveau au pas de 10 m sont disponibles sous forme de données vectorielles (Shapefile ou GeoPackage) auprès de l'IGN. Elles peuvent être intégrées au pipeline comme n'importe quelle autre source de données : il suffit de les déclarer dans le fichier de configuration YAML de mpforge.

```yaml
inputs:
  # Courbes de niveau vectorielles IGN
  - path: "data/courbes_niveau/*.shp"
```

Les courbes de niveau seront alors découpées en tuiles Polish Map et compilées dans la carte Garmin finale, indépendamment du DEM utilisé par imgforge pour le hill shading.
