# Procédure d'installation

Deux approches : **binaires pré-compilés** (rapide, recommandé) ou **compilation depuis les sources** (pour les développeurs).

---

## Option 1 : Binaires pré-compilés (recommandé)

### mpforge

```bash
# Télécharger et extraire l'archive
wget https://forgejo.allfabox.fr/allfab/garmin-img-forge/releases/download/mpforge-v0.4.2/mpforge-linux-amd64.tar.gz
tar xzf mpforge-linux-amd64.tar.gz

# Rendre exécutable
chmod +x mpforge

# Installer
sudo mv mpforge /usr/local/bin/

# Vérifier
mpforge --version
# → mpforge 0.4.2
```

!!! success "Zéro configuration"
    Le binaire statique de mpforge inclut PROJ 9.3.1, GEOS 3.13.0, GDAL 3.10.1 et le driver ogr-polishmap. Aucune dépendance système requise.

### imgforge

```bash
# Télécharger et extraire l'archive
wget https://forgejo.allfabox.fr/allfab/garmin-img-forge/releases/download/imgforge-v0.4.3/imgforge-linux-amd64.tar.gz
tar xzf imgforge-linux-amd64.tar.gz

# Rendre exécutable
chmod +x imgforge

# Installer
sudo mv imgforge /usr/local/bin/

# Vérifier
imgforge --version
# → imgforge v0.4.3
```

---

## Option 2 : Compilation depuis les sources

### 1. Cloner le dépôt

```bash
git clone https://forgejo.allfabox.fr/allfab/garmin-img-forge.git
cd garmin-img-forge
```

### 2. Installer les dépendances système

**Fedora :**
```bash
sudo dnf install gdal-devel gdal cmake g++ rust cargo
```

**Ubuntu/Debian :**
```bash
sudo apt-get install -y libgdal-dev gdal-bin cmake g++
# Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 3. Compiler le driver ogr-polishmap

```bash
cd tools/ogr-polishmap
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)

# Installer comme plugin GDAL
sudo cp ogr_PolishMap.so $(gdal-config --plugindir)/

# Vérifier
ogrinfo --formats | grep -i polish
# → PolishMap -vector- (rw+v): Polish Map Format (*.mp)

cd ../..
```

### 4. Compiler mpforge

```bash
cd tools/mpforge
cargo build --release

# Vérifier
./target/release/mpforge --version

cd ../..
```

### 5. Compiler imgforge

```bash
cd tools/imgforge
cargo build --release

# Vérifier
./target/release/imgforge --version

cd ../..
```

### 6. Vérifier l'environnement

```bash
# Vérifier que tout est en place
which mpforge || echo "mpforge: utiliser ./tools/mpforge/target/release/mpforge"
which imgforge || echo "imgforge: utiliser ./tools/imgforge/target/release/imgforge"
ogrinfo --formats | grep -i polish
```

---

## Première carte de test

Pour valider l'installation, générez une carte à partir d'un seul département :

```bash
# 1. Télécharger un département (Isère)
./scripts/download-bdtopo.sh --zones D038 --data-root ./data/bdtopo

# 2. Lancer le tuilage
mpforge build --config configs/test-isere.yaml --jobs 4

# 3. Compiler
imgforge build output/tiles/ --output output/gmapsupp.img --jobs 4 --latin1

# 4. Vérifier
ls -lh output/gmapsupp.img
```

Si le fichier `gmapsupp.img` est généré sans erreur, l'installation est fonctionnelle.
