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

### La compatibilité Garmin Alpha 100 (avril 2026)

L'une des batailles les plus techniques du projet a été de rendre les fichiers `gmapsupp.img` produits par imgforge compatibles avec le **Garmin Alpha 100** — un GPS de terrain avec un firmware particulièrement strict sur la structure binaire des cartes.

Les cartes compilées par imgforge fonctionnaient parfaitement sur Garmin BaseCamp (logiciel PC), mais le GPS physique affichait systématiquement « pas de données » ou ignorait complètement le fichier.

**La méthodologie d'investigation** a été chirurgicale :

1. Compilation de la même tuile `.mp` avec les deux outils (imgforge et mkgmap)
2. Comparaison binaire des sous-fichiers (TRE, RGN, LBL) octet par octet
3. **Tests hybrides** : remplacement de sous-fichiers entre les deux outils pour isoler le composant défaillant
4. Tests itératifs sur le GPS physique, cycle après cycle

Les tests hybrides ont été la clé : en combinant le TRE+RGN de mkgmap avec le LBL d'imgforge, le GPS fonctionnait. L'inverse ne marchait pas. Le problème était donc localisé dans le **TRE+RGN** (index spatial + données features) et pas dans le LBL (labels).

**10 corrections** ont été nécessaires avant d'obtenir un fichier fonctionnel :

| Phase | Corrections |
|-------|-------------|
| **Structure gmapsupp** | Ordre des sous-fichiers (MPS en premier), SRT sort descriptor obligatoire, TYP obligatoire, TDB interdit dans le conteneur |
| **TRE (index spatial)** | Demi-largeur des subdivisions (half-extent vs full), sections ext type toujours présentes, niveaux de zoom complets même vides, flag `is_last` par groupe parent |
| **RGN (données)** | **Background polygon 0x4B** manquant dans chaque subdivision, points mal classés en section indexed (0x20) au lieu de regular (0x10) |

Les deux dernières corrections — le **polygone background 0x4B** et la **classification des points** — ont résolu le problème. mkgmap ajoute automatiquement un polygone de type 0x4B couvrant la zone de chaque subdivision (c'est le « fond de carte »), et classe les points normaux dans la section regular du RGN. imgforge ne faisait ni l'un ni l'autre.

Cette investigation a mobilisé l'analyse du code source de mkgmap (~100 000 lignes Java), de cGPSmapper, et la comparaison structurelle de dizaines de fichiers IMG. Le détail complet est documenté dans `docs/investigation-imgforge-alpha100.md` (documentation interne).

### Les quadrants FRANCE-SE — bataille d'avril 2026

Après la victoire sur le rendu départemental, passer à l'échelle **quadrant** (25 départements sur `FRANCE-SE` — Auvergne-Rhône-Alpes, PACA, sud d'Occitanie, Corse) a révélé deux nouveaux blocages sur l'Alpha 100.

#### Bug 1 — L'Alpha 100 plante au boot sur les gros quadrants

Le premier build FRANCE-SE (3,5 Go) faisait littéralement **redémarrer l'appareil** au moment du chargement de la carte. Aucune erreur, aucun message : reboot sec. Les builds départementaux (~170 Mo) restaient parfaitement fonctionnels.

**La cause n'était pas la taille du fichier** — la référence mkgmap FRANCE-SUD (moitié sud complète, 3,19 Gio) se chargeait sans problème. C'est le **nombre d'entrées FAT dans le gmapsupp.img** qui était le facteur limitant :

| Métrique | mkgmap FRANCE-SUD (OK) | imgforge FRANCE-SE (plantait) |
|---|---|---|
| Tuiles | **98** | **702** |
| Subfiles par tuile | 4 (TRE/RGN/LBL/DEM) | jusqu'à 6 (+ NET+NOD selon routing) |
| **Entrées FAT mesurées** *(parse du gmapsupp.img réel)* | **~392** | **4 095** |

Le firmware Alpha 100 charge la table d'allocation des fichiers en RAM au boot. À 4 095 entrées, la mémoire disponible est dépassée et l'appareil redémarre.

**Fix** : augmenter la taille des tuiles mpforge de `cell_size: 0.15°` (~16 km, 193 km²) à `0.45°` (~50 km, 1 750 km²). FRANCE-SE est alors tombé à **136 tuiles** soit ~550 entrées FAT — proche de la référence mkgmap. La carte se charge maintenant sans problème.

La nouveauté conceptuelle : `cell_size` n'impacte pas la qualité de rendu (le splitter RGN d'imgforge subdivise automatiquement les grosses tuiles en interne), seulement le découpage du filesystem du gmapsupp. Pour tout nouveau quadrant, viser ≤ 250 tuiles est la règle — `0.45°` pour un quadrant, `0.30°` pour un régional ou un département.

