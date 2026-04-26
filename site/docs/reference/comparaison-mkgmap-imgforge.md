# Comparaison mkgmap / imgforge — taille IMG et lissage géométrique

Cette page analyse pourquoi mkgmap r4924 et imgforge produisent des IMG de tailles différentes et des géométries visuellement distinctes, à partir de la même tuile `.mp`. Elle s'appuie sur les mesures directes de la tuile `BDTOPO-001-004` (Vienne, D038) et sur le code source Java de mkgmap r4924 (`build/MapBuilder.java` L929-1354).

---

## Introduction — la question posée

Pour la même tuile MP (`BDTOPO-001-004.mp`, 43 MB), mkgmap r4924 et imgforge produisent des IMG de tailles différentes. Le ratio de taille évoqué en amont ("6×") résultait d'une comparaison non rigoureuse (périmètres différents : nombre de tuiles, sections incluses). La mesure instrumentée sur la **même** tuile `BDTOPO-001-004` donne un résultat très différent, détaillé ci-dessous.

!!! note "Note historique"
    Un premier audit (commit `8acb0c2`, avril 2026) avait mesuré que la section RGN géométrique d'imgforge était **1,58× plus grande** que celle de mkgmap pour cette même tuile. Depuis, le fix `EndLevel filtering` (commit `6478c47`) a réduit cette section de **63 %**. L'état actuel (commit `975f432`) est documenté dans la section suivante.

---

## §1 — Mesures de référence (état actuel)

### Conditions de mesure

| Paramètre | Valeur |
|---|---|
| Tuile | `BDTOPO-001-004` (Vienne, D038, la plus dense) |
| Fichier `.mp` | `pipeline/output/2026/v2026.03/D038/mp/BDTOPO-001-004.mp` (43 MB) |
| Commit imgforge | `975f432` (HEAD, 2026-04-26) |
| Profil mpforge | `generalize-profiles-local.yaml` (8 couches) |
| Outil de mesure | `scripts/debug/bytes-per-level.py` |

### Méthode

Pour chaque IMG, le script extrait le TRE de la sous-map principale, lit la section `map levels` et la section `subdivisions`, puis agrège les deltas `rgn_offset(i+1) − rgn_offset(i)` par niveau. C'est la mesure utilisée par le firmware Garmin pour sélectionner les données à rendre. Elle exclut les sections étendues (types étendus, NET, NOD, RGN2-RGN5).

### Commandes de reproduction

```bash
# Mesure mkgmap (IMG déjà disponible)
python3 scripts/debug/bytes-per-level.py tmp/mkgmap-vienne-build.img

# Build imgforge mono-tuile (sous-map 00380042)
mkdir -p /tmp/vienne-mp
cp pipeline/output/2026/v2026.03/D038/mp/BDTOPO-001-004.mp /tmp/vienne-mp/
imgforge build /tmp/vienne-mp --output /tmp/vienne-local.img

# Mesure imgforge (analyser le sous-map 00380042, pas 00011855 qui est le conteneur GMP)
python3 scripts/debug/bytes-per-level.py /tmp/vienne-local.img  # lit le premier sous-map
# Note : bytes-per-level.py lit le premier sous-map alphabétique ; pour imgforge GMP,
# le sous-map tuile (00380042) vient après le conteneur (00011855). Voir §A.
```

### Tableau de comparaison

> Convention — colonne **mk÷if** : valeur > 1 signifie que mkgmap a plus de bytes que imgforge à ce niveau.

| n | bits (1) | mkgmap subdivs | mkgmap RGN | imgforge subdivs | imgforge RGN | mk÷if |
|---|----------|----------------|------------|------------------|--------------|-------|
| 6 | 16 (inh.)| 1              | 0          | 1                | 0            | —     |
| 5 | 18       | 95             | 129 713    | 4                | 8 200        | **15,8×** |
| 4 | 20       | 183            | 216 825    | 33               | 54 419       | **3,98×** |
| 3 | 21       | 188            | 242 083    | 41               | 68 354       | **3,54×** |
| 2 | 22       | 225            | 337 546    | 92               | 115 073      | **2,93×** |
| 1 | 23       | 226            | 381 806    | 101              | 143 296      | **2,66×** |
| 0 | 24       | 512            | 510 023    | 609              | 663 942      | 0,77× |
| **Σ** | | **1 430** | **1 817 996** | **881** | **1 053 284** | **1,73×** |

