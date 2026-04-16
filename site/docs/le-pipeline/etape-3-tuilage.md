# Étape 3 : Tuilage (mpforge)

C'est l'étape centrale du pipeline : `mpforge` lit les données géospatiales, les découpe en tuiles spatiales et génère un fichier Polish Map (`.mp`) par tuile.

---

## Via le script de build (recommandé)

Le script `build-garmin-map.sh` orchestre mpforge et imgforge en une seule commande :

```bash
# Un département
./scripts/build-garmin-map.sh --zones D038

# Multi-départements
./scripts/build-garmin-map.sh --zones D038,D069 --jobs 4

# Dry-run pour vérifier les chemins et commandes
./scripts/build-garmin-map.sh --zones D038,D069 --dry-run
```

Le script :

- Auto-détecte l'année et la version des données BDTOPO
- Exporte les variables d'environnement (`DATA_ROOT`, `ZONES`, `OUTPUT_DIR`...) pour mpforge
- Enchaîne mpforge (tuilage) puis imgforge (compilation) automatiquement
- Gère le DEM multi-zones (un `--dem` par département)

### Options de `build-garmin-map.sh`

#### Géographie

| Option | Description | Défaut |
|--------|-------------|--------|
| `--zones ZONES` | Départements (obligatoire) : `D038`, `D038,D069` | — |
| `--year YYYY` | Année BDTOPO | auto-détecté |
| `--version vYYYY.MM` | Version BDTOPO | auto-détecté |
| `--base-id N` | Base ID Garmin (IDs tuiles = base × 10000 + seq) | premier code département |

#### Chemins des données

| Option | Description | Défaut |
|--------|-------------|--------|
| `--data-dir DIR` | Racine des données (chemin BDTOPO = `{data-dir}/bdtopo/{year}/{version}`) | `./pipeline/data` |
| `--contours-dir DIR` | Racine des courbes de niveau | `{data-dir}/contours` |
| `--dem-dir DIR` | Racine des données DEM (BD ALTI) | `{data-dir}/dem` |
| `--osm-dir DIR` | Racine des données OSM | `{data-dir}/osm` |
| `--hiking-trails-dir DIR` | Racine des sentiers GR | `{data-dir}/hiking-trails` |
| `--output-base DIR` | Base des répertoires de sortie | `./pipeline/output` |
| `--config FILE` | Config YAML mpforge custom | `sources.yaml` |

Les options `--contours-dir`, `--dem-dir`, `--osm-dir` et `--hiking-trails-dir` permettent de pointer vers des répertoires existants sans avoir à respecter l'arborescence par défaut. Si omises, elles sont dérivées de `--data-dir`.

#### mpforge

| Option | Description | Défaut |
|--------|-------------|--------|
| `--jobs N` | Workers parallèles (valeur commune aux deux phases) | `8` |
| `--mpforge-jobs N` | Workers mpforge uniquement (surcharge `--jobs`) | `$JOBS` |
| `--skip-existing` | Passer les tuiles `.mp` déjà présentes. Skippe aussi la phase imgforge si `.img` cible déjà présent (mode publish-only). | — |

#### imgforge

| Option | Description | Défaut |
|--------|-------------|--------|
| `--imgforge-jobs N` | Workers imgforge uniquement (surcharge `--jobs`) | `$JOBS` |
| `--family-id N` | Family ID Garmin (u16) | `1100` |
| `--product-id N` | Product ID Garmin (u16) | `1` |
| `--family-name STR` | Nom de la carte | `IGN-BDTOPO-{ZONES}-{VERSION}` |
| `--series-name STR` | Nom de la série | `IGN-BDTOPO-MAP` |
| `--code-page N` | Code page encodage | `1252` |
| `--levels STR` | Niveaux de zoom (décroissants) | `24,22,20,18,16` |
| `--typ FILE` | Fichier TYP styles | `pipeline/resources/typfiles/I2023100.typ` |
| `--copyright STR` | Message copyright | auto |
| `--no-route` | Désactiver le routage | — |
| `--no-dem` | Désactiver le DEM (relief ombré) | — |

#### imgforge — options géométrie (opt-in, recommandées pour les gros scopes)

Ces options propagent les flags imgforge correspondants ; elles ne changent rien si omises. Toutes valeurs alignées sur les défauts mkgmap.

