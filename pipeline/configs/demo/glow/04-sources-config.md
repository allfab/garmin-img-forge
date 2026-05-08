# sources.yaml — Grille, couche et en-tête

## Grille de tuilage

```yaml
grid:
  cell_size: 0.225   # ~25 km par tuile
  overlap: 0.005     # Recouvrement pour éviter les artefacts de bord
```

mpforge découpe le département en tuiles carrées indépendantes.
Chaque tuile devient un fichier **Polish Map** (`.mp`).

## Déclarer une couche d'entrée

```yaml
- path: "./pipeline/data/IGN-BDTOPO/2026/v2026.03/D038/TRANSPORT/TRONCON_DE_ROUTE.shp"
  source_srs: "EPSG:2154"   # Lambert-93 (projection IGN)
  target_srs: "EPSG:4326"   # WGS84 (requis par les GPS Garmin)
  dedup_by_field: ID        # Supprime les doublons sur l'identifiant IGN
```

## En-tête Polish Map — 7 niveaux de zoom

```yaml
header:
  levels: "7"
  level0: "24"   #   ~1 m/px — zoom maximum (navigation à pied)
  level1: "23"   #   ~2 m/px
  level2: "22"   #   ~5 m/px
  level3: "21"   #  ~10 m/px
  level4: "20"   #  ~20 m/px
  level5: "18"   #  ~80 m/px
  level6: "16"   # ~320 m/px — vue large (aperçu de zone)
  routing: "Y"   # Calcul d'itinéraire activé
```
