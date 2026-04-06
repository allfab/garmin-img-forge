# Étape 4 : Compilation (imgforge)

C'est l'étape finale du pipeline logiciel : `imgforge` compile toutes les tuiles Polish Map en un seul fichier `gmapsupp.img` prêt pour le GPS.

---

## Commande de base

```bash
imgforge build output/tiles/ --output output/gmapsupp.img --jobs 8
```

imgforge va :

1. Scanner le répertoire pour trouver tous les fichiers `.mp`
2. Parser chaque fichier (header, POI, POLYLINE, POLYGON)
3. Compiler chaque tuile en parallèle (TRE, RGN, LBL, NET, NOD, DEM)
4. Assembler toutes les tuiles compilées en un seul `gmapsupp.img`
5. Générer le fichier TDB compagnon

## Commande de production complète

Pour une carte de qualité production avec toutes les options :

```bash
imgforge build output/tiles/ \
    --output output/gmapsupp.img \
    --jobs 8 \
    --family-id 1234 \
    --product-id 1 \
    --series-name "BDTOPO France" \
    --family-name "IGN BDTOPO" \
    --area-name "France métropolitaine" \
    --country-name "France" \
    --country-abbr "FRA" \
    --copyright-message "IGN BDTOPO 2026 - Licence Etalab 2.0" \
    --product-version 200 \
    --latin1 \
    --reduce-point-density 3.0 \
    --min-size-polygon 8 \
    --typ-file resources/bdtopo.typ \
    --dem ./data/srtm_hgt/ \
    --keep-going
```

Décortiquons chaque groupe d'options :

### Identité de la carte

```bash
--family-id 1234              # Identifiant unique de la famille
--product-id 1                # Identifiant du produit
--series-name "BDTOPO France" # Nom de la série (affiché dans BaseCamp)
--family-name "IGN BDTOPO"    # Nom de la famille
--area-name "France métro."   # Zone géographique couverte
```

Ces métadonnées sont écrites dans le fichier TDB et sont visibles dans les logiciels Garmin (BaseCamp, MapInstall).

### Encodage

```bash
--latin1                      # CP1252 : tous les accents français
# ou
--unicode                     # UTF-8 : tous les caractères Unicode
```

!!! tip "Pour la France"
    `--latin1` suffit et produit des fichiers plus compacts. Utilisez `--unicode` uniquement si vous intégrez des données multilingues.

### Optimisation géométrique

```bash
--reduce-point-density 3.0    # Simplification Douglas-Peucker
--min-size-polygon 8          # Filtrer les micro-polygones
```

Ces options réduisent significativement la taille du fichier final (parfois -30 à -50 %) en éliminant les détails invisibles sur un écran GPS.

### Symbologie

```bash
--typ-file resources/bdtopo.typ  # Personnaliser les couleurs et icônes
```

Le fichier TYP définit le rendu visuel : couleurs des routes, motifs de remplissage des forêts, icônes des POI...

### DEM / Hill Shading

```bash
--dem ./data/srtm_hgt/                  # Données SRTM (HGT)
# ou
--dem ./data/bdaltiv2/ --dem-source-srs EPSG:2154  # BDAltiv2 (ASC, Lambert 93)
```

Active l'ombrage du relief et les profils d'altitude sur les GPS compatibles.

#### Contrôler la résolution DEM avec `--dem-dists`

Le DEM peut représenter une part très importante de la taille du fichier final. Le paramètre `--dem-dists` contrôle la densité des points d'élévation encodés pour chaque niveau de zoom :

```bash
# Profil équilibré (recommandé) — bon compromis taille/qualité
--dem-dists 3,3,4,6,8,12,16,24,32

# Profil compact — fichier léger, suffisant pour la randonnée
--dem-dists 4,6,8,12,16,24,32

# Profil haute résolution — détail maximum, fichier volumineux
--dem-dists 1,1,2,3,4,6,8,12,16
```

