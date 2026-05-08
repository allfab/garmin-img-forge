# generalize-profiles.yaml — Dispatch conditionnel

## Routes

```yaml
TRONCON_DE_ROUTE:
  topology: true        # Intersections ancrées — carrefours préservés
  when:
    - field: CL_ADMIN
      values: [Autoroute, Nationale]
      levels: [...]     # Simplification conservative (axes structurants)
    - field: CL_ADMIN
      values: [Sentier, Chemin]
      levels: [...]     # Simplification plus agressive (chemins secondaires)
```

> `topology: true` ancre les intersections — les carrefours ne bougent pas.
> `when` dispatche vers des profils différents selon la valeur d'un attribut.
