# garmin-rules.yaml

**BDTOPO → Garmin types mapping** file.
It defines how each geographic object is translated into symbology on the GPS map.

## Principle

Each **ruleset** is associated with a BDTOPO source layer.
For each object, rules are evaluated top-down:
the first rule whose **match** is satisfied applies.

```
BDTOPO object ──► match field/value ──► set Garmin Type + EndLevel + Label
```

## Rulesets in the demo configuration

| BDTOPO layer | Geometry | Rules |
|--------------|----------|-------|
| TRONCON_DE_ROUTE | Polyline | Motorway, National, Track, Path, default |
| TRONCON_HYDROGRAPHIQUE | Polyline | Permanent, default |
| BATIMENT | Polygon | Religious, Industrial, default |
| TOPONYMIE | Point | Summit, Lake, Forest… |

> The production configuration contains **more than 200 rules** covering the entire BDTOPO.
