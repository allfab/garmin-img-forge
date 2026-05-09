# generalize-profiles.yaml — Conditional dispatch

## Roads

```yaml
TRONCON_DE_ROUTE:
  topology: true        # Anchored intersections — junctions preserved
  when:
    - field: CL_ADMIN
      values: [Autoroute, Nationale]
      levels: [...]     # Conservative simplification (major axes)
    - field: CL_ADMIN
      values: [Sentier, Chemin]
      levels: [...]     # More aggressive simplification (secondary paths)
```

> `topology: true` anchors intersections — crossroads do not move.
> `when` dispatches to different profiles based on an attribute value.
