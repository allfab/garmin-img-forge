# Étape 2 : Configuration

Avant de lancer le tuilage, il faut préparer trois fichiers de configuration qui décrivent **quoi** traiter, **comment** mapper les champs, et **quelles** métadonnées embarquer dans la carte.

---

## Architecture des fichiers de configuration

```
configs/
├── france-bdtopo.yaml         ← Configuration principale (sources, grille, output)
├── bdtopo-mapping.yaml        ← Field mapping (champs sources → Polish Map)
└── header_template.mp         ← Template du header Polish Map
```

Ces trois fichiers fonctionnent ensemble mais sont séparés pour permettre la réutilisation. Le même mapping peut servir à plusieurs configurations (France Nord, France Sud, une région...).

## 1. Configuration principale (YAML)

C'est le fichier central qui pilote `mpforge` :

```yaml
# sources.yaml
version: 1

# --- Grille de tuilage ---
grid:
  cell_size: 0.15        # Taille de cellule en degrés (~16.5 km)
  overlap: 0.005         # Léger chevauchement pour éviter les artefacts aux bords

# --- Sources de données ---
inputs:
  # BDTOPO Shapefiles — multi-zones via brace expansion
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  - path: "${DATA_ROOT}/{${ZONES}}/HYDROGRAPHIE/SURFACE_HYDROGRAPHIQUE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  # Courbes de niveau — wildcards + brace expansion
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

  # OSM POIs — données régionales, filtrées sur les communes des zones sélectionnées
  - path: "${OSM_DATA_ROOT}/gpkg/*-amenity-points.gpkg"
    layer_alias: "osm_amenity"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

# --- Sortie ---
output:
  directory: "${OUTPUT_DIR}/mp/"
  filename_pattern: "BDTOPO-{col:03}-{row:03}.mp"
  overwrite: true
  base_id: ${BASE_ID}

# --- Header Polish Map ---
header:
  name: "BDTOPO-{col:03}-{row:03}"
  copyright: "2026 Allfab Studio - IGN BDTOPO 2025"
  levels: "5"
  level0: "24"
  level1: "22"
  level2: "20"
  level3: "18"
  level4: "16"
  routing: "Y"

# Règles de transformation BDTOPO → types Garmin
rules: pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml

# --- Comportement en cas d'erreur ---
error_handling: "continue"
```

### Variables d'environnement

Tous les champs du YAML acceptent la syntaxe `${VAR}` pour injecter des variables d'environnement. Les variables sont substituées **avant** le parsing YAML, ce qui fonctionne aussi pour les champs numériques :

```yaml
inputs:
  - path: "${DATA_ROOT}/TRANSPORT/TRONCON_DE_ROUTE.shp"
  - path: "${CONTOURS_DATA_ROOT}/**/COURBE_*.shp"

output:
  directory: "${OUTPUT_DIR}/tiles/"
  base_id: ${BASE_ID}      # u32 — la variable doit contenir un nombre
```

```bash
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12
export CONTOURS_DATA_ROOT=./pipeline/data/contours
export OSM_DATA_ROOT=./pipeline/data/osm
export HIKING_TRAILS_DATA_ROOT=./pipeline/data/hiking-trails
export OUTPUT_DIR=./pipeline/output/2025/v2025.12/D038
export BASE_ID=38
export ZONES=D038

mpforge build --config config.yaml --jobs 8
```

!!! tip "Validation des variables"
    Utilisez `mpforge validate` pour vérifier que toutes les variables sont bien définies avant de lancer un long export. Les variables non résolues sont signalées par un warning :
    ```
    ⚠ Unresolved environment variable: ${DATA_ROOT} (not set)
    ```

Seuls les noms POSIX valides sont reconnus : lettres, chiffres et underscores, commençant par une lettre ou un underscore (ex: `DATA_ROOT`, `_MY_VAR`). Les patterns comme `${123}` ou `${foo bar}` sont ignorés.

### Brace expansion (multi-zones)

En plus des wildcards classiques (`*`, `?`, `**`), mpforge supporte la **brace expansion** dans les chemins de fichiers. Cela permet de cibler plusieurs sous-dossiers sans matcher tout le contenu d'un répertoire :

