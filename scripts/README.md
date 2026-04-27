# Pipeline BDTOPO → Garmin

> Scripts pour transformer les données IGN BD TOPO en carte Garmin (`gmapsupp.img`)

## Organisation

```
scripts/
├── download-data.sh        ← pipeline production (interface publique)
├── build-garmin-map.sh       ← pipeline production (interface publique)
├── check_environment.sh      ← validation des prérequis (interface publique)
├── generate-typ-reference.py ← génération doc styles TYP (interface publique)
├── README.md
│
├── typ/                      ← build et test des fichiers TYP
├── release/                  ← publication de versions (release-tool, retag)
├── ops/                      ← opérations infrastructure (prune S3)
├── dev/                      ← tests et validation développement
│
├── debug/                    ← inspection bas niveau IMG (usage ponctuel)
├── ci/                       ← golden tests tech-spec
├── common/, gpkg/, postgis/, shapefile/  ← exports SIG utilitaires
```

## Vue d'ensemble

```
download-data.sh          build-garmin-map.sh
       |                            |
       v                            v
  Télécharge les          mpforge → imgforge
  données BDTOPO          Shapefile → .mp → gmapsupp.img
  depuis l'IGN
```

Le pipeline se décompose en 2 étapes principales :

1. **Télécharger** les données BDTOPO (+ courbes de niveau optionnel) depuis la Géoplateforme IGN
2. **Construire** la carte Garmin (export `.mp` + compilation `.img`) en une seule commande

---

## Prérequis

### Outils requis

- **mpforge** — Générateur de tuiles Polish Map (`.mp`)
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

### Vérification de l'environnement

```bash
./scripts/check_environment.sh
```

Vérifie la présence et les versions de tous les outils requis (GCC, CMake, GDAL, Rust, Python, QGIS, etc.). À lancer en premier sur une nouvelle machine.

---

## Configuration

### Variables d'environnement

Le fichier `pipeline/.env.example` centralise toutes les variables du pipeline :

```bash
cp pipeline/.env.example pipeline/.env
nano pipeline/.env
source pipeline/.env
```

### Fichiers de configuration

| Fichier | Rôle |
|---|---|
| `pipeline/.env.example` | Template des variables d'environnement (`DATA_ROOT`, `CONTOURS_DATA_ROOT`, etc.) |
| `pipeline/configs/ign-bdtopo/departement/sources.yaml` | Sources (SHP BDTOPO, SHP courbes, GPKG OSM, SHP sentiers GR) pour un département métro (EPSG:2154) |
| `pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml` | Règles de mapping BDTOPO + courbes → types Garmin (métropole) |
| `pipeline/configs/ign-bdtopo/france-quadrant/{sources,garmin-rules}.yaml` | Variante quadrants Garmin (FRANCE-SE/SO/NE/NO) — EndLevel rabaissés pour taille IMG |
| `pipeline/configs/ign-bdtopo/outre-mer/garmin-rules.yaml` | Règles de mapping partagées par tous les DOM |
| `pipeline/configs/ign-bdtopo/outre-mer/<slug>/sources.yaml` | Sources DOM (la-guadeloupe, la-martinique, la-guyane, la-reunion, mayotte) — EPSG spécifique par territoire |

Convention de nommage : un dossier par scope (`departement/`, `france-quadrant/`, `outre-mer/<slug>/`) avec `sources.yaml` (inputs + grille + header) et éventuellement `garmin-rules.yaml` (mapping) côte à côte.

---

## Étape 1 : Téléchargement des données BDTOPO

Script : `scripts/download-data.sh`

```bash
# Département unique (Isère)
./scripts/download-data.sh --zones D038 --format SHP

# Région entière (Auvergne-Rhône-Alpes)
./scripts/download-data.sh --region ARA --format SHP

# Simulation sans téléchargement
./scripts/download-data.sh --zones D038 --dry-run
```

Les données sont organisées dans `pipeline/data/bdtopo/{YYYY}/v{YYYY.MM}/{DXXX}/`.

### Cibler un millésime particulier

Par défaut le script prend l'édition la plus récente disponible via l'API Géoplateforme. Trois options permettent de cibler une version antérieure :

```bash
# Lister les millésimes disponibles pour une zone (ne télécharge rien)
./scripts/download-data.sh --zones D038 --list-editions

# Résolution mensuelle via API (dernière édition publiée en septembre 2025)
./scripts/download-data.sh --zones D038 --bdtopo-version v2025.09

# Date d'édition exacte (format IGN YYYY-MM-DD)
./scripts/download-data.sh --zones D038 --date 2025-09-15
```

| Option | Effet |
|---|---|
| `--list-editions` | Liste les millésimes disponibles par zone puis quitte |
| `--bdtopo-version vYYYY.MM` | Résout via API vers la dernière édition publiée ce mois-là |
| `--date YYYY-MM-DD` | Force une date d'édition précise (bypass API pour la résolution) |

