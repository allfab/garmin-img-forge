# Procédure d'installation

## 1. Cloner le dépôt

```bash
git clone https://forgejo.allfabox.fr/allfab/garmin-ign-bdtopo-map.git
cd garmin-ign-bdtopo-map
```

## 2. Compiler les outils Rust

```bash
# mpforge
cd mpforge && cargo build --release && cd ..

# imgforge
cd imgforge && cargo build --release && cd ..
```

## 3. Compiler le driver OGR

```bash
cd ogr-polishmap
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)
cd ..
```

## 4. Vérifier l'environnement

```bash
./scripts/check_environment.sh
```

## 5. Lancer le pipeline

```bash
./scripts/build-garmin-map.sh --config configs/france-bdtopo.yaml --jobs 8
```

Le fichier `output/gmapsupp.img` sera généré à la fin du processus.

## Installation sur GPS

1. Connectez votre GPS en USB ou insérez la carte SD dans votre ordinateur
2. Copiez `output/gmapsupp.img` dans le dossier `Garmin/` de votre GPS
3. Redémarrez le GPS — la carte apparaît automatiquement dans la gestion des cartes
