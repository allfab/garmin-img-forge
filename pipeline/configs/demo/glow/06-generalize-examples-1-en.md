# generalize-profiles.yaml — Simple profile

## Contour lines

```yaml
COURBE:
  levels:
    - { n: 0 }                      # No simplification at max zoom
    - { n: 1, simplify: 0.00002 }
    - { n: 2, simplify: 0.00004 }
    - { n: 3, simplify: 0.00006 }
    - { n: 4, simplify: 0.00030 }
    - { n: 6, simplify: 0.00100 }   # Highly simplified in wide view
```

> Each `n` level corresponds to a level in the Polish Map header.
> `simplify` is the Douglas-Peucker tolerance in WGS84 degrees.
