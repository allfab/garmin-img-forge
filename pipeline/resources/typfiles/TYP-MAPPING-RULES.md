## 5. Règles de mapping BDTOPO → Garmin — référence complète

Ce qui suit est la traduction de toute la logique FME en règles YAML. C'est le fichier `bdtopo-garmin-rules.yaml` cible.

### 5.1 Administratif — Communes

| Source | Champ BDTOPO | Valeur | Type | EndLevel |
|--------|-------------|--------|------|----------|
| COMMUNE.shp | — | (toutes) | `0x54` | 7 |

Attributs supplémentaires : `Country="France~[0x1d]FRA"`, `CityName` depuis NOM, `Zip` depuis CODE_POST.

### 5.2 Administratif — Zones d'habitation

| Source | Champ BDTOPO | Valeur NATURE | Type | EndLevel |
|--------|-------------|--------------|------|----------|
| ZONE_D_HABITATION.shp | NATURE | Château | `0x10f0a` | 4 |
| | | Grange | `0x03` | 4 |
| | | Habitat temporaire | `0x03` | 4 |
| | | Lieu-dit habité | `0x03` | 4 |
| | | Moulin | `0x03` | 4 |
| | | Quartier | `0x03` | 4 |
| | | Ruines | `0x03` | 4 |

### 5.3 Transport — Routes

| CL_ADMIN | NATURE | Type | EndLevel | Label |
|----------|--------|------|----------|-------|
| Autoroute | * | `0x01` | 7 | `${NUMERO}` |
| Nationale | != Rond-point | `0x04` | 7 | `${NUMERO}` |
| Nationale | Rond-point | `0x04` | 7 | — |
| Départementale | != Rond-point | `0x05` | 7 | `${NUMERO}` |
| Départementale | Rond-point | `0x04` | 7 | — |
| Route intercommunale | * | `0x05` | 7 | `${NUMERO}` |
| * | Route à 1 chaussée | `0x06` | 2 | — |
| * | Route à 2 chaussées | `0x06` | 2 | — |
| * | Rond-point (sans CL_ADMIN) | `0x06` | 2 | — |
| * | Route empierrée | `0x07` | 1 | — |
| * | Bretelle | `0x09` | 7 | — |
| * | Chemin | `0x0a` | 1 | — |
| * | Escalier | `0x0f` | 1 | — |
| * | Piste cyclable | `0x0e` | 1 | — |
| * | Sentier | `0x10` | 1 | — |
| * | Type autoroutier | `0x09` | 7 | — |
| * | Bac ou liaison maritime | `0x1b` | 1 | — |

**Correspondance EndLevel → MPBITLEVEL :**

| EndLevel | MPBITLEVEL | Utilisé pour |
|----------|-----------|-------------|
| 7 | 17 | Autoroutes, nationales, départementales |
| 2 | 22 | Routes communales |
| 1 | 23 | Chemins, sentiers, pistes cyclables |

### 5.4 Transport — Voies ferrées

| NATURE | POS_SOL | Type | EndLevel |
|--------|---------|------|----------|
| Voie ferrée principale | != -1 | `0x10c00` | 5 |
| LGV | != -1 | `0x10e02` | 5 |
| Voie de service | != -1 | `0x10e03` | 5 |
| Tramway | != -1 | `0x10e04` | 5 |
| Funiculaire ou crémaillère | != -1 | `0x10e05` | 5 |
| Sans objet | != -1 | `0x10c00` | 5 |
| * (souterrain) | -1 | `0x10e06` | 5 |

Label : `${TOPONYME}`

### 5.5 Transport — Aérodromes et câbles

**Lignes / Polygones :**

| Source | NATURE | Type | EndLevel |
|--------|--------|------|----------|
| PISTE_D_AERODROME | Piste en dur | `0x1090b` | 4 |
| PISTE_D_AERODROME | Piste en herbe | `0x10f17` | 4 |
| TRANSPORT_PAR_CABLE | * | `0x10f0b` | 2 |

**POI Aérodrome :**

