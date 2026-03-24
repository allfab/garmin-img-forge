# MPForge

> On forge des cartes Garmin à partir de données SIG massives.

MPForge est composé de deux briques :

| Composant | Description | Documentation |
|-----------|-------------|---------------|
| **[ogr-polishmap](./ogr-polishmap/)** | Driver GDAL/OGR pour le format Polish Map (.mp) | [README](./ogr-polishmap/README.md), [Spec RST](./ogr-polishmap/doc/polishmap.rst) |
| **[mpforge-cli](./mpforge-cli/)** | CLI Rust pour générer des cartes Polish Map depuis des sources SIG | [README](./mpforge-cli/README.md), [Exemples](./mpforge-cli/examples/) |

---

## CI/CD : Woodpecker CI

Le projet utilise **Woodpecker CI** (plutôt que Forgejo Actions) pour sa légèreté, son intégration native Docker, et sa configuration YAML simple sans dépendance à un écosystème GitHub Actions.

- **Pipeline** : [`.woodpecker/multi-platform.yml`](./.woodpecker/multi-platform.yml)
- **Déclencheur** : Push d'un tag `v*`
- **Résultat** : Build statique Linux x64 (GDAL + GEOS + PROJ + driver PolishMap intégrés) + release Forgejo automatique

### Configuration initiale Woodpecker

Pour activer le CI sur un nouveau dépôt :

1. Se connecter à [Woodpecker CI](https://forgejo.ci.allfabox.fr)
2. Activer le dépôt dans **Settings > Repositories**
3. Créer un secret `forgejo_token` dans **Settings > Secrets** (token API Forgejo avec droits `write:package`)
4. Le webhook Forgejo → Woodpecker est créé automatiquement

### Architecture du build statique

```
Tag v* poussé --> Woodpecker CI déclenche multi-platform.yml
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
  Phase 12 : Package tar.gz + checksums + upload release Forgejo
```

Le binaire produit est **100% autonome** : aucune dépendance externe, `proj.db` embarqué dans le binaire.

> **Troubleshooting proj.db** : Si `proj_create_from_database: Cannot find proj.db` apparaît, c'est que PROJ ne trouve pas sa base de données. En production, ce problème est résolu par l'embarquement de `proj.db` directement dans le binaire (extraction automatique dans un tempdir au démarrage). En développement local, positionner `PROJ_DATA` vers le répertoire contenant `proj.db` (typiquement `/usr/share/proj`).

### Versioning automatique

La version affichée par `mpforge-cli --version` est dérivée du tag Git via `build.rs` :

```
Sur un tag       : mpforge-cli v0.2.0
Entre deux tags  : mpforge-cli v0.2.0-3-g1a2b3c4
Dirty            : mpforge-cli v0.2.0-dirty
```

Fallbacks : `git describe --tags` > `git rev-parse --short HEAD` > `CARGO_PKG_VERSION`.

### Créer une release

```bash
# 1. Vérifier que tout est propre
git status
git push

# 2. Créer un tag annoté (recommandé)
git tag -a v0.2.0 -m "Release v0.2.0

Features:
- Feature A
- Feature B

Bug fixes:
- Fix #123
"

# 3. Pousser le tag (déclenche le build)
git push origin v0.2.0

# 4. Surveiller le build (~15-20 min)
# https://forgejo.ci.allfabox.fr
```

### Remplacer un tag (re-déclencher un build)

```bash
# Méthode propre : supprimer puis recréer
git tag -d v0.2.0
git push --delete origin v0.2.0
# Supprimer aussi la release dans Forgejo UI si elle existe

git tag -a v0.2.0 -m "Release v0.2.0 (corrected)"
git push origin v0.2.0
```

Ou plus simplement, créer un patch : `git tag -a v0.2.1 -m "Fix for v0.2.0"`.

### Supprimer un tag

```bash
# Local + remote
git tag -d v0.2.0
git push --delete origin v0.2.0
```

Note : supprimer un tag ne supprime **pas** la release Forgejo. Il faut la supprimer manuellement via l'UI ou l'API.

### Référence rapide des commandes

| Action | Commande |
|--------|----------|
| Créer tag annoté | `git tag -a v0.2.0 -m "Release v0.2.0"` |
| Pousser tag | `git push origin v0.2.0` |
| Lister tags | `git tag -l` |
| Voir détails tag | `git show v0.2.0` |
| Supprimer tag local | `git tag -d v0.2.0` |
| Supprimer tag remote | `git push --delete origin v0.2.0` |
| Forcer tag | `git tag -f v0.2.0 && git push --force origin v0.2.0` |
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

# mpforge-cli
cd mpforge-cli
cargo build --release
```

---

## Structure du projet

```
mpforge/
├── mpforge-cli/              # CLI Rust
│   ├── src/                  # Code source
│   ├── examples/             # Exemples de configuration
│   └── resources/            # proj.db (embarqué dans le binaire)
│
├── ogr-polishmap/            # Driver GDAL/OGR C++
│   ├── src/                  # Code source
│   ├── doc/                  # Spec format, compliance GDAL
│   ├── examples/             # Scripts Python d'exemple
│   └── test/                 # Tests et données de test
│
├── .woodpecker/              # Pipeline CI/CD
│   └── multi-platform.yml
│
└── docs/                     # Documentation projet (BMAD, specs format)
    ├── planning-artifacts/   # PRD, architecture, epics
    ├── implementation-artifacts/  # Stories, rétrospectives
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
