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
# france-bdtopo.yaml
version: 1

# --- Grille de tuilage ---
grid:
  cell_size: 0.15        # Taille de cellule en degrés (~16.5 km)
  overlap: 0.01          # Chevauchement entre tuiles (évite les artefacts)
  origin: [-5.0, 41.0]   # Coin sud-ouest de la grille (optionnel)

# --- Sources de données ---
inputs:
  # Option A : Shapefiles (un fichier par couche)
  - path: "data/bdtopo/2026/v3.0/D038/TRANSPORT/TRONCON_DE_ROUTE.shp"
  - path: "data/bdtopo/2026/v3.0/D038/HYDROGRAPHIE/*.shp"

  # Option B : GeoPackage (toutes les couches dans un fichier)
  - path: "data/bdtopo/2026/v3.0/D038/BDTOPO.gpkg"
    layers:
      - "batiment"
      - "troncon_de_route"
      - "cours_d_eau"
      - "plan_d_eau"
      - "zone_vegetation"
      - "lieu_dit_non_habite"

  # Option C : Wildcards (tous les shapefiles d'un dossier)
  - path: "data/bdtopo/**/*.shp"

# --- Sortie ---
output:
  directory: "output/tiles/"
  filename_pattern: "{col:03}_{row:03}.mp"   # Zero-padded à 3 chiffres
  field_mapping_path: "configs/bdtopo-mapping.yaml"

# --- Header Polish Map ---
header:
  template: "configs/header_template.mp"

# --- Filtres optionnels ---
filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]  # France métropolitaine

# --- Comportement en cas d'erreur ---
error_handling: "continue"   # "continue" ou "fail-fast"
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
export DATA_ROOT=/data/bdtopo/2026/v3.0/D038
export CONTOURS_DATA_ROOT=/data/contours
export OUTPUT_DIR=/output
export BASE_ID=38

mpforge build --config config.yaml --jobs 8
```

!!! tip "Validation des variables"
    Utilisez `mpforge validate` pour vérifier que toutes les variables sont bien définies avant de lancer un long export. Les variables non résolues sont signalées par un warning :
    ```
    ⚠ Unresolved environment variable: ${DATA_ROOT} (not set)
    ```

Seuls les noms POSIX valides sont reconnus : lettres, chiffres et underscores, commençant par une lettre ou un underscore (ex: `DATA_ROOT`, `_MY_VAR`). Les patterns comme `${123}` ou `${foo bar}` sont ignorés.

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
| **Core** | `Type`, `Label`, `EndLevel`, `Levels`, `Data0`-`Data9` |
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

## Valider la configuration

Avant de lancer un tuilage qui peut durer plusieurs heures, vérifiez la configuration avec `mpforge validate` :

```bash
mpforge validate --config configs/france-bdtopo.yaml
```

Six vérifications sont effectuées en chaîne :

| # | Check | Ce qui est vérifié |
|---|-------|--------------------|
| 1 | `yaml_syntax` | Syntaxe YAML valide, types corrects (ex: `base_id` est bien un nombre) |
| 2 | `semantic_validation` | Règles métier : grille cohérente, inputs non vides, bbox valide, SRS, base_id dans 1..9999, filename pattern |
| 3 | `input_files` | Existence de chaque fichier source sur disque (après résolution des wildcards) |
| 4 | `rules_file` | Parsing et validation du fichier de règles de catégorisation |
| 5 | `field_mapping` | Parsing du fichier de field mapping |
| 6 | `header_template` | Existence du fichier template header |

Exemple de sortie :

```
✓ yaml_syntax          — Parsed successfully
✓ semantic_validation  — All validations passed
✓ input_files          — 21/21 files found
✓ rules_file           — 22 rulesets, 283 rules total
✓ field_mapping        — 6 field mappings loaded
- header_template      — No template configured

Config valid. (5/5 checks passed)
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
