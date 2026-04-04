# Types Garmin

Les codes types (`Type=0xNNNN`) déterminent comment chaque objet est rendu sur les appareils Garmin. C'est le lien fondamental entre les données géographiques et l'affichage sur le GPS.

---

## Points d'intérêt (POI)

| Plage | Catégorie | Exemples |
|-------|-----------|----------|
| `0x2A00`-`0x2AFF` | Attractions | Musées, parcs, écoles |
| `0x2B00`-`0x2BFF` | Loisirs | Théâtres, bars, cinémas |
| `0x2C00`-`0x2CFF` | Restauration | Restaurants, fast-food |
| `0x2D00`-`0x2DFF` | Hébergement | Hôtels, campings |
| `0x2E00`-`0x2EFF` | Shopping | Magasins, centres commerciaux |
| `0x2F00`-`0x2FFF` | Services | Stations-service, gares, aéroports |
| `0x3000`-`0x30FF` | Santé/Communauté | Hôpitaux, pharmacies, mairies |
| `0x6400`-`0x6416` | Géographie | Sommets, cols, lacs, plages |

## Routes (Polylines)

| Code | Description | Niveau de zoom typique |
|------|-------------|----------------------|
| `0x0001` | Autoroute | 0-3 (visible à tous les zooms) |
| `0x0002` | Route nationale | 0-3 |
| `0x0003` | Route régionale / départementale | 0-2 |
| `0x0004` | Route artérielle | 0-2 |
| `0x0005` | Route collectrice | 0-1 |
| `0x0006` | Rue résidentielle | 0-1 |
| `0x0007` | Allée / voie de desserte | 0 |
| `0x000A` | Route non revêtue / piste | 0-1 |
| `0x000C` | Rond-point | 0 |
| `0x000E` | Piste 4x4 / chemin forestier | 0 |
| `0x0014` | Chemin de fer | 0-2 |
| `0x0015` | Sentier pédestre | 0 |
| `0x0016` | Piste cyclable | 0 |
| `0x001A` | Rivière / canal (ligne) | 0-2 |
| `0x001B` | Ruisseau | 0-1 |
| `0x0020` | Courbe de niveau majeure | 0 |
| `0x0021` | Courbe de niveau mineure | 0 |
| `0x0022` | Courbe de niveau supplémentaire | 0 |

## Polygones

| Plage / Code | Catégorie | Exemples |
|-------------|-----------|----------|
| `0x0001`-`0x000E` | Zones urbaines | Centre-ville, zone industrielle |
| `0x0010`-`0x0019` | Parcs et loisirs | Parc urbain, terrain de sport |
| `0x0013` | Bâtiment | Emprise du bâti |
| `0x001A` | Cimetière | |
| `0x0028`-`0x0032` | Lacs et étangs | Plan d'eau |
| `0x003C`-`0x0048` | Rivières et cours d'eau | Surface hydrographique |
| `0x004C` | Glacier | |
| `0x004F` | Marais | Zone humide |
| `0x0050` | Forêt / bois | Zone de végétation arborée |
| `0x0051` | Verger / vigne | Culture pérenne |
| `0x0052` | Prairie | Zone herbacée |
| `0x0053` | Toundra / lande | Végétation basse |

## Types personnalisés

Les codes `0x10000`-`0x1FFFF` sont réservés aux types personnalisés. Ils nécessitent un fichier TYP (`.typ`) pour définir leur rendu visuel (couleur, motif, icône).

```
[POLYLINE]
Type=0x10001
Label=GR20
Data0=(42.0,9.0),(42.1,9.1)
[END]
```

Le fichier TYP associé définit comment `0x10001` sera affiché : une ligne rouge en pointillés, par exemple, pour un sentier de Grande Randonnée.
