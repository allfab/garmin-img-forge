# imgforge

> Compilateur Garmin IMG pour convertir des fichiers Polish Map (.mp) en cartes Garmin (.img / gmapsupp.img)

**imgforge** est un outil en ligne de commande Rust qui compile vos fichiers Polish Map (.mp) en fichiers Garmin IMG, directement exploitables sur les GPS Garmin. Il remplace `cgpsmapper` / `mkgmap` avec un binaire unique sans dépendance.

## Caractéristiques principales

- **Compilation single-tile** : Convertir un fichier `.mp` en `.img`
- **Build multi-tile** : Assembler un répertoire de `.mp` en `gmapsupp.img` prêt pour le GPS
- **Parallélisation** : Compilation multi-thread des tuiles via rayon
- **Encodage configurable** : Format 6 (ASCII), Format 9 (CP1252/CP1250/CP1251), Format 10 (UTF-8)
- **Optimisation géométrique** : Simplification Douglas-Peucker, filtrage polygones par taille, tri par aire
- **Contrôle routing** : NET+NOD complet, NET seul, ou désactivation du routing
- **Cartes overlay** : Support transparent avec priorité d'affichage configurable
- **Métadonnées complètes** : Copyright, pays, région, version produit dans le TDB
- **Résilience** : Mode `--keep-going` pour continuer malgré les tuiles en erreur
- **Symbologie TYP** : Intégration d'un fichier `.typ` personnalisé pour le rendu des cartes
- **Format Garmin complet** : Génération des sous-fichiers TRE, RGN, LBL, NET, NOD
- **Fichier TDB** : Génération automatique du fichier compagnon `.tdb`
- **Rapport JSON** : Statistiques de compilation en sortie structurée
- **Zéro dépendance** : Binaire autonome, pas besoin de GDAL ni de librairie externe

## Installation

### Option 1 : Binaire pré-compilé (recommandé)

**Linux x64** :
```bash
# Télécharger la release
wget https://forgejo.allfabox.fr/allfab/garmin-ign-bdtopo-map/releases/download/imgforge-v0.1.0/imgforge

# Rendre exécutable
chmod +x imgforge

# Installer
sudo mv imgforge /usr/local/bin/

# Tester
imgforge --version
```

### Option 2 : Compilation depuis les sources

#### Prérequis

