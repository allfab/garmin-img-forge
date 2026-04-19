<h1 align="center">
  <br>
  Garmin IMG Forge
  <br>
</h1>

<h4 align="center">Du SIG au GPS en une ligne de commande.<br/>Des cartes Garmin <code>.img</code> de qualité professionnelle, pilotées par de simples fichiers <strong>YAML</strong>.</h4>

<p align="center">
  <a href="https://imgforge.garmin.allfabox.fr/" target="_blank"><img src="https://img.shields.io/badge/Site-imgforge.garmin.allfabox.fr-526CFE?style=for-the-badge&logoColor=white" /></a>
  <a href="https://www.rust-lang.org/" target="_blank"><img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" /></a>
  <a href="https://gdal.org/" target="_blank"><img src="https://img.shields.io/badge/GDAL-5CAE58?style=for-the-badge&logo=osgeo&logoColor=white" /></a>
  <a href="https://woodpecker-ci.org/" target="_blank"><img src="https://img.shields.io/badge/Woodpecker_CI-4CAF50?style=for-the-badge&logo=woodpeckerci&logoColor=white" /></a>
  <a href="./LICENSE" target="_blank"><img src="https://img.shields.io/badge/Licence-GPL_v3_%2F_MIT-blue?style=for-the-badge" /></a>
</p>

<p align="center">
  <a href="#pourquoi-garmin-img-forge-">Pourquoi</a> •
  <a href="#-démarrage-rapide--première-carte-en-5-minutes">Démarrage rapide</a> •
  <a href="#-mise-en-place-dun-pipeline-de-production">Pipeline de production</a> •
  <a href="#galerie">Galerie</a> •
  <a href="#-ressources">Ressources</a>
</p>

<p align="center">
  <img src="site/docs/assets/images/readme/hero-pipeline.svg" alt="Pipeline Garmin IMG Forge : SIG → mpforge → imgforge → GPS" width="100%"/>
</p>

> **Miroir public en lecture.** Ce dépôt GitHub est un clone miroir filtré d'un dépôt Forgejo interne. Les issues et pull requests GitHub sont bienvenues, mais mergées côté source (voir [CONTRIBUTING.md](./CONTRIBUTING.md)).

---

## Pourquoi Garmin IMG Forge ?

Vous disposez de données SIG vectorielles — BDTOPO, OpenStreetMap, shapefiles métier, couches cadastrales — et souhaitez les exploiter sur un GPS Garmin (Edge, eTrex, Oregon, GPSMAP) sans recourir à une chaîne d'outils propriétaires ni à des manipulations manuelles.

**Garmin IMG Forge** est une suite open source qui transforme vos sources SIG en cartes Garmin prêtes à déployer, au moyen de fichiers YAML déclaratifs.

| Atout                     | Description |
|---------------------------|-------------|
| **Approche déclarative**  | Un fichier YAML décrit la carte cible ; la chaîne se charge du reste. Versionnable, diffable, rejouable à l'identique. |
| **Déploiement simplifié** | `mpforge` embarque GDAL, PROJ et GEOS en liaison statique. `imgforge` est écrit en Rust pur. Un binaire unique, aucune dépendance système à l'exécution. |
| **Multi-zoom natif**      | Profils de simplification conditionnels par attribut (`CL_ADMIN`, `IMPORTANCE`…) — jusqu'à **10 géométries par feature** selon le niveau de zoom. Fonctionnalité absente de `mkgmap`. |
| **Prêt pour la production** | Pipeline CI/CD complet, releases reproductibles, métadonnées et checksums SHA-256 signés. |
| **Logiciel libre**        | Licences GPL v3 / MIT. Vos données, vos règles, votre infrastructure. |

### Les briques