(1) "inh." pour *inherited* : le niveau 6 hérite sa subdivision du niveau 5 ; aucune feature n'y est émise directement.

### Observations clé

1. **imgforge est désormais 1,73× plus compact que mkgmap** en données géométriques RGN : 1 053 284 bytes vs 1 817 996 bytes. Le fix `EndLevel filtering` (commit `6478c47`) a éliminé l'émission erronée aux niveaux larges.

2. **L'écart est maximal aux niveaux larges** : à n=5, mkgmap stocke 15,8× plus de données. Mais à n=0 la tendance s'inverse : imgforge a 30 % de données supplémentaires au niveau de détail maximal.

3. **mkgmap inclut plus de features aux niveaux larges** : 95 subdivisions à n=5 vs 4 pour imgforge. Ce n'est pas de la mauvaise qualité — c'est un choix de conception : mkgmap garde plus de features visibles en dézoom mais les simplifie agressivement via sa chaîne de filtres. imgforge applique le `EndLevel` filtering (features absentes des niveaux au-dessus de leur EndLevel) mais ne simplifie pas la géométrie des features présentes.

4. **Le total IMG mono-tuile reste plus grand pour imgforge** (4,9 MB vs 3,8 MB pour mkgmap), malgré une géométrie ordinale plus petite. La différence vient de l'overhead du conteneur GMP et des sections étendues : la RGN déclarée imgforge est 3,37 MB dont 2,32 MB d'extended types/NET/NOD, contre 3,79 MB déclarée mkgmap dont 1,97 MB d'extended. Ce n'est pas la section géométrique qui explique la différence de taille de l'IMG total — c'est l'overhead de format.

### Baseline historique (avant fix EndLevel)

| n | mkgmap RGN | imgforge RGN (8acb0c2) | imgforge RGN (975f432) | Δ après fix |
|---|------------|------------------------|------------------------|-------------|
| 5 | 129 713    | 346 182                | 8 200                  | −97,6 %     |
| 4 | 216 825    | 394 252                | 54 419                 | −86,2 %     |
| 3 | 242 083    | 434 098                | 68 354                 | −84,3 %     |
| 2 | 337 546    | 496 136                | 115 073                | −76,8 %     |
| 1 | 381 806    | 548 017                | 143 296                | −73,9 %     |
| 0 | 510 023    | 657 224                | 663 942                | +1,0 %      |
| **Σ** | **1 817 996** | **2 875 909** | **1 053 284** | **−63,4 %** |

Le niveau 0 est quasi-identique avant et après le fix : les features à `EndLevel=0` n'étaient pas affectées par le bug. L'amélioration se concentre entièrement sur les niveaux n=1..5.

---

## §2 — La chaîne de filtres mkgmap

### Vue d'ensemble

mkgmap applique une chaîne de filtres par résolution, en deux passes selon le type de feature. Le code source de référence est `MapBuilder.java` L929-1354.

**Gate préliminaire (L929-930) :**

```java
lines = lines.stream().filter(l -> l.getMinResolution() <= res).collect(Collectors.toList());
shapes = shapes.stream().filter(s -> s.getMinResolution() <= res).collect(Collectors.toList());
```

Ce gate est l'équivalent du `EndLevel filtering` d'imgforge (fix TD-1). Une feature dont `MinResolution > res` n'est pas simplifiée — elle n'est pas émise du tout.

### Chaîne polylignes (L1248-1283)

Pour les polylignes normales (`res < 24`, tous les niveaux sauf n=0) :

```
RoundCoordsFilter
→ SizeFilter(MIN_SIZE_LINE=1)
→ RemoveObsoletePointsFilter
→ DouglasPeuckerFilter(2.6 × (1 << shift))
→ RemoveEmpty → LineSplitterFilter → LinePreparerFilter → LineAddFilter
```

!!! note "Courbes de niveau — ordre différent"
    Pour les courbes de niveau (`isContourLine`) et les features overview, mkgmap utilise `keepParallelFilters` avec un ordre différent : **DP en premier**, puis RoundCoords → SizeFilter → RemoveObsolete. Cela évite que RoundCoords introduise de fausses colinéarités avant la simplification.

### Chaîne polygones (L1313-1335)

