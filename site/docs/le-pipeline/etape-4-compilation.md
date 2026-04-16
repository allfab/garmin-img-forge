# Étape 4 : Compilation (imgforge)

C'est l'étape finale du pipeline logiciel : `imgforge` compile toutes les tuiles Polish Map en un seul fichier `gmapsupp.img` prêt pour le GPS.

---

## Via le script de build (recommandé)

Si vous utilisez `build-garmin-map.sh`, la compilation est automatique (étape 2/2). Le script passe tous les paramètres imgforge :

```bash
# Tout-en-un : mpforge + imgforge
./scripts/build-garmin-map.sh --zones D038 --jobs 4

# Personnaliser imgforge via le script
./scripts/build-garmin-map.sh --zones D038,D069 \
    --family-id 1100 --series-name "IGN-BDTOPO-MAP" \
    --levels "24,22,20,18,16" --no-dem

# Pointer vers un répertoire DEM personnalisé
./scripts/build-garmin-map.sh --zones D038 \
    --dem-dir ./pipeline/data/bdaltiv2 --jobs 4
```

Le script gère automatiquement le DEM multi-zones : pour chaque zone dans `--zones`, il passe un `--dem {dem-dir}/{zone}` à imgforge. Si le répertoire DEM d'une zone n'existe pas, un warning est affiché et la zone est ignorée (la compilation continue sans DEM pour cette zone).

Voir l'[étape 3 (tuilage)](etape-3-tuilage.md#options-de-build-garmin-mapsh) pour la référence complète des options de `build-garmin-map.sh`.

## Commande imgforge directe

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

### Cas 1 — Département (scope petit, imgforge direct)

```bash
imgforge build ./pipeline/output/2025/v2025.12/D038/mp/ \
    --output ./pipeline/output/2025/v2025.12/D038/img/gmapsupp.img \
    --jobs 4 \
    --family-id 1100 --product-id 1 \
    --family-name "IGN-BDTOPO-D038-v2025.12" \
    --series-name "IGN-BDTOPO-MAP" \
    --code-page 1252 --lower-case \
    --levels "24,22,20,18,16" \
    --route \
    --typ-file pipeline/resources/typfiles/I2023100.typ \
    --copyright-message "©2026 Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap" \
    --dem ./pipeline/data/dem/D038/ \
    --dem-source-srs EPSG:2154 \
    --keep-going
```

Décortiquons les options utilisées dans ce cas 1 — chaque groupe est détaillé plus bas dans la page ([Identité](#identite-de-la-carte), [Encodage](#encodage), [Optimisation géométrique](#optimisation-geometrique), [Symbologie](#symbologie), [DEM / Hill Shading](#dem-hill-shading)).

### Cas 2 — Quadrant FRANCE-SE (scope quadrant, 25 départements, via `build-garmin-map.sh`)

Pour les gros scopes, c'est le script wrapper qui pilote les deux phases (download + mpforge + imgforge + publication). Exemple validé sur Alpha 100 le 16 avril 2026 :

```bash
# 1. Télécharger les données (SHP + contours + OSM + DEM)
./scripts/download-bdtopo.sh \
    --region FRANCE-SE \
    --bdtopo-version v2026.03 \
    --format SHP \
    --with-contours --with-osm --with-dem

# 2. Build + publication
./scripts/build-garmin-map.sh \
    --region FRANCE-SE \
    --base-id 940 \
    --year 2026 \
    --version v2026.03 \
    --data-dir ./pipeline/data \
    --contours-dir ./pipeline/data/contours \
    --dem-dir ./pipeline/data/dem \
    --osm-dir ./pipeline/data/osm \
    --hiking-trails-dir ./pipeline/data/hiking-trails \
    --output-base ./pipeline/output \
    --mpforge-jobs 4 \
    --imgforge-jobs 2 \
    --family-id 940 --product-id 1 \
    --family-name "IGN-BDTOPO-FRANCE-SE-v2026.03" \
    --series-name "IGN-BDTOPO-MAP" \
    --code-page 1252 \
    --levels "24,22,20,18,16" \
    --reduce-point-density 4.0 \
    --simplify-polygons "24:12,18:10,16:8" \
    --min-size-polygon 8 \
    --merge-lines \
    --typ pipeline/resources/typfiles/I2023100.typ \
    --copyright "©2026 Allfab Studio - ©IGN BDTOPO - ©OpenStreetMap Les Contributeurs - Licence Ouverte Etalab 2.0" \
    --skip-existing \
    --publish \
    --publish-target local
```

!!! tip "Ce qui change par rapport au cas département"
    - **Config auto-résolue** : `build-garmin-map.sh` détecte le quadrant (`--region FRANCE-SE/SO/NE/NO`) et charge `pipeline/configs/ign-bdtopo/france-quadrant/sources.yaml` avec un `cell_size: 0.45°` adapté ([voir stratégie cell_size](etape-3-tuilage.md#strategie-cell_size-par-scope)). Ajoutez `--config <chemin>` pour forcer un fichier custom.
    - **`--mpforge-jobs 4 --imgforge-jobs 2`** : phase 1 tuilage avec 4 workers, phase 2 compilation avec 2 workers pour éviter l'OOM killer sur les zones très denses (Marseille/Nice/Lyon).
    - **`--reduce-point-density 4.0 --simplify-polygons "24:12,18:10,16:8" --min-size-polygon 8`** : simplification géométrique alignée sur les défauts mkgmap ; indispensable dès qu'on dépasse quelques départements.
    - **`--merge-lines`** : fusion des polylignes adjacentes (par défaut dans mkgmap). Réduit significativement la taille IMG et le pic mémoire imgforge.
    - **`--skip-existing`** : les tuiles `.mp` déjà générées sont réutilisées. Bonus : si le `.img` cible existe déjà, la phase 2 imgforge est elle aussi skippée — utile pour republier sans rebuilder.

!!! warning "Données `--hiking-trails-dir`"
    Le script `download-bdtopo.sh` ne télécharge pas automatiquement les sentiers GR ; le flag `--hiking-trails-dir` de `build-garmin-map.sh` pointe vers un répertoire optionnel qui peut être vide. Si vous ne disposez pas de données trails, omettez ce flag ou laissez-le pointer vers un répertoire vide — la config `france-quadrant/sources.yaml` gère l'absence sans erreur.

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
--reduce-point-density 4.0    # Simplification Douglas-Peucker (défaut mkgmap)
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
--dem ./pipeline/data/dem/D038/                    # BDAltiv2 (ASC, Lambert 93)
--dem-source-srs EPSG:2154

# Multi-zones : un --dem par département
--dem ./pipeline/data/dem/D038/ --dem ./pipeline/data/dem/D069/ --dem-source-srs EPSG:2154
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
imgforge build ./pipeline/output/2025/v2025.12/D038/mp/ \
    --output ./pipeline/output/2025/v2025.12/D038/img/gmapsupp.img \
    --jobs 4 \
    --dem ./pipeline/data/dem/D038/ \
    --dem-source-srs EPSG:2154 \
    --dem-dists 3,3,4,6,8,12,16,24,32 \
    --dem-interpolation bicubic \
    --code-page 1252 --lower-case \
    --levels "24,22,20,18,16" \
    --typ-file pipeline/resources/typfiles/I2023100.typ \
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
