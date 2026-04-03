# ogr-polishmap — Le Driver GDAL/OGR

## Le problème : un format mal supporté

Le format **Polish Map** (`.mp`) est le format intermédiaire indispensable pour créer des cartes Garmin. C'est un format texte de type INI, inventé par le logiciel cGPSmapper dans les années 2000, qui décrit des points d'intérêt (POI), des lignes (routes, rivières) et des polygones (forêts, lacs, bâtiments) avec leurs codes types Garmin.

**Le problème** : aucun outil SIG majeur — ni open-source (QGIS), ni propriétaire (ArcGIS) — ne sait lire ou écrire ce format nativement, et aucun des 200+ formats supportés par GDAL/OGR ne le couvre. Seul **Global Mapper** (propriétaire, licence payante) sait lire et enregistrer le format Polish Map — c'est d'ailleurs grâce à cet outil que j'ai pu appréhender la structure du format `.mp`. Pour le reste, il fallait utiliser GPSMapEdit (propriétaire, Windows uniquement) ou écrire des scripts ad hoc fragiles.

## La solution : un driver GDAL natif

**ogr-polishmap** est un driver C++ qui s'intègre directement dans GDAL/OGR — la bibliothèque de référence mondiale pour les données géospatiales. Une fois installé, le format Polish Map devient un citoyen de premier rang dans tout l'écosystème GDAL :

```bash
# Convertir un Shapefile en Polish Map
ogr2ogr -f "PolishMap" output.mp COMMUNE.shp

# Convertir un Polish Map en GeoJSON
ogr2ogr -f "GeoJSON" output.geojson input.mp

# Lire un Polish Map dans QGIS
# → Ouvrir directement le fichier .mp comme n'importe quel autre format
```

Cela signifie que **tous les outils basés sur GDAL** (QGIS, ogr2ogr, Python/GDAL, R/sf, PostGIS...) peuvent désormais manipuler des fichiers Polish Map nativement.

## Fonctionnalités

| Fonctionnalité | Lecture | Écriture |
|----------------|---------|----------|
| POI (Point) | Oui | Oui |
| POLYLINE (LineString) | Oui | Oui |
| POLYGON (Polygon) | Oui | Oui |
| Champs attributaires | Oui | Oui |
| Filtre spatial | Oui | N/A |
| Filtre attributaire | Oui | N/A |
| Labels UTF-8 | Oui | Oui (auto-conversion CP1252) |
| Décomposition multi-géométrie | N/A | Oui (MultiPolygon vers N Polygon) |
| Field mapping YAML | N/A | Oui (`-dsco FIELD_MAPPING`) |

## Field mapping YAML

Le driver supporte un mapping configurable des noms de champs. Quand vos données sources utilisent des noms de colonnes personnalisés (`MP_TYPE`, `NAME`), le field mapping les transpose automatiquement vers les champs Polish Map standards (`Type`, `Label`) :

```yaml
# bdtopo-mapping.yaml
field_mapping:
  MP_TYPE: Type          # Code type Garmin (ex: 0x4e00)
  NAME: Label            # Nom de la feature
  Country: CountryName   # Pays
  MPBITLEVEL: Levels     # Niveaux de zoom
```

```bash
ogr2ogr -f "PolishMap" communes.mp COMMUNE.shp \
    -dsco FIELD_MAPPING=bdtopo-mapping.yaml
```

## Le format Polish Map en détail

### Structure d'un fichier .mp

Un fichier Polish Map est un fichier texte structuré en sections INI. Chaque fichier commence par un header `[IMG ID]` suivi d'une série d'objets géographiques :

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

[POLYGON]
Type=0x0050
Label=Forêt de Chartreuse
Data0=(45.35,5.78),(45.36,5.79),(45.35,5.80),(45.35,5.78)
[END]
```

### Règles fondamentales

- **Coordonnées** en WGS84 (EPSG:4326), format `(latitude,longitude)` en degrés décimaux
- **Encodage** CP1252 par défaut (UTF-8 via `CodePage=65001`)
- Chaque objet a un **Type** (code hexadécimal Garmin) qui détermine le rendu sur le GPS
- Les polygones doivent être **fermés** (premier point = dernier point)
- Maximum **1024 points** par polyligne

### Pourquoi ce format intermédiaire ?

Le format Garmin IMG est un binaire opaque et complexe. Le Polish Map sert de **représentation lisible** entre les données SIG et le binaire final :

```mermaid
%%{init: {'theme': 'base', 'themeVariables': {'primaryColor': '#4caf50', 'primaryTextColor': '#000', 'lineColor': '#666'}}}%%
flowchart LR
    A["Données SIG<br/>.shp, .gpkg"] -->|ogr-polishmap| B["Polish Map<br/>.mp"]
    B -->|imgforge| C["Garmin IMG<br/>.img"]

    style A fill:#4caf50,stroke:#2e7d32,color:#fff
    style B fill:#ff9800,stroke:#e65100,color:#fff
    style C fill:#2196f3,stroke:#1565c0,color:#fff
```

Le rôle d'ogr-polishmap se situe sur la **première flèche** : la conversion des données SIG vers le format Polish Map. La seconde étape (Polish Map → Garmin IMG) est assurée par imgforge.

C'est cette architecture en deux étapes qui rend le pipeline modulaire et débogable. On peut inspecter les fichiers `.mp` à tout moment pour vérifier que les données sont correctement transformées avant la compilation finale.

## Conformité GDAL

Le driver est **100 % conforme** aux 12 conventions GDAL/OGR (audit février 2026) :

- Pattern d'enregistrement standard (plugin + intégré)
- Conventions de nommage OGR (PascalCase, préfixes hongrois)
- Logging CPL exclusif (pas de printf/cout)
- Comptage de références correct sur FeatureDefn et SRS
- Ownership RAII (unique_ptr, dataset possède les layers)
- Filtres spatial et attributaire par couche
- Capabilities (TestCapability, metadata GDAL_DMD_*)
- Build CMake 3.20+ (C++17, in-tree et out-of-tree)
- Aucune dépendance externe (stdlib + GDAL uniquement)
- Suite de tests C++ complète (14 fichiers)

## Installation

### Linux (Debian/Ubuntu)

```bash
# Dépendances
sudo apt-get install -y libgdal-dev gdal-bin cmake g++

# Build
cd tools/ogr-polishmap
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)

# Installation comme plugin GDAL
sudo cp ogr_PolishMap.so $(gdal-config --plugindir)/

# Vérification
ogrinfo --formats | grep -i polish
# → PolishMap -vector- (rw+v): Polish Map Format (*.mp)
```

### Windows / QGIS (via OSGeo4W)

```cmd
cd tools\ogr-polishmap
cmake -B build -G "NMake Makefiles" -DCMAKE_BUILD_TYPE=Release ^
    -DGDAL_INCLUDE_DIR=C:/OSGeo4W/include ^
    -DGDAL_LIBRARY=C:/OSGeo4W/lib/gdal_i.lib
cmake --build build

copy build\ogr_PolishMap.dll C:\OSGeo4W\apps\gdal\lib\gdalplugins\
```

Après redémarrage de QGIS, les fichiers `.mp` s'ouvrent directement.

## Python

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