```
PolygonSplitterFilter
→ RoundCoordsFilter
→ RemoveObsoletePointsFilter
→ SizeFilter(min-size-polygon=8)
→ DouglasPeuckerFilter(2.6 × (1 << shift))
→ RemoveEmpty → LinePreparerFilter → ShapeAddFilter
```

### Paramètres par défaut

| Paramètre CLI | Valeur défaut | Effet |
|---|---|---|
| `reduce-point-density` | `2.6` | Multiplicateur DP (coefficient) |
| `reduce-point-density-polygon` | `−1` (= identique à lignes) | Multiplicateur DP polygones |
| `min-size-polygon` | `8` | Taille min (× shift) pour polygones |
| `merge-lines` | *non activé* | Fusion des segments de même type — nécessite `--merge-lines` |
| `MIN_SIZE_LINE` | `1` (constante Java) | Taille min pour polylignes |

---

## §3 — Filtre par filtre

### RoundCoordsFilter

Quantifie chaque coordonnée à la grille de résolution `(1 << shift)` unités Garmin. Une unité Garmin ≈ 2,14 m (latitude, aux latitudes de la France). **Désactivé à res=24** (`enableLineCleanFilters` requiert `res < 24`).

| n | res | shift | Taille de cellule | Effet |
|---|-----|-------|-------------------|-------|
| 0 | 24  | —     | *filtre désactivé (res=24)* | Aucun |
| 1 | 23  | 1     | 2 unités ≈ 4 m    | Micro-jitter < 4 m fusionné |
| 2 | 22  | 2     | 4 unités ≈ 9 m    | — |
| 3 | 21  | 3     | 8 unités ≈ 17 m   | Virages < 17 m fusionnés |
| 4 | 20  | 4     | 16 unités ≈ 34 m  | — |
| 5 | 18  | 6     | 64 unités ≈ 137 m | Tout détail < 137 m disparaît |
| 6 | 16  | 8     | 256 unités ≈ 549 m | Seulement les grandes inflexions |

**Mode spécial courbes de niveau** : pour chaque point intermédiaire, mkgmap teste les **4 coins de la cellule** et choisit celui qui minimise la somme des distances au segment avant et après (`calcDistortion`). Ce mode "best-fit" produit un tracé naturellement aligné sur la grille, visuellement plus fluide que l'arrondi naïf.

!!! warning "Absent d'imgforge"
    imgforge n'implémente pas `RoundCoordsFilter`. Les coordonnées sont écrites à pleine précision WGS84 (24 bits), quelle que soit la résolution. L'absence de quantification produit des points "sub-pixel" qui ne contribuent pas au rendu mais occupent des bytes RGN.

### SizeFilter

Supprime les features dont la bounding box est trop petite pour être visible à la résolution courante. **Désactivé à res=24.**

```
maxDimension < minSize × (1 << shift)
```

| Feature | minSize | shift=6 (n=5) | seuil ≈ |
|---|---|---|---|
| Polyligne | `MIN_SIZE_LINE = 1` | 1 × 64 = 64 unités | ~137 m |
| Polygone | `min-size-polygon = 8` (défaut) | 8 × 64 = 512 unités | ~1 096 m |

Exemple : une portion de `CONSTRUCTION_LINEAIRE` (mur, haie) de 20 m de long disparaît dès n=3 (shift=3, seuil = 8 unités ≈ 17 m pour les lignes). Cela réduit la RGN aux niveaux larges sans affecter le zoom maximal.

!!! warning "Absent d'imgforge"
    imgforge n'implémente pas `SizeFilter`. Des features trop petites pour être visibles sont émises à tous les niveaux traversés (jusqu'à leur EndLevel), augmentant inutilement la RGN.

### DouglasPeuckerFilter

L'erreur maximale est scalée exponentiellement par niveau :

```java
// DouglasPeuckerFilter.java L43
maxErrorDistance = filterDistance * (1 << config.getShift());
// avec filterDistance = 2.6 (défaut) et shift = 24 - res
// Désactivé à res=24 (enableLineCleanFilters requiert res < 24)
```