Chaque valeur correspond à un niveau de zoom (dans l'ordre de `--levels`). Plus la valeur est grande, moins il y a de points d'élévation. Si vous fournissez moins de valeurs que de niveaux, les restants sont calculés en doublant la dernière valeur.

!!! warning "Impact sur la taille"
    Sans `--dem-dists`, imgforge utilise une densité élevée par défaut, ce qui peut produire des fichiers très volumineux (ex: 500+ Mo pour un seul département). **Spécifiez toujours ce paramètre en production.**

#### Interpolation

```bash
--dem-interpolation bilinear   # Rapide, 4 points (défaut via auto)
--dem-interpolation bicubic    # Lissé, 16 points (Catmull-Rom)
```

`bicubic` est recommandé avec des données haute résolution (BDAltiv2 25m) pour un relief plus lisse. `bilinear` suffit pour les données SRTM.

#### Exemple complet avec DEM optimisé

```bash
imgforge build output/tiles/ \
    --output output/gmapsupp.img \
    --jobs 8 \
    --dem ./data/bdaltiv2/D038/ \
    --dem-source-srs EPSG:2154 \
    --dem-dists 3,3,4,6,8,12,16,24,32 \
    --dem-interpolation bicubic \
    --latin1 \
    --levels "24,20,16" \
    --typ-file resources/bdtopo.typ \
    --keep-going \
    -vv
```

### Résilience

```bash
--keep-going                  # Continuer si une tuile échoue
```

En production, certaines tuiles peuvent contenir des géométries invalides. `--keep-going` les ignore et poursuit la compilation.

## Compiler une seule tuile (debug)

Pour tester ou déboguer, compilez une tuile isolée :

```bash
imgforge compile output/tiles/015_042.mp \
    --output test.img \
    --description "Tuile de test Chartreuse" \
    --latin1 \
    -vv
```

Le mode `-vv` (DEBUG) affiche les détails de l'encodage — utile pour diagnostiquer les problèmes.

## Rapport de compilation

La sortie standard d'imgforge est un rapport JSON :

```json
{
  "tiles_compiled": 2047,
  "total_points": 152340,
  "total_polylines": 87210,
  "total_polygons": 34560,
  "errors": [],
  "duration_ms": 234000,
  "output_file": "gmapsupp.img",
  "output_size_bytes": 524288000
}
```

## Niveaux de zoom

imgforge supporte la configuration des niveaux de zoom via `--levels` :

```bash
# Format simple : liste de résolutions (bits)
imgforge build tiles/ --levels "24,20,16"

# Format explicite : niveau:bits
imgforge build tiles/ --levels "0:24,1:20,2:16"
```

Si non spécifié, imgforge utilise les niveaux définis dans le header de chaque fichier `.mp`.

Chaque niveau crée un jeu de subdivisions contenant les features dont l'`EndLevel` est supérieur ou égal au numéro du niveau. Plus il y a de niveaux, plus le fichier est volumineux car les features sont dupliquées.

| Configuration | Niveaux | Taille relative |
|--------------|---------|-----------------|
| `"24,18"` | 2 | Référence |
| `"24,20,16"` | 3 | +30-50% |
| `"24,22,20,18,16"` | 5 | +100-150% |
| `"24,23,22,21,20,19,18,17,16"` | 9 | +200-400% |

!!! tip "Recommandation"
    **3 niveaux** avec des sauts de 4+ bits (`"24,20,16"`) offrent le meilleur compromis taille/navigation. Les niveaux consécutifs (24→23→22) n'apportent aucune différence visuelle perceptible sur un GPS Garmin.

    Voir la [référence complète sur les niveaux de zoom](../reference/niveaux-et-zoom.md) pour comprendre la correspondance avec `EndLevel`.

## Contrôle du routing

!!! danger "Routing expérimental"
    Le réseau routier est **routable à titre expérimental uniquement**. Les itinéraires calculés sont **indicatifs et non prescriptifs** — ne vous y fiez pas pour la navigation, quel que soit le mode de déplacement.

```bash
# Navigation turn-by-turn complète (NET + NOD)
imgforge build tiles/ --route

# Recherche d'adresse uniquement (NET seul, pas de navigation)
imgforge build tiles/ --net

# Carte de consultation uniquement (pas de routing)
imgforge build tiles/ --no-route
```

Par défaut, imgforge auto-détecte : si des routes avec `RouteParam` sont présentes, le routing complet est activé.

## Vérification du résultat

```bash
# Taille du fichier
ls -lh output/gmapsupp.img

# Vérifier avec mkgmap (optionnel, pour comparaison)
java -jar mkgmap.jar --check-roundabouts output/tiles/*.mp
```

Le fichier `gmapsupp.img` est maintenant prêt pour l'installation sur le GPS.
