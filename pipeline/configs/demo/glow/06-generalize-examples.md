# generalize-profiles.yaml — Exemples de profils

## Profil simple — Courbes de niveau

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

## Avec lissage — Végétation

```yaml
ZONE_DE_VEGETATION:
  levels:
    - { n: 0, smooth: chaikin, iterations: 1, simplify: 0.00005 }
    - { n: 1, simplify: 0.00006 }
    - { n: 6, simplify: 0.00100 }
```

> Chaikin adoucit les polygones à fort zoom pour un rendu plus naturel.

## Dispatch conditionnel — Routes

```yaml
TRONCON_DE_ROUTE:
  topology: true        # Intersections ancrées — carrefours préservés
  when:
    - field: CL_ADMIN
      values: [Autoroute, Nationale]
      levels: [...]     # Simplification conservative (axes structurants)
    - field: CL_ADMIN
      values: [Sentier, Chemin]
      levels: [...]     # Simplification plus agressive (chemins secondaires)
```