| Outil                                             | Rôle                                                   | Langage         | Licence |
|---------------------------------------------------|--------------------------------------------------------|-----------------|---------|
| [`ogr-polishmap`](./tools/ogr-polishmap/)         | Driver GDAL/OGR pour le format Polish Map (`.mp`)      | C++             | MIT     |
| [`mpforge`](./tools/mpforge/)                     | SIG → tuiles `.mp` (règles YAML, multi-zoom)           | Rust + GDAL     | GPL v3  |
| [`imgforge`](./tools/imgforge/)                   | `.mp` → Garmin `.img` (remplace `cGPSmapper`)          | Rust pur        | GPL v3  |
| [`ogr-garminimg`](./tools/ogr-garminimg/) *(WIP)* | Driver GDAL/OGR de lecture pour les `.img` (diagnostic) | C++            | —       |

---

## 🚀 Démarrage rapide — première carte en 5 minutes

> **Objectif :** produire un `gmapsupp.img` à partir d'un exemple YAML fourni, puis le déployer sur un GPS Garmin.

### 1. Récupérer les binaires *(ou compiler depuis les sources — voir [pré-requis](#pré-requis-détaillés))*

```bash
# Binaires statiques Linux x64 publiés à chaque release
curl -LO https://github.com/allfab/garmin-img-forge/releases/latest/download/mpforge
curl -LO https://github.com/allfab/garmin-img-forge/releases/latest/download/imgforge
chmod +x mpforge imgforge
```

### 2. Sélectionner un exemple YAML livré avec le dépôt

```bash
git clone https://github.com/allfab/garmin-img-forge.git
cd garmin-img-forge/tools/mpforge/examples
ls *.yaml
# simple.yaml • simple-with-mapping.yaml • bdtopo.yaml • france-nord-bdtopo.yaml ...
```

### 3. Générer les tuiles `.mp`

Le répertoire de sortie est défini dans le YAML (`output.directory`) :

```bash
mpforge build --config simple.yaml
```

### 4. Compiler en `.img` pour le GPS

`imgforge build` prend en argument le répertoire contenant les `.mp` :

```bash
imgforge build tiles/ --output gmapsupp.img
```

### 5. Déployer sur le GPS

Connectez le GPS en USB, copiez `gmapsupp.img` dans le dossier `Garmin/` de la carte SD (ou de la mémoire interne), puis déconnectez. La carte est immédiatement disponible dans le gestionnaire de cartes Garmin.

<p align="center">
  <img src="site/docs/assets/images/readme/gps-preview.svg" alt="Carte chargée sur un GPS Garmin" width="80%"/>
</p>

---

## 🏭 Mise en place d'un pipeline de production

Pour passer d'un exemple de démonstration à une production régulière de cartes régionales ou thématiques, le dépôt fournit un **squelette de pipeline** prêt à personnaliser.

### Anatomie d'un pipeline

```
pipeline/
├── configs/
│   ├── ign-bdtopo/
│   │   ├── generalize-profiles.yaml   # profils de simplification multi-zoom (mutualisé)
│   │   ├── departement/               # un département BDTOPO
│   │   │   ├── sources.yaml           #   ├── couches + grille + header MP
│   │   │   └── garmin-rules.yaml      #   └── règles BDTOPO → types Garmin
│   │   ├── france-quadrant/           # idem, à l'échelle quadrant national
│   │   └── outre-mer/                 # idem, par territoire DROM
│   └── osm/
├── data/              # Sources SIG téléchargées (BDTOPO, extraits OSM…)
├── output/<année>/    # Tuiles .mp + gmapsupp.img versionnés
└── resources/         # Fichiers TYP, icônes, ressources partagées
```

La production s'articule autour de **trois fichiers YAML** par scope :

- **`sources.yaml`** — couches d'entrée (chemins SHP, SRS source/cible, dédoublonnage), grille de tuilage, en-tête Polish Map, et pointeur vers les règles de transformation
- **`garmin-rules.yaml`** — règles de mapping BDTOPO → types Garmin (Type, EndLevel, Label) par `source_layer`
- **`generalize-profiles.yaml`** — profils de simplification multi-niveaux (Douglas-Peucker, Chaikin) mutualisés

### Mise en place en 4 étapes