| n | res | shift | Erreur DP (unités) | Erreur DP (mètres) | Profil local mpforge (routes communales) |
|---|-----|-------|---------------------|---------------------|------------------------------------------|
| 0 | 24  | —     | *filtre désactivé*  | —                   | simplify_vw 0,000003° ≈ 0,33 m |
| 1 | 23  | 1     | 5,2                 | ~11 m               | 0,000005° ≈ 0,56 m |
| 2 | 22  | 2     | 10,4                | ~22 m               | 0,000010° ≈ 1,1 m |
| 3 | 21  | 3     | 20,8                | ~45 m               | 0,000018° ≈ 2 m |
| 4 | 20  | 4     | 41,6                | ~89 m               | 0,000070° ≈ 7,8 m |
| 5 | 18  | 6     | 166,4               | ~356 m              | 0,000130° ≈ 14 m |
| 6 | 16  | 8     | 665,6               | ~1 426 m            | 0,000300° ≈ 33 m |

**mkgmap est 10 à 40× plus agressif** que notre profil aux niveaux de dézoom (n=4..6). À n=6, mkgmap accepte une erreur de ~1,4 km — seules les grandes inflexions du réseau routier survivent.

### RemoveObsoletePointsFilter

Élimine après chaque filtre :

- Les points dupliqués (coordonnées identiques après arrondi)
- Les points colinéaires stricts (`STRICTLY_STRAIGHT`)
- Les spikes (inversions brusques de direction)

Appliqué après `RoundCoordsFilter` pour les polylignes et polygones. Le nettoyage post-quantification est crucial : `RoundCoordsFilter` peut projeter deux coordonnées distinctes vers la même valeur arrondie, produisant des doublons que `RemoveObsolete` élimine immédiatement.

!!! warning "Absent d'imgforge"
    imgforge n'implémente pas `RemoveObsoletePointsFilter`. Les doublons et colinéaires stricts restent dans la RGN à tous les niveaux.

### SmoothingFilter — code mort

```java
// SmoothingFilter.java — jamais instancié dans MapBuilder r4924
// stepsize = 5 << shift — moyenne glissante de groupes de points adjacents
```

`SmoothingFilter` est présent dans les sources de mkgmap r4924 mais **jamais instancié** dans `MapBuilder.java`. C'est un artefact historique. L'effet lissant visuel de mkgmap vient de `RoundCoordsFilter` (quantification grille, mode best-fit courbes de niveau) et de `DouglasPeuckerFilter` (élimination des micro-déviations), pas de ce filtre.

Le lissage `smooth: chaikin` d'imgforge/mpforge (itérations de subdivision de courbes) n'a pas d'équivalent dans mkgmap r4924 — les deux outils lissent par des mécanismes orthogonaux.

### LineMergeFilter — option avancée

`LineMergeFilter` est instancié dans `MapBuilder.java` L933 uniquement si l'option `--merge-lines` est passée à mkgmap. **Non activé par défaut.** Ce filtre fusionne des segments de même type adjacents pour réduire le nombre de features distinctes. Il n'est pas inclus dans la chaîne standard documentée ici.

---

## §4 — Comparaison avec notre pipeline

### Ce qu'implémente mpforge/imgforge

| Mécanisme mkgmap | Équivalent mpforge/imgforge | Couverture |
|---|---|---|
| Gate `minResolution <= res` | `filter_features_for_level` (`writer.rs`) | ✅ Fix TD-1 commit `6478c47` |
| `DouglasPeuckerFilter` | `simplify` / `simplify_vw` dans profil YAML | ⚠️ 8 couches seulement, tolérances 10-40× inférieures aux niveaux larges |
| `RoundCoordsFilter` | Absent | ❌ |
| `SizeFilter` lignes/polygones | Absent | ❌ |
| `RemoveObsoletePointsFilter` | Absent | ❌ |
| Lissage `smooth: chaikin` | `geometry_smoother.rs` | ⚠️ Quelques couches polygones seulement, pas de scaling par résolution |
| `LineMergeFilter` | Absent (option mkgmap non standard) | — |
| `SmoothingFilter` | N/A (code mort mkgmap) | — |

### Couverture des couches par generalize-profiles-local.yaml

Sur les **124 863 features** de `BDTOPO-001-004.mp` (Vienne), le profil local couvre **8 couches** représentant **34 % des features**. Les 66 % restants traversent le pipeline sans simplification géométrique par niveau.

**Couches couvertes (profil avec simplification) :**

