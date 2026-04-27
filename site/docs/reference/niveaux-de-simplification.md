# Niveaux de simplification géométrique

Le pipeline dispose de **deux couches de simplification indépendantes** qui s'appliquent à des étapes différentes. Comprendre leur interaction permet de calibrer précisément le compromis taille / fidélité géométrique de la carte produite.

---

## Les deux couches

| Couche | Outil | Active par défaut | Portée |
|--------|-------|:-----------------:|--------|
| Profils de généralisation (`generalize-profiles-local.yaml`) | mpforge | Oui | Chaque feature reçoit plusieurs géométries `Data0..Data6` selon le zoom ; algorithmes VW/DP + Chaikin |
| Filtres DP lignes / polygones + filtre taille | imgforge | Non (opt-in) | Réduction des vertices et micro-polygones à l'encodage IMG |
| Quantification + SizeFilter + RemoveObsoletePoints | imgforge | Oui | Chaîne de filtres mkgmap r4924 appliquée à chaque subdivision à `n > 0` |

Les profils mpforge et les filtres opt-in imgforge sont **cumulatifs** : la donnée sort des shapefiles, traverse les profils mpforge (simplification multi-Data), puis imgforge applique sa propre chaîne de filtres. Les options `--no-*` d'imgforge désactivent quant à elles des filtres actifs par défaut.

---

## Les 4 niveaux — du moins au plus détaillé

| # | Profils mpforge | imgforge — DP/taille (opt-in) | imgforge — filtres géom (défaut) | Cas d'usage |
|---|:-:|:-:|:-:|---|
| **1 — Max simplifié** | actifs | `reduce + simplify-poly + min-size + merge` | actifs | Quadrants, France entière — taille minimale |
| **2 — Standard** | actifs | aucun | actifs | **Production département** — recommandé |
| **3 — mpforge brut** | désactivés | aucun | actifs | Mesure de l'apport des profils |
| **4 — Données brutes** | désactivés | aucun | désactivés | Debug / mesure d'impact des filtres imgforge |

!!! warning "Niveau 4 et matériel Garmin"
    Le niveau 4 désactive `--no-round-coords`, ce qui produit un IMG avec des coordonnées non quantifiées sur la grille de subdivision. Toléré par QMapShack et QGIS, **potentiellement non conforme au rendu firmware** (notamment Alpha 100). Réserver à la mesure d'impact et au debug — ne pas utiliser en production.

---

## Variables d'environnement (commandes standalone)

Avant d'appeler mpforge ou imgforge directement (hors script), exporter ces variables — le script `build-garmin-map.sh` s'en charge automatiquement :

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

---

## Niveau 1 — Maximum simplifié

Profils mpforge actifs + tous les filtres imgforge opt-in. Recommandé pour les quadrants et la France entière.

=== "build-garmin-map.sh (recommandé)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038 \
      --reduce-point-density 4.0 \
      --simplify-polygons "24:12,18:10,16:8" \
      --min-size-polygon 8 \
      --merge-lines
    ```

=== "mpforge (standalone)"

    ```bash
    # Les profils sont actifs par défaut (generalize_profiles_path dans sources.yaml)
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8
    ```

=== "imgforge (standalone)"

    ```bash
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
      --packaging legacy \
      --reduce-point-density 4.0 \
      --simplify-polygons "24:12,18:10,16:8" \
      --min-size-polygon 8 \
      --merge-lines
    ```

---

## Niveau 2 — Standard (production département)

Profils mpforge actifs, filtres imgforge par défaut. C'est la configuration de référence pour un département.

=== "build-garmin-map.sh (recommandé)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8
    ```

=== "imgforge (standalone)"

    ```bash
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

---

## Niveau 3 — Géométries brutes mpforge

Profils désactivés, imgforge par défaut. Permet de mesurer l'apport des profils de généralisation sur la taille et la fluidité de la carte.

=== "build-garmin-map.sh (recommandé)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038 \
      --disable-profiles
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8 \
      --disable-profiles
    ```

=== "imgforge (standalone)"

    ```bash
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

!!! note "Bypass ciblé via variable d'environnement"
    `MPFORGE_PROFILES=off mpforge build --config …` est équivalent à `--disable-profiles`. Utile pour les scripts CI qui ne veulent pas modifier les arguments.

---

## Niveau 4 — Données brutes complètes

Profils désactivés + tous les filtres imgforge par défaut désactivés. Réservé à la mesure d'impact et au debug.

=== "build-garmin-map.sh (recommandé)"

    ```bash
    ./scripts/build-garmin-map.sh --zones D038 \
      --disable-profiles \
      --no-round-coords \
      --no-size-filter \
      --no-remove-obsolete-points
    ```

=== "mpforge (standalone)"

    ```bash
    mpforge build \
      --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
      --report "$OUTPUT_DIR/mpforge-report.json" \
      --jobs 8 \
      --disable-profiles
    ```

=== "imgforge (standalone)"

    ```bash
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
      --packaging legacy \
      --no-round-coords \
      --no-size-filter \
      --no-remove-obsolete-points
    ```

---

## Référence des options de simplification

### Options mpforge

| Option | Description |
|--------|-------------|
| _(par défaut)_ | Profils `generalize-profiles-local.yaml` actifs — chaque feature reçoit `Data0..Data6` selon ses tolérances VW/DP |
| `--disable-profiles` | Bypasse le catalogue externe ; les directives `generalize:` inline dans `sources.yaml` restent actives |
| `MPFORGE_PROFILES=off` | Équivalent variable d'environnement de `--disable-profiles` |

### Options imgforge opt-in (simplification supplémentaire)

| Option | Référence mkgmap | Description |
|--------|:-----------------:|-------------|
| `--reduce-point-density 4.0` | `4.0` | Douglas-Peucker sur les polylignes (epsilon en unités carte) |
| `--simplify-polygons "24:12,18:10,16:8"` | — | DP sur les polygones par résolution (bits:epsilon) |
| `--min-size-polygon 8` | `8` | Filtre les polygones < N unités carte (élimine les micro-surfaces) |
| `--merge-lines` | activé | Fusionne les polylignes adjacentes de même type et label |

!!! tip "Quand activer les options opt-in"
    Pour un **département**, les valeurs par défaut suffisent (niveau 2 standard).
    Pour un **quadrant** (≥ 20 départements), activez les 4 options : la taille IMG baisse de 15-25 % et imgforge tient en RAM avec moins de workers.

### Options imgforge filtres par défaut (opt-out)

Ces filtres reprennent la chaîne mkgmap r4924 — ils s'appliquent à chaque subdivision à `n > 0`.

| Option | Description |
|--------|-------------|
| `--no-round-coords` | Désactive la quantification des coordonnées sur la grille de subdivision (`RoundCoordsFilter`) |
| `--no-size-filter` | Désactive le rejet des features sous-pixel (`SizeFilter`) |
| `--no-remove-obsolete-points` | Désactive la suppression des points colinéaires/spikes post-quantification (`RemoveObsoletePointsFilter`) |

---

## Pour aller plus loin

- [Profils de généralisation](generalize-profiles.md) — structure YAML, algorithmes VW/DP, dispatch conditionnel, profils BDTOPO de production
- [Comparaison mkgmap/imgforge](comparaison-mkgmap-imgforge.md) — mesures bytes RGN par niveau et analyse de la chaîne de filtres
- [Étape 3 — Tuilage (mpforge)](../le-pipeline/etape-3-tuilage.md) — référence complète des options de `build-garmin-map.sh`
- [Étape 4 — Compilation (imgforge)](../le-pipeline/etape-4-compilation.md) — optimisation géométrique et DEM
