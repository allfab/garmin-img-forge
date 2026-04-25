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
- **Symbologie TYP** : Intégration d'un fichier `.typ` personnalisé pour le rendu des cartes, plus une sous-commande `imgforge typ` pour compiler/décompiler les fichiers TYP (texte ↔ binaire)
- **DEM / Hill Shading** : Génération du sous-fichier DEM Garmin pour l'ombrage du relief et les profils d'altitude
  - Lecture native HGT (SRTM 1/3 arc-sec) et ASC (ESRI ASCII Grid, BDAltiv2)
  - Reprojection intégrée via proj4rs (Lambert 93, UTM, LAEA — zéro dépendance système)
  - Interpolation bilinéaire / bicubique (Catmull-Rom)
  - Encodage multi-niveaux, tuiles 64×64, compression bitstream delta+hybrid+plateau
- **Format Garmin complet** : Génération des sous-fichiers TRE, RGN, LBL, NET, NOD, DEM
- **Fichier TDB** : Génération automatique du fichier compagnon `.tdb`
- **Rapport JSON** : Statistiques de compilation en sortie structurée
- **Zéro dépendance** : Binaire autonome, pas besoin de GDAL ni de librairie externe

## Installation

### Option 1 : Binaire pré-compilé (recommandé)

**Linux x64** :

