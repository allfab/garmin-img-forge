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

## Profils de simplification par couche

`mpforge` applique des **profils de simplification multi-niveaux** aux couches BD TOPO via le catalogue `pipeline/configs/ign-bdtopo/generalize-profiles.yaml`. Chaque feature peut porter plusieurs géométries (`Data0=` détaillée, `Data2=` pour zoom moyen), sélectionnées par `imgforge` au rendu. Tolérances Douglas-Peucker en degrés WGS84.

| Couche BD TOPO | Profil | `Data0` (détaillé) | `Data2` (zoom moyen) | Rationale |
|---|---|---|---|---|
| `BATIMENT` | **aucun** | raw (pas de DP) | — | Géométrie préservée telle que livrée par l'IGN |
| `TRONCON_HYDROGRAPHIQUE` | mono-level | `simplify: 0.00005` (~5 m) | `simplify: 0.00020` (~22 m) | Cours d'eau détaillés + version zoom moyen |
| `ZONE_DE_VEGETATION` | mono-level + Chaikin | Chaikin 1× + `simplify: 0.00005` | `simplify: 0.00020` | Lissage naturel des contours |
| `TRONCON_DE_ROUTE` (Autoroute, Nationale) | dispatch `when: CL_ADMIN` | `simplify: 0.00001` (~1 m) | `simplify: 0.00008` | Préservation routing max |
| `TRONCON_DE_ROUTE` (Départementale) | dispatch `when: CL_ADMIN` | `simplify: 0.00003` | `simplify: 0.00010` | Équilibre détail / taille |
| `TRONCON_DE_ROUTE` (Communale, Sans objet) | dispatch `when: CL_ADMIN` | `simplify: 0.00005` | `simplify: 0.00015` | Défauts raisonnables |
| `TRONCON_DE_ROUTE` (Chemin, Sentier) | dispatch `when: CL_ADMIN` | `simplify: 0.00010` | `simplify: 0.00030` | Simplification plus agressive |
| `TRONCON_DE_ROUTE` (autres) | fallback `levels` default | `simplify: 0.00005` | `simplify: 0.00015` | CL_ADMIN inconnue |
| `COURBE` | mono-level | `simplify: 0.00008` | `simplify: 0.00025` | Courbes de niveau |

**Contraintes** : toute couche routable (`TRONCON_DE_ROUTE`) **doit** déclarer `n: 0` sur chaque branche (garantie routing). Les tolérances `n: 0` des classes routables sont bornées à `≤ 0.00010°` (~11 m) pour préserver la connexité aux intersections.

Voir [Étape 2 — Profils multi-niveaux](../le-pipeline/etape-2-configuration.md#profils-multi-niveaux) pour la sémantique YAML.

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

## Correspondances OSM / Garmin

Les données OpenStreetMap sont une source complémentaire à la BD TOPO pour les POIs et features naturelles.

### Amenity (24 types)

| Tag OSM | Type Garmin | Description |
|---------|-------------|-------------|
| `amenity=bar` | `POI 0x15200` | Bar |
| `amenity=biergarten` | `POI 0x15201` | Brasserie en plein air |
| `amenity=cafe` | `POI 0x15202` | Café |
| `amenity=fast_food` | `POI 0x15203` | Restauration rapide |
| `amenity=restaurant` / `food_court` | `POI 0x15204` | Restaurant |
| `amenity=ice_cream` | `POI 0x15205` | Glacier |
| `amenity=pub` | `POI 0x15206` | Pub |
| `amenity=library` | `POI 0x15207` | Bibliothèque |
| `amenity=bicycle_parking` | `POI 0x15208` | Parking vélo |
| `amenity=bicycle_repair_station` | `POI 0x15209` | Réparation vélo |
| `amenity=fuel` | `POI 0x1520a` | Station-service |
| `amenity=charging_station` | `POI 0x1520b` | Borne de recharge |
| `amenity=motorcycle_parking` | `POI 0x1520c` | Parking moto |
| `amenity=parking` | `POI 0x1520d` | Parking |
| `amenity=taxi` | `POI 0x1520e` | Station taxi |
| `amenity=clinic` / `doctors` | `POI 0x1520f` | Médecin / clinique |
| `amenity=dentist` | `POI 0x15210` | Dentiste |
| `amenity=hospital` | `POI 0x15211` | Hôpital |
| `amenity=pharmacy` | `POI 0x15212` | Pharmacie |
| `amenity=veterinary` | `POI 0x15213` | Vétérinaire |
| `amenity=shelter` | `POI 0x15214` | Abri |
| `amenity=toilets` | `POI 0x15215` | Toilettes |
| `amenity=*` (default) | `POI 0x15216` | Autre amenity |

### Shop (53 types — sélection)

| Tag OSM | Type Garmin | Description |
|---------|-------------|-------------|
| `shop=bakery` | `POI 0x15001` | Boulangerie |
| `shop=supermarket` | `POI 0x15006` | Supermarché |
| `shop=convenience` | `POI 0x15004` | Épicerie |
| `shop=butcher` | `POI 0x15007` | Boucherie |
| `shop=hairdresser` | `POI 0x15002` | Coiffeur |
| `shop=pharmacy` | — | *(via amenity=pharmacy)* |
| `shop=*` (default) | `POI 0x1500e` | Autre commerce |

Les 53 types shop complets sont définis dans `garmin-rules.yaml`. Les types partageant le même code Garmin sont regroupés avec l'opérateur `in:`.

### Natural

| Tag OSM | Géométrie | Type Garmin | Description |
|---------|-----------|-------------|-------------|
| `natural=ridge` | LINE | `POLYLINE 0x11a00` | Crête |
| `natural=arete` | LINE | `POLYLINE 0x11a00` | Arête |
| `natural=cliff` | LINE | `POLYLINE 0x11a01` | Falaise |
| `natural=cave_entrance` | POINT | `POI 0x15301` | Entrée de grotte |
| `natural=rock` | POINT | `POI 0x06614` | Rocher |
| `natural=sinkhole` | POINT | `POI 0x11509` | Doline |
| `natural=cave` (default) | POINT | `POI 0x15300` | Grotte |

### Tourism

| Tag OSM | Type Garmin | Description |
|---------|-------------|-------------|
| `tourism=viewpoint` | `POI 0x16` | Point de vue (Scenic Area) |

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
