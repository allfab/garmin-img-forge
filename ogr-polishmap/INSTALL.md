# Installation Guide

This document provides detailed instructions for building and installing the ogr-polishmap GDAL driver on Linux, macOS, and Windows.

## Prerequisites

### Required Dependencies

- **GDAL 3.6+** with development headers
- **CMake 3.20+**
- **C++17 compatible compiler**:
  - GCC 13+ (Linux)
  - Clang 15+ (macOS)
  - MSVC 2022 (Windows)

### Optional Dependencies

- **Python 3.8+** with GDAL bindings (for Python examples and tests)
- **pytest** (for running the test suite)
- **Sphinx** (for building RST documentation)
- **Doxygen** (for generating API documentation)

## Linux (Ubuntu/Debian)

### Install Dependencies

```bash
# Update package list
sudo apt-get update

# Install GDAL with development headers
sudo apt-get install -y libgdal-dev gdal-bin

# Install build tools
sudo apt-get install -y cmake g++ make

# Verify GDAL version (must be 3.6+)
gdal-config --version

# Optional: Python bindings for examples
sudo apt-get install -y python3-gdal python3-pytest
```

### Build

```bash
# Clone the repository (if not already done)
git clone https://forgejo.allfabox.fr/allfab/mpforge.git
cd mpforge/ogr-polishmap

# Create build directory
mkdir build && cd build

# Configure with CMake
cmake .. -DCMAKE_BUILD_TYPE=Release

# Build
make -j$(nproc)

# Run tests to verify build
ctest --output-on-failure
```

### Install

**Option 1: Install as GDAL plugin (recommended)**

```bash
# Find GDAL plugin directory
gdal-config --plugindir
# Usually: /usr/lib/gdal/plugins or /usr/local/lib/gdal/plugins

# Install the plugin
sudo make install

# Or manually copy the shared library
sudo cp ogr_PolishMap.so $(gdal-config --plugindir)/
```

**Option 2: Set GDAL_DRIVER_PATH (no root required)**

```bash
# Create local plugin directory
mkdir -p ~/.gdal/plugins

# Copy the library
cp ogr_PolishMap.so ~/.gdal/plugins/

# Add to your shell profile (~/.bashrc or ~/.zshrc)
echo 'export GDAL_DRIVER_PATH=$HOME/.gdal/plugins' >> ~/.bashrc
source ~/.bashrc
```

### Verify Installation

```bash
# Check if driver is loaded
ogrinfo --formats | grep -i polish
# Expected output: PolishMap -vector- (rw): Polish Map (.mp)

# Test with a sample file
ogrinfo test/data/valid-minimal/poi-single.mp
```

## macOS

### Install Dependencies

```bash
# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install GDAL and build tools
brew install gdal cmake

# Verify GDAL version
gdal-config --version

# Optional: Python bindings
pip3 install gdal pytest
```

### Build

```bash
# Clone and enter directory
git clone https://forgejo.allfabox.fr/allfab/mpforge.git
cd mpforge/ogr-polishmap

# Create build directory
mkdir build && cd build

# Configure with CMake
cmake .. -DCMAKE_BUILD_TYPE=Release

# Build
make -j$(sysctl -n hw.ncpu)

# Run tests
ctest --output-on-failure
```

### Install

```bash
# Find GDAL plugin directory
gdal-config --plugindir

# Install the plugin
sudo make install

# Or set GDAL_DRIVER_PATH for local installation
mkdir -p ~/.gdal/plugins
cp ogr_PolishMap.dylib ~/.gdal/plugins/
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
```

## Windows

### Install Dependencies

**Option 1: Using vcpkg (recommended)**

```powershell
# Clone vcpkg
git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
.\bootstrap-vcpkg.bat

# Install GDAL
.\vcpkg install gdal:x64-windows

# Integrate with Visual Studio
.\vcpkg integrate install
```

**Option 2: Using Conda**

```powershell
# Create conda environment
conda create -n gdal-dev python=3.11 gdal cmake
conda activate gdal-dev
```

**Option 3: Using OSGeo4W**