| Couche | Features | EndLevel max | Notes |
|---|---|---|---|
| `TRONCON_DE_ROUTE` | 24 571 | 6 | Algorithme VW, dispatch par `CL_ADMIN` |
| `ZONE_DE_VEGETATION` | 12 263 | 6 | Chaikin + DP |
| `TRONCON_HYDROGRAPHIQUE` | 3 205 | 6 | DP |
| `CONSTRUCTION_LINEAIRE` | 1 038 | 4 | DP — ponts, murs, haies |
| `SURFACE_HYDROGRAPHIQUE` | 531 | 6 | Chaikin + DP |
| `COURBE` | 767 | 4 | DP — courbes de niveau |
| `ZONE_D_HABITATION` | 116 | 6 | Chaikin + DP |
| `COMMUNE` | 55 | 6 | VW, topology |
| **Sous-total couvert** | **42 546** | | **34,1 %** |

**Principales couches non couvertes :**

| Couche | Features | EndLevel | Type | Impact |
|---|---|---|---|---|
| `BATIMENT` | 66 161 | 0 | Polygone | Écarté intentionnellement (voir note) |
| `FRANCE_GR` | 4 480 | 4 | Polyligne | Déjà simplifié inline (34K pts→393 pts aux niveaux 1-4) |
| `osm_amenity` | 3 545 | var. | POI | Points — pas de géométrie complexe |
| `TOPONYMIE` | 2 641 | var. | POI | Points |
| `LIGNE_OROGRAPHIQUE` | 2 529 | **0** | Polyligne | EndLevel=0 uniquement → profil sans effet |
| `ZONE_D_ACTIVITE_OU_D_INTERET` | 840 | var. | Polygone | Zones économiques |
| `osm_shop` | 710 | var. | POI | Points |
| `PYLONE` | 564 | var. | POI | Points |
| `TERRAIN_DE_SPORT` | 262 | var. | Polygone | |
| `TRONCON_DE_VOIE_FERREE` | 225 | **4** | Polyligne | Géométrie identique aux niveaux 0-4 — candidat profil |
| `DETAIL_HYDROGRAPHIQUE` | 123 | var. | Polygone/Polyligne | |
| Autres | 1 312 | — | — | CIMETIERE, LIGNE_ELECTRIQUE, etc. |
| **Sous-total non couvert** | **82 317** | | | **65,9 %** |

!!! note "BATIMENT — exclusion intentionnelle"
    Les 66 161 bâtiments ont EndLevel=0 : ils n'apparaissent qu'au niveau de détail maximal (n=0, res=24) où les filtres géométriques sont désactivés. Les ajouter au profil serait sans effet sur la RGN des niveaux larges, et la simplification de bâtiments produit des angles non orthogonaux visibles sur GPS.

!!! note "LIGNE_OROGRAPHIQUE — profil inutile"
    Les 2 529 features ont toutes EndLevel=0 (talus, levées, carrières). Elles n'émettent qu'à n=0 où DP est désactivé. Ajouter ces couches au catalogue de profils n'apporterait aucun gain RGN.

---

## §5 — Comportement selon la configuration de profils

| Configuration | Comportement mpforge | Taille MP | Impact RGN estimé |
|---|---|---|---|
| `generalize_profiles_path: generalize-profiles-local.yaml` | DP/VW sur 8 couches, Data0..DataN selon EndLevel | ~43 MB (production) | Référence (1 053 284 bytes) |
| `generalize_profiles_path: generalize-profiles-no-simplify.yaml` | Profil chargé, levels n=0..6 sans `simplify` → géométrie brute identique à tous les niveaux | Plus grand | Plus grand que le profil local : même EndLevel mais sans réduction de points par niveau, donc plus de bytes par DataN |
| Clé `generalize_profiles_path` absente | Pas de catalogue → features avec `Data0=` uniquement | Plus petit (1 DataN par feature) | Minimal |
| `mpforge build --disable-profiles` | Vide le catalogue externe, conserve `generalize:` inline par-input | Intermédiaire | Seules les `generalize:` inline s'appliquent |

---

## §6 — Recommandations priorisées

Candidats d'implémentation classés par rapport gain/complexité, basés sur les mesures §1.

Les niveaux n=1..5 représentent **389 342 bytes** (37 % du RGN total) — c'est sur cette tranche que les filtres mkgmap ont le plus d'effet. Le niveau n=0 (663 942 bytes, 63 %) est non affecté par les filtres mkgmap (`res < 24`).