| NATURE (AERODROME) | Type | EndLevel |
|--------------------|------|----------|
| Aérodrome | `0x02d0b` | 2 |
| Altiport | `0x15500` | 2 |
| Héliport | `0x15500` | 2 |

**POI Transport par câble :**

| NATURE (TRANSPORT_PAR_CABLE) | Type | EndLevel |
|------------------------------|------|----------|
| Télécabine, téléphérique | `0x15501` | 2 |
| Télésiège | `0x15501` | 2 |
| Téléski | `0x15501` | 2 |

### 5.6 Hydrographie — Cours d'eau (lignes)

| NATURE (TRONCON_HYDROGRAPHIQUE) | PERSISTANC | Type | EndLevel |
|---------------------------------|-----------|------|----------|
| Cours d'eau | Permanent | `0x18` | 2 |
| Cours d'eau | Intermittent | `0x26` | 2 |
| Ruisseau | * | `0x18` | 2 |
| Canal | * | `0x18` | 2 |

Label : `${TOPONYME}`

### 5.7 Hydrographie — Surfaces

| NATURE (SURFACE_HYDROGRAPHIQUE) | Type | EndLevel |
|---------------------------------|------|----------|
| Eau libre (grande surface) | `0x3c` | 5 |
| Eau libre (surface moyenne) | `0x3f` | 4 |
| Eau libre (petite surface) | `0x41` | 2 |
| Glacier, névé | `0x4d` | 4 |
| Marais | `0x10c04` | 2 |
| Lac intermittent | `0x4c` | 2 |

Note : la classification par taille de lac (0x3c → 0x41) dépend de la surface géométrique du polygone (voir Procedure.txt pour les seuils en sq mi).

### 5.8 Hydrographie — Détails (POI)

| NATURE (DETAIL_HYDROGRAPHIQUE) | Type | EndLevel |
|-------------------------------|------|----------|
| Arroyo | `0x06501` | 2 |
| Baie | `0x06503` | 2 |
| Cascade | `0x06508` | 2 |
| Citerne | `0x06414` | 2 |
| Crique | `0x06507` | 2 |
| Fontaine | `0x06509` | 2 |
| Glacier | `0x0650a` | 2 |
| Lac | `0x0650d` | 2 |
| Lavoir | `0x06414` | 2 |
| Marais | `0x06513` | 2 |
| Mer | `0x06510` | 2 |
| Perte | `0x06414` | 2 |
| Point d'eau | `0x06414` | 2 |
| Embouchure | `0x06414` | 2 |
| Réservoir | `0x0650f` | 2 |
| Résurgence | `0x06414` | 2 |
| Source | `0x06511` | 2 |
| Source captée | `0x06511` | 2 |

Label : `${TOPONYME}`

### 5.9 Végétation

| NATURE (ZONE_DE_VEGETATION) | Type | EndLevel | Label |
|----------------------------|------|----------|-------|
| Bois | `0x11005` | 6 | Bois |
| Forêt fermée de conifères | `0x10f1f` | 6 | Forêt de conifères |
| Forêt fermée de feuillus | `0x10f1e` | 6 | Forêt de feuillus |
| Forêt fermée mixte | `0x4e` | 6 | — |
| Forêt ouverte | `0x11000` | 6 | — |
| Haie | `0x11002` | 4 | — |
| Lande ligneuse | `0x11003` | 4 | — |
| Peupleraie | `0x11001` | 4 | Peupleraie |
| Verger | `0x11004` | 4 | Verger |
| Vigne | `0x11004` | 4 | Vigne |

### 5.10 Zones d'activité

**Polygone :**

| Source | Type | EndLevel |
|--------|------|----------|
| ZONE_D_ACTIVITE_OU_D_INTERET | `0x0c` | 2 |

**POI Zone d'activité ou d'intérêt :**