```yaml
inputs:
  # Un seul département
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
  # Avec ZONES=D038 → résolu en : data/.../D038/TRANSPORT/TRONCON_DE_ROUTE.shp

  # Multi-départements
  # Avec ZONES=D038,D069 → résolu en 2 entrées :
  #   data/.../D038/TRANSPORT/TRONCON_DE_ROUTE.shp
  #   data/.../D069/TRANSPORT/TRONCON_DE_ROUTE.shp
```

Le fichier de configuration `sources.yaml` du projet utilise cette syntaxe pour toutes les couches BDTOPO. Le script `build-garmin-map.sh` se charge de définir les variables `ZONES`, `DATA_ROOT`, etc. automatiquement depuis ses paramètres CLI.

La brace expansion fonctionne aussi dans `spatial_filter.source` : les géométries de tous les fichiers matchés sont automatiquement unies en un seul filtre spatial.

### Paramètres de la grille

| Paramètre | Description | Valeur recommandée |
|-----------|-------------|-------------------|
| `cell_size` | Taille de chaque tuile en degrés | `0.15` (~16.5 km) |
| `overlap` | Chevauchement entre tuiles adjacentes | `0.01` (~1.1 km) |
| `origin` | Coin sud-ouest de la grille | `[-5.0, 41.0]` pour la France |

!!! tip "Choisir la taille de cellule"
    - **0.10** : Petites tuiles, plus de fichiers, adapté aux zones denses (Île-de-France)
    - **0.15** : Bon compromis pour la France entière (~2000 tuiles)
    - **0.25** : Grandes tuiles, moins de fichiers, adapté aux zones rurales

### Patterns de nommage des tuiles

| Pattern | Résultat (col=15, row=42) | Description |
|---------|---------------------------|-------------|
| `{col}_{row}.mp` | `15_42.mp` | Simple |
| `{col:03}_{row:03}.mp` | `015_042.mp` | Zero-padded |
| `{seq:04}.mp` | `0157.mp` | Séquentiel |
| `tile_{col}_{row}.mp` | `tile_15_42.mp` | Préfixe personnalisé |

### Généralisation de géométrie

Pour certaines couches, les géométries brutes (polygones anguleux, polylignes en escalier) gagnent à être lissées avant export. mpforge propose une directive `generalize` par source qui reproduit les transformations FME type Generalizer (McMaster).

```yaml
inputs:
  - path: "${DATA_ROOT}/LIEUX_NOMMES/ZONE_D_HABITATION.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    generalize:
      smooth: "chaikin"       # Algorithme : Chaikin corner-cutting
      iterations: 2           # Nombre de passes (chaque passe double les vertices)
      simplify: 0.00005       # Douglas-Peucker après lissage (degrés WGS84, optionnel)
```

| Paramètre | Type | Défaut | Description |
|-----------|------|--------|-------------|
| `smooth` | string | — | Algorithme de lissage. Seul `"chaikin"` est disponible actuellement |
| `iterations` | entier | 1 | Nombre de passes de lissage |
| `simplify` | flottant | — | Tolérance Douglas-Peucker post-lissage (en degrés WGS84) |

!!! tip "Équivalence FME"
    Le **Chaikin corner-cutting** avec `iterations: 2` produit un résultat visuel proche du **McMaster sliding average** de FME (voisins=2, offset=25%). Combinez avec `simplify` pour éviter l'explosion du nombre de vertices.

!!! note "Pipeline"
    La généralisation s'applique **après** le clipping sur les tuiles et **avant** l'export en Polish Map. Les points (POI) ne sont pas affectés.

### Profils multi-niveaux

Le `generalize:` inline ci-dessus produit **une seule** géométrie simplifiée (`Data0=`). Pour des cartes plus riches, `mpforge` accepte un **catalogue externe** qui déclare des profils **multi-niveaux** : chaque feature porte plusieurs géométries, de la plus détaillée à la plus grossière, consommées par `imgforge` selon le zoom.

