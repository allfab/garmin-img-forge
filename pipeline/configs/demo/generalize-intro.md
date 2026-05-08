# Profils de généralisation multi-niveaux

`generalize-profiles.yaml` contrôle la **simplification géométrique** de chaque couche selon le niveau de zoom Garmin.

## Concept clé

À chaque niveau `n` correspond un palier du header Polish Map (`level0`…`level6`).  
Plus `n` est grand, plus le zoom est éloigné → simplification agressive.

## Algorithmes disponibles

| Algorithme | Clé YAML | Usage typique |
|------------|----------|---------------|
| Douglas-Peucker | `simplify` | Rivières, courbes de niveau, polygones |
| Visvalingam-Whyatt | `simplify_vw` | Routes (préserve carrefours et virages) |
| Chaikin (lissage) | `smooth: chaikin` | Végétation, zones d'habitation |
| Topologie | `topology: true` | Routes et communes — ancre les intersections |

## Dispatch conditionnel

Le champ `when` permet d'appliquer des paliers différents selon un attribut :

```yaml
when:
  - field: CL_ADMIN
    values: [Autoroute, Nationale]
    levels: [...]
  - field: CL_ADMIN
    values: [Sentier, Chemin]
    levels: [...]
```