#### Bug 2 — Artefacts géométriques sur les communes denses

Après le fix Bug 1, la carte se chargeait... mais **des communes entières manquaient par blocs** (Marseille, Nice, Lyon) sous QmapShack et Alpha 100. Les logs du build affichaient alors des **milliers de warnings** `Subdivision X RGN size Y exceeds MAX_RGN_SIZE 65528`, avec certaines subdivisions à **252 KiB — quatre fois la limite Garmin** de 64 KiB.

La cause : une constante à une seule ligne dans `tools/imgforge/src/img/splitter.rs` :

```rust
// Step 2: Recursive splitting until all areas fit limits
add_areas_to_list(initial, 8)  // max_depth = 8
```

Avec `cell_size: 0.45°` (1 750 km²/tuile) et les agglomérations denses, le splitter abandonnait à la profondeur 8 sans avoir suffisamment subdivisé les zones urbaines. Les subdivisions restantes étaient alors écrites telles quelles, trop grosses pour que le format Garmin puisse les encoder → données corrompues → communes manquantes.

**Le piège** : la lecture attentive du code source mkgmap a révélé que **mkgmap n'impose aucune limite de profondeur**. Dans `MapSplitter.java`, la fonction `addAreasToList(areas, alist, 0)` est initiée avec `depth=0` (L113) et se rappelle avec `depth+1` (L186) sans jamais tester de plafond — le paramètre `depth` n'est utilisé que comme padding visuel des logs (L140-141). Les vraies conditions d'arrêt sont la taille atteinte, la dimension minimale, et l'incapacité à splitter une feature unique.

`max_depth=8` était donc un écart silencieux d'imgforge par rapport à mkgmap, pas une fidélité. Le fix a consisté à passer `usize::MAX` :

```rust
add_areas_to_list(initial, usize::MAX)
```

Après recompilation, zéro warning `MAX_RGN_SIZE` dans les logs — les subdivisions tiennent toutes sous 64 KiB. Les communes sont toutes rendues correctement sur Alpha 100 et QmapShack.

**Leçon** : tout plafond hard-codé dans imgforge qui n'a pas son équivalent explicite dans mkgmap est suspect par défaut. L'analyse comparative ligne-à-ligne reste la bonne méthode.

#### Conséquence — OOM mémoire au build

Le fix Bug 2 a débloqué la qualité géométrique, mais avec `usize::MAX` de profondeur, le splitter fait exploser la RAM sur les zones très denses : chaque subdivision clone ses features (points/lignes/polygones), et avec 4 workers imgforge en parallèle sur des tuiles Marseille/Lyon, le pic mémoire dépasse les 32 Go disponibles → OOM killer (`exit 137`).

