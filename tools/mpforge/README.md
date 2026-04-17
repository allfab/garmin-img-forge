# mpforge

> Outil CLI pour générer des tuiles au format Polish Map (.mp) à partir de données SIG massives

**mpforge** est un outil en ligne de commande Rust qui découpe vos données géospatiales (Shapefiles, GeoPackage, PostGIS) en tuiles au format Polish Map pour créer des cartes Garmin personnalisées.

## Caractéristiques principales

- **Multi-sources** : Shapefiles, GeoPackage, PostGIS
- **Multi-couches** : Support de GeoPackage avec dizaines de couches
- **Parallélisation** : Traitement multi-thread pour datasets massifs (BDTOPO 35 GB)
- **Tuilage spatial** : Découpage automatique en grille avec chevauchement configurable
- **Filtrage** : Bounding box pour extraire des zones géographiques
- **Wildcards & Brace Expansion** : Patterns de fichiers (`data/*.shp`, `data/**/*.gpkg`, `data/{D038,D069}/*.shp`)
- **Robustesse** : Modes `continue` (tolérant) ou `fail-fast` (strict)
- **CI/CD** : Rapports JSON structurés et codes de sortie
- **Performance** : Barre de progression temps réel et logs multi-niveaux

## Installation

### Option 1 : Binaire pré-compilé (recommandé)

✨ **Zéro configuration** : Les binaires incluent GDAL 3.10.1 et le driver PolishMap. Aucune dépendance système n'est requise !

**Linux x64** :
```bash
# Télécharger la release
wget https://forgejo.allfabox.fr/allfab/mpforge/releases/download/v0.2.0/mpforge-linux-x64-static.tar.gz

# Extraire
tar xzf mpforge-linux-x64-static.tar.gz

# Installer
sudo mv mpforge /usr/local/bin/

# Tester
mpforge --version
```

**Linux ARM64** (Raspberry Pi, serveurs ARM) :
```bash
# À venir dans une prochaine release
# Le build ARM64 statique sera disponible prochainement
```

**Compatibilité** :
- ✅ Ubuntu 18.04+ / Debian 10+
- ✅ Fedora 28+ / RHEL 8+
- ✅ Alpine Linux 3.12+ (glibc)
- ✅ Arch Linux / Manjaro
- ✅ WSL2

> 💡 **Anciennes releases (< v0.2.0)** : Nécessitaient GDAL installé. À partir de v0.2.0, GDAL est intégré dans le binaire.

### Option 2 : Compilation depuis les sources

#### Prérequis

