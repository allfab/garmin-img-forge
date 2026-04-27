# pipeline/

Répertoire de production du pipeline `mpforge → imgforge → gmapsupp.img`.

---

## Structure

```
pipeline/
├── configs/                        # Configurations YAML mpforge
│   └── ign-bdtopo/
│       ├── departement/            # Scope département (sources.yaml, garmin-rules.yaml)
│       ├── france-quadrant/        # Scope quadrant Garmin (FRANCE-SE/SO/NE/NO)
│       ├── outre-mer/              # Scope DOM (la-guadeloupe, la-reunion, …)
│       └── generalize-profiles*.yaml  # Catalogues de profils de simplification
├── data/                           # Données source (non versionnées)
│   ├── bdtopo/YYYY/vYYYY.MM/D0XX/ # BDTOPO IGN par département
│   ├── contours/                   # Courbes de niveau IGN
│   ├── dem/                        # BD ALTI (MNT) par département
│   ├── osm/                        # Données OSM (GPKG pré-convertis)
│   └── hiking-trails/              # Sentiers GR (FRANCE-GR.shp)
├── output/                         # Sorties générées (non versionnées)
│   └── YYYY/vYYYY.MM/D0XX/
│       ├── mp/                     # Tuiles Polish Map (.mp) — sortie mpforge
│       └── img/                    # Carte compilée (.img) — sortie imgforge
└── resources/
    └── typfiles/I2023100.typ       # Fichier de styles Garmin (CP1252)
```

---

## Téléchargement des données — D038 (Isère)

Avant tout build, télécharger les données source avec `scripts/download-data.sh` :

```bash
./scripts/download-data.sh \
    --zones D038 \
    --bdtopo-version v2026.03 \
    --format SHP \
    --with-contours \
    --with-osm \
    --with-dem
```

Cela peuple `pipeline/data/bdtopo/2026/v2026.03/D038/`, `pipeline/data/contours/`, `pipeline/data/osm/` et `pipeline/data/dem/D038/`.

---

## Lancement rapide — D038 (Isère)

Le script `scripts/build-garmin-map.sh` orchestre mpforge et imgforge en une seule commande. Données disponibles : `YEAR=2026`, `VERSION=v2026.03`.

### Niveau 1 — Maximum simplifié (quadrants, gros scopes)

```bash
./scripts/build-garmin-map.sh --zones D038 \
  --reduce-point-density 4.0 \
  --simplify-polygons "24:12,18:10,16:8" \
  --min-size-polygon 8 \
  --merge-lines
```

### Niveau 2 — Standard (production département, recommandé)

```bash
./scripts/build-garmin-map.sh --zones D038
```

### Niveau 3 — Géométries brutes mpforge (mesure de l'apport des profils)

```bash
./scripts/build-garmin-map.sh --zones D038 \
  --disable-profiles
```

### Niveau 4 — Données brutes complètes (debug uniquement)

```bash
./scripts/build-garmin-map.sh --zones D038 \
  --disable-profiles \
  --no-round-coords \
  --no-size-filter \
  --no-remove-obsolete-points
```

> **Niveau 4 — attention firmware :** `--no-round-coords` produit un IMG non quantifié sur la grille de subdivision. Toléré par QMapShack/QGIS, potentiellement non conforme sur Garmin Alpha 100. Réserver au debug.

---

## Commandes standalone (sans le script)

Pour appeler mpforge et imgforge directement, exporter d'abord les variables d'environnement que le script prépare automatiquement :

```bash
export DATA_ROOT="./pipeline/data/bdtopo/2026/v2026.03"
export CONTOURS_DATA_ROOT="./pipeline/data/contours"
export OSM_DATA_ROOT="./pipeline/data/osm"
export HIKING_TRAILS_DATA_ROOT="./pipeline/data/hiking-trails"
export OUTPUT_DIR="./pipeline/output/2026/v2026.03/D038"
export BASE_ID=38
export ZONES=D038
mkdir -p "$OUTPUT_DIR/mp" "$OUTPUT_DIR/img"
```

Puis :

```bash
# Étape 1 — tuilage
mpforge build \
  --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
  --report "$OUTPUT_DIR/mpforge-report.json" \
  --jobs 8

# Étape 2 — compilation
imgforge build "$OUTPUT_DIR/mp" \
  --output "$OUTPUT_DIR/img/IGN-BDTOPO-D038-v2026.03.img" \
  --jobs 8 \
  --family-id 1100 --product-id 1 \
  --family-name "IGN-BDTOPO-D038-v2026.03" \
  --series-name "IGN-BDTOPO-MAP" \
  --code-page 1252 --lower-case \
  --levels "24,22,20,18,16" \
  --typ-file pipeline/resources/typfiles/I2023100.typ \
  --route \
  --dem ./pipeline/data/dem/D038 --dem-source-srs EPSG:2154 \
  --packaging legacy
```

Ajouter `--disable-profiles` à mpforge et/ou les options de simplification / `--no-*` à imgforge selon le niveau voulu (voir tableau ci-dessus).

---

## Références

| Ressource | Chemin |
|-----------|--------|
| Config mpforge département | `pipeline/configs/ign-bdtopo/departement/sources.yaml` |
| Config mpforge quadrant | `pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml` |
| Règles Garmin département | `pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml` |
| Profils de simplification (scopes locaux) | `pipeline/configs/ign-bdtopo/generalize-profiles-local.yaml` |
| Profils de simplification (partagés) | `pipeline/configs/ign-bdtopo/generalize-profiles.yaml` |
| Styles TYP | `pipeline/resources/typfiles/I2023100.typ` |
| Script de build | `scripts/build-garmin-map.sh` |
| Script de téléchargement | `scripts/download-bdtopo.sh` |

Documentation complète : [site/docs/reference/niveaux-de-simplification.md](../site/docs/reference/niveaux-de-simplification.md) — [site/docs/le-pipeline/](../site/docs/le-pipeline/)
