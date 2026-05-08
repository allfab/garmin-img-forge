# garmin-rules.yaml — Exemples de règles

## Match simple — Autoroute

```yaml
- match:
    CL_ADMIN: "Autoroute"
  set:
    Type: "0x01"               # Symbole Autoroute dans la librairie Garmin
    EndLevel: "6"              # Visible à tous les niveaux de zoom
    Label: "~[0x04]${NUMERO}"  # Étiquette avec formatage Garmin
```

## Négation ( ! ) — Nationale hors Rond-point

```yaml
- match:
    CL_ADMIN: "Nationale"
    NATURE: "!Rond-point"   # ! = exclure ce cas précis
  set:
    Type: "0x04"
    EndLevel: "4"
```

## Règle catch-all — aucun match explicite

```yaml
- set:               # Pas de match = s'applique à tout le reste
    Type: "0x06"
    EndLevel: "2"
```

> **EndLevel** contrôle la visibilité : `0` = zoom maximum uniquement, `6` = toujours visible.
