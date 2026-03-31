# ogr-polishmap

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Driver GDAL/OGR pour la lecture et l'écriture de fichiers Polish Map (.mp), utilisés pour créer des cartes GPS Garmin.

**Fonctionnalités :**
- Lecture et écriture des couches POI, POLYLINE et POLYGON
- Conversion bidirectionnelle avec tous les formats GDAL (GeoJSON, Shapefile, GeoPackage, etc.)
- Field mapping configurable via YAML (`-dsco FIELD_MAPPING=config.yaml`)
- Filtrage spatial et attributaire
- Conversion automatique UTF-8 ↔ CP1252
- Décomposition automatique des multi-géométries

---

## Quick Start

```bash
# Vérifier que le driver est chargé
ogrinfo --formats | grep -i polish

# Lire un fichier Polish Map
ogrinfo -al sample.mp

# Convertir Polish Map → GeoJSON
ogr2ogr -f "GeoJSON" output.geojson input.mp

# Convertir GeoJSON → Polish Map
ogr2ogr -f "PolishMap" output.mp input.geojson

# Filtre spatial (bounding box)
ogr2ogr -f "GeoJSON" paris.geojson france.mp -spat 2.2 48.8 2.5 49.0

# Convertir avec field mapping YAML
ogr2ogr -f "PolishMap" communes.mp COMMUNE.shp \
    -dsco FIELD_MAPPING=bdtopo-mapping.yaml
```

---

## Installation

### Prérequis

- **GDAL 3.6+** avec headers de développement
- **CMake 3.20+**
- **GCC 13+** (compilateur C++17)

### Build et installation (Debian/Ubuntu)

```bash
# Dépendances
sudo apt-get install -y libgdal-dev gdal-bin cmake g++

# Build
cd ogr-polishmap
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)

# Tests
ctest --output-on-failure
```

**Installation comme plugin GDAL (recommandé) :**

```bash
# Option 1 : Répertoire système (nécessite sudo)
sudo make install
# ou manuellement :
sudo cp ogr_PolishMap.so $(gdal-config --plugindir)/

# Option 2 : Répertoire utilisateur (sans sudo)
mkdir -p ~/.gdal/plugins
cp ogr_PolishMap.so ~/.gdal/plugins/
echo 'export GDAL_DRIVER_PATH=$HOME/.gdal/plugins' >> ~/.bashrc
source ~/.bashrc
```

### Vérification

```bash
ogrinfo --formats | grep -i polish
# Attendu : PolishMap -vector- (rw): Polish Map (.mp)
```

### Intégration directe dans GDAL (avancé)

Pour compiler le driver directement dans GDAL (au lieu de plugin) :

1. Copier `src/` vers `gdal/ogr/ogrsf_frmts/polishmap/`
2. Ajouter l'entrée dans `gdal/ogr/ogrsf_frmts/CMakeLists.txt`
3. Recompiler GDAL

C'est ce que fait le pipeline CI/CD pour produire le binaire statique `mpforge`.

---

## Utilisation

### Ligne de commande (ogr2ogr)

```bash
# Convertir un Shapefile BDTOPO avec field mapping
ogr2ogr -f "PolishMap" communes.mp COMMUNE.shp \
    -dsco FIELD_MAPPING=bdtopo-mapping.yaml

# Convertir OpenStreetMap avec field mapping
ogr2ogr -f "PolishMap" buildings.mp buildings.geojson \
    -dsco FIELD_MAPPING=osm-mapping.yaml

# Extraire une couche spécifique
ogr2ogr -f "GeoJSON" roads.geojson map.mp POLYLINE

# Filtre attributaire
ogrinfo -al sample.mp -where "Type='0x2C00'"
```

### Python

```python
from osgeo import ogr, gdal
gdal.UseExceptions()

# Lecture
ds = ogr.Open("sample.mp")
for i in range(ds.GetLayerCount()):
    layer = ds.GetLayer(i)
    print(f"Layer: {layer.GetName()}, Features: {layer.GetFeatureCount()}")
    for feature in layer:
        print(f"  Type: {feature.GetField('Type')}, Label: {feature.GetField('Label')}")

# Écriture
driver = ogr.GetDriverByName("PolishMap")
ds = driver.CreateDataSource("output.mp")
poi_layer = ds.GetLayer(0)

feature = ogr.Feature(poi_layer.GetLayerDefn())
feature.SetField("Type", "0x2C00")
feature.SetField("Label", "Restaurant")
point = ogr.Geometry(ogr.wkbPoint)
point.AddPoint(2.3522, 48.8566)
feature.SetGeometry(point)
poi_layer.CreateFeature(feature)
ds = None
```

Voir le dossier [examples/](examples/) pour des scripts Python complets (lecture, écriture, conversion, filtrage).

