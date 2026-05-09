# mpforge — Forges Polish Map tiles

**mpforge** reads the layers defined in `sources.yaml`, splits them according to the grid,
applies Garmin rules and geometric simplification,
and writes **Polish Map** files (`.mp`) — one per tile.

## Command

```
mpforge build --config <sources.yaml>
```

A JSON report (`mpforge-report.json`) summarizes the build:
tiles produced, features processed, duration.
