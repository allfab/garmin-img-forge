<h1 align="center">
  <br>
  Garmin IMG Forge
  <br>
</h1>

<h4 align="center">Chaîne d'outils open source pour transformer des données SIG vectorielles en cartes Garmin (<code>.img</code>) téléchargeables sur GPS.<br />Configuration déclarative YAML — aucune étape manuelle, du SIG au terrain.</h4>

<p align="center">
  <a href="https://maps.garmin.allfabox.fr/" target="_blank"><img src="https://img.shields.io/badge/Site-maps.garmin.allfabox.fr-526CFE?style=for-the-badge&logoColor=white" /></a>
  <a href="https://www.rust-lang.org/" target="_blank"><img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" /></a>
  <a href="https://gdal.org/" target="_blank"><img src="https://img.shields.io/badge/GDAL-5CAE58?style=for-the-badge&logo=osgeo&logoColor=white" /></a>
  <a href="https://woodpecker-ci.org/" target="_blank"><img src="https://img.shields.io/badge/Woodpecker_CI-4CAF50?style=for-the-badge&logo=woodpeckerci&logoColor=white" /></a>
  <a href="./LICENSE" target="_blank"><img src="https://img.shields.io/badge/Licence-GPL_v3_%2F_MIT-blue?style=for-the-badge" /></a>
</p>

<p align="center">
  <a href="#pourquoi-ce-dépôt-">Pourquoi ce dépôt ?</a> •
  <a href="#site-de-documentation">Site</a> •
  <a href="#pré-requis">Pré-requis</a> •
  <a href="#démarrage-rapide">Démarrage rapide</a> •
  <a href="#cicd--woodpecker--github-actions">CI/CD</a> •
  <a href="#structure-du-projet">Structure</a> •
  <a href="#crédits">Crédits</a> •
  <a href="#licences">Licences</a>
</p>

> 🪞 **Miroir public en lecture.** Ce dépôt GitHub est un clone miroir
> filtré d'un dépôt hébergé sur une instance Forgejo de développement
> locale. Les issues et PR GitHub sont bienvenues mais mergées côté
> source (voir [CONTRIBUTING.md](./CONTRIBUTING.md)).
>
> La CI métier (`mpforge`, `imgforge`, génération des cartes) tourne sur
> un Woodpecker interne. GitHub Actions est strictement limité au build
> du site Pages et à la republication des binaires de release.

---

# Pourquoi ce dépôt ?

**Garmin IMG Forge** est une chaîne d'outils pour transformer des jeux de données SIG vectoriels (IGN BDTOPO, OpenStreetMap, cadastre, couches métier…) en cartes téléchargeables sur GPS Garmin, via des **fichiers de configuration YAML déclaratifs**. Aucun clic, aucune usine à gaz : vous décrivez vos règles de symbologie et de découpage, l'outil forge les `.img`.

Le dépôt contient **trois briques principales** plus une **brique expérimentale** :

### `ogr-polishmap` — Driver GDAL/OGR pour le format Polish Map (.mp)

Driver C++ qui permet à GDAL et à toute la chaîne QGIS/Python/Rust d'écrire nativement le format Polish Map. C'est la fondation : sans lui, pas de tuilage SIG standardisé.

- Code : [`tools/ogr-polishmap/`](./tools/ogr-polishmap/)
- Documentation : [README](./tools/ogr-polishmap/README.md), [Spec RST](./tools/ogr-polishmap/doc/polishmap.rst)
- Licence : MIT

### `mpforge` — Générateur de tuiles Polish Map depuis sources SIG

CLI Rust qui lit des sources SIG (BDTOPO, OSM…), applique des règles de transformation déclarées en YAML, et produit des tuiles `.mp` prêtes à être compilées. Embarque GDAL + PROJ + GEOS en statique : **un seul binaire, zéro dépendance système** à l'exécution.

- Code : [`tools/mpforge/`](./tools/mpforge/)
- Documentation : [README](./tools/mpforge/README.md), [Exemples YAML](./tools/mpforge/examples/)
- Licence : GPL v3