| NATURE (ZONE_D_ACTIVITE_OU_D_INTERET) | Type | EndLevel |
|----------------------------------------|------|----------|
| Abri de montagne | `0x02b04` | 2 |
| Administration centrale de l'Etat | `0x03007` | 2 |
| Aire d'accueil des gens du voyage | `0x02f0c` | 2 |
| Aire de détente | `0x02c04` | 2 |
| Aquaculture | `0x02900` | 2 |
| Autre équipement sportif | `0x11601` | 2 |
| Autre établissement d'enseignement | `0x02c05` | 2 |
| Autre service déconcentré de l'Etat | `0x03007` | 2 |
| Baignade surveillée | `0x1151f` | 2 |
| Borne | `0x0660f` | 2 |
| Borne frontière | `0x0660f` | 2 |
| Camp militaire non clos | `0x0640b` | 2 |
| Camping | `0x02b03` | 2 |
| Capitainerie | `0x02f09` | 2 |
| Carrière | `0x0640c` | 2 |
| Caserne | `0x03008` | 2 |
| Caserne de pompiers | `0x03008` | 2 |
| Centrale électrique | `0x02900` | 2 |
| Centre de documentation | `0x02c03` | 2 |
| Centre équestre | `0x02d0a` | 2 |
| Champ de tir | `0x0640b` | 2 |
| Collège | `0x02c05` | 2 |
| Complexe sportif couvert | `0x11601` | 2 |
| Construction | `0x06402` | 2 |
| Culte chrétien | `0x02c0e` | 2 |
| Culte divers | `0x02c0b` | 2 |
| Culte israélite | `0x02c10` | 2 |
| Culte musulman | `0x02c0d` | 2 |
| Déchèterie | `0x02900` | 2 |
| Divers agricole | `0x1150e` | 2 |
| Divers commercial | `0x02e04` | 2 |
| Divers industriel | `0x02900` | 2 |
| Divers public ou administratif | `0x03007` | 2 |
| Ecomusée | `0x11600` | 2 |
| Elevage | `0x02900` | 2 |
| Enceinte militaire | `0x0640b` | 2 |
| Enseignement primaire | `0x02c05` | 2 |
| Enseignement supérieur | `0x02c05` | 2 |
| Equipement de cyclisme | `0x1160a` | 2 |
| Espace public | `0x1160d` | 2 |
| Etablissement extraterritorial | `0x03007` | 2 |
| Etablissement hospitalier | `0x03002` | 2 |
| Etablissement pénitentiaire | `0x02900` | 2 |
| Etablissement thermal | `0x02900` | 2 |
| Gendarmerie | `0x03001` | 2 |
| Golf | `0x02d05` | 2 |
| Habitation troglodytique | `0x11509` | 2 |
| Haras | `0x02d0a` | 2 |
| Hébergement de loisirs | `0x02b04` | 2 |
| Hippodrome | `0x02c08` | 2 |
| Hôpital | `0x03002` | 2 |
| Hôtel de département | `0x03003` | 2 |
| Hôtel de région | `0x03003` | 2 |
| Lycée | `0x02c05` | 2 |
| Mairie | `0x03003` | 2 |
| Maison de retraite | `0x02900` | 2 |
| Maison du parc | `0x06402` | 2 |
| Maison forestière | `0x14e01` | 2 |
| Marché | `0x02e04` | 2 |
| Mégalithe | `0x11508` | 2 |
| Mine | `0x0640c` | 2 |
| Monument | `0x14e0f` | 2 |
| Musée | `0x02c02` | 2 |
| Office de tourisme | `0x02f0c` | 2 |
| Ouvrage militaire | `0x0640b` | 2 |
| Palais de justice | `0x03004` | 2 |
| Parc de loisirs | `0x02c01` | 2 |
| Parc des expositions | `0x10d09` | 2 |
| Parc zoologique | `0x02c07` | 2 |
| Patinoire | `0x02d08` | 2 |
| Piscine | `0x02d09` | 2 |
| Point de vue | `0x02c04` | 2 |
| Police | `0x03001` | 2 |
| Poste | `0x02f05` | 2 |
| Préfecture | `0x03007` | 2 |
| Préfecture de région | `0x03007` | 2 |
| Refuge | `0x02b04` | 2 |
| Salle de danse ou de jeux | `0x02d04` | 2 |
| Salle de spectacle ou conférence | `0x02d01` | 2 |
| Science | `0x02c05` | 2 |
| Sentier de découverte | `0x06412` | 2 |
| Siège d'EPCI | `0x03003` | 2 |
| Site de vol libre | `0x02d0b` | 2 |
| Site d'escalade | `0x11601` | 2 |
| Sous-préfecture | `0x03007` | 2 |
| Sports en eaux vives | `0x11601` | 2 |
| Sports mécaniques | `0x11601` | 2 |
| Sports nautiques | `0x11601` | 2 |
| Stade | `0x02c08` | 2 |
| Stand de tir | `0x02900` | 2 |
| Station de pompage | `0x02900` | 2 |
| Station d'épuration | `0x02900` | 2 |
| Structure d'accueil pour personnes handicapées | `0x02900` | 2 |
| Tombeau | `0x06403` | 2 |
| Université | `0x02c05` | 2 |
| Usine | `0x02900` | 2 |
| Usine de production d'eau potable | `0x02900` | 2 |
| Vestige archéologique | `0x1150b` | 2 |
| Zone industrielle | `0x02900` | 2 |

