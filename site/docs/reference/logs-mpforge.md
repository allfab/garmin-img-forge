# Logs mpforge — Guide de lecture

`mpforge` utilise la bibliothèque **tracing** (Rust) pour émettre des messages structurés. Par défaut (sans `-v`), seuls les avertissements (`WARN`) et erreurs (`ERROR`) sont affichés. Chaque niveau de verbosité débloque une couche de détail supplémentaire.

## Niveaux de verbosité

| Flag | Niveau activé | Usage recommandé |
|------|---------------|-----------------|
| *(aucun)* | `WARN` + `ERROR` | Production — ne voir que les problèmes |
| `-v` | + `INFO` | Suivi de progression phase par phase |
| `-vv` | + `DEBUG` | Diagnostic feature/tile, sans barre de progression |
| `-vvv` | + `TRACE` | Débogage fin (géométries, règles, clipping) |

!!! tip "Filtrer par cible (target)"
    Les messages GDAL/GEOS sont émis sous le target `gdal`, les messages mpforge sous `mpforge`. La variable `RUST_LOG` permet un filtrage fin :
    ```bash
    # Voir DEBUG mpforge sans le bruit GDAL
    RUST_LOG=mpforge=debug,gdal=warn mpforge build --config config.yaml -vv
    ```

---

## Messages par phase

### Phase 1a — Filtres spatiaux

Ces messages apparaissent avec `-v` quand des entrées déclarent un `spatial_filter`.

| Message | Signification |
|---------|---------------|
| `Building spatial filter geometry for source` | Construction de l'union géométrique du filtre spatial pour la source N (peut prendre plusieurs secondes sur un shapefile COMMUNE volumineux) |
| `Spatial filter geometries pre-built` | Résumé : N filtres construits, M uniques (déduplication automatique par `(source, buffer)`) |

### Phase 1b — Analyse des extents

| Message | Signification |
|---------|---------------|
| `Phase 1b: Scanning source extents` | Scan de l'emprise de toutes les sources (sans charger les features) |
| `Extent scan completed` | Scan terminé ; affiche le nombre de couches et la durée |
| `Grid generated` | Grille de tuiles calculée ; affiche le nombre de tuiles à traiter |
| `No input sources configured, nothing to process` | ⚠️ Aucune source configurée — pipeline terminé sans rien générer |
| `No tiles generated from extents, pipeline has nothing to process` | ⚠️ La grille est vide (emprise nulle ou filtre `bbox` trop restrictif) |

### Phase 1.5 — Pré-simplification topologique

Cette phase apparaît uniquement si des couches avec `topology: true` sont déclarées dans `generalize-profiles.yaml` (ex: `COMMUNE`, `TRONCON_DE_ROUTE`).

| Message | Signification |
|---------|---------------|
| `Phase 1.5: pré-simplification topologique globale` | Lecture globale de toutes les features des couches topologiques (sans filtre spatial) avant tuilage |
| `Phase 1.5: pré-simplification topologique terminée` | Résumé : N features lues, M simplifiées (avec durée) |

!!! note "Pourquoi une phase globale ?"
    Les couches topologiques partagent des vertices aux frontières (ex: communes adjacentes). Une simplification tuile par tuile produirait des trous visibles. La pré-simplification globale garantit des frontières bit-exactes dans toutes les tuiles.

### Phase 2 — Traitement des tuiles

| Message | Signification |
|---------|---------------|
| `Phase 2: Processing N tiles (tile-centric)` | Début du traitement parallèle/séquentiel des N tuiles |
| `Pipeline parallèle : N workers rayon` | Mode parallèle avec N workers rayon (affiché avec `-v` uniquement) |
| `Pipeline séquentiel : 1 thread` | Mode séquentiel (debug) |
| `Multi-level generalization profiles resolved` | Profils chargés depuis `generalize_profiles_path` ; liste les couches concernées et le niveau `Data` maximal |
| `Existing tile skipped` | Tuile sautée car le fichier `.mp` existe déjà (`--skip-existing`) |

### Fin de pipeline

| Message | Signification |
|---------|---------------|
| `Pipeline completed successfully` | Toutes les tuiles traitées sans erreur |
| `Rapport JSON écrit avec succès` | Rapport JSON exporté avec succès au chemin spécifié |

---

## Avertissements courants

### Avertissements mpforge