| Candidat | Gain estimé (% RGN total) | Complexité impl. | Priorité |
|---|---|---|---|
| **Augmenter tolérances DP/VW aux niveaux n=4..6** (aligner sur scaling mkgmap × 10-40) | ~3-5 % — niveaux n=1..5 couverts à 34 %, réduction 20-40 % sur cette fraction | **Faible** (YAML seul) | **Haute** |
| **Étendre le profil à `TRONCON_DE_VOIE_FERREE`** (EndLevel=4, géométrie identique aux 5 niveaux — 1534 points × 4 niveaux sans simplification ≈ 24 KB) | ~2 % sur les niveaux n=1..4 | **Faible** (YAML seul) | **Haute** |
| **`RoundCoordsFilter` dans imgforge** | ~4-7 % — élimine points sub-pixel sur toutes les features aux niveaux n=1..5 | Moyenne (Rust, nouveau filtre par résolution) | Moyenne |
| **`SizeFilter` (polylignes + polygones) dans imgforge** | ~2-6 % — supprime features trop petites au zoom courant | Faible (Rust, quelques lignes) | Moyenne |
| **`RemoveObsoletePointsFilter` dans imgforge** | ~1-3 % — nettoyage colinéaires post-arrondi | Faible (Rust) | Basse (utile surtout après `RoundCoordsFilter`) |

### Note sur l'ordre d'implémentation

Les deux premières recommandations (YAML uniquement) sont réalisables immédiatement. `RoundCoordsFilter` est le filtre avec le plus fort impact architectural : il réduit le nombre de points effectifs avant DP et avant écriture RGN, et bénéficie pleinement de `RemoveObsolete` en aval. Ces deux filtres forment un tandem naturel à implémenter ensemble.

Le gain total estimé des 5 candidats combinés est de l'ordre de **12-21 % du RGN total** (1 053 284 → ~830-925 KB) — en supposant une indépendance approximative des effets. L'effet principal de mkgmap au niveau visuel (lissage géométrique aux zooms larges) vient surtout de `RoundCoordsFilter` + DP scalé, pas du nombre de features.

!!! info "Anomalie DataN mkgmap — piste ouverte"
    Une anomalie dans la façon dont mkgmap concatène les DataN et utilise `maxResolution` reste à investiguer (note mémoire `project_mkgmap_anomaly_concat_datan`). Elle pourrait expliquer partiellement pourquoi mkgmap a 95 subdivisions à n=5 vs 4 pour imgforge, au-delà du seul EndLevel filtering.

---

## Annexe A — Extraction manuelle du sous-map imgforge

Le script `bytes-per-level.py` lit le **premier** sous-map alphabétique trouvant un TRE. Pour un IMG imgforge au format GMP, deux sous-maps ont un TRE :

| Sous-map | Sections | Rôle |
|---|---|---|
| `00011855` | LBL, RGN, TRE | Conteneur GMP (2 niveaux, ~24 bytes RGN) |
| `00380042` | LBL, NET, NOD, RGN, TRE | Tuile BDTOPO-001-004 (7 niveaux, 1 053 284 bytes) |

Le script choisit `00011855` (ordre alphabétique) — le résultat affiché par défaut est le conteneur vide. Pour analyser la vraie tuile, il faut modifier le script pour cibler `00380042` explicitement, ou ajouter un argument `--submap <nom>`.

---

## Annexe B — Sources de référence

| Fichier | Lignes | Contenu |
|---|---|---|
| `tmp/mkgmap/src/.../build/MapBuilder.java` | L929-930, L1248-1283, L1313-1354 | Chaîne de filtres complète |
| `tmp/mkgmap/src/.../filters/RoundCoordsFilter.java` | — | Quantification + best-fit courbes de niveau |
| `tmp/mkgmap/src/.../filters/DouglasPeuckerFilter.java` | L43 | Scaling `filterDistance * (1 << shift)` |
| `tmp/mkgmap/src/.../filters/SizeFilter.java` | — | Filtrage bounding box |
| `tmp/mkgmap/src/.../filters/RemoveObsoletePointsFilter.java` | — | Nettoyage post-RoundCoords |
| `tmp/mkgmap/src/.../filters/SmoothingFilter.java` | — | Code mort (historique) |
| `scripts/debug/bytes-per-level.py` | — | Mesure RGN bytes par niveau depuis TRE |
| `docs/implementation-artifacts/audit-mkgmap-r4924-wide-zoom.md` | §1 | Baseline historique commit `8acb0c2` |
| `pipeline/configs/ign-bdtopo/departement/garmin-rules.yaml` | — | EndLevel par couche BDTOPO |