Les binaires sont publiés sur la page [GitHub Releases](https://github.com/allfab/garmin-img-forge/releases) (tags `imgforge-v*`). Récupère la version voulue :

```bash
# Adapter la version — voir la page releases pour la dernière
VERSION=imgforge-v0.4.3
wget https://github.com/allfab/garmin-img-forge/releases/download/$VERSION/imgforge

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
# Output: imgforge v0.4.3    (ou le tag Git courant)

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
imgforge typ --help
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

# Avec DEM (hill shading) depuis des fichiers HGT SRTM
imgforge compile tile_0_0.mp --dem ./srtm_hgt/

# Avec DEM depuis des fichiers ASC BDAltiv2 en Lambert 93
imgforge compile tile_0_0.mp --dem ./bdaltiv2/ --dem-source-srs EPSG:2154

# DEM avec résolutions personnalisées et interpolation bicubique
imgforge compile tile_0_0.mp --dem ./hgt/ --dem-dists 3312,13248,26512 --dem-interpolation bicubic
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

# Build avec DEM (hill shading sur GPS Garmin)
imgforge build tiles/ --jobs 8 --dem ./srtm_hgt/

# Build avec DEM BDAltiv2 Lambert 93
imgforge build tiles/ --jobs 8 --dem ./bdaltiv2/ --dem-source-srs EPSG:2154

# Build avec packaging GMP (format Garmin NT consolidé : 1 .GMP par tuile au lieu de 6 FAT files)
imgforge build tiles/ --packaging gmp
```

**Note sur `--packaging`** : `legacy` (défaut) produit 6 entrées FAT par tuile (TRE/RGN/LBL/NET/NOD/DEM) — format historique mkgmap/cGPSmapper. `gmp` consolide ces sections dans un unique sous-fichier `.GMP` (format Garmin NT, utilisé par les cartes Garmin modernes telles que Topo France v6 Pro) — validé en production sur Alpha 100. Spec binaire : [`docs/implementation-artifacts/imgforge-gmp-format.md`](../../docs/implementation-artifacts/imgforge-gmp-format.md).

### Commande `typ` : Compilation/décompilation TYP

imgforge embarque un compilateur/décompilateur du format Garmin TYP, utile pour éditer la symbologie avant de l'injecter via `--typ-file`.

```bash
# Compiler un fichier texte TYP en binaire
imgforge typ compile style.txt --output style.typ

# Encodage explicite de l'entrée (auto-détecté par défaut)
imgforge typ compile style.txt --encoding cp1252
imgforge typ compile style.txt --encoding utf8

# Décompiler un binaire TYP en texte (UTF-8 par défaut)
imgforge typ decompile style.typ --output style.txt

# Décompiler en CP1252 (compatibilité TYPViewer)
imgforge typ decompile style.typ --encoding cp1252
```

| Option | Description | Défaut |
|--------|-------------|--------|
| `<INPUT>` | Fichier d'entrée (`.txt` pour compile, `.typ` pour decompile) | **REQUIS** |
| `-o, --output <FILE>` | Fichier de sortie | extension swappée |
| `--encoding <ENC>` | `auto` / `utf8` / `cp1252` | `auto` en compile, `utf8` en decompile |

> **`auto`** détecte le BOM UTF-8, sinon retombe sur CP1252 (format historique TYPViewer).

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
| `--levels <LEVELS>` | Niveaux de zoom : `"24,20,16"` ou `"0:24,1:20,2:16"` | header .mp |
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

#### DEM / Hill Shading

| Option | Description | Défaut |
|--------|-------------|--------|
| `--dem <PATH,...>` | Chemins vers répertoires/fichiers d'élévation (`.hgt`, `.asc`), séparés par virgules | - |
| `--dem-dists <DISTS,...>` | Distances entre points DEM par niveau de zoom. Ex: `3,3,4,6,8,12,16,24,32` | auto |
| `--dem-interpolation <METHOD>` | Méthode d'interpolation : `auto`, `bilinear`, `bicubic` | `auto` |
| `--dem-source-srs <SRS>` | SRS source pour les fichiers ASC (ex: `EPSG:2154` pour Lambert 93) | WGS84 |

> **Formats supportés** : HGT SRTM (1/3 arc-sec, binaire big-endian i16) et ASC ESRI ASCII Grid (BDAltiv2, etc.)
>
> **EPSG supportés** : 2154 (Lambert 93), 4326 (WGS84), 32631-32633 (UTM), 25831-25833 (ETRS89/UTM), 3035 (LAEA), 3857 (Web Mercator), ou toute chaîne proj4.

##### Comprendre `--dem-dists`

Ce paramètre contrôle la **densité des points d'élévation** encodés dans le fichier Garmin pour chaque niveau de zoom. Plus la valeur est élevée, moins il y a de points = fichier plus petit mais relief moins détaillé.

Chaque valeur correspond à un niveau de zoom (dans l'ordre de `--levels`). Si vous fournissez moins de valeurs que de niveaux, les niveaux restants sont calculés automatiquement en doublant la dernière valeur.

**Exemples de configurations** (pour `--levels "24,23,22,21,20,19,18,17,16"`) :

| Profil | `--dem-dists` | Taille | Qualité |
|--------|---------------|--------|---------|
| Haute résolution | `1,1,2,3,4,6,8,12,16` | Grande | Détail max |
| Équilibré | `3,3,4,6,8,12,16,24,32` | Moyenne | Bon compromis |
| Compact | `4,6,8,12,16,24,32` | Petite | Suffisant pour randonnée |

> **Sans `--dem-dists`**, imgforge utilise une densité élevée par défaut sur tous les niveaux, ce qui peut produire des fichiers très volumineux (ex: 500+ Mo pour un département).
>
> **Recommandation** : Commencez par le profil « Équilibré » et ajustez selon vos besoins.

##### Interpolation

| Méthode | Description | Usage |
|---------|-------------|-------|
| `auto` | Bilinéaire par défaut | Recommandé |
| `bilinear` | 4 points voisins, rapide | Données basse résolution (SRTM 3") |
| `bicubic` | 16 points (Catmull-Rom), lissé | Données haute résolution (BDAltiv2 25m) |

`bicubic` produit un relief plus lisse mais retombe automatiquement sur `bilinear` si le voisinage 4×4 n'est pas disponible (bords de grille).

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
| `--report <FILE>` | Écrire le rapport JSON de compilation dans FILE | - |

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

| Flag | Niveau activé | Utilisation |
|------|---------------|-------------|
| _(aucun)_ | `WARN` + `ERROR` | Production — barre de progression + résumé console uniquement |
| `-v` | + `INFO` | Monitoring — tuile par tuile, messages routing |
| `-vv` | + `DEBUG` | Troubleshooting — détails encodage, barre de progression désactivée |
| `-vvv` | + `TRACE` | Développement — verbosité maximale |

En production (sans `-v`), imgforge affiche uniquement la barre de progression pendant la compilation et un résumé structuré en fin d'exécution. Les messages de niveau INFO (ex : routing désactivé, tuile compilée) n'apparaissent qu'avec `-v`.

### Rapport JSON (`--report`)

| Option | Description | Défaut |
|--------|-------------|--------|
| `--report <FILE>` | Écrire le rapport JSON de compilation dans FILE | - (pas de rapport) |

Disponible sur les commandes `compile` et `build`. Utile pour l'intégration CI/CD ou le suivi des métriques de build.

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

## Niveaux de zoom (`--levels`)

L'option `--levels` contrôle les niveaux de zoom de la carte Garmin. Chaque niveau crée un jeu de subdivisions contenant les features visibles à cette échelle.

```bash
# 3 niveaux (recommandé) : détail, intermédiaire, vue large
imgforge build tiles/ --levels "24,20,16"

# 2 niveaux : détail + vue large
imgforge build tiles/ --levels "24,18"
```

**Correspondance avec `EndLevel`** : une feature avec `EndLevel=N` dans le fichier `.mp` est écrite aux niveaux **0 à N**. Plus il y a de niveaux et plus les EndLevel sont élevés, plus le fichier est gros :

| Niveaux | EndLevel max | Copies max | Taille relative |
|---------|-------------|------------|-----------------|
| `"24,18"` | 1 | x2 | Référence |
| `"24,20,16"` | 2 | x3 | +30-50% |
| `"24,22,20,18,16"` | 4 | x5 | +100-150% |
| `"24,23,...,16"` (9) | 8 | x9 | +200-400% |

> **Recommandation** : 3 niveaux avec des sauts de 4+ bits d'écart offrent le meilleur compromis taille/navigation. Les niveaux consécutifs (24→23→22) n'apportent aucune différence perceptible sur un GPS Garmin.

## Rapport JSON

Avec `--report <FILE>`, imgforge écrit un fichier JSON structuré exploitable en CI/CD. La sortie console (stdout) affiche un résumé lisible.

```bash
imgforge build tiles/ --output gmapsupp.img --jobs 8 --report build-report.json
```

```json
{
  "tiles_compiled": 55,
  "tiles_failed": 0,
  "total_points": 182340,
  "total_polylines": 94710,
  "total_polygons": 31820,
  "duration_ms": 8420,
  "duration_seconds": 8.42,
  "output_file": "gmapsupp.img",
  "img_size_bytes": 52428800
}
```

| Champ | Description |
|-------|-------------|
| `tiles_compiled` | Tuiles compilées avec succès |
| `tiles_failed` | Tuiles en erreur (non-zero → problème) |
| `total_points` | Total POI compilés (toutes tuiles) |
| `total_polylines` | Total polylignes compilées |
| `total_polygons` | Total polygones compilés |
| `duration_ms` | Durée d'exécution en millisecondes |
| `duration_seconds` | Durée d'exécution en secondes (flottant) |
| `output_file` | Chemin du fichier IMG produit |
| `img_size_bytes` | Taille du fichier IMG en octets |

### Intégration CI/CD

```bash
#!/bin/bash
# Pipeline : mpforge (tuilage) → imgforge (compilation)

# Étape 1 : Générer les tuiles .mp
mpforge build --config bdtopo.yaml --jobs 8

# Étape 2 : Compiler en gmapsupp.img avec rapport JSON
imgforge build tiles/ --output gmapsupp.img --jobs 8 \
  --report build-report.json \
  --family-name "BDTOPO France" \
  --series-name "IGN BDTOPO 2026" \
  --latin1 \
  --reduce-point-density 3.0 \
  --min-size-polygon 8 \
  --typ-file bdtopo.typ \
  --dem ./srtm_hgt/ \
  --keep-going

# Lire les métriques depuis le rapport
TILES=$(jq '.tiles_compiled' build-report.json)
FAILED=$(jq '.tiles_failed' build-report.json)
echo "Compilation : ${TILES} tuile(s), ${FAILED} échec(s)"
ls -lh gmapsupp.img
```

## Pipeline complet mpforge + imgforge

imgforge s'inscrit dans le pipeline de création de cartes Garmin :

```
Données SIG (Shapefile, GPKG, PostGIS)      Données d'élévation
    │                                         │
    ▼                                         │  HGT (SRTM)
 mpforge build     ← Tuilage spatial          │  ASC (BDAltiv2)
    │                                         │
    ▼                                         │
 tiles/*.mp        ← Fichiers Polish Map      │
    │                                         │
    ▼                                         ▼
 imgforge build    ← Compilation Garmin IMG + DEM hill shading
    │
    ▼
 gmapsupp.img      ← Carte Garmin avec relief (prête pour GPS)
```

## Architecture Garmin IMG

imgforge génère les sous-fichiers standards du format Garmin IMG :

| Sous-fichier | Description |
|-------------|-------------|
| **TRE** | Table des régions (index spatial, niveaux de zoom) |
| **RGN** | Données régions (géométries : points, lignes, polygones) |
| **LBL** | Labels (noms, encodage Format 6/9/10 — ASCII, codepage, UTF-8) |
| **NET** | Réseau routier (topologie) |
| **NOD** | Nœuds de routage |
| **DEM** | Données d'élévation (hill shading, profils altitude sur GPS Garmin) |
| **TYP** | Symbologie personnalisée (couleurs, motifs, icônes de points/lignes/polygones) |
| **TDB** | Table de description (métadonnées de la carte) |

## Développement

### Structure du projet

```
imgforge/
├── build.rs                 # Injection de GIT_VERSION à la compilation
├── src/
│   ├── main.rs              # Orchestration compile/build/typ
│   ├── cli.rs               # Définition des arguments CLI (clap)
│   ├── lib.rs               # Exports publics
│   ├── error.rs             # Types d'erreurs
│   ├── report.rs            # Rapport JSON d'exécution
│   ├── dem/
│   │   ├── mod.rs           # Types partagés, détection format, chargement multi-fichiers
│   │   ├── hgt.rs           # Lecteur HGT (SRTM, big-endian i16)
│   │   ├── asc.rs           # Lecteur ASC (ESRI ASCII Grid, reprojection proj4rs)
│   │   └── converter.rs     # Interpolation bilinéaire/bicubique, resampling
│   ├── img/
│   │   ├── mod.rs           # Exports du module img
│   │   ├── writer.rs        # Génération IMG (pipeline complet single-tile)
│   │   ├── assembler.rs     # Assemblage gmapsupp multi-tile
│   │   ├── mps.rs           # Table MPS (multi-produit, lien avec TDB)
│   │   ├── tre.rs           # Sous-fichier TRE
│   │   ├── rgn.rs           # Sous-fichier RGN
│   │   ├── lbl.rs           # Sous-fichier LBL
│   │   ├── net.rs           # Sous-fichier NET
│   │   ├── nod.rs           # Sous-fichier NOD
│   │   ├── dem.rs           # Sous-fichier DEM (encodeur Garmin, compression bitstream)
│   │   ├── tdb.rs           # Fichier compagnon TDB
│   │   ├── overview.rs      # Header carte d'ensemble
│   │   ├── overview_map.rs  # Génération de la carte d'ensemble
│   │   ├── splitter.rs      # Découpage en subdivisions
│   │   ├── subdivision.rs   # Gestion des subdivisions
│   │   ├── zoom.rs          # Niveaux de zoom
│   │   ├── coord.rs         # Encodage coordonnées Garmin
│   │   ├── header.rs        # Header IMG
│   │   ├── common_header.rs # Header commun sous-fichiers
│   │   ├── directory.rs     # Répertoire FAT
│   │   ├── filesystem.rs    # Système de fichiers IMG
│   │   ├── bit_writer.rs    # Écriture bitstream
│   │   ├── point.rs         # Encodage POI
│   │   ├── polyline.rs      # Encodage polylignes
│   │   ├── polygon.rs       # Encodage polygones
│   │   ├── line_preparer.rs # Préparation des lignes avant écriture
│   │   ├── line_clipper.rs  # Clipping des lignes sur les subdivisions
│   │   ├── area.rs          # Gestion des zones
│   │   ├── srt.rs           # Table de tri (collation)
│   │   └── labelenc/        # Encodage labels (Format 6/9/10, CP1252, UTF-8)
│   ├── typ/
│   │   ├── mod.rs           # Exports du module TYP
│   │   ├── data.rs          # Modèle de données TYP (types point/ligne/polygone)
│   │   ├── encoding.rs      # Détection & conversion d'encodage (auto/utf8/cp1252)
│   │   ├── text_reader.rs   # Parseur TYP texte (.txt)
│   │   ├── text_writer.rs   # Générateur TYP texte
│   │   ├── binary_reader.rs # Parseur TYP binaire (.typ)
│   │   └── binary_writer.rs # Générateur TYP binaire
│   ├── parser/
│   │   ├── mod.rs           # Parseur Polish Map (.mp)
│   │   └── mp_types.rs      # Types du format .mp
│   └── routing/
│       ├── mod.rs           # Exports routing
│       └── graph_builder.rs # Construction du graphe routier (NET+NOD)
├── tests/
│   ├── integration_test.rs       # Tests d'intégration IMG (compile/build)
│   ├── typ_integration_test.rs   # Tests d'intégration TYP (compile/decompile)
│   └── fixtures/                 # Fichiers de test
├── examples/                # Exemples (hybrid_test.rs, quick_hybrid.rs — DEM)
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

Ce projet fait partie de **garmin-img-forge** et est distribué sous licence MIT. Voir le fichier [LICENSE](../../LICENSE) à la racine du dépôt.

## Support

- **Issues** : https://github.com/allfab/garmin-img-forge/issues
