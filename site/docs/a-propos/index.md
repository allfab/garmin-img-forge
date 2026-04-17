# À propos

## L'auteur

**Fabien ALLAMANCHE** (@allfab) — Géomaticien à Vienne Condrieu Agglomération.

> *L'informatique et les nouvelles technologies sont mes passions de toujours.*

### Parcours

Titulaire d'un DUT en Génie Civil obtenu à l'IUT A de Lyon (2004), Fabien s'est formé en autodidacte dans le domaine de la géomatique. Il cumule plus de 20 ans d'expérience professionnelle dans ce secteur.

### Domaines d'expertise

- **Géomaticien-généraliste** — Gestion complète de la donnée géographique : acquisition, analyse, représentation
- **Géomaticien-informaticien** — Développement spécialisé en géomatique et administration système
- **Géomaticien-thématicien** — Analyse territoriale des projets

Il apporte également conseils en gestion de projets, assistance technique et veille technologique en recherche et développement.

### Liens

- [Forgejo](https://forgejo.allfabox.fr/allfab) — Dépôt source du projet
- [GitHub](https://github.com/allfab) — Profil GitHub
- [Blog](https://f84.allfab.fr) — Blog personnel

---

## Le projet

**GARMIN IMG FORGE** est un projet personnel né de la volonté de créer des cartes topographiques Garmin en utilisant exclusivement des logiciels et données libres.

Le projet s'inscrit dans une démarche **FOSS** (Free and Open Source Software) de bout en bout : des données ouvertes (BD TOPO IGN, licence Etalab 2.0) transformées par des outils open-source (ogr-polishmap, mpforge, imgforge) en cartes prêtes à l'emploi pour les GPS Garmin.

### Inspirations

Ce travail s'appuie sur les fondations posées par la communauté cartographique Garmin, notamment :

- Les articles de **GPSFileDepot** (2008, mis à jour 2016) sur la création de cartes Garmin personnalisées
- Le projet **mkgmap** — compilateur Java open-source qui a démontré qu'il était possible de produire des fichiers IMG sans outils propriétaires
- La documentation de **cGPSmapper** — qui a défini le format Polish Map comme standard intermédiaire

### Licences

Le projet adopte un modèle de licences hybride, adapté à la nature de chaque composant :

| Composant | Licence | Raison |
|-----------|---------|--------|
| **ogr-polishmap** | MIT | Driver GDAL — compatibilité avec l'écosystème GDAL (MIT/X), facilite une éventuelle intégration upstream |
| **mpforge** | GPL v3 | Outil standalone — copyleft, les dérivés doivent rester open-source |
| **imgforge** | GPL v3 | Compilateur Garmin IMG — alignement avec mkgmap (GPL v2), les dérivés doivent rester ouverts |
| **Documentation / site** | CC BY-SA 4.0 | Standard pour la documentation, avec attribution obligatoire |
| **Cartes produites** | Etalab 2.0 | Héritée des données IGN (BD TOPO) |

### Contribuer

Les contributions sont les bienvenues :

- **Issues** : [forgejo.allfabox.fr/allfab/garmin-img-forge/issues](https://forgejo.allfabox.fr/allfab/garmin-img-forge/issues)
- **Code source** : [forgejo.allfabox.fr/allfab/garmin-img-forge](https://forgejo.allfabox.fr/allfab/garmin-img-forge)

!!! tip "Ancien site"
    L'ancien site du projet reste disponible à l'adresse [allfab.github.io/garmin-ign-bdtopo-map](https://allfab.github.io/garmin-ign-bdtopo-map/).
