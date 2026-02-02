# Tests de Compilation Polish Map

Ce document décrit les procédures de test de compilation pour valider que les fichiers `.mp` générés par le driver GDAL PolishMap sont compatibles avec les compilateurs de cartes Garmin.

## Aperçu

Les fichiers Polish Map (`.mp`) peuvent être compilés en fichiers `.img` utilisables sur les appareils GPS Garmin. Deux compilateurs principaux existent :

1. **mkgmap** - Compilateur open-source Java
2. **cGPSmapper** - Compilateur propriétaire Windows (original)

## mkgmap

### Installation

#### Linux/macOS

```bash
# Prérequis: Java 11+
sudo dnf install java-11-openjdk  # Fedora
sudo apt-get install openjdk-11-jre  # Debian/Ubuntu

# Télécharger mkgmap
wget https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz
tar -xzf mkgmap-latest.tar.gz

# Configurer le chemin
export MKGMAP_JAR=/chemin/vers/mkgmap/mkgmap.jar
```

#### Windows

1. Installer Java 11+ depuis [adoptium.net](https://adoptium.net/)
2. Télécharger mkgmap depuis [mkgmap.org.uk](https://www.mkgmap.org.uk/download/)
3. Extraire l'archive
4. Définir `MKGMAP_JAR` dans les variables d'environnement

### Utilisation

```bash
# Compilation basique
java -jar mkgmap.jar output.mp

# Avec options de configuration
java -jar mkgmap.jar \
    --family-id=1 \
    --product-id=1 \
    --family-name="Ma Carte" \
    output.mp

# Avec fichier de style personnalisé
java -jar mkgmap.jar --style-file=style.txt output.mp
```

### Validation des Résultats (NFR6)

Une compilation réussie doit :

- **Exit code 0** - Pas d'erreur fatale
- **Fichier .img généré** - Présence du fichier de sortie
- **Aucune erreur de format** - Pas de messages "ERROR" ou "SEVERE" dans les logs
- **Warnings acceptables** - "Unknown type" est acceptable pour les types Garmin non standard

### Exécution des Tests Automatisés

```bash
# Depuis le répertoire ogr-polishmap
./test/test_mkgmap_compilation.sh

# Avec chemin mkgmap personnalisé
./test/test_mkgmap_compilation.sh test/data/valid-minimal /chemin/vers/mkgmap.jar
```

## cGPSmapper

### Installation

#### Windows

1. Télécharger depuis [cgpsmapper.com](http://www.cgpsmapper.com/)
2. Extraire l'archive
3. Définir `CGPSMAPPER_PATH` vers `cgpsmapper.exe`

#### Linux (via Wine)

```bash
# Installer Wine
sudo dnf install wine  # Fedora
sudo apt-get install wine  # Debian/Ubuntu

# Configurer cGPSmapper
export CGPSMAPPER_PATH=/chemin/vers/cgpsmapper.exe
```

### Utilisation

```bash
# Windows
cgpsmapper.exe output.mp

# Linux via Wine
wine cgpsmapper.exe output.mp

# Avec options
cgpsmapper.exe output.mp -o output.img
```

### Validation des Résultats (NFR7)

Une compilation réussie doit :

- **Exit code 0** - Succès
- **Fichier .img généré** - Présence du fichier de sortie
- **Aucune erreur fatale** - Pas de messages d'erreur bloquants

### Exécution des Tests Automatisés

```bash
# Depuis le répertoire ogr-polishmap
./test/test_cgpsmapper_compilation.sh

# Avec chemin personnalisé
./test/test_cgpsmapper_compilation.sh test/data/valid-minimal /chemin/vers/cgpsmapper.exe
```

## Procédure de Validation GPS Manuelle (AC4)

### Matériel Requis

- Appareil GPS Garmin compatible (eTrex, Edge, Montana, ou similaire)
- Câble USB ou lecteur de carte SD
- Fichier `.img` compilé avec mkgmap ou cGPSmapper

### Étapes

1. **Générer le fichier .mp**
   ```bash
   ogr2ogr -f "PolishMap" test.mp input.geojson
   ```

2. **Compiler en .img**
   ```bash
   java -jar mkgmap.jar test.mp
   ```

3. **Copier sur le GPS**
   - Connecter l'appareil via USB
   - Copier le fichier `.img` vers :
     - `/Garmin/` (eTrex, Montana)
     - `/Map/` (certains modèles Edge)
   - Éjecter proprement l'appareil

4. **Vérifier l'affichage**
   - Démarrer l'appareil
   - Naviguer vers la zone des données de test
   - Vérifier :
     - POIs visibles et cliquables
     - Routes/chemins affichés
     - Zones/polygones remplis correctement

5. **Documenter les résultats**
   - Captures d'écran si possible
   - Noter les problèmes d'affichage
   - Indiquer le modèle GPS et version firmware

### Appareils Testés

> **Note:** La validation GPS manuelle doit être effectuée avant la mise en production.
> Les tests automatisés vérifient la compilation, mais le rendu final nécessite
> une vérification visuelle sur appareil réel.

| Appareil | Firmware | Résultat | Notes |
|----------|----------|----------|-------|
| eTrex 32x | v5.10 | À tester | Validation manuelle requise |
| Edge 830 | v10.00 | À tester | Validation manuelle requise |
| Montana 700 | v6.00 | À tester | Validation manuelle requise |

## Structure des Tests

```
test/
├── test_mkgmap_compilation.sh     # Tests automatisés mkgmap
├── test_cgpsmapper_compilation.sh # Tests automatisés cGPSmapper
├── test_roundtrip_geojson.sh      # Tests round-trip GeoJSON
├── test_roundtrip_shapefile.sh    # Tests round-trip Shapefile
└── data/
    ├── valid-minimal/             # Fichiers de test simples
    └── valid-complex/             # Fichiers de test complexes
        └── mixed-all-types.mp     # Multi-geometry pour compilation
```

## Intégration CI/CD

Les tests de compilation peuvent être intégrés dans la CI si les outils sont disponibles :

```yaml
# Exemple GitHub Actions
jobs:
  compilation-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Java
        uses: actions/setup-java@v3
        with:
          java-version: '11'
          distribution: 'temurin'

      - name: Download mkgmap
        run: |
          wget https://www.mkgmap.org.uk/download/mkgmap-latest.tar.gz
          tar -xzf mkgmap-latest.tar.gz
          echo "MKGMAP_JAR=$(pwd)/mkgmap-*/mkgmap.jar" >> $GITHUB_ENV

      - name: Run compilation tests
        run: ./test/test_mkgmap_compilation.sh
```

## Dépannage

### mkgmap

| Erreur | Cause | Solution |
|--------|-------|----------|
| "Java not found" | Java non installé | Installer Java 11+ |
| "Unknown type 0xXXXX" | Type Garmin non standard | Warning acceptable |
| "Input error" | Fichier .mp invalide | Vérifier syntaxe du fichier |

### cGPSmapper

| Erreur | Cause | Solution |
|--------|-------|----------|
| "Wine not found" | Wine non installé (Linux) | Installer Wine |
| "License expired" | Version gratuite limitée | Utiliser mkgmap à la place |
| "Invalid header" | [IMG ID] manquant | Vérifier génération du fichier |

## Références

- [Documentation mkgmap](https://www.mkgmap.org.uk/doc/index.html)
- [Manuel cGPSmapper](http://www.cgpsmapper.com/manual.htm)
- [Spécification Polish Map Format](http://www.cgpsmapper.com/manual.htm#Polish_Format)
- [Types Garmin](https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/POI_Types)
