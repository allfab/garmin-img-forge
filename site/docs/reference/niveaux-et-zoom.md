# Niveaux de zoom et EndLevel

Le format Garmin IMG organise les données cartographiques en **niveaux de zoom**. Chaque niveau correspond a une résolution de coordonnées et détermine quelles features sont visibles quand l'utilisateur zoome ou dézoome sur son GPS. Bien configurer les niveaux est essentiel pour obtenir une carte performante et lisible.

---

## Concepts clés

### Résolution (bits)

La résolution d'un niveau est exprimée en **bits** (1 à 24). Plus la valeur est élevée, plus le niveau est détaillé :

| Bits | Précision approx. | Usage typique |
|------|-------------------|---------------|
| 24 | ~5 m | Détail maximum (sentiers, bâtiments) |
| 22 | ~20 m | Détail élevé |
| 20 | ~80 m | Zoom moyen (routes, contours) |
| 18 | ~300 m | Vue régionale (routes principales, lacs) |
| 16 | ~1,2 km | Vue départementale |
| 14 | ~5 km | Vue nationale |

### Niveaux (levels)

Les niveaux sont numérotés à partir de **0** (le plus détaillé). Chaque niveau est associé à une résolution :

```
--levels "24,20,16"
```

Crée 3 niveaux :

| Level | Résolution | Zoom GPS |
|-------|------------|----------|
| 0 | 24 bits | Le plus zoomé (détail max) |
| 1 | 20 bits | Zoom intermédiaire |
| 2 | 16 bits | Le plus dézoomé (vue large) |

### EndLevel (dans le fichier .mp)

Chaque feature (route, bâtiment, contour...) porte un attribut `EndLevel` qui définit **jusqu'à quel niveau elle reste visible** :

```
[POLYLINE]
Type=0x01
EndLevel=2
Data0=(45.18,5.16),(45.19,5.17)
[END]
```

La règle est simple : **une feature avec `EndLevel=N` est visible aux niveaux 0 à N**.

---

## Correspondance EndLevel / Levels

### Avec `--levels "24,20,16"` (3 niveaux)

| EndLevel | Visible aux niveaux | Résolutions | Copies dans le fichier |
|----------|-------------------|-------------|----------------------|
| 0 | 0 uniquement | 24 | x1 |
| 1 | 0, 1 | 24, 20 | x2 |
| 2 | 0, 1, 2 | 24, 20, 16 | x3 |

### Avec `--levels "24,22,20,18,16"` (5 niveaux)

| EndLevel | Visible aux niveaux | Copies |
|----------|-------------------|--------|
| 0 | 0 | x1 |
| 1 | 0, 1 | x2 |
| 2 | 0, 1, 2 | x3 |
| 3 | 0, 1, 2, 3 | x4 |
| 4 | 0, 1, 2, 3, 4 | x5 |

!!! warning "Impact sur la taille du fichier"
    Chaque copie supplémentaire augmente la taille du fichier IMG. Une feature avec `EndLevel=7` dans une configuration à 9 niveaux est écrite **8 fois**. C'est le levier principal pour contrôler la taille de sortie.

---

## Recommandations

### Nombre de niveaux

| Niveaux | Usage | Impact taille |
|---------|-------|---------------|
| 2 (`"24,18"`) | Carte simple, taille minimale | Référence |
| 3 (`"24,20,16"`) | Bon compromis taille/navigation | +30-50% |
| 5 (`"24,22,20,18,16"`) | Navigation détaillée | +100-150% |
| 9 (`"24,23,...,16"`) | Maximum théorique | +200-400% |

