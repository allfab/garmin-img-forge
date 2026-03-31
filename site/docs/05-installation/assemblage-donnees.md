# Assemblage des données

## Téléchargement BD TOPO

```bash
./scripts/download-bdtopo.sh --zones D038 --data-root ./data/bdtopo
```

Le script télécharge automatiquement les données pour le département spécifié depuis le
Géoportail IGN et les organise dans `data/bdtopo/{année}/{version}/{zone}/`.

## Zones disponibles

- Par département : `D001` à `D976`
- Par région : `R11`, `R24`, `R27`, `R28`, `R32`, `R44`, `R52`, `R53`, `R75`, `R76`, `R84`, `R93`, `R94`
- France entière : `FRANCE`