Activation : une ligne à la racine de `sources.yaml` pointant vers un fichier YAML adjacent :

```yaml
generalize_profiles_path: "../generalize-profiles.yaml"
```

Contenu du catalogue — exemple BDTOPO (`pipeline/configs/ign-bdtopo/generalize-profiles.yaml`) :

```yaml
profiles:
  # BATIMENT : volontairement absent → émis en Data0 seul, raw.
  # Préserve les bâtiments tels que livrés par BD TOPO.

  TRONCON_HYDROGRAPHIQUE:
    levels:
      - { n: 0, simplify: 0.00005 }   # ~5 m : cours d'eau détaillés
      - { n: 2, simplify: 0.00020 }   # ~22 m : zoom moyen

  TRONCON_DE_ROUTE:
    # Dispatch conditionnel par attribut : premier match gagne.
    when:
      - field: CL_ADMIN
        values: [Autoroute, Nationale]
        levels:
          - { n: 0, simplify: 0.00002 }   # ~2 m : préservation routing max
          - { n: 2, simplify: 0.00008 }
      - field: CL_ADMIN
        values: [Chemin, Sentier]
        levels:
          - { n: 0, simplify: 0.00010 }
          - { n: 2, simplify: 0.00030 }
    levels:                               # fallback si aucun when ne matche
      - { n: 0, simplify: 0.00005 }
      - { n: 2, simplify: 0.00015 }
```

**Sémantique** :

| Clé | Rôle |
|---|---|
| `n` | index du niveau dans `MpHeader.levels` (`0` = le plus détaillé = `Data0=`, `2` = `Data2=`, etc.) |
| `smooth` | `"chaikin"` ou absent (optionnel) |
| `iterations` | itérations Chaikin, borne `[0, 5]` |
| `simplify` | tolérance Douglas-Peucker en degrés WGS84, borne `[0, 0.001]` (≈ 110 m) |
| `when` | dispatch par attribut (premier match gagne) ; les `when.levels` remplacent les `levels` default |

**Contraintes fail-fast au `load_config`** :

