# Geographic Data Download

The `./scripts/download-data.sh` script downloads and organizes
all the sources needed to produce the Garmin map.

## Downloaded Sources

| Source | Content |
|--------|---------|
| IGN BDTOPO | Roads, hydrography, buildings, vegetation, toponymy… |
| Contour lines | 10 m isolines (SRTM / RGE Alti) |
| OpenStreetMap | GR hiking trails, cycle paths, POIs |
| DEM / Hill shading | Relief data for hillshading |
| Pools | OSM Overpass polygons |

## Local Organization

```
./pipeline/data/
  bdtopo/2026/v2026.03/D038/   ← BDTOPO shapefiles
  osm/D038/                    ← OSM extract
  contours/D038/               ← contour lines
  dem/D038/                    ← DEM relief
```
