# Format Polish Map

Le format **Polish Map** (`.mp`) est le format intermédiaire utilisé par `mpforge` pour stocker
les données géographiques avant compilation en `.img` Garmin.

## Structure d'un fichier .mp

```
[IMG ID]
ID=00001234
Name=Ma carte
Levels=4
Level0=24
Level1=21
Level2=18
Level3=15
[END-IMG ID]

[POLYLINE]
Type=0x01
Label=Route nationale
Data0=(45.00,2.00),(45.01,2.01)
[END]
```

## Types d'objets Garmin

| Type | Description | Exemple |
|------|-------------|---------|
| `POLYLINE` | Lignes | Routes, rivières, courbes de niveau |
| `POLYGON` | Surfaces | Forêts, lacs, bâtiments |
| `POI` | Points d'intérêt | Villes, sommets, refuges |
