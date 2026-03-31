# Socle de données

## BD TOPO IGN

La **BD TOPO** est la base de données topographique de référence de l'IGN. Elle couvre l'ensemble
du territoire français métropolitain et ultra-marin.

### Caractéristiques

- Précision : métrique à décamétrique selon les thèmes
- Format : GeoPackage (`.gpkg`) ou Shapefile (`.shp`)
- Projection : Lambert-93 (EPSG:2154)
- Licence : Etalab 2.0 (ouverte et gratuite)

### Téléchargement

Les données sont disponibles sur le [Géoportail de l'IGN](https://geoservices.ign.fr/bdtopo).

Le script `download-bdtopo.sh` automatise le téléchargement par département ou région.
