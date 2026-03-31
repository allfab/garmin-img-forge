# Préambule

## Philosophie du projet

Ce projet est né de la volonté de créer des cartes topographiques Garmin en utilisant
exclusivement des logiciels et données libres, sans dépendance à FME ou mkgmap.

## Ce que vous obtiendrez

Une carte Garmin topographique de la France (ou d'une région) incluant :

- Routes et chemins (BD TOPO IGN)
- Hydrographie (rivières, lacs, zones humides)
- Bâtiments et zones urbanisées
- Végétation (forêts, haies)
- Relief (courbes de niveau à partir du MNT IGN)
- Toponymie (noms de lieux, communes, massifs)

## Prérequis techniques

- Linux ou WSL2 (recommandé)
- Rust (pour compiler `mpforge` et `imgforge`)
- GDAL 3.6+ (pour `ogr-polishmap`)
- Environ 50 Go d'espace disque pour la France entière