Label : `${TOPONYME}`

### 5.11 Bâtiments

| NATURE (BATIMENT) | Type | EndLevel |
|-------------------|------|----------|
| Arène ou théâtre antique | `0x10f08` | 2 |
| Chapelle | `0x10f09` | 2 |
| Château | `0x10f0a` | 2 |
| Eglise | `0x10f0b` | 2 |
| Fort, blockhaus, casemate | `0x10f0c` | 2 |
| Indifférenciée | `0x1101c` | 2 |
| Industriel, agricole ou commercial | `0x10f04` | 2 |
| Monument | `0x10f0d` | 2 |
| Préfecture | `0x10f0f` | 2 |
| Sous-préfecture | `0x10f10` | 2 |
| Serre | `0x10f05` | 2 |
| Silo | `0x10f06` | 2 |
| Tour, donjon | `0x10f11` | 2 |
| Tribune | `0x10f12` | 2 |
| Construction légère | `0x10f14` | 2 |

Label : `${TOPONYME}`

### 5.12 Cimetières

**Polygone :**

| NATURE (CIMETIERE) | Type | EndLevel |
|--------------------|------|----------|
| Civil | `0x1a` | 4 |
| Militaire | `0x10f13` | 4 |
| Militaire étranger | `0x10f13` | 4 |

**POI Cimetière :**

| NATURE (CIMETIERE) | Type | EndLevel |
|--------------------|------|----------|
| Civil | `0x06403` | 2 |
| Militaire | `0x06403` | 2 |
| Militaire étranger | `0x06403` | 2 |

### 5.13 Constructions linéaires

| NATURE (CONSTRUCTION_LINEAIRE) | Type | EndLevel |
|-------------------------------|------|----------|
| Autre ligne descriptive | `0x10c04` | 2 |
| Barrage | `0x10f08` | 2 |
| Clôture | `0x13309` | 2 |
| Mur | `0x13308` | 2 |
| Mur anti-bruit | `0x10e13` | 2 |
| Mur de soutènement | `0x10e18` | 2 |
| Pont | `0x10e14` | 2 |
| Quai | `0x10e16` | 2 |
| Ruines | `0x10e15` | 2 |
| Sport de montagne | `0x10f0c` | 2 |
| Tunnel | `0x10e08` | 2 |

Label : `${TOPONYME}` ou `${NATURE}`

### 5.14 Lignes orographiques

| NATURE (LIGNE_OROGRAPHIQUE) | Type | EndLevel |
|----------------------------|------|----------|
| Carrière | `0x10e1a` | 2 |
| Levée | `0x10e17` | 2 |
| Talus | `0x10e19` | 2 |

### 5.15 Pylônes et constructions ponctuelles

| NATURE | Type | EndLevel |
|--------|------|----------|
| Pylône (PYLONE) | `0x11500` | 1 |
| Antenne | `0x11503` | 1 |
| Autre construction élevée | `0x06402` | 1 |
| Calvaire | `0x11507` | 1 |
| Cheminée | `0x11504` | 1 |
| Clocher | `0x10d0e` | 1 |
| Croix | `0x11507` | 1 |
| Eolienne | `0x11505` | 1 |
| Minaret | `0x10d0d` | 1 |
| Phare | `0x10101` | 1 |
| Puits d'hydrocarbures | `0x0640d` | 1 |
| Torchère | `0x11108` | 1 |
| Transformateur | `0x11506` | 1 |

