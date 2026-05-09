# garmin-rules.yaml — Rule examples

## Simple match — Motorway

```yaml
- match:
    CL_ADMIN: "Autoroute"
  set:
    Type: "0x01"               # Motorway symbol in Garmin library
    EndLevel: "6"              # Visible at all zoom levels
    Label: "~[0x04]${NUMERO}"  # Label with Garmin formatting
```

## Negation ( ! ) — National road excluding roundabout

```yaml
- match:
    CL_ADMIN: "Nationale"
    NATURE: "!Rond-point"   # ! = exclude this specific case
  set:
    Type: "0x04"
    EndLevel: "4"
```

## Catch-all rule — no explicit match

```yaml
- set:               # No match = applies to everything else
    Type: "0x06"
    EndLevel: "2"
```

> **EndLevel** controls visibility: `0` = max zoom only, `6` = always visible.
