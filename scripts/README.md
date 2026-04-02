# Pipeline BDTOPO → Garmin

> Scripts et documentation pour transformer les données IGN BD TOPO en carte Garmin (gmapsupp.img)

## Vue d'ensemble du pipeline

```
download-bdtopo.sh       01-export-mp.sh         02-build-img.sh
       |                       |                       |
       v                       v                       v
  Télécharge les         mpforge build             imgforge build
  données BDTOPO         (tuiles .mp)              (gmapsupp.img)
  depuis l'IGN           Shapefile → .mp           .mp → Garmin IMG
```

Le pipeline se décompose en 3 étapes indépendantes :

1. **Télécharger** les données BDTOPO (+ courbes de niveau optionnel) depuis la Géoplateforme IGN
2. **Exporter** les données en tuiles Polish Map (.mp) via mpforge
3. **Compiler** les tuiles en carte Garmin (gmapsupp.img) via imgforge

---

## Prérequis

### Outils requis

- **mpforge** — Générateur de tuiles Polish Map (.mp)
- **imgforge** — Compilateur Garmin IMG
- **envsubst** — Substitution de variables dans les configs YAML (`sudo apt install gettext-base`)
- **curl**, **7z** — Pour le téléchargement BDTOPO (`sudo apt install curl p7zip-full`)

### Compilation des outils Rust

```bash
# mpforge (depuis la racine du dépôt)
cargo build --release --manifest-path tools/mpforge/Cargo.toml

# imgforge (depuis la racine du dépôt)
cargo build --release --manifest-path tools/imgforge/Cargo.toml
```

Les binaires sont dans `tools/mpforge/target/release/mpforge` et `tools/imgforge/target/release/imgforge`.

Pour les installer globalement :

```bash
sudo cp tools/mpforge/target/release/mpforge /usr/local/bin/
sudo cp tools/imgforge/target/release/imgforge /usr/local/bin/
```

---

## Configuration

### Variables d'environnement

Le fichier `pipeline/.env.example` centralise toutes les variables du pipeline :

```bash
# Copier le template
cp pipeline/.env.example pipeline/.env

# Éditer les chemins selon votre installation
nano pipeline/.env

# Charger les variables avant de lancer les scripts
source pipeline/.env
```

### Fichiers de configuration

| Fichier | Rôle |
|---|---|
| `pipeline/.env.example` | Template des variables d'environnement (`DATA_ROOT`, `CONTOURS_DATA_ROOT`, etc.) |
| `pipeline/configs/ign-bdtopo/ign-bdtopo-sources-shp.yaml` | Sources SHP : couches BDTOPO + courbes de niveau, grille de tuilage, header MP |
| `pipeline/configs/ign-bdtopo/ign-bdtopo-garmin-rules.yaml` | Règles de mapping BDTOPO + courbes → types Garmin |

Convention de nommage : `ign-bdtopo-<theme>.yaml` (sources par format, règles de mapping).

---

## Arborescence

```
scripts/
  download-bdtopo.sh          # Étape 0 : téléchargement BDTOPO
  shapefile/
    01-export-mp.sh            # Étape 1 : export SHP → tuiles .mp
  gpkg/
    01-export-mp.sh            # Étape 1 : export GPKG → .mp (à venir)
  postgis/
    01-export-mp.sh            # Étape 1 : export PostGIS → .mp (à venir)
  common/
    02-build-img.sh            # Étape 2 : compilation .mp → gmapsupp.img
  build-garmin-map.sh          # Pipeline tout-en-un (étapes 1+2)
  check_environment.sh         # Vérification de l'environnement
  test-static-build.sh         # Validation du build statique
  release.sh                   # Création de release
  retag.sh                     # Re-tag d'une release

pipeline/
  .env.example                 # Template des variables
  configs/ign-bdtopo/
    ign-bdtopo-sources-shp.yaml
    ign-bdtopo-garmin-rules.yaml
  data/bdtopo/                 # Données BDTOPO (gitignore)
  data/courbes/                # Courbes de niveau IGN (gitignore)
    D038/                      # Dalles SHP par département
      COURBE_0840_6440.shp
      COURBE_0880_6440.shp
      ...
  output/                      # Sortie du pipeline (gitignore)
    tiles/                     # Tuiles .mp générées
    gmapsupp.img               # Carte Garmin finale
```

---

## Étape 0 : Téléchargement des données BDTOPO

Script : `scripts/download-bdtopo.sh`

```bash
# Département unique (Isère)
./scripts/download-bdtopo.sh --zones D038 --format SHP

# Région entière (Auvergne-Rhône-Alpes)
./scripts/download-bdtopo.sh --region ARA --format SHP

# Simulation sans téléchargement
./scripts/download-bdtopo.sh --zones D038 --dry-run
```

Les données sont organisées dans `pipeline/data/bdtopo/{YYYY}/v{YYYY.MM}/{DXXX}/`.

### Courbes de niveau