**1. Déclarer les couches et la grille** — `pipeline/configs/ign-bdtopo/mon-scope/sources.yaml` :

```yaml
version: 1
generalize_profiles_path: "../generalize-profiles.yaml"

grid:
  cell_size: 0.225        # ~25 km par tuile
  overlap: 0.005

inputs:
  - path: "${DATA_ROOT}/TRANSPORT/TRONCON_DE_ROUTE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    dedup_by_field: ID    # CLEABS IGN (tronqué à 10 chars par le format DBF)
  - path: "${DATA_ROOT}/HYDROGRAPHIE/TRONCON_HYDROGRAPHIQUE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    dedup_by_field: ID

output:
  directory: "${OUTPUT_DIR}/mp/"
  filename_pattern: "BDTOPO-{col:03}-{row:03}.mp"
  base_id: ${BASE_ID}     # IDs uniques par tuile (ex. 00380001…)

header:
  name: "BDTOPO-{col:03}-{row:03}"
  copyright: "© IGN BDTOPO 2026"
  levels: "5"
  level0: "24"
  routing: "Y"

rules: pipeline/configs/ign-bdtopo/mon-scope/garmin-rules.yaml
error_handling: "continue"
```

**2. Déclarer les règles de mapping Garmin** — `garmin-rules.yaml` :

```yaml
version: 1

rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match: { CL_ADMIN: "Autoroute" }
        set:
          Type: "0x01"
          EndLevel: "2"
          Label: "~[0x04]${NUMERO}"
      - match: { CL_ADMIN: "Départementale", NATURE: "!Rond-point" }
        set:
          Type: "0x05"
          EndLevel: "2"
          Label: "~[0x06]${NUMERO}"

  - name: "Hydrographie"
    source_layer: "TRONCON_HYDROGRAPHIQUE"
    rules:
      - match: { NATURE: "Cours d'eau naturel" }
        set:
          Type: "0x1F"
          EndLevel: "1"
          Label: "${NOM_ENTITE}"
```

> Références complètes et documentées :
> - [`pipeline/configs/ign-bdtopo/departement/sources.yaml`](./pipeline/configs/ign-bdtopo/departement/sources.yaml) — 297 lignes, toutes couches BDTOPO
> - [`pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml`](./pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml) — 3 442 lignes, toutes les règles de mapping
> - [`pipeline/configs/ign-bdtopo/generalize-profiles.yaml`](./pipeline/configs/ign-bdtopo/generalize-profiles.yaml) — profils de simplification

