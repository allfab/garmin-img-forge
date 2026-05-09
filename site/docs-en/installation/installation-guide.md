# Installation Guide

Two approaches: **pre-compiled binaries** (quick, recommended) or **compiling from sources** (for developers).

---

## Option 1: Pre-compiled binaries (recommended)

### mpforge

```bash
# Download and extract the archive
wget https://github.com/allfab/garmin-img-forge/releases/download/mpforge-v0.5.0/mpforge-linux-amd64.tar.gz
tar xzf mpforge-linux-amd64.tar.gz

# Make executable
chmod +x mpforge

# Install
sudo mv mpforge /usr/local/bin/

# Verify
mpforge --version
# → mpforge 0.5.0
```

!!! success "Zero configuration"
    The mpforge static binary includes PROJ 9.3.1, GEOS 3.13.0, GDAL 3.10.1 and the ogr-polishmap driver. No system dependencies required.

### imgforge

```bash
# Download and extract the archive
wget https://github.com/allfab/garmin-img-forge/releases/download/imgforge-v0.5.1/imgforge-linux-amd64.tar.gz
tar xzf imgforge-linux-amd64.tar.gz

# Make executable
chmod +x imgforge

# Install
sudo mv imgforge /usr/local/bin/

# Verify
imgforge --version
# → imgforge v0.5.1
```

---

## Option 2: Compiling from sources

### 1. Clone the repository

```bash
git clone https://github.com/allfab/garmin-img-forge.git
cd garmin-img-forge
```

### 2. Install system dependencies

**Fedora:**
```bash
sudo dnf install gdal-devel gdal cmake g++ rust cargo
```

**Ubuntu/Debian:**
```bash
sudo apt-get install -y libgdal-dev gdal-bin cmake g++
# Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 3. Compile the ogr-polishmap driver

```bash
cd tools/ogr-polishmap
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)

# Install as a GDAL plugin
sudo cp ogr_PolishMap.so $(gdal-config --plugindir)/

# Verify
ogrinfo --formats | grep -i polish
# → PolishMap -vector- (rw+v): Polish Map Format (*.mp)

cd ../..
```

### 4. Compile mpforge

```bash
cd tools/mpforge
cargo build --release

# Verify
./target/release/mpforge --version

cd ../..
```

### 5. Compile imgforge

```bash
cd tools/imgforge
cargo build --release

# Verify
./target/release/imgforge --version

cd ../..
```

### 6. Verify the environment

```bash
# Verify everything is in place
which mpforge || echo "mpforge: use ./tools/mpforge/target/release/mpforge"
which imgforge || echo "imgforge: use ./tools/imgforge/target/release/imgforge"
ogrinfo --formats | grep -i polish
```

---

## First test map

To validate the installation, generate a map from a single department:

```bash
# 1. Download a department (Isère)
./scripts/download-data.sh --zones D038 --data-root ./data/bdtopo

# 2. Run tiling
mpforge build --config configs/test-isere.yaml --jobs 4

# 3. Compile
imgforge build output/tiles/ --output output/gmapsupp.img --jobs 4 --latin1

# 4. Verify
ls -lh output/gmapsupp.img
```

If the `gmapsupp.img` file is generated without errors, the installation is functional.