### 5.16 Terrains de sport

| NATURE (TERRAIN_DE_SPORT) | Type | EndLevel |
|--------------------------|------|----------|
| Terrain de tennis | `0x10f1c` | 2 |
| Bassin de natation | `0x10f1d` | 2 |
| Grand terrain de sport | `0x1090d` | 2 |
| Petit terrain multi-sports | `0x1100a` | 2 |
| Piste de sport | `0x10f1b` | 2 |

### 5.17 Services et activités

| Source | Type | EndLevel | Label |
|--------|------|----------|-------|
| LIGNE_ELECTRIQUE | `0x29` | 2 | `Ligne ${VOLTAGE}` |

### 5.18 Forêt publique (zones réglementées)

| Source | Type | EndLevel | Label |
|--------|------|----------|-------|
| FORET_PUBLIQUE | `0x10a03` | 3 | `${TOPONYME}` |

### 5.19 Toponymie — Lieux nommés (POI)

| NATURE (TOPONYMIE) | Type | EndLevel |
|--------------------|------|----------|
| Arbre | `0x1150c` | 2 |
| Bois | `0x0660a` | 2 |
| Château | `0x1150f` | 2 |
| Cirque | `0x06608` | 2 |
| Col | `0x06601` | 2 |
| Crête | `0x06613` | 2 |
| Dépression | `0x0660b` | 2 |
| Escarpement | `0x06607` | 2 |
| Gorge | `0x06611` | 2 |
| Gouffre | `0x11515` | 2 |
| Grange | `0x11510` | 2 |
| Grotte | `0x11515` | 2 |
| Île | `0x06501` | 2 |
| Lieu-dit habité | `0x11511` | 4 |
| Lieu-dit non habité | `0x1150e` | 2 |
| Montagne | `0x06601` | 2 |
| Moulin | `0x11512` | 2 |
| Pic | `0x06616` | 2 |
| Plage | `0x1160e` | 2 |
| Plaine | `0x06610` | 2 |
| Quartier | `0x11513` | 4 |
| Rochers | `0x06614` | 2 |
| Ruines | `0x11514` | 2 |
| Sommet | `0x06616` | 2 |
| Vallée | `0x06617` | 2 |
| Versant | `0x06615` | 2 |
| Cap | `0x06606` | 2 |
| Volcan | `0x06608` | 2 |

Label : `${TOPONYME}`

### 5.20 Equipement de transport (POI)

| NATURE (EQUIPEMENT_DE_TRANSPORT) | Type | EndLevel |
|----------------------------------|------|----------|
| Carrefour | `0x06406` | 2 |
| Parking | `0x1100b` | 2 |
| Port | `0x06401` | 2 |
| Station de métro | `0x02f08` | 2 |
| Station de tramway | `0x02f08` | 2 |

Label : `${TOPONYME}`

### 5.21 Construction surfacique (POI)

| NATURE (CONSTRUCTION_SURFACIQUE) | Type | EndLevel |
|----------------------------------|------|----------|
| Barrage | `0x06407` | 2 |
| Dalle | `0x06401` | 2 |
| Ecluse | `0x06401` | 2 |
| Pont | `0x06401` | 2 |

Label : `${TOPONYME}`

### 5.22 Construction linéaire (POI)

| NATURE (CONSTRUCTION_LINEAIRE) | Type | EndLevel |
|-------------------------------|------|----------|
| Barrage | `0x06407` | 2 |
| Mur | `0x06402` | 2 |
| Mur de soutènement | `0x06402` | 2 |
| Pont | `0x06401` | 2 |
| Quai | `0x06402` | 2 |
| Ruines | `0x11514` | 2 |
| Sport de montagne | `0x11601` | 2 |
| Tunnel | `0x06413` | 2 |

