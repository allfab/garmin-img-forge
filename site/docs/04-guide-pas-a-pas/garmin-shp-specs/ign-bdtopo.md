# Correspondances IGN BD TOPO → Garmin

## Thèmes BD TOPO utilisés

| Thème BD TOPO | Couche | Type Garmin |
|---------------|--------|-------------|
| Transport | TRONCON_DE_ROUTE | POLYLINE (0x01–0x0B) |
| Transport | TRONCON_DE_VOIE_FERREE | POLYLINE (0x14) |
| Hydrographie | TRONCON_HYDROGRAPHIQUE | POLYLINE (0x26) |
| Hydrographie | SURFACE_HYDROGRAPHIQUE | POLYGON (0x29) |
| Végétation | ZONE_DE_VEGETATION | POLYGON (0x50) |
| Bâti | CONSTRUCTION_SURFACIQUE | POLYGON (0x01) |
| Toponymie | LIEU_DIT_NON_HABITE | POI |

## Règles de catégorisation

Les règles sont définies dans `mpforge/rules/bdtopo-garmin-rules.yaml`.
