# generalize-profiles.yaml

Catalogue de **simplification géométrique** par couche et par niveau de zoom.

À mesure que l'utilisateur dézoome sur son GPS, mpforge réduit la précision
des géométries pour alléger les tuiles — sans dégrader l'expérience de navigation.

## Algorithmes disponibles

| Algorithme | Clé YAML | Cas d'usage typique |
|------------|----------|---------------------|
| Douglas-Peucker | `simplify` | Rivières, courbes de niveau, polygones |
| Visvalingam-Whyatt | `simplify_vw` | Routes — préserve virages et carrefours |
| Chaikin | `smooth: chaikin` | Lissage des contours (végétation, bâti) |
| Topologie | `topology: true` | Ancrage des intersections (routes, communes) |

## Le champ `n` — index de niveau

```
n = 0  →  level0 (24 bits, ~1 m/px)   →  simplification minimale
n = 6  →  level6 (16 bits, ~320 m/px) →  simplification maximale
```

Chaque profil déclare les paliers `n = 0` à `n = max(EndLevel)` de la couche.
