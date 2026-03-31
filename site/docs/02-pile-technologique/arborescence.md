# Arborescence du dépôt

```
garmin-ign-bdtopo-map/
├── mpforge/          ← CLI Rust : vecteur → tuiles Polish Map
├── imgforge/         ← CLI Rust : tuiles .mp → Garmin .img
├── ogr-polishmap/    ← Driver OGR/GDAL pour le format .mp
├── configs/          ← Configurations YAML du pipeline
├── resources/        ← Fichiers TYP, icônes, etc.
├── scripts/          ← Scripts d'automatisation
├── data/             ← Données BD TOPO (non versionnées)
│   └── bdtopo/
├── output/           ← Artefacts de build (non versionnés)
│   └── tiles/
└── site/             ← Ce site de documentation
```