### Field mapping YAML

Le driver supporte un mapping configurable des noms de champs via `-dsco FIELD_MAPPING=config.yaml` :

```yaml
# bdtopo-mapping.yaml
field_mapping:
  NAME: Label
  MP_TYPE: Type
  Country: CountryName
  MPBITLEVEL: Levels
```

**Champs Polish Map disponibles :**
- **Core** : Type, Label, EndLevel, Levels, Data0-Data9
- **Localisation** : CityName, RegionName, CountryName, Zip
- **POI** : SubType, Marine, City, StreetDesc, HouseNumber, PhoneNumber, Highway
- **POLYLINE** : DirIndicator, RouteParam

Sans field mapping, le driver utilise des alias intégrés (`NAME`/`NOM` → Label, `MP_TYPE` → Type, etc.).

---

## Capacités du driver

| Fonctionnalité | Lecture | Écriture |
|----------------|---------|----------|
| POI (Point) | Oui | Oui |
| POLYLINE (LineString) | Oui | Oui |
| POLYGON (Polygon) | Oui | Oui |
| Champs attributaires | Oui | Oui |
| Filtre spatial | Oui | N/A |
| Filtre attributaire | Oui | N/A |
| Labels UTF-8 | Oui | Oui (auto-conversion CP1252) |
| Décomposition multi-géométrie | N/A | Oui (MultiPolygon → N Polygon) |
| Field mapping YAML | N/A | Oui (-dsco FIELD_MAPPING) |

---

## Spécification du format Polish Map

Le format Polish Map (`.mp`) est un format vectoriel texte pour créer des cartes GPS Garmin.

- **Extension** : `.mp`
- **Encodage** : CP1252 par défaut (UTF-8 via `CodePage=65001`)
- **Coordonnées** : WGS84 (EPSG:4326), degrés décimaux `(latitude,longitude)`
- **Structure** : Sections INI avec `[IMG ID]`, `[POI]`, `[POLYLINE]`, `[POLYGON]`, terminées par `[END]`

### Exemple de fichier

```
[IMG ID]
Name=Ma Carte
CodePage=1252
ID=12345678
[END]

[POI]
Type=0x2C00
Label=Restaurant
Data0=(48.8566,2.3522)
[END]

[POLYLINE]
Type=0x0001
Label=Route principale
Data0=(48.8500,2.3400),(48.8550,2.3500),(48.8600,2.3450)
[END]
```

### Header ([IMG ID])

| Champ | Obligatoire | Description |
|-------|-------------|-------------|
| `CodePage` | Oui | Code page d'encodage |
| `Name` | Non | Nom de la carte |
| `ID` | Non | Identifiant unique (8 chiffres) |
| `Datum` | Non | Datum géodésique (défaut: WGS 84) |
| `Elevation` | Non | Unité d'altitude (M/F) |
| `PreProcess` | Non | Flags de prétraitement |
| `TreSize` | Non | Taille TRE |
| `RgnLimit` | Non | Limite de région |

### Règles de validation

- Le fichier commence par `[IMG ID]` et chaque section se termine par `[END]`
- `Type` et `Data0` obligatoires pour toutes les features
- Latitude : [-90, +90], Longitude : [-180, +180]
- POI : 1 coordonnée, POLYLINE : min. 2, POLYGON : min. 3 uniques + fermeture

### Types Garmin

Les codes types (`Type=0xNNNN`) déterminent le rendu sur les appareils Garmin.

**POI :**

| Plage | Catégorie |
|-------|-----------|
| 0x2A00-0x2AFF | Attractions (musées, parcs, écoles) |
| 0x2B00-0x2BFF | Loisirs (théâtres, bars, cinémas) |
| 0x2C00-0x2CFF | Restauration |
| 0x2D00-0x2DFF | Hébergement |
| 0x2E00-0x2EFF | Shopping |
| 0x2F00-0x2FFF | Services (stations-service, gares, aéroports) |
| 0x3000-0x30FF | Santé/Communauté |
| 0x6400-0x6416 | Géographie |

**Routes (Polylines) :**

| Code | Description |
|------|-------------|
| 0x0001 | Autoroute |
| 0x0002 | Route nationale |
| 0x0003 | Route régionale |
| 0x0004 | Route artérielle |
| 0x0005 | Route collectrice |
| 0x0006 | Rue résidentielle |
| 0x000A | Route non revêtue |
| 0x000C | Rond-point |
| 0x000E | Piste 4x4 |
| 0x0014 | Chemin de fer |
| 0x001A | Rivière/Canal |

**Polygones :**

