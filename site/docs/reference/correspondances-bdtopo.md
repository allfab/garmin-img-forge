# Correspondances BD TOPO / Garmin

Cette page documente comment les couches de la BD TOPO IGN sont transposées en types Garmin dans le pipeline mpforge.

---

## Thèmes BD TOPO utilisés

### Transport

| Couche BD TOPO | Type Garmin | Description |
|---------------|-------------|-------------|
| `TRONCON_DE_ROUTE` (autoroute) | `POLYLINE 0x0001` | Autoroute |
| `TRONCON_DE_ROUTE` (nationale) | `POLYLINE 0x0002` | Route nationale |
| `TRONCON_DE_ROUTE` (départementale) | `POLYLINE 0x0003` | Route départementale |
| `TRONCON_DE_ROUTE` (communale) | `POLYLINE 0x0006` | Rue résidentielle |
| `TRONCON_DE_ROUTE` (chemin) | `POLYLINE 0x000A` | Chemin non revêtu |
| `TRONCON_DE_ROUTE` (sentier) | `POLYLINE 0x000E` | Piste / sentier |
| `TRONCON_DE_VOIE_FERREE` | `POLYLINE 0x0014` | Chemin de fer |

### Hydrographie

| Couche BD TOPO | Type Garmin | Description |
|---------------|-------------|-------------|
| `TRONCON_HYDROGRAPHIQUE` | `POLYLINE 0x001A` | Cours d'eau (ligne) |
| `SURFACE_HYDROGRAPHIQUE` | `POLYGON 0x0028` | Plan d'eau (surface) |

### Végétation

| Couche BD TOPO | Type Garmin | Description |
|---------------|-------------|-------------|
| `ZONE_DE_VEGETATION` (forêt) | `POLYGON 0x0050` | Forêt / bois |
| `ZONE_DE_VEGETATION` (verger) | `POLYGON 0x0051` | Verger / vigne |
| `ZONE_DE_VEGETATION` (prairie) | `POLYGON 0x0052` | Prairie |

### Bâti

| Couche BD TOPO | Type Garmin | Description |
|---------------|-------------|-------------|
| `CONSTRUCTION_SURFACIQUE` | `POLYGON 0x0013` | Emprise bâtiment |

### Toponymie

| Couche BD TOPO | Type Garmin | Description |
|---------------|-------------|-------------|
| `LIEU_DIT_NON_HABITE` | `POI 0x6400+` | Lieu-dit, sommet, col |
| `COMMUNE` | `POI 0x0400+` | Chef-lieu de commune |

## Règles de catégorisation

Les correspondances entre les attributs BD TOPO et les codes types Garmin sont définies dans les fichiers de configuration YAML du pipeline. Le field mapping (`bdtopo-mapping.yaml`) fait le pont entre les noms de colonnes BD TOPO et les champs Polish Map standard.

### Exemple de transposition

Une route dans la BD TOPO :

```
Couche : TRONCON_DE_ROUTE
Attribut IMPORTANCE : 2
Attribut NATURE : Route à 2 chaussées
Nom : Route Nationale 7
```

Devient dans le fichier `.mp` (après field mapping) :

```
[POLYLINE]
Type=0x0002
Label=Route Nationale 7
Levels=0-2
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(45.1234,5.6789),(45.1235,5.6790),...
[END]
```

## Couches non intégrées (à ce jour)

Certaines couches BD TOPO ne sont pas encore intégrées dans le pipeline :

| Couche | Raison |
|--------|--------|
| Réseau électrique haute tension | Pas de type Garmin standard approprié |
| Zones réglementées détaillées | Complexité des attributs |
| Limites administratives fines | Redondance avec les données cadastrales |

!!! note "Courbes de niveau et DEM : ne pas confondre"
    Les **courbes de niveau** (isolignes au pas de 10 m) sont des **données vectorielles** issues des couches altimétriques de l'IGN. Elles sont intégrées au pipeline comme n'importe quelle source de données via la configuration YAML de mpforge.

    Le **DEM** (BDAltiv2 IGN ou SRTM NASA) est un modèle numérique de terrain en raster, utilisé par imgforge (`--dem`) pour l'**ombrage du relief** (hill shading) et les **profils d'altitude** sur le GPS. Ce sont deux données complémentaires mais distinctes.