Le produit IGN "Courbes de niveau" est livré séparément de la BD TOPO, par département. L'option `--with-contours` télécharge les courbes en parallèle de la BDTOPO.

```bash
# BDTOPO + courbes de niveau pour l'Isère
./scripts/download-bdtopo.sh --zones D038 --with-contours

# BDTOPO région ARA + courbes de niveau des 12 départements ARA
./scripts/download-bdtopo.sh --region ARA --with-contours

# Multi-départements
./scripts/download-bdtopo.sh --zones D038,D073,D074 --with-contours

# Simulation
./scripts/download-bdtopo.sh --zones D038 --with-contours --dry-run

# Répertoire de stockage personnalisé
./scripts/download-bdtopo.sh --zones D038 --with-contours --contours-root /data/courbes
```

Les courbes sont téléchargées dans `pipeline/data/courbes/{DXXX}/` (arborescence séparée de la BDTOPO). Chaque département contient plusieurs dalles SHP (`COURBE_0840_6440.shp`, `COURBE_0880_6440.shp`, etc.).

| Option | Description | Défaut |
|---|---|---|
| `--with-contours` | Télécharger aussi les courbes de niveau | `false` |
| `--contours-root <dir>` | Racine des données courbes | `./pipeline/data/courbes` |

> **Note** : `--with-contours` nécessite `--zones` ou `--region`. Sans zone, une erreur explicite est affichée.

---

## Étape 1 : Export des tuiles Polish Map (.mp)

### Shapefile (supporté)

Script : `scripts/shapefile/01-export-mp.sh`

#### Commande manuelle — binaire installé

```bash
# Préparer la config (substitution des placeholders)
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12/D038
export OUTPUT_DIR=./pipeline/output
envsubst '${DATA_ROOT} ${OUTPUT_DIR}' \
  < pipeline/configs/ign-bdtopo/ign-bdtopo-sources-shp.yaml \
  > /tmp/mpforge-config-expanded.yaml

# Lancer mpforge
mpforge build \
  --config /tmp/mpforge-config-expanded.yaml \
  --jobs 8 \
  --report ./pipeline/output/mpforge-report.json \
  --skip-existing \
  -v
```

#### Commande manuelle — release locale

```bash
# Préparer la config (substitution des placeholders)
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12/D038
export OUTPUT_DIR=./pipeline/output
envsubst '${DATA_ROOT} ${OUTPUT_DIR}' \
  < pipeline/configs/ign-bdtopo/ign-bdtopo-sources-shp.yaml \
  > /tmp/mpforge-config-expanded.yaml

# Lancer mpforge (binaire compilé localement)
./tools/mpforge/target/release/mpforge build \
  --config /tmp/mpforge-config-expanded.yaml \
  --jobs 8 \
  --report ./pipeline/output/mpforge-report.json \
  --skip-existing \
  -v
```

#### Via le script

```bash
source pipeline/.env
./scripts/shapefile/01-export-mp.sh --skip-existing -v
```

#### Paramètres mpforge build

| Paramètre | Description | Défaut |
|---|---|---|
| `--config <file>` | Fichier de configuration YAML (obligatoire) | — |
| `--input <dir>` | Override du répertoire d'entrée | depuis la config |
| `--output <dir>` | Override du répertoire de sortie | depuis la config |
| `--jobs <n>` | Nombre de jobs parallèles | `1` |
| `--report <file>` | Chemin du rapport JSON | *(aucun)* |
| `--skip-existing` | Passer les tuiles déjà générées | `false` |
| `--fail-fast` | Arrêter au premier échec | `false` |
| `--dry-run` | Simuler sans écrire de fichiers | `false` |
| `-v`, `-vv`, `-vvv` | Verbosity (INFO, DEBUG, TRACE) | off |

### GeoPackage (à venir)

Script : `scripts/gpkg/01-export-mp.sh`

> **Non supporté** — mpforge ne supporte actuellement que les Shapefiles comme source SIG.
> Ce script affiche un message d'erreur et exit 1.

### PostGIS (à venir)

Script : `scripts/postgis/01-export-mp.sh`

> **Non supporté** — mpforge ne supporte actuellement que les Shapefiles comme source SIG.
> Ce script affiche un message d'erreur et exit 1.

---

## Étape 2 : Compilation Garmin IMG

Script : `scripts/common/02-build-img.sh`

#### Commande manuelle — binaire installé

```bash
imgforge build ./pipeline/output/tiles/ \
  --output ./pipeline/output/gmapsupp.img \
  --jobs 8 \
  --family-id 6324 \
  --product-id 1 \
  --series-name "BDTOPO France" \
  --family-name "IGN BDTOPO" \
  -v
```

#### Commande manuelle — release locale

```bash
./tools/imgforge/target/release/imgforge build ./pipeline/output/tiles/ \
  --output ./pipeline/output/gmapsupp.img \
  --jobs 8 \
  --family-id 6324 \
  --product-id 1 \
  --series-name "BDTOPO France" \
  --family-name "IGN BDTOPO" \
  -v
```

