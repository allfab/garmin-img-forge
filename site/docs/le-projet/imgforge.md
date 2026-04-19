# imgforge — Le Compilateur Garmin

## Le problème : un format binaire opaque

Le format **Garmin IMG** est un système de fichiers propriétaire contenant plusieurs sous-fichiers (TRE, RGN, LBL, NET, NOD, DEM...) encodés dans un format binaire non documenté publiquement. Jusqu'ici, deux outils savaient le produire :

- **cGPSmapper** — propriétaire, abandonné, Windows uniquement
- **mkgmap** — open-source mais écrit en Java, volumineux, lent sur les gros datasets

Mon objectif : un **compilateur Garmin IMG natif en Rust**, sans dépendance, capable de remplacer mkgmap tout en ajoutant des fonctionnalités modernes.

## La solution : imgforge

**imgforge** est un binaire Rust autonome qui compile des fichiers Polish Map (`.mp`) en fichiers Garmin IMG. Il génère l'intégralité des sous-fichiers nécessaires :

| Sous-fichier | Rôle |
|-------------|------|
| **TRE** | Index spatial, niveaux de zoom |
| **RGN** | Géométries (points, lignes, polygones) |
| **LBL** | Labels et encodage (ASCII, CP1252, UTF-8) |
| **NET** | Topologie du réseau routier |
| **NOD** | Noeuds de routage (turn-by-turn) |
| **DEM** | Données d'élévation (hill shading, profils altitude) |
| **TYP** | Symbologie personnalisée (couleurs, motifs, icônes) |
| **TDB** | Métadonnées de la carte |

## Deux modes d'utilisation

### `compile` : une tuile

```bash
# Compilation basique
imgforge compile tile_0_0.mp

# Avec options
imgforge compile tile_0_0.mp \
    --output ma_carte.img \
    --description "BDTOPO Réunion" \
    --latin1 \
    --reduce-point-density 5.0
```

### `build` : carte complète (gmapsupp)

```bash
# Assembler toutes les tuiles en un gmapsupp.img
imgforge build tiles/ \
    --output gmapsupp.img \
    --jobs 8 \
    --family-id 4136 \
    --product-id 1 \
    --family-name "BDTOPO France" \
    --series-name "IGN BDTOPO 2026" \
    --area-name "France métropolitaine" \
    --country-name "France" \
    --country-abbr "FRA" \
    --product-version 100 \
    --copyright-message "IGN BDTOPO 2026" \
    --latin1 \
    --levels "24,20,16" \
    --reduce-point-density 3.0 \
    --min-size-polygon 8 \
    --typ-file bdtopo.typ \
    --dem ./srtm_hgt/ \
    --keep-going
```

La commande `build` est le coeur du pipeline de production. Elle :

1. Scanne le répertoire pour trouver tous les fichiers `.mp`
2. Compile chaque tuile en parallèle (rayon, N workers)
3. Assemble les tuiles compilées en un seul `gmapsupp.img`
4. Génère le fichier TDB compagnon
5. Intègre optionnellement le fichier TYP et les données DEM

## Identité et métadonnées Garmin

La commande `build` accepte des options pour identifier la carte dans les logiciels Garmin (BaseCamp, MapInstall) :

| Option | Description | Défaut |
|--------|-------------|--------|
| `--family-id <N>` | Identifiant famille (unique par carte) | 1 |
| `--product-id <N>` | Identifiant produit | 1 |
| `--family-name <TEXT>` | Nom de la famille de cartes | `Map` |
| `--series-name <TEXT>` | Nom de la série (affiché dans BaseCamp) | `imgforge` |
| `--area-name <TEXT>` | Zone géographique couverte | - |
| `--country-name <TEXT>` | Nom du pays | - |
| `--country-abbr <TEXT>` | Abréviation pays (ex: `FRA`) | - |
| `--region-name <TEXT>` | Nom de la région | - |
| `--region-abbr <TEXT>` | Abréviation région | - |
| `--product-version <N>` | Version (100 = v1.00) | 100 |
| `--copyright-message <TEXT>` | Copyright intégré dans TRE et TDB | - |

## Niveaux de zoom

L'option `--levels` définit la résolution en bits de chaque niveau de zoom :

```bash
# Format simple (bits par niveau, du plus détaillé au plus large)
imgforge build tiles/ --levels "24,20,16"

# Format explicite (niveau:bits)
imgforge build tiles/ --levels "0:24,1:20,2:16"
```