!!! tip "Recommandation pour la BD TOPO"
    **3 à 4 niveaux** avec des sauts de résolution significatifs (4-6 bits d'écart) offrent le meilleur compromis. Les niveaux intermédiaires (24→23→22) n'apportent quasiment aucune différence visuelle sur un GPS et gonflent inutilement le fichier.

### EndLevel par catégorie de feature

Le tableau ci-dessous propose des valeurs d'`EndLevel` optimisées pour une configuration à 3 niveaux (`--levels "24,20,16"`) :

| Catégorie | Type Garmin | EndLevel | Justification |
|-----------|-------------|----------|---------------|
| **Autoroutes** | 0x01 | 2 | Visibles à tous les zooms |
| **Nationales, départementales** | 0x04, 0x05 | 2 | Réseau structurant |
| **Communales** | 0x06, 0x07 | 1 | Visibles au zoom moyen |
| **Chemins, sentiers** | 0x0a, 0x16 | 0 | Détail uniquement |
| **Cours d'eau principaux** | 0x1f | 2 | Repères à tout zoom |
| **Ruisseaux** | 0x18 | 0 | Détail uniquement |
| **Grandes surfaces d'eau** | 0x3c, 0x29 | 2 | Visibles partout |
| **Petits plans d'eau** | 0x40-0x44 | 0 | Détail uniquement |
| **Bâtiments** | 0x13 | 0 | Détail uniquement |
| **Forêts** | 0x50 | 1 | Visibles au zoom moyen |
| **Contours maîtres (25m)** | 0x22 | 1 | Visibles au zoom moyen |
| **Contours intermédiaires (10m)** | 0x21 | 0 | Détail uniquement |

### Cohérence Levels du header MP et `--levels`

Les fichiers `.mp` générés par mpforge contiennent un header avec les niveaux de zoom :

```ini
[IMG ID]
Levels=2
Level0=24
Level1=18
[END]
```

L'option `--levels` d'imgforge **remplace** ces valeurs. Il est recommandé de maintenir la cohérence :

- Si le header déclare `Levels=2` avec `Level0=24, Level1=18`, utilisez `--levels "24,18"` ou `--levels "24,20,16"` avec des EndLevel adaptés
- Les EndLevel dans les features ne doivent **jamais dépasser** le nombre de niveaux - 1. Un `EndLevel=7` avec seulement 3 niveaux n'a pas plus d'effet qu'un `EndLevel=2`
- Si vous changez le nombre de niveaux, **réajustez les EndLevel** dans les règles de transformation

---

## Exemple complet

### Configuration 3 niveaux optimisée pour la BD TOPO

**Règles mpforge** (dans `garmin-rules.yaml`) :
```yaml
# Autoroutes : visibles à tous les zooms
- match:
    CL_ADMIN: "Autoroute"
  set:
    Type: "0x01"
    EndLevel: "2"    # niveaux 0, 1, 2

# Sentiers : détail uniquement
- match:
    NATURE: "Sentier"
  set:
    Type: "0x16"
    EndLevel: "0"    # niveau 0 uniquement

# Contours maîtres : zoom moyen
- match:
    IMPORTANCE: "1"
  set:
    Type: "0x22"
    EndLevel: "1"    # niveaux 0, 1
```

**Compilation imgforge** :
```bash
imgforge build tiles/ \
    --levels "24,20,16" \
    --output gmapsupp.img \
    --jobs 8
```

### Multi-Data : coupler niveau ↔ bucket

Une feature peut porter **plusieurs géométries** (`Data0=` très détaillée, `Data2=` simplifiée pour zoom moyen, etc.). `imgforge` sélectionne le bucket approprié au moment du rendu. L'indice `n` d'un `LevelSpec` dans `generalize-profiles.yaml` correspond directement à l'**index** dans `MpHeader.levels` :

| Index `n` | Header | Bucket émis | Consommé par imgforge à |
|---|---|---|---|
| `0` | `Level0=24` | `Data0=` | zoom très détaillé (`Level0`) |
| `2` | `Level2=20` | `Data2=` | zoom moyen (`Level2`) |
| `4` | `Level4=16` | `Data4=` | zoom grossier (`Level4`) |

**Contrainte fail-fast** : `max(n)` sur tous les profils doit être `< header.levels.len()` — sinon `imgforge` drop silencieusement les buckets hors plage. `mpforge` valide au `load_config` et échoue avec un message explicite.

Voir [mpforge — profils multi-niveaux](../le-projet/mpforge.md#profils-multi-niveaux) et [Étape 2 — Profils multi-niveaux](../le-pipeline/etape-2-configuration.md#profils-multi-niveaux).

### Estimation de l'impact sur la taille

Pour un département montagneux (Isère, 169 tuiles) :

| Configuration | Taille estimée | Temps compilation |
|--------------|----------------|-------------------|
| 9 niveaux, EndLevel max 7 | ~460 Mo | ~35s |
| 3 niveaux, EndLevel max 2 | ~150-180 Mo | ~15-20s |
| 2 niveaux, EndLevel max 1 | ~120-150 Mo | ~10-15s |