- Toute couche routable (`TRONCON_DE_ROUTE`) **doit** déclarer `n: 0` dans chaque branche visible (default ET chaque `when`). Sans ça, le routing côté `imgforge` casse (pas de `Data0=` = pas d'arc NET/NOD).
- Un même `source_layer` ne peut pas apparaître à la fois en `generalize:` inline **et** dans le catalogue externe → conflit rejeté.
- `max(n)` sur tous les profils doit être `< header.levels.len()` (sinon `imgforge` drop silencieusement les `DataN` hors de portée).
- `iterations` hors `[0, 5]` ou `simplify` hors `[0, 0.001]` → erreur explicite au chargement.

!!! tip "Opt-out strict"
    `mpforge build --disable-profiles` (ou env var `MPFORGE_PROFILES=off`) bypasse **uniquement** le catalogue externe. Les `generalize:` inline restent actifs.

!!! note "Pré-requis driver"
    Le writer multi-Data nécessite le driver `ogr-polishmap` à jour. Le script `build-garmin-map.sh` auto-détecte `~/.gdal/plugins/ogr_PolishMap.so` ou `tools/ogr-polishmap/build/ogr_PolishMap.so` et expose `GDAL_DRIVER_PATH` automatiquement. Si `mpforge` est lancé directement, vérifier que le plugin système est à jour (`ogrinfo --formats | grep Polish` pour valider).

## 2. Field mapping

Le field mapping traduit les noms de colonnes de vos données sources vers les champs standard du format Polish Map :

```yaml
# bdtopo-mapping.yaml
field_mapping:
  # Champs principaux
  MP_TYPE: Type          # Code type Garmin (ex: 0x4e00)
  NAME: Label            # Nom de la feature

  # Localisation
  Country: CountryName   # Pays (ex: "France~[0x1d]FRA")
  CityName: CityName     # Ville/commune
  Zip: Zip               # Code postal

  # Paramètres d'affichage
  MPBITLEVEL: Levels     # Niveaux de zoom (ex: "0-3")
  EndLevel: EndLevel     # Niveau max (0-9)
```

!!! warning "Où placer le field mapping"
    Le chemin du fichier de mapping va dans `output.field_mapping_path` (pas dans `inputs`). C'est une erreur fréquente.

### Champs Polish Map disponibles

| Catégorie | Champs |
|-----------|--------|
| **Core** | `Type`, `Label`, `EndLevel`, `Levels`, `Data0`-`Data9` (le champ `Label` peut être transformé via l'option [`label_case`](../le-projet/mpforge.md#formatage-de-casse-des-labels-label_case) dans les règles) |
| **Localisation** | `CityName`, `RegionName`, `CountryName`, `Zip` |
| **POI** | `SubType`, `Marine`, `City`, `StreetDesc`, `HouseNumber`, `PhoneNumber` |
| **Routing** | `DirIndicator`, `RouteParam` |

## 3. Header template

Le header définit les métadonnées communes à toutes les tuiles :

```
[IMG ID]
Name=BDTOPO France
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

### Niveaux de zoom

Les niveaux (`Level0` à `Level3`) contrôlent à quel zoom chaque objet est visible :

| Niveau | Bits | Zoom approximatif | Visible |
|--------|------|-------------------|---------|
| Level0 = 24 | 24 | Très détaillé (~50m) | Tout |
| Level1 = 21 | 21 | Détaillé (~500m) | Routes principales, plans d'eau |
| Level2 = 18 | 18 | Moyen (~5km) | Autoroutes, grandes villes |
| Level3 = 15 | 15 | Large (~50km) | Métropoles, frontières |

## Configuration alternative : tout en inline

Si vous ne voulez pas de fichiers séparés, le header peut être défini directement dans le YAML :

```yaml
header:
  name: "BDTOPO Réunion"
  id: "0"
  copyright: "IGN 2026"
  levels: "4"
  level0: "24"
  level1: "21"
  level2: "18"
  level3: "15"
  tree_size: "3000"
  rgn_limit: "1024"
  lbl_coding: "9"
```

!!! info "Précédence"
    Si `template` ET champs individuels sont spécifiés, le template prend le dessus.

## Configuration des sources OSM PBF

Pour intégrer les données OpenStreetMap, ajoutez des entrées PBF dans la section `inputs` avec `layers`, `layer_alias` et `attribute_filter` :

```yaml
inputs:
  # --- Sources BD TOPO (multi-zones via brace expansion) ---
  - path: "${DATA_ROOT}/{${ZONES}}/TRANSPORT/TRONCON_DE_ROUTE.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"

  # --- Sources OSM GPKG ---
  # Les PBF Geofabrik sont pré-convertis en GPKG par download-bdtopo.sh (--with-osm)
  # Le spatial_filter utilise les communes de TOUTES les zones sélectionnées

  # Amenity POIs (restaurants, pharmacies, parking, etc.)
  - path: "${OSM_DATA_ROOT}/gpkg/*-amenity-points.gpkg"
    layer_alias: "osm_amenity"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500

  # Shop POIs (boulangeries, supermarchés, etc.)
  - path: "${OSM_DATA_ROOT}/gpkg/*-shop-points.gpkg"
    layer_alias: "osm_shop"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

### Points clés

- **Chemin glob** : `**/*.osm.pbf` intègre automatiquement tous les fichiers PBF du dossier (multi-régions)
- **`layer_alias`** : route les features vers le bon ruleset dans les règles de catégorisation
- **`attribute_filter`** : filtre GDAL appliqué avant chargement en mémoire
- **`spatial_filter`** : restreint les features à l'emprise communale + buffer (recommandé car les PBF Geofabrik couvrent des régions entières)
- Les données OSM sont en EPSG:4326 natif — pas de `source_srs`/`target_srs` nécessaire
- Seules les couches `points` et `lines` sont supportées (pas `multipolygons` — limitation du driver GDAL OSM)
- Positionner `OSM_MAX_TMPFILE_SIZE=1024` pour éviter l'erreur "Too many features accumulated" sur les gros PBF
- Positionner `OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES` pour supprimer les warnings de géométries invalides
- Positionner `OSM_CONFIG_FILE=./pipeline/configs/osm/osmconf.ini` pour utiliser l'`osmconf.ini` personnalisé du projet : il expose les tags `amenity`, `shop`, `tourism`, `natural` comme attributs GDAL directs (au lieu de les regrouper dans `other_tags`), ce qui permet au moteur de règles mpforge de matcher dessus. Sans cette variable, les POI OSM (refuges, sources, sommets nommés, etc.) et les features linéaires `natural=ridge`/`cliff` restent invisibles dans la carte Garmin finale.

## Valider la configuration

Avant de lancer un tuilage qui peut durer plusieurs heures, vérifiez la configuration avec `mpforge validate` :

```bash
mpforge validate --config configs/france-bdtopo.yaml
```

Neuf vérifications sont effectuées en chaîne :

| # | Check | Ce qui est vérifié |
|---|-------|--------------------|
| 1 | `yaml_syntax` | Syntaxe YAML valide, types corrects (ex: `base_id` est bien un nombre) |
| 2 | `semantic_validation` | Règles métier : grille cohérente, inputs non vides, bbox valide, SRS, base_id dans 1..9999, filename pattern, spatial_filter (buffer ≥ 0, source non vide), generalize (iterations ≥ 1, simplify > 0, algorithme connu) |
| 3 | `input_files` | Existence de chaque fichier source sur disque (après résolution des wildcards) |
| 4 | `rules_file` | Parsing et validation du fichier de règles de catégorisation |
| 5 | `field_mapping` | Parsing du fichier de renommage de champs GDAL — **distinct de `garmin-rules.yaml`** : renomme les *clés* d'attributs bruts avant que les règles ne s'appliquent (ex: `NOM_COMMUN` → `NAME`). Utile quand la source de données change ses noms de colonnes entre millésimes. |
| 6 | `header_template` | Présence d'un fichier template header, ou valeurs directes dans la section `header:` |
| 7 | `spatial_filter` | Existence des fichiers source de filtrage spatial (regroupés par source unique) |
| 8 | `generalize` | Catalogue externe (`generalize_profiles_path`) et/ou directives inline par-input |
| 9 | `label_case` | Cohérence label_case dans les règles : warning si aucune règle du ruleset ne set `Label` |

Exemple de sortie (config BDTOPO D038 sans `field_mapping` ni template header) :

```
✓ yaml_syntax          — Parsed successfully
✓ semantic_validation  — All validations passed
✓ input_files          — 104 files found
✓ rules_file           — 28 rulesets, 351 rules total
- field_mapping        — Not configured (optional — renomme les clés d'attributs GDAL bruts avant l'application des règles garmin-rules.yaml)
✓ header_template      — Header configured (direct values, no template file)
✓ spatial_filter       — inputs #21-#103 (83): data/COMMUNE.shp (pattern)
✓ generalize           — catalog: ../generalize-profiles.yaml (8 profil(s), 84 niveau(x))
✓ label_case           — 20 ruleset(s): Voies ferrees: Title, Communes: Title, ...

Config valid. (7/10 checks passed)
```

Exemple avec `field_mapping` configuré :

```
✓ field_mapping        — 6 field mappings loaded
```

### Rapport JSON

Pour une intégration CI/CD, exportez le résultat en JSON :

```bash
mpforge validate --config configs/france-bdtopo.yaml --report validation.json
```

### Diagnostiquer les erreurs courantes

Les variables d'environnement non définies sont signalées :

```
  ⚠ Unresolved environment variable: ${DATA_ROOT} (not set)
```

Un champ avec un type incorrect produit une erreur explicite :

```
✗ yaml_syntax — YAML syntax error: output.base_id: invalid type: string "${BASE_ID}", expected u32
```

!!! tip "Workflow recommandé"
    1. Écrire/modifier la configuration
    2. `mpforge validate --config config.yaml` pour vérifier
    3. `mpforge build --config config.yaml --dry-run` pour prévisualiser les tuiles
    4. `mpforge build --config config.yaml --jobs 8` pour lancer la production

Code de sortie : `0` si la configuration est valide, `1` si invalide.