**Workaround immédiat** : `--imgforge-jobs 2 --merge-lines`. `--merge-lines` fusionne les polylignes adjacentes (option par défaut de mkgmap, jamais activée côté imgforge jusqu'ici) — réduction significative du nombre de polylignes en mémoire. Avec 2 jobs au lieu de 4, le build FRANCE-SE tient en RAM.

**Solution propre documentée** : une tech-spec de refactor du splitter (move-not-clone + drop parent) permettra à terme de revenir à 4 jobs. À implémenter dans une itération dédiée.

Commits de référence : [`e6fce3f`](https://github.com/allfab/garmin-img-forge/commit/e6fce3f) (cell_size), [`7cef948`](https://github.com/allfab/garmin-img-forge/commit/7cef948) (splitter max_depth), [`7e4a8f2`](https://github.com/allfab/garmin-img-forge/commit/7e4a8f2) (`--skip-existing` publish-only).

### Le packaging GMP (format Garmin NT consolidé) — avril 2026

Toutes les cartes Garmin commerciales modernes (Topo France v6 Pro, Topo Active…) utilisent le format **GMP** : au lieu de 6 fichiers FAT séparés par tuile (`TRE/RGN/LBL/NET/NOD/DEM`), un seul fichier `.GMP` les encapsule tous. Sur un build France entière (~1 500 tuiles), cela représente ~9 000 entrées FAT en mode `legacy` contre ~1 500 en mode `gmp` — une réduction de 83 %.

L'implémentation du `GmpWriter` a été plus complexe que prévu. Le format du conteneur lui-même est relativement simple (header 61 bytes + copyright 179 bytes + blobs concatenés avec relocalisation des offsets), mais le firmware Alpha 100 impose des contraintes sur le **contenu interne** du TRE embarqué dans le GMP — contraintes que les cartes commerciales Garmin satisfont implicitement, et qui ne sont documentées nulle part.

La validation a demandé **5 cycles de test hardware** (GC1-GC5) et un bug de relocalisation DEM découvert uniquement sur build production avec données d'altimétrie réelles. La root cause finale : un TRE avec extension NT (`hlen=309`) et sections vides à l'intérieur d'un GMP est rejeté par le firmware Alpha 100 — le TRE standard (`hlen=188`) fonctionne parfaitement.

**Résultat** : `--packaging gmp` est fonctionnel en production depuis le 25 avril 2026, validé sur Alpha 100 avec les données IGN BD TOPO D038 (routing + altimétrie BDAltiv2).

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

~40 Go de données vectorielles pour la moitié sud de la France, c'est massif. Les premiers prototypes de mpforge prenaient des heures. L'ajout de la parallélisation (rayon), de l'indexation spatiale (R-tree) et de l'option `--skip-existing` a été nécessaire pour rendre le pipeline viable en production.

### L'implémentation du conteneur GMP — contraintes firmware non documentées

Implémentation de `GmpWriter` pour produire le format GMP Garmin NT. Le conteneur lui-même n'est pas difficile à implémenter (spec partiellement disponible dans `tmp/gimgtools/garmin_struct.h`). La difficulté venait entièrement des **contraintes firmware Alpha 100** sur le contenu interne, révélées par tests hardware itératifs.

**Obstacle 1 — L'extension NT du TRE (`hlen=309`)**

Les cartes Garmin officielles ont un TRE avec `hlen=309` à l'intérieur du GMP, et leurs 121 bytes d'extension NT contiennent des données valides. Notre TRE produisait ces 121 bytes tous à zéro (sections absentes). Le firmware Alpha 100 a des comportements différents selon la valeur de `hlen` :

- `hlen=309` + `tre10_rec_size=0` → **crash** (division par zéro : `count = size / rec_size`)
- `hlen=309` + `tre10_rec_size=1` + reste à zéro → **tuile invisible** (sections NT vides invalident l'enregistrement)
- `hlen=188` (TRE standard) → ✅ **tuile visible et fonctionnelle**

Cinq cycles de test hardware ont été nécessaires pour converger sur cette conclusion : le firmware Alpha 100 préfère un TRE standard à l'intérieur d'un GMP plutôt qu'un TRE NT avec des sections vides. La substitution d'un GMP officiel Garmin (GC1) a d'abord permis de confirmer que le format du conteneur était correct — le problème venait du contenu du TRE produit par `GmpWriter`.

**Obstacle 2 — La relocalisation DEM (`relocate_dem`)**

Les offsets internes des blobs standalone doivent être relocalisés en GMP-absolus. Pour le DEM, chaque section-header de 60 bytes contient deux champs à patcher (`data_offset` à +32 et `data_offset2` à +36). La première implémentation patchait les mauvaises positions (+20 et +24, soit `tiles_lon-1` et `tiles_lat-1`).

Ce bug est resté invisible pendant toute la phase de tests synthétiques, car les tests d'intégration utilisaient `dem: None`. Il n'est apparu qu'en build production avec des données BDAltiv2 réelles : `tiles_lon-1` passait de 1 à ~1290, le firmware tentait d'allouer une table de 1290 descripteurs DEM et rejetait le fichier.

**Leçon** : pour tout format binaire avec des sections optionnelles, les tests d'intégration doivent couvrir tous les sous-types — y compris NET, NOD et DEM. Un test avec `dem: None` ne valide pas `relocate_dem`.

---

## Leçons apprises

1. **Commencer par le driver GDAL** a été le bon choix. En m'intégrant dans l'écosystème existant plutôt que de tout réinventer, j'ai immédiatement bénéficié de toute la puissance de GDAL.

2. **Le format intermédiaire Polish Map** est essentiel pour le débogage. Pouvoir inspecter les fichiers texte `.mp` avant la compilation binaire a sauvé des centaines d'heures de débogage.

3. **Rust** s'est révélé un excellent choix : performances proches du C, sécurité mémoire, écosystème de bibliothèques (rayon, clap, serde), et surtout la capacité de produire des binaires statiques sans dépendances.

4. **La configuration déclarative YAML** rend le pipeline accessible aux non-développeurs. On décrit *ce qu'on veut*, pas *comment le faire*.

5. **L'ingénierie inverse est un marathon**, pas un sprint. Il faut accepter de ne pas comprendre certaines structures pendant des semaines, puis d'avoir un éclair de compréhension en comparant deux fichiers hexadécimaux.

6. **Tester sur le matériel cible** est irremplaçable. Un fichier qui fonctionne sur BaseCamp peut échouer silencieusement sur un GPS physique. Le firmware Garmin Alpha 100 impose des contraintes non documentées (polygone background obligatoire, structure RGN stricte) que seul un test sur device peut révéler.

7. **Les tests hybrides** (mélanger des sous-fichiers de deux sources) sont une technique de débogage redoutablement efficace pour les formats binaires. En remplaçant un composant à la fois, on isole le coupable en quelques itérations au lieu de chercher dans des centaines de milliers d'octets.
