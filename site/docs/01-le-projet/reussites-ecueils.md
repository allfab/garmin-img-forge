# Réussites et écueils

Ce projet a été un parcours d'apprentissage intense. Voici un retour d'expérience honnête sur ce qui a fonctionné, ce qui a été difficile, et les leçons tirées.

---

## Réussites

### Le driver GDAL/OGR fonctionne

La première grande victoire a été de faire accepter par GDAL un format qu'il n'avait jamais connu. Le driver **ogr-polishmap** est 100 % conforme aux 12 conventions GDAL, passe tous les tests, et s'intègre naturellement dans l'écosystème (ogr2ogr, QGIS, Python/GDAL).

Cela a débloqué tout le reste : une fois que GDAL sait écrire du Polish Map, n'importe quel outil de l'écosystème SIG peut devenir une source de données pour les cartes Garmin.

### Le binaire statique mpforge avec GDAL embarqué

Compiler un binaire Rust qui embarque statiquement GDAL 3.10.1, PROJ, GEOS et le driver ogr-polishmap a été un défi technique majeur — mais le résultat est spectaculaire : **un seul fichier exécutable, zéro dépendance**, qui tourne sur n'importe quelle distribution Linux.

Plus besoin d'installer GDAL manuellement, plus de problèmes de versions incompatibles.

### imgforge remplace mkgmap

Écrire un compilateur Garmin IMG from scratch en Rust, capable de générer les sous-fichiers TRE, RGN, LBL, NET, NOD et DEM, a été le défi le plus ambitieux du projet. Le résultat : un binaire unique de quelques Mo qui remplace un JAR Java de 40+ Mo, avec des performances nettement supérieures grâce à la parallélisation native.

### Le routing fonctionne

Générer les sous-fichiers NET et NOD pour le calcul d'itinéraire turn-by-turn a été un travail d'ingénierie inverse minutieux. Le routing Garmin est l'une des parties les plus complexes et les moins documentées du format IMG. Après de nombreuses itérations, les cartes produites par imgforge permettent la navigation GPS.

### Le DEM/Hill Shading

L'intégration du DEM (Digital Elevation Model) avec support natif des formats HGT (SRTM) et ASC (BDAltiv2 IGN), reprojection intégrée et encodage multi-niveaux, permet de produire des cartes avec ombrage du relief et profils d'altitude — directement sur le GPS, sans post-traitement.

---

## Écueils et difficultés

### Le format Garmin IMG n'est pas documenté

Le format IMG est propriétaire et Garmin ne publie pas de spécification. Tout le travail de développement d'imgforge a reposé sur de l'ingénierie inverse : analyser des fichiers IMG existants octet par octet, étudier le code source de mkgmap (Java, 100 000+ lignes), et tester empiriquement sur des GPS physiques.

Certains sous-fichiers (NOD en particulier) ont des structures d'encodage extrêmement complexes avec des formats de compression bitstream, des deltas signés et des plateaux — décoder puis ré-encoder ces structures a nécessité de nombreuses itérations.

### Les multi-géométries et le format Polish Map

Le format Polish Map ne supporte que les géométries simples (Point, LineString, Polygon). Or la BD TOPO contient des MultiPolygon, MultiLineString, etc. Le driver ogr-polishmap décompose automatiquement les multi-géométries, mais cette étape peut générer un grand nombre de features supplémentaires et nécessite une attention particulière à la qualité géométrique.

### L'encodage des caractères

Le passage UTF-8 (données sources) vers CP1252 (format Polish Map par défaut) puis vers les formats d'encodage Garmin (Format 6/9/10) est un nid à bugs. Les caractères spéciaux, les accents, les caractères non-latins... chaque étape de la chaîne peut corrompre les labels si l'encodage n'est pas géré correctement.

### Les limites du format Polish Map

- Maximum 1024 points par polyligne — les longues rivières ou routes doivent être découpées
- Coordonnées en degrés décimaux WGS84 uniquement — les données en projection locale doivent être reprojetées
- Pas de support natif des courbes de Bézier ou des arcs
- Encodage CP1252 par défaut — les caractères hors du jeu latin-1 nécessitent UTF-8

### La taille des données BD TOPO

35 Go de données vectorielles pour la France entière, c'est massif. Les premiers prototypes de mpforge prenaient des heures. L'ajout de la parallélisation (rayon), de l'indexation spatiale (R-tree) et de l'option `--skip-existing` a été nécessaire pour rendre le pipeline viable en production.

---

## Leçons apprises

1. **Commencer par le driver GDAL** a été le bon choix. En s'intégrant dans l'écosystème existant plutôt que de tout réinventer, on a immédiatement bénéficié de toute la puissance de GDAL.

2. **Le format intermédiaire Polish Map** est essentiel pour le débogage. Pouvoir inspecter les fichiers texte `.mp` avant la compilation binaire a sauvé des centaines d'heures de débogage.

3. **Rust** s'est révélé un excellent choix : performances proches du C, sécurité mémoire, écosystème de bibliothèques (rayon, clap, serde), et surtout la capacité de produire des binaires statiques sans dépendances.

4. **La configuration déclarative YAML** rend le pipeline accessible aux non-développeurs. On décrit *ce qu'on veut*, pas *comment le faire*.

5. **L'ingénierie inverse est un marathon**, pas un sprint. Il faut accepter de ne pas comprendre certaines structures pendant des semaines, puis d'avoir un éclair de compréhension en comparant deux fichiers hexadécimaux.
