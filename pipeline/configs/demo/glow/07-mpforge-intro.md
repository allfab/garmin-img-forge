# mpforge — Forge les tuiles Polish Map

**mpforge** lit les couches définies dans `sources.yaml`, les découpe selon la grille,
applique les règles Garmin et la simplification géométrique,
et écrit les fichiers **Polish Map** (`.mp`) — un par tuile.

## Commande

```
mpforge build --config <sources.yaml>
```

Un rapport JSON (`mpforge-report.json`) récapitule le build :
tuiles produites, features traitées, durée.