Label : `${TOPONYME}`

### 5.23 Plan d'eau (POI)

| NATURE (PLAN_D_EAU) | Type | EndLevel |
|----------------------|------|----------|
| Canal | `0x06506` | 2 |
| Ecoulement naturel | `0x06414` | 2 |
| Glacier, névé | `0x0650a` | 2 |
| Lac | `0x0650d` | 2 |
| Marais | `0x06513` | 2 |
| Mare | `0x06513` | 2 |
| Plan d'eau de gravière | `0x06414` | 2 |
| Réservoir-bassin | `0x0650f` | 2 |
| Réservoir-bassin piscicole | `0x0650f` | 2 |
| Retenue | `0x0650f` | 2 |
| Retenue-barrage | `0x0650f` | 2 |
| Retenue-bassin portuaire | `0x0650f` | 2 |
| Retenue-digue | `0x0650f` | 2 |

Label : `${TOPONYME}`

### 5.24 Parc ou réserve (POI — TOPONYMIE CLASSE)

| NATURE (PARC_OU_RESERVE) | Type | EndLevel |
|--------------------------|------|----------|
| Arrêté de protection | `0x02c06` | 2 |
| Parc national | `0x02c06` | 2 |
| Parc naturel régional | `0x02c06` | 2 |
| Réserve biologique | `0x02c06` | 2 |
| Réserve nationale de chasse et de faune sauvage | `0x02c06` | 2 |
| Réserve naturelle | `0x02c06` | 2 |
| Site acquis ou assimilé des conservatoires d'espaces | `0x15502` | 2 |
| Site Natura 2000 | `0x02c06` | 2 |
| Terrain du Conservatoire du Littoral | `0x15502` | 2 |
| Zone de silence | `0x02c06` | 2 |
| Zone naturelle | `0x02c06` | 2 |
| * (catch-all) | `0x02c06` | 2 |

Label : `${GRAPHIE}`

### 5.25 Point du réseau (POI — TOPONYMIE CLASSE)

| NATURE (POINT_DU_RESEAU) | Type | EndLevel |
|--------------------------|------|----------|
| Autre point du réseau | `0x06401` | 2 |
| Passage à niveau | `0x1160b` | 2 |
| Poteau de balisage de randonnée | `0x0660f` | 2 |
| * (catch-all) | `0x06401` | 2 |

Label : `${GRAPHIE}`

### 5.26 Forêt publique (POI)

| NATURE (FORET_PUBLIQUE) | Type | EndLevel |
|-------------------------|------|----------|
| Autre forêt publique | `0x0660a` | 2 |
| Forêt domaniale | `0x0660a` | 2 |

Label : `${TOPONYME}`

### 5.27 Cours d'eau (POI — TOPONYMIE CLASSE)

