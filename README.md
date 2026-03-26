# MPForge

> On forge des cartes Garmin à partir de données SIG massives.

MPForge est composé de trois briques :

| Composant | Description | Documentation |
|-----------|-------------|---------------|
| **[ogr-polishmap](./ogr-polishmap/)** | Driver GDAL/OGR pour le format Polish Map (.mp) | [README](./ogr-polishmap/README.md), [Spec RST](./ogr-polishmap/doc/polishmap.rst) |
| **[mpforge-cli](./mpforge-cli/)** | CLI Rust pour générer des tuiles Polish Map depuis des sources SIG | [README](./mpforge-cli/README.md), [Exemples](./mpforge-cli/examples/) |
| **[imgforge-cli](./imgforge-cli/)** | CLI Rust pour compiler des fichiers Polish Map (.mp) en Garmin IMG (.img) | [README](./imgforge-cli/README.md) |

**Pipeline complet** : Données SIG → `mpforge-cli` (tuiles .mp) → `imgforge-cli` (fichiers .img) → GPS Garmin

---

## CI/CD : Woodpecker CI

Le projet utilise **Woodpecker CI** (plutôt que Forgejo Actions) pour sa légèreté, son intégration native Docker, et sa configuration YAML simple sans dépendance à un écosystème GitHub Actions.

Chaque outil a son propre pipeline CI avec des tags préfixés pour des cycles de release indépendants :

| Pipeline | Déclencheur | Description |
|----------|-------------|-------------|
| [`.woodpecker/mpforge-cli.yml`](./.woodpecker/mpforge-cli.yml) | Tag `mpforge-v*` | Build statique Linux x64 (GDAL + GEOS + PROJ + driver PolishMap intégrés) |
| [`.woodpecker/imgforge-cli.yml`](./.woodpecker/imgforge-cli.yml) | Tag `imgforge-v*` | Build standard Linux x64 (Pure Rust, zéro dépendance native) |

Les deux pipelines produisent automatiquement une release Forgejo avec binaire, checksums SHA-256 et metadata JSON.

### Configuration initiale Woodpecker

Pour activer le CI sur un nouveau dépôt :

