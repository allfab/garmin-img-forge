# generalize-profiles.yaml — Profil simple

## Courbes de niveau

```yaml
COURBE:
  levels:
    - { n: 0 }                      # Pas de simplification au zoom max
    - { n: 1, simplify: 0.00002 }
    - { n: 2, simplify: 0.00004 }
    - { n: 3, simplify: 0.00006 }
    - { n: 4, simplify: 0.00030 }
    - { n: 6, simplify: 0.00100 }   # Très simplifié en vue large
```

> Chaque palier `n` correspond à un niveau du header Polish Map.
> `simplify` est la tolérance Douglas-Peucker en degrés WGS84.
