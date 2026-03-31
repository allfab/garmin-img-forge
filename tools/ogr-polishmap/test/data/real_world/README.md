# Real-World Test Data Corpus

Corpus de données test synthétiques pour les tests d'intégration real-world (Story 4.3).

## Structure

```
real_world/
├── bdtopo/
│   ├── COMMUNE_sample.shp (+.shx, .dbf, .prj)  # 3 communes La Réunion (MultiPolygon)
│   ├── ROUTE_sample.shp (+.shx, .dbf, .prj)     # 10 routes (LineString)
│   └── bdtopo_mapping.yaml                       # Config YAML field mapping BDTOPO
├── osm/
│   ├── roads.geojson                              # 10 routes (7 LineString + 3 MultiLineString)
│   ├── pois.geojson                               # 20 POIs (Point)
│   └── osm_mapping.yaml                          # Config YAML field mapping OSM
└── generic/
    ├── encoding_test.shp (+.shx, .dbf, .prj)    # 10 noms FR/ES/DE avec accents
    ├── large_multipolygon.shp (+.shx, .dbf, .prj) # 1 feature, 100-part MultiPolygon
    └── mixed_geometries.shp (+.shx, .dbf, .prj) # 5 polygons variés
```

## Détails des datasets

### BDTOPO (IGN-like)

**COMMUNE_sample.shp** — 3 communes de La Réunion :
- Les Avirons (MultiPolygon 2 parts, Zip=97425)
- Saint-Pierre (MultiPolygon 1 part, Zip=97410)
- Le Tampon (MultiPolygon 3 parts, Zip=97430)

Champs : NAME, MP_TYPE, Country, RegionName, CityName, Zip, EndLevel, MPBITLEVEL

**ROUTE_sample.shp** — 10 routes :
- 3 Routes Nationales (type 0x02)
- 3 Routes Départementales (type 0x04)
- 2 Rues (type 0x06)
- 1 Chemin Rural (type 0x0A)
- 1 Sentier (type 0x16)

Champs : NAME, MP_TYPE, RoadID, EndLevel

### OSM (OpenStreetMap-like)

**roads.geojson** — 10 features :
- 7 LineString (residential, secondary, primary, trunk, track, living_street)
- 3 MultiLineString (primary N20, motorway A6, motorway BP) — 3 parts chacune

Champs : name, highway, ref

**pois.geojson** — 20 POIs :
- Variété de types (bakery, pharmacy, school, restaurant, hotel, etc.)
- Noms français avec accents

Champs : name, [amenity|shop|tourism|...]

### Generic

**encoding_test.shp** — 10 noms avec caractères spéciaux CP1252 :
- Français : Château-Thierry, Île-de-France, Béziers, etc.
- Espagnol : Peñíscola
- Allemand : München, Köln, Düsseldorf

**large_multipolygon.shp** — Stress test décomposition :
- 1 feature avec MultiPolygon de 100 parts (grille 10x10)

**mixed_geometries.shp** — Round-trip test :
- 5 polygons : Zone Industrielle, Parc National, Lac, Aéroport, Zone Militaire

## Reproduction

Les données sont générées par le script `tools/generate_real_world_test_data.py` :

```bash
python3 tools/generate_real_world_test_data.py
```

Prérequis : Python 3, GDAL Python bindings (`osgeo`).

## Taille

Total corpus : ~34 KB (bien en dessous de la limite CI de 5 MB).

## Configs YAML Field Mapping

Les configs YAML utilisent le format Story 4.4 (`-dsco FIELD_MAPPING=path`) :

```yaml
field_mapping:
  SOURCE_FIELD: TARGET_FIELD
```

- `bdtopo_mapping.yaml` : NAME→Label, MP_TYPE→Type, Country→CountryName, etc.
- `osm_mapping.yaml` : name→Label, ref→RoadID, highway→Type
