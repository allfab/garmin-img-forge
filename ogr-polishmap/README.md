# ogr-polishmap

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

GDAL/OGR driver for reading and writing Polish Map (.mp) files used to create Garmin GPS maps.

## Introduction

**ogr-polishmap** is a GDAL/OGR vector driver that enables seamless reading and writing of Polish Map (.mp) format files. The Polish Map format is a text-based format commonly used with the cGPSmapper tool to create custom maps for Garmin GPS devices.

**Key Features:**
- Read POI (Point of Interest), Polyline, and Polygon layers
- Write features with full attribute support (Type, Label, EndLevel, Levels)
- Bidirectional conversion with any GDAL-supported format (GeoJSON, Shapefile, GeoPackage, etc.)
- Spatial and attribute filtering support
- Automatic UTF-8 to CP1252 encoding conversion

## Quick Start

```bash
# Check if the driver is loaded
ogrinfo --formats | grep -i polish

# Read a Polish Map file
ogrinfo sample.mp

# Convert Polish Map to GeoJSON
ogr2ogr -f "GeoJSON" output.geojson input.mp

# Convert GeoJSON to Polish Map
ogr2ogr -f "PolishMap" output.mp input.geojson

# Convert Shapefile to Polish Map
ogr2ogr -f "PolishMap" roads.mp roads.shp
```

## Installation

See [INSTALL.md](INSTALL.md) for detailed build and installation instructions.

**Quick install (Linux):**
```bash
# Prerequisites
sudo apt-get install libgdal-dev cmake g++

# Build
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make

# Install (as plugin or system-wide)
make install
```

## Usage Examples

### Command Line (ogrinfo/ogr2ogr)

```bash
# Display file information
ogrinfo -al sample.mp

# List only POI features
ogrinfo -al sample.mp POI

# Filter by attribute
ogrinfo -al sample.mp -where "Type='0x2C00'"

# Spatial filter (bounding box)
ogr2ogr -f "GeoJSON" paris.geojson france.mp -spat 2.2 48.8 2.5 49.0

# Convert specific layer
ogr2ogr -f "GeoJSON" roads.geojson map.mp POLYLINE
```

### Python

```python
from osgeo import ogr, gdal

gdal.UseExceptions()

# Open and read Polish Map file
ds = ogr.Open("sample.mp")
for layer in [ds.GetLayer(i) for i in range(ds.GetLayerCount())]:
    print(f"Layer: {layer.GetName()}, Features: {layer.GetFeatureCount()}")
    for feature in layer:
        print(f"  Type: {feature.GetField('Type')}, Label: {feature.GetField('Label')}")

# Create new Polish Map file
driver = ogr.GetDriverByName("PolishMap")
ds = driver.CreateDataSource("output.mp")
poi_layer = ds.GetLayer(0)  # POI layer

feature = ogr.Feature(poi_layer.GetLayerDefn())
feature.SetField("Type", "0x2C00")
feature.SetField("Label", "Restaurant")
point = ogr.Geometry(ogr.wkbPoint)
point.AddPoint(2.3522, 48.8566)
feature.SetGeometry(point)
poi_layer.CreateFeature(feature)
ds = None
```

See the [examples/](examples/) directory for more comprehensive Python examples.

## Documentation

- [Driver Documentation](doc/polishmap.rst) - Complete GDAL RST driver documentation
- [Format Specification](doc/format-specification.md) - Polish Map format details
- [Garmin Type Codes](doc/garmin-types.md) - Type code reference (0x0001-0xFFFF)
- [Python Examples](examples/) - Working code examples

## Project Structure

```
ogr-polishmap/
├── src/                    # Driver source code (C++)
├── test/                   # Test suite and test data
│   └── data/               # Test corpus (valid, edge-cases, error-recovery)
├── doc/                    # Documentation
│   └── polishmap.rst       # GDAL-format RST documentation
├── examples/               # Python usage examples
├── CMakeLists.txt          # CMake build configuration
├── README.md               # This file
└── INSTALL.md              # Build and installation instructions
```

## Supported Features

| Feature | Read | Write |
|---------|------|-------|
| POI (Point) | Yes | Yes |
| POLYLINE (LineString) | Yes | Yes |
| POLYGON (Polygon) | Yes | Yes |
| Attribute Fields | Yes | Yes |
| Spatial Filter | Yes | N/A |
| Attribute Filter | Yes | N/A |
| UTF-8 Labels | Yes | Yes (auto-converts to CP1252) |

## Contributing

This driver is developed as part of the mpforge project. Contributions are welcome!

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Please ensure all tests pass before submitting:
```bash
cd build && ctest --output-on-failure
```

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.

## References

- [GDAL Vector Driver Tutorial](https://gdal.org/tutorials/vector_driver_tut.html)
- [Polish Map Format (cGPSmapper)](http://www.cgpsmapper.com/manual.htm)
- [OSM to Garmin POI Types](https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/POI_Types)
