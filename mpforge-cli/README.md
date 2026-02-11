# mpforge-cli

> Outil CLI pour générer des tuiles au format Polish Map (.mp) à partir de données SIG massives

**mpforge-cli** est un outil en ligne de commande Rust qui découpe vos données géospatiales (Shapefiles, GeoPackage, PostGIS) en tuiles au format Polish Map pour créer des cartes Garmin personnalisées.

## Caractéristiques principales

- **Multi-sources** : Shapefiles, GeoPackage, PostGIS
- **Multi-couches** : Support de GeoPackage avec dizaines de couches
- **Parallélisation** : Traitement multi-thread pour datasets massifs (BDTOPO 35 GB)
- **Tuilage spatial** : Découpage automatique en grille avec chevauchement configurable
- **Filtrage** : Bounding box pour extraire des zones géographiques
- **Wildcards** : Patterns de fichiers (`data/*.shp`, `data/**/*.gpkg`)
- **Robustesse** : Modes `continue` (tolérant) ou `fail-fast` (strict)
- **CI/CD** : Rapports JSON structurés et codes de sortie
- **Performance** : Barre de progression temps réel et logs multi-niveaux

## Installation

### Prérequis

- **Rust** : 1.70+ ([rustup](https://rustup.rs/))
- **GDAL** : 3.0+ (avec support OGR)

### Compilation

```bash
cd mpforge-cli
cargo build --release
```

L'exécutable sera disponible dans `target/release/mpforge-cli`.

### Installation globale

```bash
cargo install --path .
```

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
  filename_pattern: "{x}_{y}.mp"
```

### 2. Exécuter le tuilage

```bash
# Mode séquentiel (debug)
mpforge-cli build --config config.yaml

# Mode production (parallèle)
mpforge-cli build --config config.yaml --jobs 4

# Avec rapport JSON
mpforge-cli build --config config.yaml --jobs 4 --report report.json
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

Voir la [documentation complète du schéma](doc/config-schema.md) pour tous les détails.

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
  - path: "data/*.shp"     # Wildcards supportés

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
  filename_pattern: "{x}_{y}.mp"  # Variables: {x}, {y}

filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]  # [min_lon, min_lat, max_lon, max_lat]

error_handling: "continue"  # "continue" ou "fail-fast"
```

### Exemples de configuration

Voir le répertoire [`examples/`](examples/) :

- **[simple.yaml](examples/simple.yaml)** : Configuration minimale pour débuter
- **[bdtopo.yaml](examples/bdtopo.yaml)** : Configuration production pour BDTOPO (35 GB, 50+ couches)

## Options CLI

### Commande `build`

```bash
mpforge-cli build [OPTIONS] --config <CONFIG>
```

| Option | Description | Défaut |
|--------|-------------|--------|
| `-c, --config <FILE>` | Fichier de configuration YAML | **REQUIS** |
| `-j, --jobs <N>` | Nombre de threads parallèles | `1` |
| `-r, --report <FILE>` | Générer un rapport JSON | - |
| `-v, --verbose...` | Verbosité (`-v`, `-vv`, `-vvv`) | WARN |
| `--fail-fast` | Arrêter à la première erreur | - |
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

```bash
# Vérifier le nombre de CPUs
nproc

# Petit dataset (<50 tuiles) : mode séquentiel
mpforge-cli build --config config.yaml

# Dataset moyen (50-500 tuiles) : 4 threads
mpforge-cli build --config config.yaml --jobs 4

# Large dataset (>500 tuiles) : 8 threads
mpforge-cli build --config config.yaml --jobs 8
```

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
mpforge-cli build --config config.yaml --jobs 4
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
  filename_pattern: "reunion_{x}_{y}.mp"

filters:
  bbox: [55.2, -21.4, 55.8, -20.9]  # Île de La Réunion

error_handling: "continue"
```

```bash
mpforge-cli build --config bdtopo.yaml --jobs 8 --report rapport.json
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
mpforge-cli build --config postgis.yaml --jobs 4 -v
```

## Rapport JSON (CI/CD)

### Génération du rapport

```bash
mpforge-cli build --config config.yaml --report report.json
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

mpforge-cli build --config bdtopo.yaml --jobs 8 --report report.json

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
```

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
mpforge-cli build --config config.yaml --fail-fast
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

## Développement

### Structure du projet

```
mpforge-cli/
├── src/
│   ├── main.rs              # Point d'entrée CLI
│   ├── config.rs            # Parsing YAML et validation
│   ├── grid.rs              # Génération de grille spatiale
│   ├── tiler.rs             # Logique de tuilage
│   ├── writer.rs            # Export Polish Map
│   └── parallel.rs          # Traitement parallèle
├── tests/
│   ├── config_tests.rs
│   ├── grid_tests.rs
│   └── integration_tests.rs
├── examples/
│   ├── simple.yaml
│   └── bdtopo.yaml
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
- **Documentation** : `/doc/config-schema.md`

### Historique des versions

- **Epic 5** : Configuration YAML multi-sources
- **Epic 6** : Tuilage spatial et export Polish Map
- **Epic 7** : Parallélisation, progress bar, rapports JSON

## Licence

Ce projet fait partie de **MPForge** et est distribué sous licence MIT. Voir le fichier [LICENSE](../LICENSE) à la racine du dépôt.

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

- **Documentation** : Voir [`doc/config-schema.md`](doc/config-schema.md)
- **Exemples** : Voir [`examples/`](examples/)
- **Issues** : https://github.com/anthropics/claude-code/issues

## Auteurs

Développé dans le cadre du projet **MPForge** pour générer des cartes Garmin à partir de données SIG massives.
