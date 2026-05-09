# generalize-profiles.yaml

**Geometric simplification** catalog by layer and zoom level.

As the user zooms out on their GPS, mpforge reduces geometry precision
to lighten tiles — without degrading the navigation experience.

## Available algorithms

| Algorithm | YAML key | Typical use case |
|-----------|----------|-----------------|
| Douglas-Peucker | `simplify` | Rivers, contour lines, polygons |
| Visvalingam-Whyatt | `simplify_vw` | Roads — preserves turns and junctions |
| Chaikin | `smooth: chaikin` | Contour smoothing (vegetation, buildings) |
| Topology | `topology: true` | Intersection anchoring (roads, municipalities) |

## The `n` field — level index

```
n = 0  →  level0 (24 bits, ~1 m/px)   →  minimal simplification
n = 6  →  level6 (16 bits, ~320 m/px) →  maximum simplification
```

Each profile declares levels `n = 0` to `n = max(EndLevel)` of the layer.