| Message | Cause | Action |
|---------|-------|--------|
| `WARNING: --jobs exceeds available CPUs, may degrade performance` | `--jobs` > nombre de CPUs physiques | Réduire `--jobs` à `nproc` ou moins |
| `All tiles share the same fixed ID 'N'` | `output.base_id` absent et plusieurs tuiles ont le même ID fixe | Ajouter `base_id` dans la config ou utiliser `{col}_{row}` dans `filename_pattern` |
| `Le pattern {seq} produit des noms non-déterministes en mode parallèle` | `{seq}` dans `filename_pattern` + `--jobs > 1` | Utiliser `{col}_{row}` pour des noms reproductibles |
| `base_id génère les IDs de tuiles via un compteur séquentiel non-déterministe en mode parallèle` | `base_id` configuré + `--jobs > 1` | Comportement attendu en parallèle ; IDs stables en séquentiel |
| `Invalid error_handling mode in config, defaulting to 'continue'` | Valeur inconnue dans `error_handling` | Utiliser `"continue"` ou `"fail-fast"` |
| `No features to export, dataset will be empty` | Tuile vide après clipping | Normal pour les tuiles en bordure de données |
| `Feature rejected during validation: <raison>` | Géométrie invalide rejetée après tentative de réparation | Inspecter les données sources (souvent un artefact de numérisation) |
| `Intersection produced invalid geometry` | Le clipping GDAL a produit une géométrie invalide | Souvent bénin ; la feature est skippée pour cette tuile |
| `Skipping POLYGON feature with less than 4 points` | Polygone trop petit pour être valide (anneau non fermé) | Filtrer en amont ou ignorer |

### Avertissements GDAL (target: `gdal`)

Ces messages proviennent du moteur GDAL/GEOS sous-jacent, pas de mpforge directement.

| Préfixe | Cause typique |
|---------|---------------|
| `WARN gdal: ...` | Warning GDAL/GEOS (ex: géométrie auto-intersectante, SRS non reconnu) |
| `ERROR gdal: ...` | Erreur GDAL (ex: fichier corrompu, pilote non supporté) |

Les warnings GDAL sont souvent bénins et correspondent à des clippings aux bords de tuiles. Pour les silencer en production :
```bash
RUST_LOG=gdal=error mpforge build --config config.yaml -v
```

---

## Messages DEBUG utiles

Avec `-vv`, mpforge affiche le détail feature par feature :

| Message | Signification |
|---------|---------------|
| `Tile has no features, skipping` | Tuile entièrement vide après R-tree query |
| `Feature outside tile, skipping` | Feature hors des bounds de la tuile (normal) |
| `Intersection empty, skipping` | Intersection feature/tuile vide (feature en bordure) |
| `Point geometry, no clipping needed` | POI — pas de clipping nécessaire |
| `Using repaired geometry for clipping` | Géométrie invalide réparée automatiquement avant clipping |
| `Repaired invalid additional_geometry before tile clip` | Géométrie multi-Data réparée |
| `MultiPoint: extracted all sub-points` | Multi-géométrie décomposée en primitives |

---

## Rapport JSON d'exécution

Avec `--report rapport.json`, mpforge écrit un fichier JSON structuré :

```json
{
  "status": "success",
  "tiles_generated": 2047,
  "tiles_failed": 0,
  "tiles_skipped": 150,
  "features_processed": 1234567,
  "duration_seconds": 1845.3,
  "errors": [],
  "quality": {
    "unsupported_types": {
      "MultiPolygon": { "count": 12, "sources": ["SURFACE_HYDROGRAPHIQUE"] }
    },
    "multi_geometries_decomposed": {
      "MultiPoint": 45
    }
  }
}
```

| Champ | Description |
|-------|-------------|
| `status` | `"success"` ou `"failure"` |
| `tiles_generated` | Tuiles exportées avec succès |
| `tiles_failed` | Tuiles en erreur (non-zero → `status: "failure"`) |
| `tiles_skipped` | Tuiles vides ou sautées (`--skip-existing`) |
| `features_processed` | Total des features traitées (toutes tuiles) |
| `duration_seconds` | Durée totale d'exécution en secondes (flottant) |
| `skipped_additional_geom` | Features dont un `Data<n>=` additionnel a échoué (mode multi-Data uniquement, omis si 0) |
| `dry_run` | `true` si `--dry-run` (omis si `false`) |
| `quality.unsupported_types` | Types de géométrie non supportés (compteurs + sources) |
| `quality.multi_geometries_decomposed` | Multi-géométries décomposées en primitives (compteurs) |
| `errors` | Détail des erreurs par tuile : `{ "tile": "003_012", "error": "..." }` |