#### Via le script

```bash
source pipeline/.env
./scripts/common/02-build-img.sh -v

# Simulation sans exécuter
./scripts/common/02-build-img.sh --dry-run
```

#### Paramètres imgforge build

| Paramètre | Description | Défaut |
|---|---|---|
| `<input>` | Répertoire contenant les tuiles .mp (obligatoire) | — |
| `--output <file>` | Fichier de sortie | `gmapsupp.img` |
| `--jobs <n>` | Nombre de jobs parallèles | *(auto)* |
| `--family-id <id>` | Family ID Garmin | *(aucun)* |
| `--product-id <id>` | Product ID Garmin | *(aucun)* |
| `--series-name <name>` | Nom de la série | *(aucun)* |
| `--family-name <name>` | Nom de la famille | *(aucun)* |
| `-v`, `-vv`, `-vvv` | Verbosity (INFO, DEBUG, TRACE) | off |

#### Paramètres imgforge compile (fichier unique)

```bash
imgforge compile input.mp --output output.img --description "Ma carte"
```

| Paramètre | Description | Défaut |
|---|---|---|
| `<input>` | Fichier .mp à compiler (obligatoire) | — |
| `--output <file>` | Fichier .img de sortie | *(dérivé du nom d'entrée)* |
| `--description <desc>` | Description de la carte | *(aucun)* |
| `-v`, `-vv`, `-vvv` | Verbosity | off |

---

## Pipeline automatique (tout-en-un)

Script : `scripts/build-garmin-map.sh`

Ce script enchaîne automatiquement les étapes 1 et 2 avec auto-découverte des binaires et génération dynamique de la config.

```bash
# Auto-découverte de tout
./scripts/build-garmin-map.sh

# Département spécifique
./scripts/build-garmin-map.sh --data-root pipeline/data/bdtopo/2025/v2025.12/D038

# Avec config YAML explicite
./scripts/build-garmin-map.sh \
  --config pipeline/configs/ign-bdtopo/ign-bdtopo-sources-shp.yaml \
  --data-root pipeline/data/bdtopo/2025/v2025.12/D038 \
  --jobs 8

# Simulation
./scripts/build-garmin-map.sh --dry-run

# Reprise partielle
./scripts/build-garmin-map.sh --skip-existing --jobs 4

# Avec courbes de niveau (après download --with-contours)
./scripts/build-garmin-map.sh \
  --config pipeline/configs/ign-bdtopo/ign-bdtopo-sources-shp.yaml \
  --data-root pipeline/data/bdtopo/2025/v2025.12/D038 \
  --contours-root pipeline/data/courbes/D038
```

| Option | Description | Défaut |
|---|---|---|
| `--data-root <dir>` | Racine des données BDTOPO | `./pipeline/data/bdtopo` |
| `--contours-root <dir>` | Racine des courbes de niveau | `./pipeline/data/courbes` |
| `--config <file>` | Config YAML mpforge explicite | génération auto |
| `--rules <file>` | Fichier de règles YAML | auto-découverte |
| `--jobs <n>` | Parallélisation | `8` |
| `--output <dir>` | Répertoire de sortie | `./pipeline/output` |
| `--family-id <n>` | Family ID Garmin | `6324` |
| `--description <str>` | Description de la carte | `"BDTOPO Garmin"` |
| `--typ <file>` | Fichier TYP styles personnalisés | *(aucun)* |
| `--skip-existing` | Passer les tuiles déjà générées | `false` |
| `--dry-run` | Simuler sans exécuter | `false` |
| `-v`, `-vv` | Mode verbeux | off |

Structure de sortie :

```
./pipeline/output/
  tiles/               ← tuiles .mp générées par mpforge
  gmapsupp.img         ← carte Garmin finale
  mpforge-report.json  ← rapport mpforge (métriques, erreurs)
  imgforge-report.json ← rapport imgforge (métriques, routage)
```

---

## Scripts utilitaires

### check_environment.sh — Vérification de l'environnement

```bash
./scripts/check_environment.sh
```

Vérifie la présence et les versions de tous les outils requis (GCC, CMake, GDAL, Rust, Python, QGIS, etc.).

### test-static-build.sh — Validation du build statique mpforge

```bash
./scripts/test-static-build.sh <mpforge-linux-x64-static.tar.gz> [test-config.yaml]
```

Valide qu'une archive de build statique mpforge est correctement empaquetée (binaire, wrapper, proj.db).

### release.sh — Créer une release

```bash
./scripts/release.sh v0.1.0
```

Vérifie qu'on est sur `main`, qu'il n'y a pas de changements non commités, crée et push le tag.

### retag.sh — Forcer un tag existant

```bash
./scripts/retag.sh v0.1.0           # Retag HEAD
./scripts/retag.sh v0.1.0 abc123    # Retag un commit spécifique
```

Supprime le tag local et distant, re-crée le tag et push. Utile pour corriger un workflow CI qui a échoué.
