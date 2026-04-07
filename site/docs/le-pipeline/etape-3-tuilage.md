# Étape 3 : Tuilage (mpforge)

C'est l'étape centrale du pipeline : `mpforge` lit les données géospatiales, les découpe en tuiles spatiales et génère un fichier Polish Map (`.mp`) par tuile.

---

## Commande de base

```bash
mpforge build --config configs/france-bdtopo.yaml --jobs 8
```

C'est tout. mpforge va :

1. Lire toutes les sources déclarées dans la configuration
2. Indexer les features dans un R-tree spatial
3. Calculer la grille de tuilage selon `cell_size` et `overlap`
4. Distribuer les tuiles sur 8 workers parallèles
5. Pour chaque tuile : clipper les géométries, appliquer le field mapping, exporter le `.mp`
6. Afficher une barre de progression en temps réel

### Filtrage spatial (optionnel)

Si des sources volumineuses (courbes de niveau, MNT...) sont configurées avec un `spatial_filter`, mpforge pré-filtre les features par une géométrie de référence avant le tuilage. Cela réduit drastiquement le temps de traitement :

```yaml
# Dans la configuration YAML
inputs:
  - path: "data/COURBES_NIVEAU.shp"
    spatial_filter:
      source: "data/COMMUNE.shp"
      buffer: 500
```

Voir la [documentation mpforge](../le-projet/mpforge.md#filtrage-spatial) pour les détails.

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