| NATURE (COURS_D_EAU) | Type | EndLevel |
|-----------------------|------|----------|
| * (NATURE vide — label du cours d'eau) | `0x06512` | 2 |

Label : `${GRAPHIE}`

### 5.28 Détail hydrographique (POI — TOPONYMIE CLASSE)

| NATURE (DETAIL_HYDROGRAPHIQUE) | Type | EndLevel |
|-------------------------------|------|----------|
| Cascade | `0x06508` | 2 |
| Citerne | `0x06414` | 2 |
| Fontaine | `0x06509` | 2 |
| Lavoir | `0x06414` | 2 |
| Marais | `0x06513` | 2 |
| Perte | `0x06414` | 2 |
| Point d'eau | `0x06414` | 2 |
| Résurgence | `0x06414` | 2 |
| Source | `0x06511` | 2 |
| Source captée | `0x06511` | 2 |
| * (catch-all) | `0x06414` | 2 |

Label : `${GRAPHIE}`

### 5.29 Equipement de transport (POI — TOPONYMIE CLASSE)

| NATURE (EQUIPEMENT_DE_TRANSPORT) | Type | EndLevel |
|----------------------------------|------|----------|
| Aérogare | `0x02d0b` | 2 |
| Aire de repos ou de service | `0x02b03` | 2 |
| Aire de triage | `0x02f08` | 2 |
| Arrêt voyageurs | `0x02f08` | 2 |
| Autre équipement | `0x02f08` | 2 |
| Carrefour | `0x06406` | 2 |
| Gare fret uniquement | `0x02f08` | 2 |
| Gare maritime | `0x06401` | 2 |
| Gare routière | `0x02f08` | 2 |
| Gare téléphérique | `0x02f08` | 2 |
| Gare voyageurs et fret | `0x02f08` | 2 |
| Gare voyageurs uniquement | `0x02f08` | 2 |
| Parking | `0x1100b` | 2 |
| Péage | `0x02f01` | 2 |
| Port | `0x06401` | 2 |
| Service dédié aux véhicules | `0x02f01` | 2 |
| Service dédié aux vélos | `0x1160a` | 2 |
| Station de métro | `0x02f08` | 2 |
| Station de tramway | `0x02f08` | 2 |
| Tour de contrôle aérien | `0x02d0b` | 2 |
| * (catch-all) | `0x02f08` | 2 |

Label : `${GRAPHIE}`

### 5.30 Itinéraire autre (POI — TOPONYMIE CLASSE)

| NATURE (ITINERAIRE_AUTRE) | Type | EndLevel |
|---------------------------|------|----------|
| Autre | `0x06412` | 2 |
| Itinéraire cyclable | `0x1160a` | 2 |
| Itinéraire de randonnée pédestre | `0x06412` | 2 |
| Itinéraire équestre | `0x02d0a` | 2 |
| Parcours sportif | `0x11601` | 2 |
| Sentier de découverte | `0x06412` | 2 |
| * (catch-all) | `0x06412` | 2 |

Label : `${GRAPHIE}`

### 5.31 Plan d'eau (POI — TOPONYMIE CLASSE)

| NATURE (PLAN_D_EAU) | Type | EndLevel |
|----------------------|------|----------|
| Canal | `0x06506` | 2 |
| Ecoulement naturel | `0x06414` | 2 |
| Glacier, névé | `0x0650a` | 2 |
| Lac | `0x0650d` | 2 |
| Marais | `0x06513` | 2 |
| Mare | `0x06513` | 2 |
| Plan d'eau de gravière | `0x06414` | 2 |
| Réservoir-bassin | `0x0650f` | 2 |
| Réservoir-bassin piscicole | `0x0650f` | 2 |
| Retenue | `0x0650f` | 2 |
| Retenue-barrage | `0x0650f` | 2 |
| Retenue-bassin portuaire | `0x0650f` | 2 |
| Retenue-digue | `0x0650f` | 2 |
| * (catch-all) | `0x06414` | 2 |

Label : `${GRAPHIE}`

### 5.32 Poste de transformation (POI — TOPONYMIE CLASSE)

| NATURE (POSTE_DE_TRANSFORMATION) | Type | EndLevel |
|----------------------------------|------|----------|
| * (NATURE vide) | `0x11506` | 2 |

Label : `${GRAPHIE}`

### 5.33 Route nommée (POI — TOPONYMIE CLASSE)

| NATURE (ROUTE) | Type | EndLevel |
|----------------|------|----------|
| Route nommée | `0x06401` | 2 |
| * (catch-all) | `0x06401` | 2 |

Label : `${GRAPHIE}`

### 5.34 Transport par câble (POI — TOPONYMIE CLASSE)

| NATURE (TRANSPORT_PAR_CABLE) | Type | EndLevel |
|------------------------------|------|----------|
| Tapis roulant de stations de montagne | `0x15501` | 2 |
| Télécabine, téléphérique | `0x15501` | 2 |
| Télésiège | `0x15501` | 2 |
| Téléski | `0x15501` | 2 |
| * (catch-all) | `0x15501` | 2 |

Label : `${GRAPHIE}`

### 5.35 Voie ferrée nommée (POI — TOPONYMIE CLASSE)

| NATURE (VOIE_FERREE) | Type | EndLevel |
|----------------------|------|----------|
| * (NATURE vide — label de la voie ferrée) | `0x02f08` | 2 |

Label : `${GRAPHIE}`

