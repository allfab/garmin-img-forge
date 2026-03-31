# Détail des étapes

## Étape 1 : Téléchargement BD TOPO

```bash
./scripts/download-bdtopo.sh --zones D038 --data-root ./data/bdtopo
```

Le script télécharge les données par département depuis le Géoportail IGN et les organise dans
`data/bdtopo/{année}/{version}/{zone}/`.

## Étape 2 : Génération des tuiles Polish Map

```bash
./mpforge/target/release/mpforge build --config configs/france-bdtopo.yaml --jobs 8
```

`mpforge` lit les sources vecteur (GeoPackage, Shapefile) et les découpe en tuiles `.mp` selon
une grille configurable, en appliquant les règles de catégorisation Garmin.

## Étape 3 : Compilation Garmin IMG

```bash
./imgforge/target/release/imgforge --config configs/france-bdtopo.yaml
```

`imgforge` compile les tuiles `.mp` en fichier binaire Garmin `.img`, optimisé pour l'affichage
sur GPS (sans Java, sans mkgmap).

## Étape 4 : Assemblage final

Le script `build-garmin-map.sh` enchaîne ces étapes et produit `output/gmapsupp.img`,
prêt à être copié sur la carte SD du GPS.
