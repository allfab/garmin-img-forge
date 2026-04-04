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
    --family-name "BDTOPO France" \
    --series-name "IGN BDTOPO 2026" \
    --copyright-message "IGN BDTOPO 2026" \
    --latin1 \
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
wget https://forgejo.allfabox.fr/allfab/garmin-ign-bdtopo-map/releases/download/imgforge-v0.1.0/imgforge
chmod +x imgforge
sudo mv imgforge /usr/local/bin/
imgforge --version
```

### Compilation depuis les sources

```bash
# Prérequis : Rust 1.70+ (pas besoin de GDAL !)
cd tools/imgforge
cargo build --release
```

!!! success "Zéro dépendance"
    imgforge est un binaire Rust pur — il ne dépend ni de GDAL, ni de Java, ni d'aucune bibliothèque système. C'est un des avantages majeurs par rapport à mkgmap.