Voir la [référence complète sur les niveaux de zoom](../reference/niveaux-et-zoom.md) pour le détail des correspondances EndLevel, l'impact sur la taille des fichiers et les recommandations.

Options de rendu supplémentaires :

| Option | Description | Défaut |
|--------|-------------|--------|
| `--transparent` | Carte overlay transparente | false |
| `--draw-priority <N>` | Priorité d'affichage (overlay) | 25 |
| `--order-by-decreasing-area` | Trier les polygones par aire décroissante | false |
| `--lower-case` | Autoriser les minuscules dans les labels (force Format 9/10) | false |

## Encodage des labels

Le format Garmin supporte trois encodages de labels, contrôlés par les options `--latin1`, `--unicode` ou `--code-page` :

| Format | Encodage | Caractères | Option |
|--------|----------|-----------|--------|
| Format 6 | ASCII 6 bits | A-Z, 0-9, espace | (défaut sans option) |
| Format 9 | CP1252/CP1250/CP1251 | Caractères accentués latins/cyrilliques | `--latin1` |
| Format 10 | UTF-8 | Tous les caractères Unicode | `--unicode` |

!!! tip "Recommandation"
    Pour les cartes françaises, utilisez `--latin1` (CP1252) qui couvre tous les caractères accentués français tout en restant compact. `--unicode` est utile pour les cartes multilingues.

## Optimisation géométrique

imgforge propose des options pour réduire la taille des fichiers et améliorer les performances d'affichage sur GPS :

### Simplification Douglas-Peucker

```bash
# Simplifier les lignes et polygones (seuil en map units)
imgforge build tiles/ --reduce-point-density 3.0
```

Réduit le nombre de points des géométries en éliminant les points qui ne contribuent pas significativement à la forme. Plus la valeur est élevée, plus la simplification est agressive.

### Filtrage des petits polygones

```bash
# Supprimer les polygones dont l'aire < 8 map units²
imgforge build tiles/ --min-size-polygon 8
```

Élimine les micro-polygones invisibles à l'échelle du GPS (petits bâtiments, fragments de végétation...).

### Simplification par résolution

En complément de `--reduce-point-density` (seuil global), `--simplify-polygons` permet un seuil Douglas-Peucker **différent par résolution** :

```bash
# Seuil DP adapté à chaque niveau de zoom (résolution:seuil)
imgforge build tiles/ --simplify-polygons "24:12,18:10,16:8"
```

Plus la résolution est basse (vue large), plus la simplification est agressive.

### Découpage automatique des features volumineuses

imgforge découpe automatiquement les features de plus de **250 points** pour éviter les débordements dans l'encodage RGN Garmin (delta variable-width) :

- **Polylignes** : découpées en segments de ≤250 points avec 1 point de recouvrement aux jointures
- **Polygones** : découpés par clipping Sutherland-Hodgman récursif le long de l'axe le plus long de la bounding box

Ce traitement est **transparent** — aucune option à configurer.

## Contrôle du routing

!!! danger "Routing expérimental"
    Le réseau routier est **routable à titre expérimental uniquement**. Les itinéraires calculés sont **indicatifs et non prescriptifs** — ne vous y fiez pas pour la navigation, quel que soit le mode de déplacement.

    Le réseau routable est actuellement **codé en dur** en fonction des données de la BD TOPO. La configuration dynamique basée sur les attributs routables de la source n'est pas encore supportée.

imgforge gère trois modes de routing :

| Mode | Option | Génère | Usage |
|------|--------|--------|-------|
| Complet | `--route` | NET + NOD | Navigation turn-by-turn |
| Recherche | `--net` | NET seul | Recherche d'adresse sans navigation |
| Désactivé | `--no-route` | Rien | Carte de consultation uniquement |

Par défaut, imgforge **auto-détecte** : si des routes avec `RouteParam` sont présentes dans les données, le routing complet est activé.

## DEM / Hill Shading

imgforge génère le sous-fichier DEM Garmin pour l'ombrage du relief et les profils d'altitude directement sur le GPS :

```bash
# Depuis des fichiers HGT (SRTM)
imgforge build tiles/ --dem ./srtm_hgt/

# Depuis des fichiers ASC (BDAltiv2 IGN, Lambert 93)
imgforge build tiles/ --dem ./bdaltiv2/ --dem-source-srs EPSG:2154

# Avec contrôle de la résolution DEM et interpolation bicubique
imgforge build tiles/ --dem ./bdaltiv2/ \
    --dem-source-srs EPSG:2154 \
    --dem-dists 3,3,4,6,8,12,16,24,32 \
    --dem-interpolation bicubic
```

