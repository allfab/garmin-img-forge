# ogr-garminimg

> Driver GDAL/OGR pour **lire** le format Garmin IMG (`.img`) — *en développement*.

## Statut

🚧 **Travail en cours** — API instable, non publié.

Complémentaire de [`ogr-polishmap`](../ogr-polishmap/) (qui lit/écrit le format Polish Map `.mp`) : `ogr-garminimg` cible le format binaire final `.img` produit par `imgforge` / `mkgmap` / `cgpsmapper`, afin d'en extraire les couches vectorielles via GDAL.

## Périmètre visé

- Lecture des sous-fichiers internes (`TRE`, `RGN`, `LBL`, `NET`, `NOD`, `TYP`)
- Exposition en layers OGR : points, lignes, polygones
- Décodage TYP → QML (utilitaire `typ2qml`)

## Structure

```
tools/ogr-garminimg/
├── CMakeLists.txt
├── src/               # Parsers/writers TRE/RGN/LBL/NET/NOD/TYP + driver GDAL
├── test/              # Tests unitaires (driver registration, identify, filesystem)
└── tools/             # Utilitaires (typ2qml)
```

## Build (développement)

```bash
cd tools/ogr-garminimg
cmake -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build
```

Prérequis : GDAL 3.6+ (dev), CMake 3.20+, compilateur C++17.

## Licence

GPL v3 (voir [`LICENSE`](../../LICENSE) à la racine du dépôt).