| Option | Description | Valeur type |
|--------|-------------|--------------|
| `--reduce-point-density F` | Épsilon Douglas-Peucker pour les polylignes | `4.0` |
| `--simplify-polygons SPEC` | Épsilon DP par résolution pour les polygones | `"24:12,18:10,16:8"` |
| `--min-size-polygon N` | Filtre les polygones < N unités carte | `8` |
| `--merge-lines` | Fusionne les polylignes adjacentes (même type + label). Activé par défaut dans mkgmap — **à activer dès qu'on génère un quadrant ou une moitié**, divise par 2-3 le nombre de polylignes et réduit le pic mémoire imgforge. | — |

!!! tip "Quand activer ces options"
    Pour un département, les valeurs par défaut d'imgforge suffisent.
    Pour un quadrant (≥ 20 départements), activez les 4 options : la taille IMG baisse de 15-25 % et imgforge tient en RAM avec moins de workers.

#### Contrôle

| Option | Description |
|--------|-------------|
| `--dry-run` | Simuler sans exécuter |
| `-v`, `--verbose` | Mode verbeux (`-vv` pour très verbeux) |
| `--version-info` | Version du script |

### Exemple complet

```bash
export PROJ_DATA=/usr/share/proj
export OSM_CONFIG_FILE=./pipeline/configs/osm/osmconf.ini
export OGR_GEOMETRY_ACCEPT_UNCLOSED_RING=YES
export OSM_MAX_TMPFILE_SIZE=1024

./scripts/build-garmin-map.sh \
  --zones D038 \
  --year 2025 \
  --version v2025.12 \
  --data-dir ./pipeline/data \
  --contours-dir ./pipeline/data/courbes \
  --dem-dir ./pipeline/data/bdaltiv2 \
  --output-base ./pipeline/output \
  --jobs 4 \
  -v
```

## Commande mpforge directe

Pour un contrôle fin, mpforge peut être appelé directement :

```bash
export DATA_ROOT=./pipeline/data/bdtopo/2025/v2025.12
export ZONES=D038
export OUTPUT_DIR=./pipeline/output/2025/v2025.12/D038
export BASE_ID=38

mpforge build --config pipeline/configs/ign-bdtopo/sources.yaml --jobs 8
```

mpforge va :

1. Substituer les variables `${DATA_ROOT}`, `${ZONES}`, etc. dans le YAML
2. Expandre les brace patterns `{D038,D069}` en chemins concrets
3. Résoudre les wildcards (`*`, `**`) via glob
4. Indexer les features dans un R-tree spatial
5. Calculer la grille de tuilage selon `cell_size` et `overlap`
6. Distribuer les tuiles sur N workers parallèles
7. Pour chaque tuile : clipper les géométries, appliquer les règles, exporter le `.mp`

### Filtrage spatial (optionnel)

Si des sources volumineuses (courbes de niveau, OSM...) sont configurées avec un `spatial_filter`, mpforge pré-filtre les features par une géométrie de référence avant le tuilage. En multi-zones, les géométries de tous les fichiers matchés sont automatiquement unies :

```yaml
inputs:
  - path: "${CONTOURS_DATA_ROOT}/{${ZONES}}/**/COURBE_*.shp"
    spatial_filter:
      source: "${DATA_ROOT}/{${ZONES}}/ADMINISTRATIF/COMMUNE.shp"
      buffer: 500
```

## Stratégie `cell_size` par scope

Le paramètre `grid.cell_size` de la config YAML contrôle la taille des tuiles mpforge en degrés. **C'est le levier le plus important à adapter quand on change d'échelle**. À la différence d'une intuition naturelle, la bonne valeur n'est pas « la plus petite possible pour de la précision » : le splitter RGN d'imgforge subdivise automatiquement les grosses tuiles en interne. Le vrai coût des petites tuiles, c'est **le nombre d'entrées FAT du gmapsupp.img** — que certains GPS comme le Garmin Alpha 100 chargent en RAM au boot, avec un plafond strict.

| Scope | `cell_size` recommandé | Taille tuile (~45°N) | Tuiles typiques | Config |
|-------|------------------------|----------------------|-----------------|--------|
| **Département** (1 zone) | `0.15°` | ~16 × 12 km (200 km²) | 10-30 | `sources.yaml` |
| **Région** (3-10 départements) | `0.30°` | ~33 × 23 km (770 km²) | 30-80 | `sources.yaml` |
| **Quadrant** (20-30 départements) | `0.45°` | ~50 × 35 km (1 750 km²) | 100-150 | `sources-france-XX.yaml` dédié |
| **Moitié / France entière** | `0.60°` à `0.90°` | ~70 × 45 km (3 000+ km²) | 150-250 | `sources-france-XX.yaml` dédié |

