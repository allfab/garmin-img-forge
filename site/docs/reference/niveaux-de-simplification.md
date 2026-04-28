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
| **1 — Quadrant (recommandé)** | actifs | `min-size + merge` | actifs | **Quadrants, France entière** — profiles mpforge actifs, pas de double simplification |
| **2 — Standard** | actifs | aucun | actifs | **Production département** — recommandé |
| **3 — mpforge brut** | désactivés | aucun | actifs | Mesure de l'apport des profils |
| **4 — Données brutes** | désactivés | aucun | désactivés | Debug / mesure d'impact des filtres imgforge |

!!! warning "Double simplification — piège à éviter"
    Les options `--reduce-point-density` et `--simplify-polygons` appliquent un DP **supplémentaire** à imgforge sur des données **déjà simplifiées** par mpforge (`generalize-profiles.yaml`). Cumuler les deux dégrade la précision géométrique aux zooms détaillés (n=0..2, GPS 25–1500 m) sans gain réel sur la taille.

    **Règle :** si les profils mpforge sont actifs, ne pas utiliser `--reduce-point-density` ni `--simplify-polygons`. Ces options ne sont pertinentes que sans profils (`--disable-profiles`).

!!! warning "Niveau 4 et matériel Garmin"
    Le niveau 4 désactive `--no-round-coords`, ce qui produit un IMG avec des coordonnées non quantifiées sur la grille de subdivision. Toléré par QMapShack et QGIS, **potentiellement non conforme au rendu firmware** (notamment Alpha 100). Réserver à la mesure d'impact et au debug — ne pas utiliser en production.

---

## Prérequis — téléchargement des données

Avant tout build, les données source doivent être présentes dans `pipeline/data/`. Utiliser `download-data.sh` :

```bash
./scripts/download-data.sh \
    --zones D038 \
    --bdtopo-version v2026.03 \
    --format SHP \
    --with-contours \
    --with-osm \
    --with-dem
```

Cela peuple `pipeline/data/bdtopo/2026/v2026.03/D038/`, `pipeline/data/contours/`, `pipeline/data/osm/` et `pipeline/data/dem/D038/` — les chemins attendus par `sources.yaml` via les variables d'environnement ci-dessous.

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

## Niveau 1 — Quadrant (recommandé)

Profils mpforge actifs + filtres imgforge `--min-size-polygon` et `--merge-lines`. Recommandé pour les quadrants et la France entière.

`--reduce-point-density` et `--simplify-polygons` sont **exclus** : les profils mpforge gèrent déjà la simplification multi-niveaux ; les ajouter produirait une double simplification qui dégrade la précision aux zooms détaillés (voir encadré ci-dessus).

=== "build-garmin-map.sh (recommandé)"

    ```bash
    ./scripts/build-garmin-map.sh \
      --region FRANCE-SE \
      --config pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml \
      --levels "24,23,22,21,20,18,16" \
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
    Pour un **quadrant** (≥ 20 départements), activez uniquement `--min-size-polygon 8` et `--merge-lines`. Ne pas utiliser `--reduce-point-density` ni `--simplify-polygons` si les profils mpforge sont actifs (double simplification — voir encadré ci-dessus).

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