### Formats d'élévation supportés

| Format | Extension | Source typique |
|--------|-----------|---------------|
| HGT | `.hgt` | SRTM 1/3 arc-sec (NASA) |
| ASC | `.asc` | ESRI ASCII Grid (BDAltiv2 IGN) |

La reprojection est intégrée via **proj4rs** (zéro dépendance système) : Lambert 93, UTM, LAEA, Web Mercator et toute chaîne proj4 sont supportés.

### Options DEM

| Option | Description | Défaut |
|--------|-------------|--------|
| `--dem <PATH,...>` | Répertoires ou fichiers d'élévation (`.hgt`, `.asc`) | - |
| `--dem-dists <DISTS>` | Distances entre points DEM par niveau de zoom | auto |
| `--dem-interpolation` | `auto`, `bilinear` ou `bicubic` | `auto` |
| `--dem-source-srs` | SRS source pour fichiers ASC (ex: `EPSG:2154`) | WGS84 |

### Contrôler la taille du fichier avec `--dem-dists`

Le paramètre `--dem-dists` est le **levier principal** pour maîtriser la taille du fichier généré. Il contrôle la densité des points d'élévation encodés pour chaque niveau de zoom. Plus la valeur est grande, moins il y a de points d'élévation dans le fichier final.

Chaque valeur correspond à un niveau de zoom (dans l'ordre de `--levels`). Si vous fournissez moins de valeurs que de niveaux de zoom, les niveaux restants sont calculés automatiquement en doublant la dernière valeur.

| Profil | `--dem-dists` | Résultat |
|--------|---------------|----------|
| Haute résolution | `1,1,2,3,4,6,8,12,16` | Fichier volumineux, détail maximum |
| Équilibré | `3,3,4,6,8,12,16,24,32` | Bon compromis taille/qualité |
| Compact | `4,6,8,12,16,24,32` | Fichier léger, suffisant pour la randonnée |

!!! warning "Impact sur la taille"
    Sans `--dem-dists`, imgforge utilise une densité élevée par défaut sur tous les niveaux de zoom, ce qui peut produire des fichiers très volumineux (ex: 500+ Mo pour un seul département). Spécifiez toujours ce paramètre en production.

### Interpolation

- **`bilinear`** — Utilise 4 points voisins. Rapide, adapté aux données basse résolution (SRTM 3 arc-sec).
- **`bicubic`** — Utilise 16 points (Catmull-Rom). Produit un relief plus lisse, idéal pour les données haute résolution (BDAltiv2 25m). Retombe automatiquement sur `bilinear` en bord de grille.
- **`auto`** — Bilinéaire par défaut (recommandé).

## Symbologie TYP

Un fichier `.typ` personnalise le rendu visuel de la carte sur le GPS (couleurs, motifs de remplissage, icônes) :

```bash
imgforge build tiles/ --typ-file bdtopo.typ
```

Le fichier TYP est intégré directement dans le `gmapsupp.img` final.

## Résilience

En production, certaines tuiles peuvent contenir des données problématiques. L'option `--keep-going` permet de continuer la compilation malgré les erreurs :

```bash
imgforge build tiles/ --jobs 8 --keep-going
```

Les tuiles en erreur sont journalisées (warning) mais n'empêchent pas la génération des autres tuiles.

## Installation

### Binaire pré-compilé

```bash
# Télécharger et extraire l'archive
wget https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.5.1/imgforge-linux-amd64.tar.gz
tar xzf imgforge-linux-amd64.tar.gz

chmod +x imgforge
sudo mv imgforge /usr/local/bin/
imgforge --version
```

!!! info "Comprendre la sortie `--version`"
    Les suffixes `-N-g<hash>` et `-dirty` ont un sens précis — voir la page [Versioning des binaires](../reference/versioning-binaires.md) pour la lecture complète de la version et le workflow de release.

### Compilation depuis les sources

```bash
# Prérequis : Rust 1.70+ (pas besoin de GDAL !)
cd tools/imgforge
cargo build --release
```

!!! success "Zéro dépendance"
    imgforge est un binaire Rust pur — il ne dépend ni de GDAL, ni de Java, ni d'aucune bibliothèque système. C'est un des avantages majeurs par rapport à mkgmap.
