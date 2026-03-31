# Diagramme du pipeline

## Nouveau pipeline (open-source)

```mermaid
flowchart LR
    A[BD TOPO IGN\n.gpkg/.shp] --> B[download-bdtopo.sh]
    B --> C[data/bdtopo/]
    C --> D[mpforge build\n--config --jobs]
    D --> E[output/tiles/\n*.mp]
    E --> F[imgforge\n--config]
    F --> G[output/\ngmapsupp.img]
    G --> H[GPS Garmin]
```

## Ancien pipeline (remplacé)

```mermaid
flowchart LR
    A[BD TOPO IGN\n.shp] --> B[FME\nWorkbench]
    B --> C[GPSMapEdit\n.mp manual]
    C --> D[mkgmap\n.jar]
    D --> E[gmapsupp.img]
    E --> F[GPS Garmin]
```

## Comparaison

| Critère | Ancien pipeline | Nouveau pipeline |
|---------|----------------|-----------------|
| Licence | FME propriétaire | 100 % open-source |
| Automatisation | Manuelle | Complète (scripts Bash) |
| Reproductibilité | Faible | Totale |
| Performance | Lente | Parallélisée (Rayon) |
| Dépendances | FME, Java, GPSMapEdit | Rust, GDAL |

!!! success "Sans Java, sans FME"
    Le nouveau pipeline élimine les dépendances propriétaires et Java. Un seul binaire `imgforge`
    remplace `mkgmap`.
