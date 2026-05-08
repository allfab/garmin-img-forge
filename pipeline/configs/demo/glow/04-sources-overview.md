# sources.yaml

Fichier de configuration central de **mpforge**.
Il décrit les données géographiques à charger, comment les découper en tuiles et les métadonnées de la carte finale.

## Structure du fichier

| Section | Rôle |
|---------|------|
| `grid` | Taille des tuiles et recouvrement |
| `inputs` | Liste des couches SHP/GPKG à ingérer |
| `output` | Répertoire et nommage des fichiers `.mp` |
| `header` | Métadonnées Polish Map (niveaux de zoom) |
| `rules` | Chemin vers les règles de mapping Garmin |
| `generalize_profiles_path` | Catalogue de simplification géométrique |

## Couches d'entrée — 15 sources géographiques

| Thème | Couches BDTOPO / autres |
|-------|------------------------|
| Transport | Routes, Voies ferrées, Câbles, Aérodromes |
| Administratif | Communes, Zones d'habitation |
| Hydrographie | Tronçons, Surfaces, Détails |
| Bâti | Bâtiments, Cimetières, Constructions… |
| Végétation | Zones de végétation |
| Courbes de niveau | Courbes 10 m (filtrées depuis les 5 m IGN) |
| OSM | Amenity POIs, Spots naturels, Tourisme |
| Cadastre | Piscines privées |
| Randonnée | Sentiers de Grande Randonnée |
