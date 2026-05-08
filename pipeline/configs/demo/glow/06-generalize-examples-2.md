# generalize-profiles.yaml — Avec lissage

## Végétation

```yaml
ZONE_DE_VEGETATION:
  levels:
    - { n: 0, smooth: chaikin, iterations: 1, simplify: 0.00005 }
    - { n: 1, simplify: 0.00006 }
    - { n: 6, simplify: 0.00100 }
```

> Chaikin adoucit les polygones à fort zoom pour un rendu plus naturel.
> Les paliers intermédiaires non déclarés sont comblés automatiquement.
