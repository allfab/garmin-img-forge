# generalize-profiles.yaml — With smoothing

## Vegetation

```yaml
ZONE_DE_VEGETATION:
  levels:
    - { n: 0, smooth: chaikin, iterations: 1, simplify: 0.00005 }
    - { n: 1, simplify: 0.00006 }
    - { n: 6, simplify: 0.00100 }
```

> Chaikin smooths polygons at high zoom for a more natural rendering.
> Intermediate undeclared levels are automatically filled.
