# Téléchargement des données géographiques

Le script `./scripts/download-data.sh` télécharge et organise
toutes les sources nécessaires à la production de la carte Garmin.

## Sources téléchargées

| Source | Contenu |
|--------|---------|
| IGN BDTOPO | Routes, hydrographie, bâti, végétation, toponymie… |
| Courbes de niveau | Isolignes 10 m (SRTM / RGE Alti) |
| OpenStreetMap | Sentiers GR, pistes cyclables, POI |
| DEM / Hill shading | Données de relief pour l'ombrage |
| Piscines | Polygones OSM Overpass |

## Organisation locale

```
./pipeline/data/
  bdtopo/2026/v2026.03/D038/   ← shapefiles BDTOPO
  osm/D038/                    ← extrait OSM
  contours/D038/               ← courbes de niveau
  dem/D038/                    ← MNT relief
```