1. Se connecter à [Woodpecker CI](https://forgejo.ci.allfabox.fr)
2. Activer le dépôt dans **Settings > Repositories**
3. Créer un secret `forgejo_token` dans **Settings > Secrets** (token API Forgejo avec droits `write:package`)
4. Le webhook Forgejo → Woodpecker est créé automatiquement

### Architecture du build statique

```
Tag mpforge-v* poussé --> Woodpecker CI déclenche mpforge-cli.yml
  Phase 1  : Installation dépendances (cmake, pkg-config, sqlite3)
  Phase 2  : Compilation PROJ 9.3.1 statique
  Phase 3  : Compilation GEOS 3.13.0 statique
  Phase 4  : Téléchargement GDAL 3.10.1
  Phase 5  : Intégration driver PolishMap dans l'arborescence GDAL
  Phase 6  : Configuration GDAL statique (avec PROJ + GEOS)
  Phase 7  : Compilation et installation GDAL
  Phase 8  : Configuration Rust (GDAL_STATIC=1, pkg-config)
  Phase 9  : Copie proj.db dans resources/
  Phase 10 : Compilation mpforge-cli (proj.db embarqué via include_bytes!)
  Phase 11 : Vérification (ldd, taille, test --version)
  Phase 12 : Package + checksums + upload release Forgejo

Tag imgforge-v* poussé --> Woodpecker CI déclenche imgforge-cli.yml
  Phase 1  : Installation dépendances (build-essential)
  Phase 2  : Compilation imgforge-cli (cargo build --release)
  Phase 3  : Vérification (ldd, taille, test --version)
  Phase 4  : Package + checksums + upload release Forgejo
```

Le binaire produit est **100% autonome** : aucune dépendance externe, `proj.db` embarqué dans le binaire.

> **Troubleshooting proj.db** : Si `proj_create_from_database: Cannot find proj.db` apparaît, c'est que PROJ ne trouve pas sa base de données. En production, ce problème est résolu par l'embarquement de `proj.db` directement dans le binaire (extraction automatique dans un tempdir au démarrage). En développement local, positionner `PROJ_DATA` vers le répertoire contenant `proj.db` (typiquement `/usr/share/proj`).

### Versioning automatique

La version affichée par `--version` est dérivée du tag Git via `build.rs` dans chaque crate. Les préfixes de tag (`mpforge-`, `imgforge-`) sont automatiquement strippés :

```
Sur un tag       : mpforge-cli v1.0.0    (tag mpforge-v1.0.0)
                   imgforge-cli v0.1.0   (tag imgforge-v0.1.0)
Entre deux tags  : mpforge-cli v1.0.0-3-g1a2b3c4
Dirty            : mpforge-cli v1.0.0-dirty
```

Fallbacks : `CI_COMMIT_TAG` (strip préfixe) > `git describe --tags` > `git rev-parse --short HEAD` > `CARGO_PKG_VERSION`.

### Créer une release

Les tags sont préfixés par le nom de l'outil pour permettre des cycles de release indépendants :

```bash
# 1. Vérifier que tout est propre
git status
git push

# 2. Release mpforge-cli (~15-20 min de build)
git tag -a mpforge-v1.0.0 -m "Release mpforge-cli v1.0.0"
git push origin mpforge-v1.0.0

# 3. Release imgforge-cli (~2-3 min de build)
git tag -a imgforge-v0.1.0 -m "Release imgforge-cli v0.1.0"
git push origin imgforge-v0.1.0

# 4. Surveiller le build
# https://forgejo.ci.allfabox.fr
```

### Remplacer un tag (re-déclencher un build)

```bash
# Méthode propre : supprimer puis recréer
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
# Supprimer aussi la release dans Forgejo UI si elle existe

git tag -a mpforge-v1.0.0 -m "Release mpforge-cli v1.0.0 (corrected)"
git push origin mpforge-v1.0.0
```

Ou plus simplement, créer un patch : `git tag -a mpforge-v1.0.1 -m "Fix for v1.0.0"`.

### Supprimer un tag

```bash
# Local + remote
git tag -d mpforge-v1.0.0
git push --delete origin mpforge-v1.0.0
```

Note : supprimer un tag ne supprime **pas** la release Forgejo. Il faut la supprimer manuellement via l'UI ou l'API.

### Référence rapide des commandes

| Action | Commande |
|--------|----------|
| Release mpforge-cli | `git tag -a mpforge-v1.0.0 -m "Release mpforge-cli v1.0.0"` |
| Release imgforge-cli | `git tag -a imgforge-v0.1.0 -m "Release imgforge-cli v0.1.0"` |
| Pousser tag | `git push origin mpforge-v1.0.0` |
| Lister tags par outil | `git tag -l 'mpforge-v*'` / `git tag -l 'imgforge-v*'` |
| Voir détails tag | `git show mpforge-v1.0.0` |
| Supprimer tag local | `git tag -d mpforge-v1.0.0` |
| Supprimer tag remote | `git push --delete origin mpforge-v1.0.0` |
| Fetch tags forcés | `git fetch --tags --force` |

### Semantic Versioning

```
vMAJOR.MINOR.PATCH

v0.1.0 -> v0.1.1  : Bug fix
v0.1.1 -> v0.2.0  : Nouvelle feature (compatible)
v0.2.0 -> v1.0.0  : Breaking change
```

---

## Environnement de développement

### Prérequis

| Composant | Requis pour |
|-----------|-------------|
| **Rust** (via rustup) | mpforge-cli |
| **GCC/Clang + CMake 3.20+** | ogr-polishmap (driver C++) |
| **GDAL 3.6+ dev** (3.10+ recommandé) | ogr-polishmap |
| **Python 3.10+ + PyQGIS** | Plugin QGIS (optionnel) |
| **Java 11+ + mkgmap** | Génération cartes Garmin (optionnel) |

> **Note** : Le CI compile GDAL 3.10.1 en statique. En développement local, GDAL 3.6+ suffit pour compiler le driver, mais les binaires de release utilisent 3.10.1.

### Installation rapide (Debian/Ubuntu)

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# C++ / GDAL
sudo apt install build-essential cmake pkg-config libgdal-dev

# QGIS + PyQGIS (optionnel)
sudo apt install qgis python3-qgis

# Java + mkgmap (optionnel)
sudo apt install openjdk-11-jre
```

### Variables d'environnement

Ajouter dans `~/.bashrc` ou `~/.zshrc` :

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
cd ogr-polishmap
cmake -B build -DCMAKE_BUILD_TYPE=Debug && cmake --build build

# mpforge-cli (nécessite GDAL installé)
cd mpforge-cli
cargo build --release

# imgforge-cli (Pure Rust, aucune dépendance système)
cd imgforge-cli
cargo build --release
```

---

## Structure du projet

```
mpforge/
├── mpforge-cli/              # CLI Rust — génération de tuiles Polish Map
│   ├── src/                  # Code source
│   ├── examples/             # Exemples de configuration YAML
│   └── resources/            # proj.db (embarqué dans le binaire)
│
├── imgforge-cli/             # CLI Rust — compilateur .mp → Garmin .img
│   ├── src/                  # Code source (parser, img writer)
│   └── tests/                # Tests d'intégration + fixtures
│
├── ogr-polishmap/            # Driver GDAL/OGR C++
│   ├── src/                  # Code source
│   ├── doc/                  # Spec format, compliance GDAL
│   ├── examples/             # Scripts Python d'exemple
│   └── test/                 # Tests et données de test
│
├── .woodpecker/              # Pipelines CI/CD
│   ├── mpforge-cli.yml       # Build statique (PROJ+GEOS+GDAL), tag mpforge-v*
│   └── imgforge-cli.yml      # Build standard (Pure Rust), tag imgforge-v*
│
└── docs/                     # Documentation projet (BMAD, specs format)
    ├── planning-artifacts/   # PRD, architecture, epics
    ├── implementation-artifacts/  # Stories, tech-specs
    ├── mp-file-syntax-description/  # Spec Polish Map (cGPSmapper)
    ├── brainstorming/        # Sessions de brainstorming
    └── samples/              # Fichiers d'exemple (MP, diagrammes)
```

---

## Liens utiles

| Ressource | URL |
|-----------|-----|
| Forgejo | https://forgejo.allfabox.fr |
| Woodpecker CI | https://forgejo.ci.allfabox.fr |
| Doc Woodpecker | https://woodpecker-ci.org/docs |
| Doc GDAL | https://gdal.org/ |
| SemVer | https://semver.org/ |