### `imgforge` — Compilateur Polish Map (.mp) vers Garmin IMG (.img)

CLI Rust **Pure Rust** (zéro dépendance native) qui remplace `cGPSmapper` dans la pipeline. Lit les `.mp` produits par `mpforge`, écrit un `.img` exploitable directement sur les GPS Garmin.

- Code : [`tools/imgforge/`](./tools/imgforge/)
- Documentation : [README](./tools/imgforge/README.md)
- Licence : GPL v3

### `ogr-garminimg` — Driver GDAL/OGR de lecture pour Garmin IMG *(en développement)*

Driver GDAL/OGR pour **lire** le format Garmin IMG (inverse d'`imgforge`). Objectif : diagnostic, comparaison de cartes, extraction vectorielle depuis des IMG existants. *Développement en cours, pas encore stable.*

- Code : [`tools/ogr-garminimg/`](./tools/ogr-garminimg/)

**Pipeline complet** :

```
Données SIG  ──►  mpforge  ──►  tuiles .mp  ──►  imgforge  ──►  fichier .img  ──►  GPS Garmin
 (BDTOPO,          (Rust +      (Polish Map     (Pure Rust)    (prêt à copier     (Edge / eTrex /
  OSM, …)           GDAL)        standard)                      sur la clé)        Oregon / GPSMAP…)
```

---

# Site de documentation

Le site **[maps.garmin.allfabox.fr](https://maps.garmin.allfabox.fr)** documente le projet et met à disposition les cartes Garmin téléchargeables.

- Généré avec **[Zensical](https://zensical.org/)** (successeur de MkDocs Material)
- Déployé via **GitHub Pages** (miroir public) + **Woodpecker** sur l'infra LXC interne en parallèle
- Les fichiers IMG sont servis depuis [`download-maps.garmin.allfabox.fr`](https://download-maps.garmin.allfabox.fr/) (S3 Garage sur Scaleway) derrière le CDN **Cloudflare** pour absorber la bande passante publique

Sources : [`site/`](./site/) — configuration `site/zensical.toml`, contenu `site/docs/`

```bash
# Build local (nécessite zensical installé)
pip install zensical
cd site && zensical build
```

---

# Pré-requis

Vous êtes **débutant** ? Lisez juste « Démarrage rapide » plus bas, c'est suffisant pour compiler et utiliser les outils.

Vous êtes **confirmé** ? Voici la liste complète :

| Composant                           | Requis pour                         |
|-------------------------------------|-------------------------------------|
| **Rust** (via [rustup](https://rustup.rs/)) | `mpforge`, `imgforge`       |
| **GCC/Clang + CMake ≥ 3.20**        | `ogr-polishmap` (driver C++)        |
| **GDAL ≥ 3.6** (3.10+ recommandé)   | `ogr-polishmap`, dev local `mpforge` |
| **Python 3.10+ + PyQGIS** *(option)* | Plugin QGIS                         |
| **Java 11+ + mkgmap** *(option)*    | Génération cartes Garmin avancées   |

> **Note** : Le CI compile GDAL 3.10.1 en statique pour les binaires de release. En dev local, GDAL 3.6+ suffit.

---

# Démarrage rapide

### Installation des dépendances (Debian/Ubuntu)

```bash
# Rust (pour mpforge et imgforge)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# C++ / GDAL (pour ogr-polishmap)
sudo apt install build-essential cmake pkg-config libgdal-dev

# QGIS + PyQGIS (optionnel)
sudo apt install qgis python3-qgis

# Java + mkgmap (optionnel)
sudo apt install openjdk-11-jre
```

### Variables d'environnement

À ajouter dans `~/.bashrc` ou `~/.zshrc` :

```bash
# GDAL
export GDAL_DATA=/usr/share/gdal
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
export GDAL_HOME=/usr

# Rust
export RUST_BACKTRACE=1
export RUST_LOG=info

# PyQGIS (si utilisé)
export PYTHONPATH=/usr/share/qgis/python:$PYTHONPATH
export QGIS_PREFIX_PATH=/usr
```

```bash
# Créer le répertoire plugins GDAL
mkdir -p ~/.gdal/plugins
```

### Build des composants

```bash
# ogr-polishmap (driver GDAL)
cd tools/ogr-polishmap
cmake -B build -DCMAKE_BUILD_TYPE=Debug && cmake --build build

# mpforge (nécessite GDAL installé)
cd tools/mpforge
cargo build --release

# imgforge (Pure Rust, aucune dépendance système)
cd tools/imgforge
cargo build --release
```

### Ou télécharger les binaires

Pas envie de compiler ? Les binaires statiques Linux x64 sont publiés à chaque release sur [`github.com/allfab/garmin-img-forge/releases`](https://github.com/allfab/garmin-img-forge/releases) (republication automatique depuis le dépôt source).

Chaque release inclut : binaire Linux x64 statique + `SHA256SUMS` + métadonnées JSON.

---

# CI/CD — Woodpecker + GitHub Actions

Le projet utilise **Woodpecker CI** comme plateforme principale (légère, intégration Docker native, YAML simple) sur l'infra interne. GitHub Actions joue un rôle d'appoint sur le miroir public.

> **Note miroir GitHub** : les fichiers `.woodpecker/*.yml` ne sont pas miroirés côté GitHub (dossier exclu du filtrage `git filter-repo`). Les descriptions ci-dessous documentent le système à titre informatif.

### Pipelines Woodpecker (canoniques)

| Pipeline                        | Déclencheur                        | Description |
|---------------------------------|------------------------------------|-------------|
| `.woodpecker/mpforge.yml`       | Tag `mpforge-v*`                   | Build statique Linux x64 (GDAL + GEOS + PROJ + driver PolishMap intégrés) |
| `.woodpecker/imgforge.yml`      | Tag `imgforge-v*`                  | Build standard Linux x64 (Pure Rust, zéro dépendance native) |
| `.woodpecker/site.yml`          | Push sur `main` (dans `site/`)     | Build et déploiement LXC du site Zensical |
| `.woodpecker/mirror-github.yml` | Push sur `main`                    | Miroir filtré Forgejo → GitHub (`git filter-repo`) |

Les pipelines `mpforge` et `imgforge` produisent automatiquement une **release Forgejo** avec binaire, checksums SHA-256 et métadonnées JSON.

### Workflows GitHub Actions (appoint, côté miroir)

| Workflow                                   | Déclencheur             | Description |
|--------------------------------------------|-------------------------|-------------|
| `.github/workflows/pages.yml`              | Push sur `main`         | Build Zensical + déploiement GitHub Pages |
| `.github/workflows/release-republish.yml`  | Tag `mpforge-v*` / `imgforge-v*` | Téléchargement des binaires depuis la release Forgejo et republication en release GitHub |

Le workflow `release-republish` attend que Forgejo ait fini son build (poll API toutes les 2 min, timeout 25 min), télécharge les assets, vérifie les SHA-256, puis crée la release GitHub équivalente. **Aucune recompilation** côté GitHub — le build serveur (~20 min pour `mpforge` avec GDAL statique) n'est exécuté qu'une seule fois, sur l'infra interne.

### Architecture du build statique `mpforge`

```
Tag mpforge-v* poussé --> Woodpecker CI déclenche mpforge.yml
  Phase 1  : Installation dépendances (cmake, pkg-config, sqlite3)
  Phase 2  : Compilation PROJ 9.3.1 statique
  Phase 3  : Compilation GEOS 3.13.0 statique
  Phase 4  : Téléchargement GDAL 3.10.1
  Phase 5  : Intégration driver PolishMap dans l'arborescence GDAL
  Phase 6  : Configuration GDAL statique (avec PROJ + GEOS)
  Phase 7  : Compilation et installation GDAL
  Phase 8  : Configuration Rust (GDAL_STATIC=1, pkg-config)
  Phase 9  : Copie proj.db dans resources/
  Phase 10 : Compilation mpforge (proj.db embarqué via include_bytes!)
  Phase 11 : Vérification (ldd, taille, test --version)
  Phase 12 : Package + checksums + upload release Forgejo
```

Le binaire produit est **100% autonome** : aucune dépendance externe, `proj.db` embarqué.

> **Troubleshooting `proj.db`** : Si `proj_create_from_database: Cannot find proj.db` apparaît, c'est que PROJ ne trouve pas sa base de données. En production, ce problème est résolu par l'embarquement de `proj.db` directement dans le binaire (extraction automatique dans un tempdir au démarrage). En développement local, positionner `PROJ_DATA` vers le répertoire contenant `proj.db` (typiquement `/usr/share/proj`).

### Configuration initiale Woodpecker

Pour activer le CI sur un nouveau dépôt :

1. Se connecter à l'instance Woodpecker interne
2. Activer le dépôt dans **Settings > Repositories**
3. Créer un secret `forgejo_token` dans **Settings > Secrets** (token API Forgejo avec droits `write:package`)
4. Le webhook Forgejo → Woodpecker est créé automatiquement

### Versioning automatique

La version affichée par `--version` est dérivée du tag Git via `build.rs` dans chaque crate. Les préfixes de tag (`mpforge-`, `imgforge-`) sont automatiquement strippés :

```
Sur un tag       : mpforge v1.0.0    (tag mpforge-v1.0.0)
                   imgforge v0.1.0   (tag imgforge-v0.1.0)
Entre deux tags  : mpforge v1.0.0-3-g1a2b3c4
Dirty            : mpforge v1.0.0-dirty
```

Fallbacks : `CI_COMMIT_TAG` (strip préfixe) > `git describe --tags` > `git rev-parse --short HEAD` > `CARGO_PKG_VERSION`.

### Créer une release

Les tags sont préfixés par le nom de l'outil pour permettre des cycles de release indépendants :

```bash
# 1. Vérifier que tout est propre
git status
git push

# 2. Release mpforge (~15-20 min de build)
git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0"
git push origin mpforge-v1.0.0

# 3. Release imgforge (~2-3 min de build)
git tag -a imgforge-v0.1.0 -m "Release imgforge v0.1.0"
git push origin imgforge-v0.1.0

# 4. Surveiller le build sur l'instance Woodpecker interne
```

### Remplacer un tag (re-déclencher un build)

```bash
# Méthode propre : supprimer puis recréer
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
# Supprimer aussi la release dans Forgejo UI si elle existe

git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0 (corrected)"
git push origin mpforge-v1.0.0
```

Ou plus simplement, créer un patch : `git tag -a mpforge-v1.0.1 -m "Fix for v1.0.0"`.

### Supprimer un tag

```bash
# Local + remote
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
```

> ⚠ Supprimer un tag ne supprime **pas** la release Forgejo ni la release GitHub. Il faut les supprimer manuellement via l'UI ou l'API.

### Référence rapide des commandes

| Action                | Commande                                                       |
|-----------------------|----------------------------------------------------------------|
| Release mpforge       | `git tag -a mpforge-v1.0.0 -m "Release mpforge v1.0.0"`        |
| Release imgforge      | `git tag -a imgforge-v0.1.0 -m "Release imgforge v0.1.0"`      |
| Pousser tag           | `git push origin mpforge-v1.0.0`                               |
| Lister tags par outil | `git tag -l 'mpforge-v*'` / `git tag -l 'imgforge-v*'`         |
| Voir détails tag      | `git show mpforge-v1.0.0`                                      |
| Supprimer tag local   | `git tag -d mpforge-v1.0.0`                                    |
| Supprimer tag remote  | `git push --delete origin mpforge-v1.0.0`                      |
| Fetch tags forcés     | `git fetch --tags --force`                                     |

### Semantic Versioning

```
vMAJOR.MINOR.PATCH

v0.1.0 -> v0.1.1  : Bug fix
v0.1.1 -> v0.2.0  : Nouvelle feature (compatible)
v0.2.0 -> v1.0.0  : Breaking change
```

---

# Structure du projet

```
garmin-img-forge/
├── tools/                        # CODE SOURCE DES OUTILS
│   ├── mpforge/                  # CLI Rust — génération de tuiles Polish Map
│   │   ├── src/                  # Code source
│   │   ├── examples/             # Exemples de configuration YAML
│   │   └── resources/            # proj.db (embarqué dans le binaire)
│   │
│   ├── imgforge/                 # CLI Rust — compilateur .mp → Garmin .img
│   │   ├── src/                  # Code source (parser, img writer)
│   │   └── tests/                # Tests d'intégration + fixtures
│   │
│   └── ogr-polishmap/            # Driver GDAL/OGR C++
│       ├── src/                  # Code source
│       ├── doc/                  # Spec format, compliance GDAL
│       ├── examples/             # Scripts Python d'exemple
│       └── test/                 # Tests et données de test
│
├── pipeline/                     # PRODUCTION DE CARTES (données, configs, sorties)
│   ├── configs/                  # Configuration YAML mpforge
│   ├── data/                     # Données BDTOPO téléchargées
│   ├── output/                   # Tuiles .mp et gmapsupp.img
│   └── resources/                # Typfiles et ressources production
│
├── scripts/                      # ORCHESTRATION (transversal) — doc: scripts/README.md
│
├── site/                         # SITE PUBLIC Zensical
│
├── .woodpecker/                  # Pipelines CI/CD (interne, non miroiré sur GitHub)
│   ├── mpforge.yml               # Build statique (PROJ+GEOS+GDAL), tag mpforge-v*
│   ├── imgforge.yml              # Build standard (Pure Rust), tag imgforge-v*
│   ├── site.yml                  # Build et déploiement LXC du site
│   └── mirror-github.yml         # Miroir filtré Forgejo → GitHub (push main)
│
├── .github/                      # Workflows et templates GitHub (miroir public)
│   ├── workflows/
│   │   ├── pages.yml             # Build Zensical + deploy GitHub Pages
│   │   └── release-republish.yml # Republish des binaires Forgejo sur releases GitHub
│   ├── ISSUE_TEMPLATE/           # Templates issue (bug, enhancement)
│   └── PULL_REQUEST_TEMPLATE.md  # Template PR (checklist CONTRIBUTING.md)
│
└── docs/                         # Documentation projet (BMAD, specs format)
    ├── planning-artifacts/       # PRD, architecture, epics
    ├── implementation-artifacts/ # Stories, tech-specs
    ├── mp-file-syntax-description/ # Spec Polish Map (cGPSmapper)
    ├── brainstorming/            # Sessions de brainstorming
    └── samples/                  # Fichiers d'exemple (MP, diagrammes)
```

---

# Liens utiles

| Ressource         | URL                                      |
|-------------------|------------------------------------------|
| Doc Woodpecker    | https://woodpecker-ci.org/docs           |
| Doc GDAL          | https://gdal.org/                        |
| Doc Zensical      | https://zensical.org/                    |
| SemVer            | https://semver.org/                      |

---

# Crédits

Ce projet s'appuie sur des géants :

- **[GDAL](https://gdal.org/)** — la bibliothèque qui rend tout ça possible
- **[cGPSmapper](https://www.cgpsmapper.com/)** — pour la spec historique du format Polish Map
- **[mkgmap](https://www.mkgmap.org.uk/)** — inspiration et référence pour la génération de cartes Garmin
- **[IGN](https://www.ign.fr/)** — pour la BDTOPO en licence ouverte
- **[OpenStreetMap](https://www.openstreetmap.org/)** — pour les données cartographiques mondiales libres
- **[Rust](https://www.rust-lang.org/) + [Zensical](https://zensical.org/) + [Woodpecker](https://woodpecker-ci.org/)** — pour une stack moderne, sobre et auto-hébergeable

---

# Licences

- **`ogr-polishmap`** : MIT
- **`mpforge`**, **`imgforge`** : GPL v3
- **Documentation du site** : [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/deed.fr)

Voir [`LICENSE`](./LICENSE) pour les détails.
