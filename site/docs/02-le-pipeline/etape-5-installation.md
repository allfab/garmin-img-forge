# Étape 5 : Installation sur le GPS

La dernière étape est la plus simple : copier le fichier `gmapsupp.img` sur le GPS Garmin.

---

## Procédure

### 1. Connecter le GPS

Connectez votre GPS Garmin en USB à votre ordinateur, ou insérez la carte SD dans un lecteur.

Le GPS (ou la carte SD) apparaît comme un périphérique de stockage de masse.

### 2. Copier le fichier

```bash
# Identifier le point de montage du GPS
lsblk
# ou
mount | grep -i garmin

# Copier le fichier
cp output/gmapsupp.img /media/$USER/GARMIN/Garmin/

# Ou sur la carte SD
cp output/gmapsupp.img /media/$USER/SD_CARD/Garmin/
```

!!! info "Emplacement du fichier"
    Le fichier `gmapsupp.img` doit être placé dans le dossier `Garmin/` à la racine du GPS ou de la carte SD. C'est le nom standard reconnu automatiquement par tous les GPS Garmin.

### 3. Redémarrer le GPS

Éjectez proprement le périphérique, puis redémarrez le GPS. La carte apparaît automatiquement dans la gestion des cartes.

## Appareils compatibles

Ces cartes sont compatibles avec tous les GPS Garmin supportant les cartes supplémentaires :

| Catégorie | Modèles |
|-----------|---------|
| **Montres outdoor** | fenix, Enduro, Instinct (certains modèles) |
| **GPS de randonnée** | Oregon, eTrex, Montana, GPSMAP |
| **GPS vélo** | Edge (certains modèles) |
| **Suivi canin** | Alpha 100F/200F/300F/50F, Astro 320 |

## Gestion de plusieurs cartes

Si vous avez déjà un fichier `gmapsupp.img` sur votre GPS (par exemple une carte OSM), renommez l'un des deux pour éviter les conflits :

```bash
# Renommer la carte existante
mv /media/$USER/GARMIN/Garmin/gmapsupp.img /media/$USER/GARMIN/Garmin/gmapsupp_osm.img

# Copier la nouvelle carte
cp output/gmapsupp.img /media/$USER/GARMIN/Garmin/gmapsupp_bdtopo.img
```

Les GPS Garmin reconnaissent automatiquement tous les fichiers `.img` dans le dossier `Garmin/`, quel que soit leur nom.

## Activation/désactivation de la carte

Sur le GPS, allez dans **Configuration > Carte > Information carte** pour activer ou désactiver les cartes installées. Cela permet de basculer entre plusieurs cartes sans les supprimer.

## Vérification sur le GPS

Une fois la carte chargée, vérifiez :

- Les **routes** s'affichent correctement (zoom avant/arrière)
- Les **POI** sont cliquables et affichent leur nom
- Les **polygones** (forêts, lacs, bâtiments) sont remplis avec les bonnes couleurs
- Le **routing** fonctionne (si activé) : calculer un itinéraire entre deux points
- Le **relief** (hill shading) est visible si les données DEM ont été intégrées