- **Rust** : 1.70+ ([rustup](https://rustup.rs/))
- **GDAL** : 3.0+ (avec support OGR)

```bash
# Installer GDAL (requis pour la compilation)
# Fedora
sudo dnf install gdal-devel

# Ubuntu/Debian
sudo apt install libgdal-dev gdal-bin

# Alpine
sudo apk add gdal-dev
```

#### Compilation

```bash
cd mpforge
cargo build --release
```

L'exécutable sera disponible dans `target/release/mpforge`.

#### Installation globale

```bash
cargo install --path .
```

## Quick Reference

### Vérifier la version installée

```bash
mpforge --version
# Output: mpforge v0.2.0

# Alternative : flag court
mpforge -V
```

> 💡 **Note** : La version est automatiquement synchronisée avec le tag Git. Voir la section [Versioning automatique](../README.md#versioning-automatique) du README principal pour plus de détails.

### Afficher l'aide complète

```bash
# Aide globale (liste des commandes)
mpforge --help

# Aide spécifique à la commande build
mpforge build --help
```

**L'aide complète documente toutes les options disponibles** (configuration, parallélisation, rapports JSON, verbosité, etc.). Utilisez `--help` pour découvrir toutes les features disponibles sans consulter la documentation externe.

## Utilisation rapide

### 1. Créer un fichier de configuration

Créez `config.yaml` :

```yaml
version: 1

grid:
  cell_size: 0.15      # Taille de cellule en degrés (~16.5 km)
  overlap: 0.01        # Chevauchement entre tuiles

inputs:
  - path: "data/routes.shp"
  - path: "data/batiments.shp"

output:
  directory: "tiles/"
  filename_pattern: "{col}_{row}.mp"
```

### 2. Exécuter le tuilage

```bash
# Mode séquentiel (debug)
mpforge build --config config.yaml

# Mode production (parallèle)
mpforge build --config config.yaml --jobs 4

# Avec rapport JSON
mpforge build --config config.yaml --jobs 4 --report report.json
```

### 3. Résultat

```
tiles/
├── 0_0.mp
├── 0_1.mp
├── 1_0.mp
└── 1_1.mp
```

Chaque fichier `.mp` contient les données géospatiales de sa tuile et peut être converti en carte Garmin avec `cgpsmapper` ou `mkgmap`.

## Configuration détaillée

Voir la section [Configuration détaillée](#configuration-détaillée) ci-dessous pour tous les détails.

### Structure du fichier YAML

```yaml
version: 1

grid:
  cell_size: 0.15          # Taille de cellule (degrés) - REQUIS
  overlap: 0.01            # Chevauchement (degrés) - optionnel
  origin: [-5.0, 41.0]     # Point d'origine [lon, lat] - optionnel

inputs:
  # Shapefiles
  - path: "data/roads.shp"
  - path: "data/*.shp"                      # Wildcards supportés
  - path: "data/{D038,D069}/roads.shp"      # Brace expansion (multi-zones)

  # GeoPackage multi-couches
  - path: "data/bdtopo.gpkg"
    layers:
      - "batiment"
      - "route"
      - "cours_d_eau"

  # PostGIS
  - connection: "PG:host=localhost dbname=gis"
    layers: ["roads", "buildings"]

output:
  directory: "tiles/"
  filename_pattern: "{col}_{row}.mp"  # Voir Patterns de nommage ci-dessous

filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]  # [min_lon, min_lat, max_lon, max_lat]

error_handling: "continue"  # "continue" ou "fail-fast"
```

### Patterns de nommage des tuiles

Le champ `filename_pattern` contrôle le nommage des fichiers tuiles exportés. Par défaut : `{col}_{row}.mp`.

| Pattern | Résultat (col=15, row=42, seq=157) | Description |
|---------|-----------------------------------|-------------|
| `{col}_{row}.mp` | `15_42.mp` | Default — colonne et ligne |
| `{col:03}_{row:03}.mp` | `015_042.mp` | Zero-padding à 3 chiffres |
| `{seq}.mp` | `157.mp` | Compteur séquentiel (1-based) |
| `{seq:04}.mp` | `0157.mp` | Séquentiel zero-padded |
| `tile_{col}_{row}.mp` | `tile_15_42.mp` | Préfixe personnalisé |
| `{col}/{row}.mp` | `15/42.mp` | Sous-dossiers automatiques |
| `{x}_{y}.mp` | `15_42.mp` | Alias rétrocompat de col/row |

**Variables :** `{col}` (index colonne), `{row}` (index ligne), `{seq}` (compteur 1-based).
**Aliases :** `{x}` = `{col}`, `{y}` = `{row}`.
**Zero-padding :** `{var:N}` pad à N chiffres avec des zéros à gauche.

### Brace Expansion (multi-zones)

mpforge supporte la **brace expansion** dans les chemins de fichiers, en plus des wildcards classiques (`*`, `?`, `**`). Cela permet de cibler précisément plusieurs sous-dossiers sans matcher tout le contenu d'un répertoire :

```yaml
inputs:
  # Sélectionner les routes de 2 départements spécifiques
  - path: "data/bdtopo/{D038,D069}/TRANSPORT/TRONCON_DE_ROUTE.shp"

  # Combiné avec des variables d'environnement
  - path: "${DATA_ROOT}/{${ZONES}}/HYDROGRAPHIE/SURFACE_HYDROGRAPHIQUE.shp"
```

Avec `ZONES=D038,D069`, mpforge :

1. Substitue les variables : `data/bdtopo/{D038,D069}/HYDROGRAPHIE/...`
2. Expande les accolades en 2 patterns : `data/bdtopo/D038/...` et `data/bdtopo/D069/...`
3. Résout chaque pattern via glob (support `*` et `**`)

La brace expansion fonctionne aussi dans `spatial_filter.source`, ce qui permet de construire un filtre spatial multi-zones :

```yaml
inputs:
  - path: "${OSM_DATA_ROOT}/gpkg/*-amenity-points.gpkg"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

Les géométries de tous les fichiers matchés sont automatiquement unies en un seul filtre spatial.

**Rétro-compatibilité** : un chemin sans accolades fonctionne exactement comme avant.

### Exemples de configuration

Voir le répertoire [`examples/`](examples/) :

- **[simple.yaml](examples/simple.yaml)** : Configuration minimale pour débuter
- **[simple-with-mapping.yaml](examples/simple-with-mapping.yaml)** : Configuration avec field mapping (sources avec champs personnalisés)
- **[bdtopo.yaml](examples/bdtopo.yaml)** : Configuration production pour BDTOPO (35 GB, 50+ couches)

### Généralisation de géométrie

**mpforge** permet de lisser et/ou simplifier les géométries par couche via la directive `generalize` dans la configuration des sources. Cela reproduit le comportement des transformeurs FME comme Generalizer (McMaster) en utilisant des algorithmes géométriques standards.

#### Configuration

Ajoutez `generalize` à n'importe quelle entrée `inputs` :

```yaml
inputs:
  - path: "${DATA_ROOT}/LIEUX_NOMMES/ZONE_D_HABITATION.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    generalize:
      smooth: "chaikin"       # Algorithme de lissage
      iterations: 2           # Nombre de passes (chaque itération double les vertices)
      simplify: 0.00005       # Tolérance Douglas-Peucker (degrés WGS84, optionnel)
```

#### Paramètres

| Paramètre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `smooth` | string | non | Algorithme de lissage : `"chaikin"` (corner-cutting) |
| `iterations` | entier | non | Nombre de passes de lissage (défaut : 1) |
| `simplify` | flottant | non | Tolérance Douglas-Peucker en degrés WGS84 (post-lissage) |

#### Algorithmes disponibles

**Chaikin corner-cutting** (`smooth: "chaikin"`) : Lisse les angles vifs des polygones et polylignes en coupant itérativement les coins. Équivalent approximatif du McMaster sliding average de FME. Chaque itération double le nombre de vertices, d'où l'intérêt de combiner avec `simplify` pour contrôler la densité finale.

| Itérations | Résultat |
|-----------|----------|
| 1 | Lissage léger — adoucit les angles |
| 2 | Lissage modéré — courbes arrondies (recommandé) |
| 3+ | Lissage prononcé — très arrondi, beaucoup de vertices |

#### Pipeline de traitement

La généralisation s'applique **après le clipping** des features sur les tuiles et **avant l'export** en Polish Map :

```
Lecture → Règles → Clipping tuile → [Lissage + Simplification] → Export .mp
```

Les points (POI) ne sont pas affectés. Seuls les polygones et polylignes sont traités.

### Field Mapping Configuration

**mpforge** supporte le mappage personnalisé des champs sources vers les champs canoniques du format Polish Map via un **fichier YAML externe**.

#### Pourquoi utiliser le field mapping ?

Lorsque vos données sources utilisent des noms de champs personnalisés (par exemple, `MP_TYPE`, `NAME` dans BDTOPO), le field mapping permet de les transposer automatiquement vers les champs standards Polish Map (`Type`, `Label`) sans modifier vos données sources.

#### Architecture : Deux fichiers séparés

Le field mapping utilise **deux fichiers distincts** :

| Fichier | Rôle | Contenu |
|---------|------|---------|
| **`config.yaml`** | Configuration du pipeline | Sources, grille, output, **référence** au fichier de mapping |
| **`bdtopo-mapping.yaml`** | Définition des mappages | Correspondances champs sources → champs Polish Map |

Cette séparation permet de **réutiliser** le même fichier de mapping pour plusieurs configurations.

#### Configuration complète

**1️⃣ Fichier `config.yaml`** (configuration principale)

```yaml
version: 1

grid:
  cell_size: 0.15

inputs:
  - path: "data/communes.shp"
  # ⚠️ PAS de field mapping ici ! Le mapping est au niveau output.

output:
  directory: "tiles/"
  filename_pattern: "{col}_{row}.mp"
  field_mapping_path: "bdtopo-mapping.yaml"  # ← Chemin vers le fichier de mapping
  # Note: Chemins relatifs résolus depuis le répertoire de travail (pwd).
  #       Utilisez un chemin absolu pour éviter toute ambiguïté.
```

**2️⃣ Fichier `bdtopo-mapping.yaml`** (définition des mappages)

```yaml
field_mapping:
  # Champs principaux
  MP_TYPE: Type          # Code type Garmin (ex: 0x4e00)
  NAME: Label            # Nom de la feature

  # Localisation
  Country: CountryName   # Pays (ex: "France~[0x1d]FRA")
  CityName: CityName     # Ville/commune
  Zip: Zip              # Code postal

  # Paramètres d'affichage
  MPBITLEVEL: Levels    # Niveaux de zoom (ex: "0-3")
  EndLevel: EndLevel    # Niveau max (0-9)
```

Exemple complet : [`examples/bdtopo-mapping.yaml`](examples/bdtopo-mapping.yaml)

#### Erreurs courantes à éviter

❌ **Erreur 1 : Mettre le mapping dans `inputs`**

```yaml
inputs:
  - path: "data.shp"
    field_mapping: {...}  # ❌ CE CHAMP N'EXISTE PAS !
```

✅ **Correct : Le mapping va dans `output`**

```yaml
output:
  directory: "tiles/"
  field_mapping_path: "mapping.yaml"  # ✅ Référence au fichier externe
```

---

❌ **Erreur 2 : Définir le mapping inline dans `config.yaml`**

```yaml
field_mapping:  # ❌ PAS au niveau racine de config.yaml !
  MP_TYPE: Type
```

✅ **Correct : Fichier séparé `bdtopo-mapping.yaml`**

```yaml
# Dans bdtopo-mapping.yaml (fichier séparé)
field_mapping:  # ✅ Au niveau racine du fichier de mapping
  MP_TYPE: Type
```

---

❌ **Erreur 3 : Syntaxe `source/target`**

```yaml
field_mapping:
  - source: "NAME"    # ❌ Syntaxe incorrecte
    target: "Label"
```

✅ **Correct : Format clé-valeur simple**

```yaml
field_mapping:
  NAME: Label  # ✅ source: destination
  MP_TYPE: Type
```

#### Équivalent ogr2ogr

Cette fonctionnalité est équivalente à :

```bash
ogr2ogr -f "PolishMap" \
  -dsco FIELD_MAPPING=bdtopo-mapping.yaml \
  output.mp input.shp
```

**mpforge** passe automatiquement cette option au driver `ogr-polishmap` lors de la création des fichiers `.mp` pour **chaque tuile générée**.

#### Backward compatibility

Si `field_mapping_path` n'est pas spécifié, le driver utilise ses aliases hardcodés (comportement par défaut des versions précédentes). Vos configurations existantes continuent de fonctionner sans modification.

### Header Configuration

**mpforge** permet de configurer les options du header Polish Map (`[IMG ID]`) pour toutes les tuiles exportées, soit via un **fichier template**, soit via des **champs individuels**.

#### Pourquoi configurer le header ?

Le header Polish Map contient des métadonnées importantes pour la compilation avec cGPSmapper et l'affichage sur GPS Garmin :
- **Nom de la carte** et **copyright**
- **Niveaux de détail** (zoom levels)
- **Paramètres de performance** (TreeSize, RgnLimit)
- **Options de rendu** (transparence, marine)

Sans configuration, le driver utilise des valeurs par défaut minimales.

#### Option 1 : Template file (recommandé pour production)

Utilisez un fichier template pour standardiser le header sur toutes les tuiles :

**Fichier `config.yaml`** :

```yaml
output:
  directory: "tiles/"
  filename_pattern: "{col}_{row}.mp"

header:
  template: "examples/header_template.mp"  # ← Fichier template
```

**Fichier `header_template.mp`** :

```
[IMG ID]
Name=BDTOPO Production Map
ID=0
Copyright=IGN 2026
Levels=4
Level0=24
Level1=21
Level2=18
Level3=15
TreeSize=3000
RgnLimit=1024
Transparent=N
Marine=N
Preprocess=F
LBLcoding=9
SimplifyLevel=2
LeftSideTraffic=N
```

**Avantages** :
- Configuration centralisée (un seul fichier à modifier)
- Réutilisable pour plusieurs projets
- Format standard Polish Map (compatible cGPSmapper)

#### Option 2 : Champs individuels (configuration inline)

Spécifiez les champs directement dans le YAML :

```yaml
output:
  directory: "tiles/"
  filename_pattern: "{col}_{row}.mp"

header:
  # Informations de base
  name: "BDTOPO Réunion"
  id: "0"
  copyright: "IGN 2026"

  # Niveaux de détail
  levels: "4"
  level0: "24"
  level1: "21"
  level2: "18"
  level3: "15"

  # Performance
  tree_size: "3000"
  rgn_limit: "1024"

  # Apparence
  transparent: "N"
  marine: "N"

  # Traitement
  preprocess: "F"
  lbl_coding: "9"
  simplify_level: "2"
  left_side_traffic: "N"

  # Champs personnalisés
  custom:
    DrawPriority: "25"
    MG: "N"
```

#### Champs disponibles

| Champ YAML | Polish Map | Description | Valeurs |
|------------|-----------|-------------|---------|
| `name` | `Name` | Nom de la carte | Texte libre |
| `id` | `ID` | ID de la carte | `0` (auto) ou entier |
| `copyright` | `Copyright` | Notice de copyright | Texte libre |
| `levels` | `Levels` | Nombre de niveaux de détail | `1`-`10` |
| `level0`-`level9` | `Level0`-`Level9` | Zoom par niveau | `24`, `21`, `18`... |
| `tree_size` | `TreeSize` | Taille d'arbre | `100`-`15000` (défaut: 3000) |
| `rgn_limit` | `RgnLimit` | Limite région | `50`-`1024` (défaut: 1024) |
| `transparent` | `Transparent` | Fond transparent | `Y`/`N`/`S` |
| `marine` | `Marine` | Mode marine | `Y`/`N` |
| `preprocess` | `Preprocess` | Mode prétraitement | `G`/`F`/`P`/`N` |
| `lbl_coding` | `LBLcoding` | Encodage labels | `6`/`9`/`10` |
| `simplify_level` | `SimplifyLevel` | Niveau simplification | `0`-`4` |
| `left_side_traffic` | `LeftSideTraffic` | Circulation à gauche | `Y`/`N` |
| `custom` | _(clés arbitraires)_ | Champs personnalisés | Map clé-valeur |

#### Comprendre les niveaux de zoom et EndLevel

Les niveaux de zoom (`Levels`, `Level0`-`Level9`) définissent l'échelle de rendu sur le GPS. L'attribut `EndLevel` de chaque feature contrôle **jusqu'à quel niveau elle reste visible** :

- `EndLevel=0` : visible uniquement au zoom le plus détaillé (Level 0)
- `EndLevel=1` : visible aux niveaux 0 et 1
- `EndLevel=N` : visible aux niveaux 0 à N

```yaml
# Header : 3 niveaux de zoom
header:
  levels: "3"
  level0: "24"    # Level 0 : détail max (~5m)
  level1: "20"    # Level 1 : intermédiaire (~80m)
  level2: "16"    # Level 2 : vue large (~1.2km)
```

```yaml
# Règles : chaque feature définit son EndLevel
rules:
  - match: { CL_ADMIN: "Autoroute" }
    set: { Type: "0x01", EndLevel: "2" }    # Visible partout

  - match: { NATURE: "Sentier" }
    set: { Type: "0x16", EndLevel: "0" }    # Détail uniquement
```

> **Impact sur la taille** : chaque copie supplémentaire augmente le fichier IMG. Avec 3 niveaux, une feature `EndLevel=2` est écrite 3 fois. Limitez les EndLevel élevés aux features structurantes (routes principales, grands lacs).

#### Formatage de casse des labels (`label_case`)

L'option `label_case` contrôle le formatage de la casse des labels écrits dans les fichiers MP.
Elle peut être définie au niveau du **ruleset** (défaut pour toutes les règles) ou au niveau
d'une **règle individuelle** (override du ruleset).

| Valeur | Description | Exemple |
|--------|-------------|---------|
| `none` | Pas de changement (défaut) | `"Mont Blanc"` → `"Mont Blanc"` |
| `upper` | Tout en majuscules | `"Mont Blanc"` → `"MONT BLANC"` |
| `lower` | Tout en minuscules | `"Mont Blanc"` → `"mont blanc"` |
| `title` | Casse de titre | `"mont blanc"` → `"Mont Blanc"` |
| `capitalize` | Première lettre en majuscule | `"mont blanc"` → `"Mont blanc"` |

Les préfixes Garmin (`~[0xNN]`) sont préservés : seule la partie texte est transformée.

```yaml
- name: "Toponymie"
  source_layer: "TOPONYMIE"
  label_case: "title"        # Défaut : casse de titre pour tout le ruleset
  rules:
    - match:
        CLASSE: "Montagne"
      set:
        Type: "0x6616"
        Label: "${GRAPHIE}"
      label_case: "upper"    # Override : sommets en majuscules
```

#### Précédence des options

Si `template` ET champs individuels sont spécifiés, **le template prend le dessus** :

```yaml
header:
  template: "header_template.mp"  # ← Utilisé
  name: "Ignored"                 # ← Ignoré
```

#### Backward compatibility

Si `header` n'est pas spécifié, le driver utilise ses valeurs par défaut (comportement des versions précédentes). Vos configurations existantes continuent de fonctionner sans modification.

### Options d'export

**mpforge** propose des options pour contrôler le comportement d'export, utiles pour les pipelines de production :

#### Reprendre un export interrompu (`--skip-existing`)

Si un export est interrompu (crash, timeout), vous pouvez reprendre sans ré-exporter les tuiles déjà générées :

```bash
# Premier export (interrompu à la tuile 1500/2000)
mpforge build --config config.yaml --jobs 8

# Reprendre : seules les tuiles manquantes sont exportées
mpforge build --config config.yaml --jobs 8 --skip-existing
```

**Équivalent YAML** : `output.overwrite: false` dans le fichier de configuration.

```yaml
output:
  directory: "tiles/"
  overwrite: false  # Équivalent à --skip-existing
```

#### Prévisualiser un export (`--dry-run`)

Vérifiez votre configuration avant un long export :

```bash
mpforge build --config config.yaml --dry-run
```

Le pipeline s'exécute normalement (lecture sources, R-tree, clipping) mais **aucun fichier n'est créé**. Le résumé affiche le nombre de tuiles et features qui seraient exportées.

#### Combinaison `--dry-run --skip-existing`

Utile pour estimer combien de tuiles restent à exporter :

```bash
mpforge build --config config.yaml --dry-run --skip-existing
```

## Options CLI

### Commande `build`

```bash
mpforge build [OPTIONS] --config <CONFIG>
```

| Option | Description | Défaut |
|--------|-------------|--------|
| `-c, --config <FILE>` | Fichier de configuration YAML | **REQUIS** |
| `-j, --jobs <N>` | Nombre de threads parallèles | `1` |
| `-r, --report <FILE>` | Générer un rapport JSON | - |
| `-v, --verbose...` | Verbosité (`-v`, `-vv`, `-vvv`) | WARN |
| `--fail-fast` | Arrêter à la première erreur | - |
| `--skip-existing` | Ignorer les tuiles déjà exportées (reprise d'export) | - |
| `--dry-run` | Mode simulation : prévisualiser sans écrire de fichiers | - |
| `-i, --input <PATH>` | Remplacer le chemin d'entrée | - |
| `-o, --output <PATH>` | Remplacer le répertoire de sortie | - |
| `-h, --help` | Afficher l'aide | - |

### Niveaux de verbosité

| Flag | Niveau | Utilisation |
|------|--------|-------------|
| _(aucun)_ | WARN | Production (barre de progression) |
| `-v` | INFO | Monitoring (étapes principales) |
| `-vv` | DEBUG | Troubleshooting (logs GDAL détaillés) |
| `-vvv` | TRACE | Développement (verbosité maximale) |

**Note** : La barre de progression est désactivée en mode `-vv` et `-vvv` pour éviter la pollution des logs.

### Parallélisation

La parallélisation utilise **rayon** pour distribuer le traitement des tuiles sur N workers indépendants. Chaque worker ouvre ses propres datasets GDAL — aucun state partagé entre threads.

```bash
# Vérifier le nombre de CPUs
nproc

# Petit dataset (<50 tuiles) : mode séquentiel (défaut)
mpforge build --config config.yaml

# Dataset moyen (50-500 tuiles) : 4 threads
mpforge build --config config.yaml --jobs 4

# Large dataset (>500 tuiles) : 8 threads
mpforge build --config config.yaml --jobs 8
```

**Comportement** :
- `--jobs 1` (défaut) : boucle séquentielle, pas de thread pool rayon
- `--jobs N` (N > 1) : thread pool rayon de N workers, traitement parallèle des tuiles
- En mode `fail-fast` + parallèle : la première erreur interrompt tous les workers
- En mode `continue` + parallèle : toutes les erreurs sont collectées thread-safe

**Performances attendues** :
- 2-4 threads : ~2× speedup
- 4-8 threads : ~2-3× speedup

## Exemples d'utilisation

### Exemple 1 : Shapefiles simples

```yaml
# config.yaml
version: 1

grid:
  cell_size: 0.15

inputs:
  - path: "data/routes.shp"
  - path: "data/batiments.shp"
  - path: "data/pois/*.shp"

output:
  directory: "tiles/"
```

```bash
mpforge build --config config.yaml --jobs 4
```

### Exemple 2 : GeoPackage multi-couches (BDTOPO)

```yaml
# bdtopo.yaml
version: 1

grid:
  cell_size: 0.15
  overlap: 0.01

inputs:
  - path: "bdtopo/BDTOPO_reunion.gpkg"
    layers:
      - "batiment"
      - "route"
      - "troncon_de_route"
      - "cours_d_eau"
      - "plan_d_eau"
      - "commune"
      - "zone_vegetation"
      # ... 40+ autres couches

output:
  directory: "tiles_bdtopo/"
  filename_pattern: "reunion_{col}_{row}.mp"

filters:
  bbox: [55.2, -21.4, 55.8, -20.9]  # Île de La Réunion

error_handling: "continue"
```

```bash
mpforge build --config bdtopo.yaml --jobs 8 --report rapport.json
```

### Exemple 3 : PostGIS

```yaml
# postgis.yaml
version: 1

grid:
  cell_size: 0.10

inputs:
  - connection: "PG:host=localhost dbname=gis user=postgres"
    layers:
      - "osm_roads"
      - "osm_buildings"
      - "osm_pois"

output:
  directory: "tiles_osm/"
```

```bash
mpforge build --config postgis.yaml --jobs 4 -v
```

### Exemple 4 : Field mapping (BDTOPO avec champs personnalisés)

Lorsque vos données sources utilisent des noms de champs personnalisés (par exemple `MP_TYPE`, `NAME` au lieu de `Type`, `Label`), utilisez le field mapping :

**Fichier `config.yaml`**

```yaml
version: 1

grid:
  cell_size: 0.15
  overlap: 0.01

inputs:
  - path: "bdtopo/COMMUNE.shp"  # Contient MP_TYPE, NAME, Country, etc.
  - path: "bdtopo/ROUTE.shp"

output:
  directory: "tiles_bdtopo/"
  filename_pattern: "france_{col}_{row}.mp"
  field_mapping_path: "bdtopo-mapping.yaml"  # ← Référence au fichier de mapping

error_handling: "continue"
```

**Fichier `bdtopo-mapping.yaml`** (à créer dans le même répertoire)

```yaml
field_mapping:
  # Champs principaux
  MP_TYPE: Type          # Code type Garmin (ex: 0x4e00)
  NAME: Label            # Nom de la feature

  # Localisation
  Country: CountryName   # Pays
  CityName: CityName     # Ville/commune
  Zip: Zip              # Code postal

  # Affichage
  MPBITLEVEL: Levels    # Niveaux de zoom
  EndLevel: EndLevel    # Niveau max
```

**Commande**

```bash
mpforge build --config config.yaml --jobs 4
```

**Résultat**

Les fichiers `.mp` générés contiennent les champs corrects :

```
[POI]
Type=0x4e00
Label=Saint-Denis
CountryName=France
...
```

Au lieu des noms sources (`MP_TYPE`, `NAME`) qui seraient ignorés sans le mapping.

## Rapport JSON (CI/CD)

### Génération du rapport

```bash
mpforge build --config config.yaml --report report.json
```

### Schéma JSON

```json
{
  "status": "success",           // "success" | "failure"
  "tiles_generated": 2047,        // Tuiles exportées avec succès
  "tiles_failed": 0,              // Tuiles en erreur
  "tiles_skipped": 150,           // Tuiles vides (pas de features)
  "features_processed": 1234567,  // Nombre total de features exportées
  "duration_seconds": 1845.3,     // Durée totale (float)
  "errors": []                    // Liste des erreurs (vide si succès)
}
```

### Codes de sortie

- **Exit code 0** : Succès (toutes les tuiles exportées)
- **Exit code 1** : Échec (une ou plusieurs tuiles en erreur)

### Intégration CI/CD

```bash
#!/bin/bash
# Pipeline de production avec validation

mpforge build --config bdtopo.yaml --jobs 8 --report report.json

# Vérifier le code de sortie
if [ $? -eq 0 ]; then
  echo "✅ Pipeline réussi"

  # Extraire les statistiques
  tiles=$(jq '.tiles_generated' report.json)
  features=$(jq '.features_processed' report.json)
  duration=$(jq '.duration_seconds' report.json)

  echo "📊 $tiles tuiles générées, $features features traitées en ${duration}s"

  # Archiver le rapport
  cp report.json /archive/$(date +%Y%m%d)-report.json
else
  echo "❌ Pipeline échoué"

  # Afficher les erreurs
  echo "Erreurs détectées:"
  jq '.errors[] | "  - Tuile \(.tile): \(.error)"' report.json

  exit 1
fi
```

### Analyse avec jq

```bash
# Statut du pipeline
jq '.status' report.json

# Nombre d'erreurs
jq '.tiles_failed' report.json

# Taux de réussite
jq '(.tiles_generated / (.tiles_generated + .tiles_failed) * 100) | floor' report.json

# Liste des tuiles en erreur
jq '.errors[].tile' report.json

# Détail d'une erreur spécifique
jq '.errors[] | select(.tile == "12_45")' report.json
```

## Validation de configuration

Le fichier YAML est validé au chargement avec messages d'erreur clairs :

```bash
# cell_size négatif
❌ Config validation failed: grid.cell_size must be positive, got: -0.15

# overlap négatif
❌ Config validation failed: grid.overlap must be non-negative, got: -0.01

# Pas d'inputs
❌ Config validation failed: At least one input source is required

# Bounding box invalide
❌ Config validation failed: Invalid bbox: min_lon (10.0) must be < max_lon (-5.0)

# error_handling invalide
❌ Config validation failed: error_handling must be "continue" or "fail-fast", got: "stop"

# spatial_filter.buffer négatif
❌ Config validation failed: InputSource #0: spatial_filter.buffer must not be negative, got -100

# spatial_filter.source vide
❌ Config validation failed: InputSource #0: spatial_filter.source must not be empty

# generalize.iterations à zéro
❌ Config validation failed: InputSource #0: generalize.iterations must be >= 1, got 0

# label_case invalide dans les règles
❌ Rules file error: unknown variant `MAJUSCULE`, expected one of `none`, `upper`, `lower`, `title`, `capitalize`
```

La sous-commande `mpforge validate` effectue 9 checks sans exécuter le pipeline :

| # | Check | Description |
|---|-------|-------------|
| 1 | `yaml_syntax` | Syntaxe YAML valide et types corrects |
| 2 | `semantic_validation` | Règles métier (grille, inputs, bbox, SRS, spatial_filter, generalize) |
| 3 | `input_files` | Existence des fichiers sources (après résolution des wildcards) |
| 4 | `rules_file` | Parsing et validation du fichier de règles |
| 5 | `field_mapping` | Parsing du fichier de field mapping |
| 6 | `header_template` | Existence du template header |
| 7 | `spatial_filter` | Existence des fichiers source de filtrage spatial |
| 8 | `generalize` | Rapport des configs de généralisation (smooth, iterations, simplify) |
| 9 | `label_case` | Cohérence label_case dans les règles (warning si aucune règle ne set Label) |

## Gestion d'erreurs

### Mode `continue` (défaut)

Continue le traitement même en cas d'erreur sur une tuile :

```yaml
error_handling: "continue"
```

```
⚠️  Tile 12_45 failed: GDAL error: Invalid geometry
✅ Processing continues with remaining tiles...
```

### Mode `fail-fast`

Arrête immédiatement à la première erreur :

```yaml
error_handling: "fail-fast"
```

```
❌ Tile 12_45 failed: GDAL error: Invalid geometry
💥 Stopping immediately (fail-fast mode)
```

Ou via CLI :

```bash
mpforge build --config config.yaml --fail-fast
```

## Formats supportés

| Format | Type | Exemple |
|--------|------|---------|
| **ESRI Shapefile** | Fichier | `data/routes.shp` |
| **GeoPackage** | Fichier | `data/bdtopo.gpkg` |
| **PostGIS** | Base de données | `PG:host=localhost dbname=gis` |
| **GeoJSON** | Fichier | `data/features.geojson` |
| **KML/KMZ** | Fichier | `data/map.kml` |

**Note** : Tous les formats supportés par GDAL/OGR sont compatibles.

### Support OSM PBF

mpforge peut lire directement les fichiers `.osm.pbf` de Geofabrik via le driver GDAL OSM. Les données OSM sont en EPSG:4326 natif — aucune reprojection n'est nécessaire.

**Prérequis** : un fichier `osmconf.ini` personnalisé est nécessaire pour exposer les tags OSM souhaités (`amenity`, `shop`, `tourism`, `natural`) comme attributs GDAL directs, au lieu de les regrouper dans `other_tags`.

```bash
# Activer le fichier de configuration OSM personnalisé
export OSM_CONFIG_FILE=pipeline/configs/osm/osmconf.ini

# Augmenter le buffer du driver OSM (défaut 100 Mo, insuffisant pour les gros PBF)
# Le driver lit le PBF séquentiellement et bufferise les couches non demandées.
# 1024 Mo couvre les PBF régionaux Geofabrik (~500 Mo pour Rhône-Alpes).
export OSM_MAX_TMPFILE_SIZE=1024

# Accepter les géométries OSM avec des anneaux non fermés (supprime les warnings GDAL)
export OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES
```

**Limitation** : seules les couches `points` et `lines` sont utilisées. La couche `multipolygons` n'est pas supportée car le driver GDAL OSM accumule les features des autres couches en mémoire lors de la lecture séquentielle, provoquant des erreurs "Too many features accumulated". Les gros POIs mappés comme polygones dans OSM (supermarchés, hôpitaux) ne sont donc pas capturés.

**Exemple de configuration YAML** :

```yaml
inputs:
  # Amenity POIs (restaurants, pharmacies, parking, etc.)
  - path: "${OSM_DATA_ROOT}/**/*.osm.pbf"
    layers: ["points"]
    layer_alias: "osm_amenity"
    attribute_filter: "amenity IS NOT NULL"
    spatial_filter:
      source: "${DATA_ROOT}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

**Conventions de `layer_alias`** :

| Alias | Couche GDAL | Filtre | Contenu |
|-------|-------------|--------|---------|
| `osm_amenity` | `points` | `amenity IS NOT NULL` | Restaurants, pharmacies, parking... |
| `osm_shop` | `points` | `shop IS NOT NULL` | Boulangeries, supermarchés... |
| `osm_natural_lines` | `lines` | `natural IN ('ridge','arete','cliff')` | Arêtes, crêtes, falaises |
| `osm_natural_points` | `points` | `natural IN ('cave_entrance','cave','rock','sinkhole')` | Grottes, rochers, dolines |
| `osm_tourism` | `points` | `tourism = 'viewpoint'` | Points de vue |

Le chemin glob `**/*.osm.pbf` permet d'intégrer automatiquement tous les fichiers PBF d'un dossier (multi-régions). Le `spatial_filter` est fortement recommandé car les PBF Geofabrik couvrent des régions entières.

## Types géométriques supportés

mpforge ne traite que les types géométriques simples compatibles avec le format Polish Map (.mp) :

| Type OGR | Supporté | Notes |
|----------|----------|-------|
| **Point** (wkbPoint) | ✅ Oui | POI, sommets, repères |
| **LineString** (wkbLineString) | ✅ Oui | Routes, rivières, sentiers |
| **Polygon** (wkbPolygon) | ✅ Oui | Bâtiments, zones, limites |
| MultiPoint | ❌ Non | Décomposer avant import |
| MultiLineString | ❌ Non | Décomposer avant import |
| MultiPolygon | ❌ Non | Décomposer avant import |
| GeometryCollection | ❌ Non | Décomposer avant import |

Les features de types non supportés sont **silencieusement filtrées** à la lecture. Un message INFO résumé est affiché en fin de lecture avec le décompte par type et les sources affectées. Le rapport JSON (`--report`) inclut une section `quality.unsupported_types` avec le détail.

### Workarounds : Pré-traitement avec ogr2ogr

Si vos données source contiennent des types Multi* ou GeometryCollection, vous pouvez les décomposer en géométries simples avant import :

```bash
# Décomposer MultiPolygon → Polygon (et autres Multi* → Simple)
ogr2ogr -f "ESRI Shapefile" output.shp input.shp -explodecollections

# Forcer conversion en type simple spécifique (alternative)
ogr2ogr -f "ESRI Shapefile" output.shp input.shp -nlt POLYGON

# Vérifier les types géométriques d'un fichier
ogrinfo -al -so input.shp | grep "Geometry:"
```

**Exemple de pré-validation :**

```bash
# Vérifier combien de features Multi* existent
ogrinfo -sql "SELECT COUNT(*) FROM my_layer WHERE OGR_GEOMETRY NOT LIKE 'POINT%' AND OGR_GEOMETRY NOT LIKE 'LINESTRING%' AND OGR_GEOMETRY NOT LIKE 'POLYGON%'" input.gpkg
```

## Développement

### Structure du projet

```
mpforge/
├── src/
│   ├── main.rs              # Point d'entrée CLI
│   ├── cli.rs               # Définition des arguments CLI (clap)
│   ├── config.rs            # Parsing YAML et validation
│   ├── error.rs             # Types d'erreurs
│   ├── proj_init.rs         # Initialisation PROJ embarqué
│   ├── report.rs            # Rapport JSON d'exécution
│   └── pipeline/
│       ├── mod.rs            # Orchestration du pipeline tile-centric
│       ├── reader.rs         # Lecture sources GDAL/OGR
│       ├── tiler.rs          # Grille spatiale et découpage
│       ├── tile_naming.rs    # Patterns de nommage des tuiles
│       ├── geometry_validator.rs  # Validation/réparation géométries
│       └── writer.rs         # Export Polish Map (.mp)
├── tests/
│   ├── cli_tests.rs          # Tests CLI (help, version, flags)
│   ├── config_parsing.rs     # Tests de parsing config YAML
│   ├── tile_naming.rs        # Tests d'intégration patterns de nommage
│   └── integration/
│       └── fixtures/          # Fichiers de test (configs, shapefiles)
├── examples/
│   ├── simple.yaml
│   ├── simple-with-mapping.yaml
│   ├── bdtopo.yaml
│   ├── france-nord-bdtopo.yaml
│   └── france-nord-simple.yaml
├── doc/
│   └── config-schema.md
└── Cargo.toml
```

### Tests

```bash
# Tests unitaires et d'intégration
cargo test

# Tests avec logs
cargo test -- --nocapture

# Test d'un module spécifique
cargo test config::tests

# Benchmarks (si disponibles)
cargo bench
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

### Build de développement

```bash
# Debug (rapide, non optimisé)
cargo build

# Release (lent, optimisé)
cargo build --release

# Avec symboles de debug
cargo build --profile release-with-debug
```

## Workflows de développement

Ce projet utilise la méthodologie **Build-Measure-Adapt-Deliver (BMAD)** :

- **Epics & Stories** : Voir `/_bmad/bmm/epics-and-stories/`
- **Sprint Status** : `/_bmad/bmm/sprint-status.yaml`
- **Documentation** : Voir ce README

### Historique des versions

- **Epic 5** : Configuration YAML multi-sources
- **Epic 6** : Tuilage spatial et export Polish Map
- **Epic 7** : Parallélisation, progress bar, rapports JSON

## Licence

Ce projet fait partie de **Garmin IMG Forge** et est distribué sous licence MIT. Voir le fichier [LICENSE](../LICENSE) à la racine du dépôt.

## Contribution

Les contributions sont les bienvenues ! Voir le workflow BMAD dans `/_bmad/` pour comprendre le processus de développement.

### Créer une issue

1. Vérifier qu'une issue similaire n'existe pas déjà
2. Utiliser les templates d'issue (bug, feature request)
3. Fournir un exemple de configuration reproductible

### Soumettre une PR

1. Fork le projet
2. Créer une branche feature (`git checkout -b feature/ma-feature`)
3. Commits avec messages clairs
4. Tests unitaires et d'intégration
5. Ouvrir une Pull Request avec description détaillée

## Support

- **Documentation** : Voir la section [Configuration détaillée](#configuration-détaillée) ci-dessus
- **Exemples** : Voir [`examples/`](examples/)
- **Issues** : https://forgejo.allfabox.fr/allfab/garmin-img-forge/issues

## Auteurs

Développé dans le cadre du projet **Garmin IMG Forge** pour générer des cartes Garmin à partir de données SIG massives.
