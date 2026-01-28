# OGR PolishMap Driver

A GDAL/OGR vector driver for reading and writing Polish Map (.mp) format files.

## Overview

This driver enables GDAL/OGR to read and write Polish Map (.mp) files, a text-based format commonly used for GPS mapping applications. The driver supports:

- Reading POI (Point), Polyline, and Polygon layers
- Writing features with attributes
- Spatial and attribute filtering
- Bidirectional conversion with ogr2ogr

## Project Status

**Under Development**

This driver is currently in active development. The project scaffolding and driver registration skeleton have been established.

## Requirements

- GDAL 3.6 or higher
- CMake 3.20 or higher
- C++17 compatible compiler
  - GCC 13+ (Linux)
  - Clang (macOS)
  - MSVC 2022 (Windows)

## Build Instructions

```bash
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make
sudo make install
```

## Usage

Once installed, the driver will be automatically registered with GDAL:

```bash
# Check driver is loaded
gdalinfo --formats | grep PolishMap

# Read a Polish Map file
ogrinfo sample.mp

# Convert to GeoJSON
ogr2ogr -f GeoJSON output.geojson input.mp
```

## Project Structure

```
ogr-polishmap/
├── src/              # Driver source code
├── test/             # Test data and test scripts
├── doc/              # Documentation
├── examples/         # Usage examples
└── README.md         # This file
```

## License

This project is licensed under the MIT License - see the LICENSE file in the parent directory for details.

## Contributing

This driver is developed as part of the mpforge project. For more information, see the main project documentation in `/docs`.

## References

- [GDAL Vector Driver Tutorial](https://gdal.org/tutorials/vector_driver_tut.html)
- [Polish Map Format Specification](http://www.cgpsmapper.com/mp_file_format.pdf)