**3. Exécuter le pipeline en local** (variables d'environnement `DATA_ROOT`, `OUTPUT_DIR`, `BASE_ID`, `ZONES` attendues) :

```bash
export DATA_ROOT=./pipeline/data/BDTOPO
export OUTPUT_DIR=./pipeline/output/2026/v2026.04
export BASE_ID=00380000
export ZONES=D038

mpforge build --config pipeline/configs/ign-bdtopo/mon-scope/sources.yaml

imgforge build "$OUTPUT_DIR/mp/" --output "$OUTPUT_DIR/img/gmapsupp.img"
```

Pour une orchestration complète (téléchargement BDTOPO, validation, publication), voir [`scripts/build-garmin-map.sh`](./scripts/build-garmin-map.sh).

**3. Intégrer en CI** — le dépôt fournit des pipelines Woodpecker pour l'infrastructure interne ainsi que des workflows GitHub Actions pour le miroir public. Documentation complète dans **[CI-CD.md](./CI-CD.md)**.

**4. Publier** — les releases sont pilotées par tag (`mpforge-v*`, `imgforge-v*`) via `scripts/release-tool.sh`. Chaque release produit un binaire, un fichier `SHA256SUMS` et des métadonnées JSON, automatiquement republiés sur le miroir GitHub.

---

## Galerie

<p align="center">
  <img src="site/docs/assets/images/readme/yaml-to-map.svg" alt="Un YAML déclaratif devient une carte Garmin" width="100%"/>
</p>

---

## Pré-requis détaillés

Aucune dépendance n'est requise pour exécuter les binaires publiés. Les dépendances suivantes ne s'appliquent qu'à la compilation depuis les sources :

| Composant                                           | Requis pour                          |
|-----------------------------------------------------|--------------------------------------|
| **Rust** (via [rustup](https://rustup.rs/))         | `mpforge`, `imgforge`                |
| **GCC/Clang + CMake ≥ 3.20**                        | `ogr-polishmap` (driver C++)         |
| **GDAL ≥ 3.6** (3.10+ recommandé)                   | `ogr-polishmap`, développement `mpforge` |
| **Python 3.10+ + PyQGIS** *(optionnel)*             | Plugin QGIS                          |
| **Java 11+ + mkgmap** *(optionnel)*                 | Génération avancée / comparaison     |

<details>
<summary><strong>Installation des dépendances (Debian/Ubuntu) et variables d'environnement</strong></summary>

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && source $HOME/.cargo/env

# C++ / GDAL
sudo apt install build-essential cmake pkg-config libgdal-dev

# Optionnel : QGIS, mkgmap
sudo apt install qgis python3-qgis openjdk-11-jre
```

```bash
# ~/.bashrc ou ~/.zshrc
export GDAL_DATA=/usr/share/gdal
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
export GDAL_HOME=/usr
export RUST_BACKTRACE=1
export RUST_LOG=info
mkdir -p ~/.gdal/plugins
```

```bash
# Build
cd tools/ogr-polishmap && cmake -B build -DCMAKE_BUILD_TYPE=Debug && cmake --build build
cd tools/mpforge       && cargo build --release
cd tools/imgforge      && cargo build --release
```

</details>

---

## 🔗 Ressources

| Ressource                          | URL |
|------------------------------------|-----|
| Site et documentation complète     | [imgforge.garmin.allfabox.fr](https://imgforge.garmin.allfabox.fr) |
| Cartes Garmin téléchargeables      | [download-maps.garmin.allfabox.fr](https://download-maps.garmin.allfabox.fr/) |
| Releases (binaires)                | [github.com/allfab/garmin-img-forge/releases](https://github.com/allfab/garmin-img-forge/releases) |
| CI/CD, tags, procédures de release | [CI-CD.md](./CI-CD.md) |
| Contribuer                         | [CONTRIBUTING.md](./CONTRIBUTING.md) |

### Structure du dépôt

```
garmin-img-forge/
├── tools/          # Code source (ogr-polishmap, mpforge, imgforge, ogr-garminimg)
├── pipeline/       # Squelette de production (configs YAML, data, output)
├── scripts/        # Orchestration — voir scripts/README.md
├── site/           # Sources du site Zensical
├── docs/           # Documentation projet (specs, planning, images README)
├── .woodpecker/    # CI Woodpecker (interne, non miroiré sur GitHub)
└── .github/        # Workflows et templates GitHub (miroir public)
```

---

## Crédits

Ce projet s'appuie sur des technologies éprouvées de l'écosystème open source :

- **[GDAL](https://gdal.org/)** — bibliothèque de traitement géospatial
- **[cGPSmapper](https://www.cgpsmapper.com/)** — spécification historique du format Polish Map
- **[mkgmap](https://www.mkgmap.org.uk/)** — inspiration et référence pour la génération de cartes Garmin
- **[IGN](https://www.ign.fr/)** — BDTOPO sous licence ouverte
- **[OpenStreetMap](https://www.openstreetmap.org/)** — données cartographiques libres
- **[Rust](https://www.rust-lang.org/) · [Zensical](https://zensical.org/) · [Woodpecker](https://woodpecker-ci.org/)** — stack moderne, sobre et auto-hébergeable

---

## Licences

- `ogr-polishmap` : **MIT**
- `mpforge`, `imgforge` : **GPL v3**
- Documentation du site : **[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/deed.fr)**

Voir [`LICENSE`](./LICENSE) pour le détail.
