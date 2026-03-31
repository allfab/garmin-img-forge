# Compilation Garmin IMG

`imgforge` compile les tuiles Polish Map (`.mp`) en fichier binaire Garmin (`.img`).

## Commande de base

```bash
./imgforge/target/release/imgforge \
  --config configs/france-bdtopo.yaml \
  --report output/imgforge-report.json
```

## Assemblage multi-tuiles

Pour un pays complet, `imgforge` assemble automatiquement toutes les tuiles en un
`gmapsupp.img` prêt à l'installation :

```bash
./scripts/build-garmin-map.sh --config configs/france-bdtopo.yaml --jobs 8
```