- **Rust** : 1.70+ ([rustup](https://rustup.rs/))

Aucune dépendance système requise (pas de GDAL).

#### Compilation

```bash
cd tools/imgforge
cargo build --release
```

L'exécutable sera disponible dans `target/release/imgforge`.

#### Installation globale

```bash
cargo install --path .
```

## Quick Reference

### Vérifier la version installée

```bash
imgforge --version
# Output: imgforge v0.1.0

# Alternative : flag court
imgforge -V
```

### Afficher l'aide complète

```bash
# Aide globale (liste des commandes)
imgforge --help

# Aide spécifique à une commande
imgforge compile --help
imgforge build --help
```

## Utilisation

### Commande `compile` : Single tile

Compile un fichier `.mp` en un fichier `.img` Garmin :

```bash
# Compilation basique (sortie : input.img)
imgforge compile tile_0_0.mp

# Spécifier le fichier de sortie
imgforge compile tile_0_0.mp --output ma_carte.img

# Avec description personnalisée
imgforge compile tile_0_0.mp --description "BDTOPO Réunion"

# Forcer l'encodage CP1252 (Format 9) au lieu du codepage du .mp
imgforge compile tile_0_0.mp --latin1

# Carte overlay transparente avec priorité haute
imgforge compile overlay.mp --transparent --draw-priority 50

# Simplification géométrique pour réduire la taille
imgforge compile tile_0_0.mp --reduce-point-density 5.0 --min-size-polygon 20

# Désactiver le routing (pas de NET/NOD)
imgforge compile tile_0_0.mp --no-route

# Avec un fichier TYP personnalisé pour la symbologie
imgforge compile tile_0_0.mp --typ-file style.typ
```

### Commande `build` : Multi-tile gmapsupp

Compile tous les fichiers `.mp` d'un répertoire en un seul `gmapsupp.img` exploitable sur GPS Garmin :

```bash
# Compilation basique (sortie : gmapsupp.img)
imgforge build tiles/

# Spécifier le fichier de sortie
imgforge build tiles/ --output ma_carte.img

# Compilation parallèle
imgforge build tiles/ --jobs 8

# Avec métadonnées complètes
imgforge build tiles/ \
  --family-id 1234 \
  --product-id 1 \
  --series-name "BDTOPO France" \
  --family-name "IGN BDTOPO" \
  --area-name "France métropolitaine" \
  --country-name "France" \
  --country-abbr "FRA" \
  --copyright-message "IGN BDTOPO 2026" \
  --product-version 200

# Build robuste : continuer si une tuile échoue
imgforge build tiles/ --jobs 8 --keep-going

# Build optimisé : simplification + encodage UTF-8
imgforge build tiles/ --unicode --reduce-point-density 3.0 --min-size-polygon 8

# Build avec fichier TYP personnalisé pour la symbologie
imgforge build tiles/ --jobs 8 --typ-file bdtopo.typ
```

## Options CLI

### Options communes (`compile` et `build`)

Les options suivantes sont disponibles sur les deux commandes. Elles sont appliquées à chaque tuile.

#### Générales

| Option | Description | Défaut |
|--------|-------------|--------|
| `-v, --verbose...` | Verbosité (`-v`, `-vv`, `-vvv`) | WARN |

#### Encodage

| Option | Description | Défaut |
|--------|-------------|--------|
| `--code-page <N>` | Codepage numérique (1252, 1250, 1251, 65001, 0...) | header .mp |
| `--unicode` | Raccourci pour `--code-page 65001` (UTF-8, Format 10) | - |
| `--latin1` | Raccourci pour `--code-page 1252` (CP1252, Format 9) | - |
| `--lower-case` | Autoriser les minuscules (force Format 9 ou 10 au lieu de Format 6) | `false` |

> `--unicode` et `--latin1` sont mutuellement exclusifs avec `--code-page`.

#### Rendu

| Option | Description | Défaut |
|--------|-------------|--------|
| `--transparent` | Carte transparente (overlay) — set le flag TRE | `false` |
| `--draw-priority <N>` | Priorité d'affichage (0-255) | `25` |
| `--levels <LEVELS>` | Niveaux de zoom : `"24,22,20,18,16"` ou `"0:24,1:22,2:20"` | header .mp |
| `--order-by-decreasing-area` | Trier les polygones par aire décroissante (plus grands rendus en premier) | `false` |

#### Optimisation géométrique

| Option | Description | Défaut |
|--------|-------------|--------|
| `--reduce-point-density <NUM>` | Seuil Douglas-Peucker pour simplification des lignes et polygones (en map units) | - |
| `--simplify-polygons <SPEC>` | DP par résolution : `"24:12,18:10,16:8"` (prioritaire sur `--reduce-point-density` pour les polygones) | - |
| `--min-size-polygon <NUM>` | Filtrer les polygones dont l'aire < NUM (en map units², mkgmap défaut: 8) | - |
| `--merge-lines` | Fusionner les polylignes adjacentes de même type/label *(réservé, non implémenté)* | `false` |

#### Contrôle routing

| Option | Description | Défaut |
|--------|-------------|--------|
| `--route` | Forcer la génération NET+NOD | auto-détection |
| `--net` | Générer NET seul (recherche d'adresse sans routing turn-by-turn) | - |
| `--no-route` | Désactiver le routing même si des roads sont présents | - |

> `--route`, `--net` et `--no-route` sont mutuellement exclusifs.

#### Copyright

| Option | Description | Défaut |
|--------|-------------|--------|
| `--copyright-message <TEXT>` | Message copyright (écrit dans TRE et TDB) | header .mp |

#### Symbologie

| Option | Description | Défaut |
|--------|-------------|--------|
| `--typ-file <FILE>` | Fichier `.typ` de symbologie personnalisée (couleurs, motifs, icônes) | - |

### Commande `compile`

```bash
imgforge compile [OPTIONS] <INPUT>
```

| Option | Description | Défaut |
|--------|-------------|--------|
| `<INPUT>` | Fichier `.mp` à compiler | **REQUIS** |
| `-o, --output <FILE>` | Fichier `.img` de sortie | `<input>.img` |
| `--description <TEXT>` | Description de la carte (override le header .mp) | - |

Plus toutes les [options communes](#options-communes-compile-et-build) ci-dessus.

### Commande `build`

```bash
imgforge build [OPTIONS] <INPUT>
```

#### Options spécifiques à `build`

| Option | Description | Défaut |
|--------|-------------|--------|
| `<INPUT>` | Répertoire contenant les fichiers `.mp` | **REQUIS** |
| `-o, --output <FILE>` | Fichier `gmapsupp.img` de sortie | `gmapsupp.img` |
| `-j, --jobs <N>` | Nombre de threads parallèles | `1` |
| `--keep-going` | Continuer si une tuile échoue (log warning, ignorer la tuile) | `false` |

#### Identité carte Garmin

| Option | Description | Défaut |
|--------|-------------|--------|
| `--family-id <ID>` | Family ID Garmin | `1` |
| `--product-id <ID>` | Product ID Garmin | `1` |
| `--series-name <NAME>` | Nom de la série de cartes | `imgforge` |
| `--family-name <NAME>` | Nom de la famille de cartes | `Map` |

#### Métadonnées TDB

| Option | Description | Défaut |
|--------|-------------|--------|
| `--mapname <NAME>` | Identifiant carte (8 chiffres) | header .mp |
| `--area-name <TEXT>` | Nom de la zone géographique | - |
| `--country-name <TEXT>` | Nom du pays | - |
| `--country-abbr <TEXT>` | Abréviation pays | - |
| `--region-name <TEXT>` | Nom de la région | - |
| `--region-abbr <TEXT>` | Abréviation région | - |
| `--product-version <N>` | Version produit (100 = v1.00) | `100` |

Plus toutes les [options communes](#options-communes-compile-et-build) ci-dessus.

### Niveaux de verbosité

| Flag | Niveau | Utilisation |
|------|--------|-------------|
| _(aucun)_ | WARN | Production |
| `-v` | INFO | Monitoring (étapes principales) |
| `-vv` | DEBUG | Troubleshooting (détails encodage) |
| `-vvv` | TRACE | Développement (verbosité maximale) |

### Parallélisation

La compilation multi-tile utilise **rayon** pour distribuer le traitement sur N workers. Chaque worker compile indépendamment un fichier `.mp` :

```bash
# Petit projet (<20 tuiles) : mode séquentiel (défaut)
imgforge build tiles/

# Projet moyen (20-100 tuiles)
imgforge build tiles/ --jobs 4

# Large projet (>100 tuiles)
imgforge build tiles/ --jobs 8
```

## Rapport JSON

La sortie standard d'imgforge est un rapport JSON structuré :

```json
{
  "tiles_compiled": 42,
  "total_points": 15234,
  "total_polylines": 8721,
  "total_polygons": 3456,
  "errors": [],
  "duration_ms": 2340,
  "output_file": "gmapsupp.img",
  "output_size_bytes": 52428800
}
```

### Intégration CI/CD

```bash
#!/bin/bash
# Pipeline : mpforge (tuilage) → imgforge (compilation)

# Étape 1 : Générer les tuiles .mp
mpforge build --config bdtopo.yaml --jobs 8

# Étape 2 : Compiler en gmapsupp.img
imgforge build tiles/ --output gmapsupp.img --jobs 8 \
  --family-name "BDTOPO France" \
  --series-name "IGN BDTOPO 2026" \
  --latin1 \
  --reduce-point-density 3.0 \
  --min-size-polygon 8 \
  --typ-file bdtopo.typ \
  --keep-going

# Vérifier le résultat
echo "Compilation terminée"
ls -lh gmapsupp.img
```

## Pipeline complet mpforge + imgforge

imgforge s'inscrit dans le pipeline de création de cartes Garmin :

```
Données SIG (Shapefile, GPKG, PostGIS)
    │
    ▼
 mpforge build     ← Tuilage spatial + export Polish Map
    │
    ▼
 tiles/*.mp        ← Fichiers Polish Map par tuile
    │
    ▼
 imgforge build    ← Compilation Garmin IMG
    │
    ▼
 gmapsupp.img      ← Carte Garmin prête pour le GPS
```

## Architecture Garmin IMG

imgforge génère les sous-fichiers standards du format Garmin IMG :

| Sous-fichier | Description |
|-------------|-------------|
| **TRE** | Table des régions (index spatial, niveaux de zoom) |
| **RGN** | Données régions (géométries : points, lignes, polygones) |
| **LBL** | Labels (noms, encodage Format 6/9/10 — ASCII, codepage, UTF-8) |
| **NET** | Réseau routier (topologie) |
| **NOD** | Noeuds de routage |
| **TYP** | Symbologie personnalisée (couleurs, motifs, icônes de points/lignes/polygones) |
| **TDB** | Table de description (métadonnées de la carte) |

## Développement

### Structure du projet

```
imgforge/
├── src/
│   ├── main.rs              # Point d'entrée CLI
│   ├── cli.rs               # Définition des arguments CLI (clap)
│   ├── lib.rs               # Exports publics
│   ├── error.rs             # Types d'erreurs
│   ├── report.rs            # Rapport JSON d'exécution
│   ├── img/
│   │   ├── writer.rs        # Génération IMG (orchestration)
│   │   ├── assembler.rs     # Assemblage gmapsupp multi-tile
│   │   ├── tre.rs           # Sous-fichier TRE
│   │   ├── rgn.rs           # Sous-fichier RGN
│   │   ├── lbl.rs           # Sous-fichier LBL
│   │   ├── net.rs           # Sous-fichier NET
│   │   ├── nod.rs           # Sous-fichier NOD
│   │   ├── tdb.rs           # Fichier compagnon TDB
│   │   ├── splitter.rs      # Découpage en subdivisions
│   │   ├── coord.rs         # Encodage coordonnées Garmin
│   │   ├── subdivision.rs   # Gestion des subdivisions
│   │   ├── zoom.rs          # Niveaux de zoom
│   │   ├── header.rs        # Header IMG
│   │   ├── common_header.rs # Header commun sous-fichiers
│   │   ├── directory.rs     # Répertoire FAT
│   │   ├── filesystem.rs    # Système de fichiers IMG
│   │   ├── bit_reader.rs    # Lecture bitstream
│   │   ├── bit_writer.rs    # Écriture bitstream
│   │   ├── point.rs         # Encodage POI
│   │   ├── polyline.rs      # Encodage polylignes
│   │   ├── polygon.rs       # Encodage polygones
│   │   ├── line_preparer.rs # Préparation lignes
│   │   ├── area.rs          # Gestion des zones
│   │   ├── overview.rs      # Carte d'ensemble
│   │   ├── places.rs        # Gestion des lieux
│   │   ├── map_object.rs    # Objet carte générique
│   │   ├── srt.rs           # Table de tri
│   │   └── labelenc/        # Encodage labels (Format 9, CP1252)
│   ├── parser/              # Parseur Polish Map (.mp)
│   └── routing/             # Routage Garmin
├── tests/
│   ├── integration_test.rs  # Tests d'intégration
│   └── fixtures/            # Fichiers de test
├── build.rs                 # Versioning Git automatique
└── Cargo.toml
```

### Tests

```bash
# Tests unitaires et d'intégration
cargo test

# Tests avec logs
cargo test -- --nocapture

# Test d'un module spécifique
cargo test report::tests
```

### Linting et formatage

```bash
# Vérifier le formatage
cargo fmt --check

# Formater le code
cargo fmt

# Linter
cargo clippy

# Linter strict
cargo clippy -- -D warnings
```

### Build

```bash
# Debug (rapide, non optimisé)
cargo build

# Release (lent, optimisé, strippé)
cargo build --release
```

## Licence

Ce projet fait partie de **garmin-ign-bdtopo-map** et est distribué sous licence MIT. Voir le fichier [LICENSE](../../LICENSE) à la racine du dépôt.

## Support

- **Issues** : https://forgejo.allfabox.fr/allfab/garmin-ign-bdtopo-map/issues