Download and install [OSGeo4W](https://trac.osgeo.org/osgeo4w/). Select the following packages:
- gdal
- gdal-devel

### Build

**Using Visual Studio 2022:**

```powershell
# Clone repository
git clone https://forgejo.allfabox.fr/allfab/mpforge.git
cd mpforge/ogr-polishmap

# Create build directory
mkdir build
cd build

# Configure with CMake (vcpkg)
cmake .. -G "Visual Studio 17 2022" -A x64 -DCMAKE_TOOLCHAIN_FILE="C:/vcpkg/scripts/buildsystems/vcpkg.cmake"

# Build Release
cmake --build . --config Release

# Run tests
ctest -C Release --output-on-failure
```

**Using Developer Command Prompt:**

```cmd
# Configure
cmake .. -G "NMake Makefiles" -DCMAKE_BUILD_TYPE=Release

# Build
nmake
```

### Install

```powershell
# Find GDAL plugin directory (Windows doesn't have gdal-config by default)
# Common locations:
# - OSGeo4W: C:\OSGeo4W\bin\gdalplugins
# - Conda: %CONDA_PREFIX%\Library\lib\gdalplugins
# - vcpkg: Check your vcpkg install location

# Copy the DLL to GDAL plugins directory
copy Release\ogr_PolishMap.dll "C:\OSGeo4W\bin\gdalplugins\"

# Or set GDAL_DRIVER_PATH environment variable (recommended)
mkdir "%USERPROFILE%\.gdal\plugins"
copy Release\ogr_PolishMap.dll "%USERPROFILE%\.gdal\plugins\"
setx GDAL_DRIVER_PATH "%USERPROFILE%\.gdal\plugins"
```

## Testing

After building, run the test suite to verify everything works correctly:

```bash
# From the build directory
cd build

# Run all tests
ctest --output-on-failure

# Run specific test
ctest -R "test_poi" --output-on-failure

# Run with verbose output
ctest -V

# Using pytest (Python tests)
cd ..
pytest test/ -v
```

### Test Categories

- **Unit tests**: Fast tests for individual components
- **Integration tests**: Test driver registration and basic operations
- **Corpus tests**: Test against valid-minimal, valid-complex, edge-cases, and error-recovery files

## Installing into GDAL

### As a Built-in Driver (Advanced)

To include the driver directly in a GDAL build:

1. Copy the source files to `gdal/ogr/ogrsf_frmts/polishmap/`
2. Add to `gdal/ogr/ogrsf_frmts/CMakeLists.txt`
3. Rebuild GDAL

### Plugin Path Configuration

GDAL searches for plugins in this order:

1. `GDAL_DRIVER_PATH` environment variable
2. Default plugin directory (`gdal-config --plugindir`)
3. System library paths

Set `GDAL_DRIVER_PATH` to a custom directory:

```bash
# Linux/macOS
export GDAL_DRIVER_PATH=/path/to/plugins:/other/path

# Windows
set GDAL_DRIVER_PATH=C:\path\to\plugins;D:\other\path
```

## Troubleshooting

### Driver Not Found

```
ERROR: PolishMap driver not available
```

**Solutions:**
1. Verify the shared library exists in the plugin directory
2. Check `GDAL_DRIVER_PATH` is set correctly
3. Ensure the library was built against the same GDAL version

```bash
# Debug: Show GDAL driver search paths
export CPL_DEBUG=GDAL
ogrinfo --formats 2>&1 | head -20
```

### Build Errors

**GDAL headers not found:**
```
fatal error: gdal_priv.h: No such file or directory
```

**Solution:** Install GDAL development package:
```bash
sudo apt-get install libgdal-dev  # Debian/Ubuntu
brew install gdal                  # macOS
```

**CMake cannot find GDAL:**
```
Could not find GDAL
```

**Solution:** Set GDAL_DIR or CMAKE_PREFIX_PATH:
```bash
cmake .. -DGDAL_DIR=/path/to/gdal/cmake
# or
cmake .. -DCMAKE_PREFIX_PATH=/usr/local
```

### Runtime Errors

**Symbol not found / undefined reference:**

The plugin was built against a different GDAL version. Rebuild against the installed GDAL:
```bash
rm -rf build && mkdir build && cd build
cmake .. && make
```

**Encoding issues with labels:**

Ensure the input file uses the correct codepage (default: CP1252). For UTF-8 files:
```bash
# Convert with iconv if needed
iconv -f UTF-8 -t CP1252 input.mp > output.mp
```

### Memory Issues

If processing large files causes memory problems:

1. Increase system swap space
2. Process in smaller batches
3. Use spatial filters to limit features

```bash
# Process only a specific region
ogr2ogr -spat xmin ymin xmax ymax output.mp input.mp
```

## Support

For issues and feature requests, please use the Forgejo issue tracker:
https://forgejo.allfabox.fr/allfab/mpforge/issues