| Plage | Catégorie |
|-------|-----------|
| 0x0001-0x000E | Zones urbaines |
| 0x0010-0x0019 | Parcs et loisirs |
| 0x003C-0x0048 | Lacs et rivières |
| 0x004C-0x004F | Forêts, marais, toundra |
| 0x0050-0x0056 | Couverture du sol |

Types personnalisés : 0x10000-0x1FFFF (nécessite un fichier TYP).

---

## Conformité GDAL

Le driver est **100% conforme** aux 12 conventions GDAL (audit 2026-02-03) :

| Exigence | Description | Statut |
|----------|-------------|--------|
| NFR-GDAL1 | Pattern d'enregistrement (RegisterOGRPolishMap, plugin entry points) | PASS |
| NFR-GDAL2 | Conventions de nommage (OGR* prefix, PascalCase, préfixes hongrois) | PASS |
| NFR-GDAL3 | Logging CPL exclusif (CPLError/CPLDebug, aucun printf/cout) | PASS |
| NFR-GDAL4 | Comptage de références (Reference/Release sur FeatureDefn et SRS) | PASS |
| NFR-GDAL5 | Ownership RAII (unique_ptr, dataset possède les layers) | PASS |
| NFR-GDAL6 | Filtres spatial + attribut par couche (héritage OGRLayer) | PASS |
| NFR-GDAL7 | Capabilities (TestCapability, GDAL_DMD_* metadata) | PASS |
| NFR-GDAL8 | Patterns de retour (nullptr en erreur) | PASS |
| NFR-GDAL9 | Build CMake 3.20+ (C++17, in-tree et out-of-tree) | PASS |
| NFR-GDAL10 | Aucune dépendance externe (stdlib + GDAL uniquement) | PASS |
| NFR-GDAL11 | Tests C++ (14 fichiers, couverture complète) | PASS |
| NFR-GDAL12 | Documentation RST ([doc/polishmap.rst](doc/polishmap.rst)) | PASS |

---

## Tests de compilation Garmin

Les fichiers `.mp` peuvent être compilés en `.img` pour appareils GPS Garmin.

### mkgmap (recommandé, open-source)

```bash
# Prérequis : Java 11+
sudo apt install openjdk-11-jre
wget https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz
tar -xzf mkgmap-latest.tar.gz

# Compilation
java -jar mkgmap.jar output.mp

# Avec options
java -jar mkgmap.jar --family-id=1 --family-name="Ma Carte" output.mp
```

Validation : exit code 0, fichier `.img` généré, aucun ERROR/SEVERE. Les warnings "Unknown type" sont acceptables.

### Validation GPS manuelle

1. `ogr2ogr -f "PolishMap" test.mp input.geojson`
2. `java -jar mkgmap.jar test.mp`
3. Copier le `.img` sur le GPS : `/Garmin/` (eTrex, Montana) ou `/Map/` (Edge)
4. Vérifier : POIs cliquables, routes affichées, polygones remplis

---

## Dépannage

### Driver non trouvé

```bash
# Activer les logs de debug GDAL
export CPL_DEBUG=GDAL
ogrinfo --formats 2>&1 | head -20
```

Vérifier : la bibliothèque existe dans le répertoire plugin, `GDAL_DRIVER_PATH` est correctement positionné, et le plugin a été compilé avec la même version de GDAL.

### Erreurs de build

| Erreur | Solution |
|--------|----------|
| `gdal_priv.h: No such file` | `sudo apt install libgdal-dev` |
| `Could not find GDAL` | `cmake .. -DCMAKE_PREFIX_PATH=/usr/local` |
| `Symbol not found` (runtime) | Recompiler contre la version GDAL installée |

### Problèmes d'encodage

Le format Polish Map utilise CP1252 par défaut. Si les labels sont corrompus :

```bash
# Convertir manuellement
iconv -f UTF-8 -t CP1252 input.mp > output.mp
```

---

## Structure du projet

```
ogr-polishmap/
├── src/                    # Code source C++ du driver
├── test/                   # Suite de tests et données de test
│   └── data/               # Corpus de test (valid, edge-cases, error-recovery)
├── doc/
│   └── polishmap.rst       # Documentation RST officielle (standard GDAL)
├── examples/               # Scripts Python d'exemple et configs YAML
└── CMakeLists.txt          # Configuration CMake
```

---

## Références

- [GDAL Vector Driver Tutorial](https://gdal.org/tutorials/vector_driver_tut.html)
- [Manuel cGPSmapper](http://www.cgpsmapper.com/manual.htm) - Spécification complète du format
- [Documentation mkgmap](https://www.mkgmap.org.uk/doc/index.html) - Compilateur open-source
- [Types Garmin OSM](https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/POI_Types)

## Licence

MIT License - voir le fichier [LICENSE](../LICENSE).
