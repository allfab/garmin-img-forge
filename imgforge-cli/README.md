# imgforge-cli

> Compilateur Polish Map (.mp) vers Garmin IMG — Pure Rust, zéro dépendance native

**imgforge-cli** est un outil en ligne de commande qui compile les fichiers Polish Map (`.mp`) produits par `mpforge-cli` en fichiers binaires Garmin IMG (`.img`), prêts à être chargés sur un GPS Garmin.

## Caractéristiques principales

- **Pure Rust** : aucune dépendance native (pas de Java, pas de mkgmap)
- **Binaire autonome** : un seul exécutable, zéro configuration
- **Parsing complet** : POI, POLYLINE, POLYGON avec attributs de routage
- **Subfiles Garmin** : génération TRE (structure), RGN (géométrie), LBL (labels)
- **Labels accentués** : encodage correct des caractères français (accents, ligatures)
- **Shield codes** : support des cartouches de numéros de route
- **Multi-niveaux** : subdivision hiérarchique pour le zoom GPS
- **Encodage delta** : compression des coordonnées conforme au format Garmin

## Installation

### Option 1 : Binaire pré-compilé (recommandé)

**Linux x64** :
```bash
# Télécharger depuis les releases Forgejo
chmod +x imgforge-cli
sudo mv imgforge-cli /usr/local/bin/
imgforge-cli --version
```

Les releases sont disponibles sur [Forgejo](https://forgejo.allfabox.fr/allfab/mpforge/releases) avec tags `imgforge-v*`.

### Option 2 : Compilation depuis les sources

**Prérequis** : Rust 1.70+ ([rustup](https://rustup.rs/))

```bash
cd imgforge-cli
cargo build --release
```

L'exécutable sera disponible dans `target/release/imgforge-cli`.

## Utilisation

### Compiler un fichier .mp en .img

```bash
imgforge-cli compile map.mp -o map.img
```

### Options

```bash
imgforge-cli compile <INPUT> -o <OUTPUT> [-v...]
```

| Option | Description |
|--------|-------------|
| `<INPUT>` | Fichier Polish Map (.mp) source |
| `-o, --output <FILE>` | Fichier IMG de sortie |
| `-v, --verbose...` | Verbosité (`-v` INFO, `-vv` DEBUG, `-vvv` TRACE) |
| `-h, --help` | Afficher l'aide |
| `-V, --version` | Afficher la version |

### Exemples

```bash
# Compilation simple
imgforge-cli compile tuile_0_0.mp -o tuile_0_0.img

# Avec logs détaillés
imgforge-cli compile tuile_0_0.mp -o tuile_0_0.img -vv

# Pipeline complet : mpforge-cli → imgforge-cli
mpforge-cli build --config config.yaml --jobs 4
for mp in tiles/*.mp; do
  imgforge-cli compile "$mp" -o "${mp%.mp}.img"
done
```

## Format de sortie

Le fichier `.img` produit est un conteneur Garmin conforme, structuré en subfiles :

| Subfile | Rôle |
|---------|------|
| **TRE** | Structure de la carte (bounds, niveaux de zoom, subdivisions) |
| **RGN** | Données géométriques (points, polylignes, polygones encodés en delta) |
| **LBL** | Labels et noms de rues (encodage 6-bit avec déduplications) |

Le header IMG inclut un filesystem FAT-like avec répertoire d'entrées, signature XOR et description.

## Versioning automatique

La version affichée par `imgforge-cli --version` est dérivée du tag Git via `build.rs` :

```
Sur un tag       : imgforge-cli v0.1.0
Entre deux tags  : imgforge-cli v0.1.0-3-g1a2b3c4
Dirty            : imgforge-cli v0.1.0-dirty
```

Fallbacks : `CI_COMMIT_TAG` (strip préfixe `imgforge-`) > `git describe --tags` > `git rev-parse --short HEAD` > `CARGO_PKG_VERSION`.

## Développement

### Structure du projet

```
imgforge-cli/
├── src/
│   ├── main.rs          # Point d'entrée CLI
│   ├── lib.rs           # API publique (compile)
│   ├── cli.rs           # Définition des arguments CLI (clap)
│   ├── error.rs         # Types d'erreurs
│   ├── parser/
│   │   ├── mod.rs       # Parseur Polish Map (.mp)
│   │   └── mp_types.rs  # Types de features Garmin
│   └── img/
│       ├── mod.rs       # Module IMG
│       ├── header.rs    # Header IMG (512 bytes, signature, XOR)
│       ├── directory.rs # Entrées FAT-like (32 bytes chacune)
│       ├── filesystem.rs# Assemblage des subfiles
│       ├── tre.rs       # Subfile TRE (bounds, niveaux, subdivisions)
│       ├── rgn.rs       # Subfile RGN (encodage géométrique delta)
│       ├── lbl.rs       # Subfile LBL (labels, shield codes)
│       └── writer.rs    # Orchestration de l'écriture IMG
├── tests/
│   ├── integration_test.rs  # 44 tests d'intégration
│   └── fixtures/            # Fichiers .mp de test
├── build.rs             # Injection GIT_VERSION
└── Cargo.toml
```

### Tests

```bash
# Tous les tests (131 unitaires + 44 intégration)
cargo test

# Tests avec logs
cargo test -- --nocapture

# Test d'un module spécifique
cargo test img::lbl::tests
cargo test parser::tests
```

### Linting et formatage

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## CI/CD

Le pipeline Woodpecker CI est déclenché par les tags `imgforge-v*` :

- **Pipeline** : [`.woodpecker/imgforge-cli.yml`](../.woodpecker/imgforge-cli.yml)
- **Déclencheur** : Push d'un tag `imgforge-v*`
- **Build** : `rust:bookworm`, `cargo build --release`
- **Artifacts** : binaire strippé + checksums SHA-256 + metadata JSON
- **Release** : Upload automatique vers Forgejo

### Créer une release

```bash
git tag -a imgforge-v0.1.0 -m "Release imgforge-cli v0.1.0"
git push origin imgforge-v0.1.0
```

## Licence

Ce projet fait partie de **MPForge** et est distribué sous licence MIT. Voir le fichier [LICENSE](../LICENSE) à la racine du dépôt.