!!! warning "Garmin Alpha 100 : limite FAT"
    L'Alpha 100 plante au boot si le gmapsupp.img contient trop d'entrées FAT.
    Règle empirique : **viser ≤ 250 tuiles × 4-6 subfiles ≈ 1 000-1 500 entrées FAT**.
    La référence mkgmap FRANCE-SUD (98 tuiles, 3,19 Gio) fonctionne ; un build
    à 973 tuiles (mêmes données, `cell_size: 0.15°`) plante systématiquement.

En pratique, chaque quadrant a son propre fichier de config dérivé (`sources-france-se.yaml`, `sources-france-so.yaml`...) qui override `grid.cell_size` et éventuellement les `EndLevel` des features volumineuses (BATIMENT, ZONE_DE_VEGETATION) pour alléger les zooms dézoomés.

## Sortie

```
output/tiles/
├── 000_000.mp
├── 000_001.mp
├── 001_000.mp
├── 001_001.mp
├── ...
└── 045_067.mp
```

Chaque fichier `.mp` est un fichier Polish Map complet, lisible dans un éditeur texte :

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
[END]

[POLYLINE]
Type=0x0002
Label=Route Nationale 7
Levels=0-2
Data0=(45.1234,5.6789),(45.1235,5.6790),(45.1240,5.6800)
[END]

[POLYGON]
Type=0x0050
Label=Forêt de Chartreuse
Data0=(45.35,5.78),(45.36,5.79),(45.35,5.80),(45.35,5.78)
[END]
```

## Options utiles en production

### Prévisualiser sans écrire

```bash
# Dry-run : voir combien de tuiles seraient générées
mpforge build --config configs/france-bdtopo.yaml --dry-run
```

Le pipeline s'exécute normalement (lecture sources, R-tree, clipping) mais **aucun fichier n'est créé**. Utile pour valider la configuration avant un long export.

### Reprendre un export interrompu

```bash
# Si l'export a été interrompu (crash, timeout, Ctrl+C)
mpforge build --config configs/france-bdtopo.yaml --jobs 8 --skip-existing
```

Seules les tuiles manquantes sont générées. Les tuiles déjà présentes sur disque sont ignorées.

### Estimer les tuiles restantes

```bash
# Combiner dry-run et skip-existing
mpforge build --config configs/france-bdtopo.yaml --dry-run --skip-existing
```

### Générer un rapport JSON

```bash
mpforge build --config configs/france-bdtopo.yaml --jobs 8 --report report.json
```

Le rapport contient les statistiques de l'export :

```json
{
  "status": "success",
  "tiles_generated": 2047,
  "tiles_failed": 0,
  "tiles_skipped": 150,
  "features_processed": 1234567,
  "duration_seconds": 1845.3,
  "errors": []
}
```

### Verbosité progressive

```bash
# INFO : étapes principales
mpforge build --config configs/france-bdtopo.yaml -v

# DEBUG : logs GDAL détaillés (désactive la barre de progression)
mpforge build --config configs/france-bdtopo.yaml -vv

# TRACE : verbosité maximale (développement uniquement)
mpforge build --config configs/france-bdtopo.yaml -vvv
```

## Parallélisation

| Taille du dataset | Threads recommandés | Temps approximatif |
|-------------------|--------------------|--------------------|
| 1 département | 4 | ~5 min |
| 1 région | 4-8 | ~15-30 min |
| France entière | 8 | ~2-3h |

```bash
# Vérifier le nombre de CPUs disponibles
nproc

# Adapter le nombre de threads
mpforge build --config configs/france-bdtopo.yaml --jobs $(nproc)
```

!!! warning "Consommation mémoire"
    Chaque worker ouvre ses propres datasets GDAL. Avec 8 threads et la France entière en GeoPackage, prévoyez 8-16 Go de RAM.

## Gestion des erreurs

En mode `continue` (défaut), les tuiles en erreur sont journalisées mais n'interrompent pas le traitement :

```
⚠️  Tile 012_045 failed: GDAL error: Invalid geometry
✅ Processing continues with remaining tiles...
```

En mode `fail-fast`, la première erreur arrête tout :

```bash
mpforge build --config configs/france-bdtopo.yaml --fail-fast
```

## Vérification des tuiles

Après le tuilage, vous pouvez vérifier le contenu d'une tuile avec les outils GDAL standard :

```bash
# Lire les métadonnées d'une tuile
ogrinfo -al output/tiles/015_042.mp

# Compter les features par couche
ogrinfo -al -so output/tiles/015_042.mp

# Convertir en GeoJSON pour visualisation dans QGIS
ogr2ogr -f "GeoJSON" tile_preview.geojson output/tiles/015_042.mp
```