> **Note** : `--bdtopo-version` et `--date` sont mutuellement exclusifs. `--list-editions` est non destructif — idéal pour préparer une reprise de build sur un millésime figé.

### Courbes de niveau

Le produit IGN "Courbes de niveau" est livré séparément de la BD TOPO, par département. L'option `--with-contours` télécharge les courbes en parallèle de la BDTOPO.

```bash
# BDTOPO + courbes de niveau pour l'Isère
./scripts/download-data.sh --zones D038 --with-contours

# BDTOPO région ARA + courbes de niveau des 12 départements ARA
./scripts/download-data.sh --region ARA --with-contours

# Multi-départements
./scripts/download-data.sh --zones D038,D073,D074 --with-contours

# Simulation
./scripts/download-data.sh --zones D038 --with-contours --dry-run

# Répertoire de stockage personnalisé
./scripts/download-data.sh --zones D038 --with-contours --contours-root /data/courbes
```

Les courbes sont téléchargées dans `pipeline/data/courbes/{DXXX}/` (arborescence séparée de la BDTOPO). Chaque département contient plusieurs dalles SHP (`COURBE_0840_6440.shp`, `COURBE_0880_6440.shp`, etc.).

| Option | Description | Défaut |
|---|---|---|
| `--with-contours` | Télécharger aussi les courbes de niveau | `false` |
| `--contours-root <dir>` | Racine des données courbes | `./pipeline/data/courbes` |

> **Note** : `--with-contours` nécessite `--zones` ou `--region`. Sans zone, une erreur explicite est affichée.

---

## Étape 2 : Construction de la carte Garmin

Script : `scripts/build-garmin-map.sh`

Enchaîne automatiquement l'export des tuiles `.mp` (via mpforge) et la compilation `gmapsupp.img` (via imgforge), avec auto-découverte des binaires et génération dynamique de la config.

```bash
# Auto-découverte de tout
./scripts/build-garmin-map.sh

# Département spécifique
./scripts/build-garmin-map.sh --data-root pipeline/data/bdtopo/2025/v2025.12/D038

# Avec config YAML explicite
./scripts/build-garmin-map.sh \
  --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
  --data-root pipeline/data/bdtopo/2025/v2025.12/D038 \
  --jobs 8

# Simulation
./scripts/build-garmin-map.sh --dry-run

# Reprise partielle
./scripts/build-garmin-map.sh --skip-existing --jobs 4

# Avec courbes de niveau (après download --with-contours)
./scripts/build-garmin-map.sh \
  --config pipeline/configs/ign-bdtopo/departement/sources.yaml \
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
| `--disable-profiles` | Bypasse le catalogue externe `generalize_profiles_path` (les `generalize:` inline restent actifs). Accepte aussi l'env var `MPFORGE_PROFILES=off`. | — |
| `--gdal-driver-path <dir>` | Override `GDAL_DRIVER_PATH` pour charger un driver `ogr-polishmap` frais. Auto-résolu sur `~/.gdal/plugins/` puis `tools/ogr-polishmap/build/` si vide. | auto |
| `--dry-run` | Simuler sans exécuter | `false` |
| `-v`, `-vv` | Mode verbeux | off |

!!! note "Pré-requis driver"
    Le writer `.mp` multi-Data nécessite un driver `ogr-polishmap` à jour. Le script le résout automatiquement ; sinon rebuild : `cmake --build tools/ogr-polishmap/build --target ogr_PolishMap` puis copie vers `~/.gdal/plugins/ogr_PolishMap.so` (user) ou `/usr/lib/gdalplugins/` (système, nécessite sudo).

Structure de sortie :

```
./pipeline/output/
  tiles/               ← tuiles .mp générées par mpforge
  gmapsupp.img         ← carte Garmin finale
  mpforge-report.json  ← rapport mpforge (métriques, erreurs)
  imgforge-report.json ← rapport imgforge (métriques, routage)
```

---

## Génération de la référence visuelle des styles TYP

Script : `scripts/generate-typ-reference.py`

Parse un fichier TYP texte (décompilé depuis un `.typ` binaire avec TYPViewer) et génère une page Markdown listant tous les styles (polygon, line, point) avec un rendu SVG inline pour chacun.

```bash
# Valeurs par défaut : pipeline/resources/typfiles/I2023100.txt → site/docs/reference/styles-typ.md
python3 scripts/generate-typ-reference.py

# Entrée / sortie explicites
python3 scripts/generate-typ-reference.py path/to/typfile.txt -o path/to/styles.md
```

| Argument | Description | Défaut |
|---|---|---|
| `input` (positionnel) | Fichier TYP texte en entrée | `pipeline/resources/typfiles/I2023100.txt` |
| `-o`, `--output` | Fichier Markdown généré en sortie | `site/docs/reference/styles-typ.md` |

> **Note encodage** : le fichier TYP texte est lu en `cp1252` (Windows-1252) — encodage natif produit par TYPViewer. Ne pas éditer ce fichier avec un outil supposant UTF-8, sous peine de corrompre les accents.
