# Python Examples for ogr-polishmap

This directory contains Python examples demonstrating how to use the ogr-polishmap GDAL driver with the GDAL Python bindings.

## Prerequisites

- Python 3.8 or higher
- GDAL Python bindings (`python3-gdal` or `pip install gdal`)
- ogr-polishmap driver installed (see [INSTALL.md](../INSTALL.md))

Verify your setup:

```bash
# Check Python GDAL bindings
python3 -c "from osgeo import ogr; print('GDAL OK')"

# Check PolishMap driver
python3 -c "from osgeo import ogr; d = ogr.GetDriverByName('PolishMap'); print('PolishMap driver:', 'OK' if d else 'NOT FOUND')"
```

## Examples

This directory contains five Python examples covering all common use cases:
- **Reading**: `read_mp.py` - Open and inspect Polish Map files
- **Writing**: `write_mp.py` - Create new Polish Map files from scratch
- **Conversion**: `convert_geojson_to_mp.py` (GeoJSON → MP) and `convert_mp_to_geojson.py` (MP → GeoJSON)
- **Filtering**: `filter_poi_by_type.py` - Query POI features by Garmin type code

### read_mp.py - Reading Polish Map files

Read and display contents of a Polish Map file:

```bash
python3 read_mp.py sample.mp
python3 read_mp.py ../test/data/valid-minimal/poi-multiple.mp
```

**Features demonstrated:**
- Opening Polish Map files with OGR
- Iterating through layers (POI, POLYLINE, POLYGON)
- Reading feature attributes (Type, Label, EndLevel)
- Accessing geometry data (coordinates)

### write_mp.py - Creating Polish Map files

Create a new Polish Map file with sample features:

```bash
python3 write_mp.py output.mp
```

**Features demonstrated:**
- Creating new Polish Map files
- Accessing predefined layers (POI, POLYLINE, POLYGON)
- Setting feature attributes (Type, Label, EndLevel)
- Creating Point, LineString, and Polygon geometries

### convert_geojson_to_mp.py - GeoJSON to Polish Map

Convert GeoJSON files to Polish Map format:

```bash
python3 convert_geojson_to_mp.py input.geojson output.mp
```

**Features demonstrated:**
- Reading GeoJSON with OGR
- Mapping geometry types to Polish Map layers
- Copying attributes during conversion
- Handling multi-geometries

### convert_mp_to_geojson.py - Polish Map to GeoJSON

Convert Polish Map files to GeoJSON format:

```bash
python3 convert_mp_to_geojson.py input.mp output.geojson
```

**Features demonstrated:**
- Reading Polish Map files
- Creating GeoJSON output
- Preserving all attributes during conversion
- Handling multiple layers

### filter_poi_by_type.py - Filtering POI features

Filter and analyze POI features by Garmin type code:

```bash
# List all unique POI types in a file
python3 filter_poi_by_type.py sample.mp

# Filter by specific type code
python3 filter_poi_by_type.py sample.mp 0x2C00
```

**Features demonstrated:**
- Applying OGR attribute filters
- Working with Garmin type codes
- Spatial filtering by bounding box
- POI type code reference

## Common Patterns

### Opening a Polish Map file

```python
from osgeo import ogr, gdal

gdal.UseExceptions()  # Enable exceptions for error handling

ds = ogr.Open("sample.mp")
if ds is None:
    raise Exception("Could not open file")

print(f"Driver: {ds.GetDriver().GetName()}")
print(f"Layers: {ds.GetLayerCount()}")
```

### Iterating through features

```python
# Get a specific layer
poi_layer = ds.GetLayer(0)  # POI layer

# Iterate all features
for feature in poi_layer:
    type_val = feature.GetField("Type")
    label = feature.GetField("Label")
    geom = feature.GetGeometryRef()
    print(f"Type: {type_val}, Label: {label}")
```

### Creating features

```python
driver = ogr.GetDriverByName("PolishMap")
ds = driver.CreateDataSource("output.mp")

# Get POI layer
poi_layer = ds.GetLayer(0)

# Create feature
feature = ogr.Feature(poi_layer.GetLayerDefn())
feature.SetField("Type", "0x2C00")  # Restaurant
feature.SetField("Label", "My Restaurant")

# Add geometry
point = ogr.Geometry(ogr.wkbPoint)
point.AddPoint(2.3522, 48.8566)  # lon, lat (Paris)
feature.SetGeometry(point)

# Write to layer
poi_layer.CreateFeature(feature)

# Close and save
ds = None
```

### Applying filters

```python
# Attribute filter
layer.SetAttributeFilter("Type = '0x2C00'")

# Spatial filter (bounding box)
layer.SetSpatialFilterRect(minx, miny, maxx, maxy)

# Clear filters
layer.SetAttributeFilter(None)
layer.SetSpatialFilter(None)
```

## Garmin Type Codes Reference

### POI Types (Point features)

| Code Range | Category |
|------------|----------|
| 0x2C00-0x2CFF | Food and Drink |
| 0x2D00-0x2DFF | Lodging |
| 0x2E00-0x2EFF | Shopping |
| 0x2F00-0x2FFF | Auto Services |
| 0x6400-0x64FF | Landmarks |

### Polyline Types (Linear features)

| Code Range | Category |
|------------|----------|
| 0x0001-0x000F | Roads (Major to Minor) |
| 0x0010-0x001F | Trails and Paths |
| 0x0020-0x002F | Water features |
| 0x0030-0x003F | Boundaries |

### Polygon Types (Area features)

| Code Range | Category |
|------------|----------|
| 0x0001-0x000F | Urban areas |
| 0x0010-0x001F | Parks and Recreation |
| 0x0020-0x002F | Natural features |
| 0x0030-0x004F | Water bodies |

See [doc/garmin-types.md](../doc/garmin-types.md) for the complete reference.

## Troubleshooting

### PolishMap driver not found

```
ERROR: PolishMap driver not available
```

Ensure the driver is installed and `GDAL_DRIVER_PATH` is set:

```bash
export GDAL_DRIVER_PATH=/path/to/plugins
# or
export GDAL_DRIVER_PATH=$HOME/.gdal/plugins
```

### Import errors

```
ModuleNotFoundError: No module named 'osgeo'
```

Install GDAL Python bindings:

```bash
# Ubuntu/Debian
sudo apt-get install python3-gdal

# pip
pip install gdal

# conda
conda install gdal
```

### Encoding issues

If labels appear garbled, the source file may use a different encoding. Polish Map format uses CP1252 by default. Ensure your terminal and Python are configured for UTF-8:

```python
import sys
sys.stdout.reconfigure(encoding='utf-8')
```
