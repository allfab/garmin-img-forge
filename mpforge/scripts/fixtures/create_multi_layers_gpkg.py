#!/usr/bin/env python3
"""
Create a GeoPackage with 3 layers for testing multi-layer support.
Story 5.5 - Fixture creation

Usage:
    python3 scripts/fixtures/create_multi_layers_gpkg.py

Output:
    tests/integration/fixtures/test_data/multi_layers.gpkg

Requirements:
    - Python 3.x
    - GDAL Python bindings (python3-gdal or pip install gdal)
"""

from osgeo import ogr, osr
import os
import sys

# Determine output path relative to script location
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.abspath(os.path.join(script_dir, "../.."))
output_path = os.path.join(
    project_root,
    "tests/integration/fixtures/test_data/multi_layers.gpkg"
)

# Remove existing file if present
if os.path.exists(output_path):
    os.remove(output_path)
    print(f"🗑️  Removed existing file: {output_path}")

# Create GeoPackage driver
driver = ogr.GetDriverByName('GPKG')
if driver is None:
    print("❌ ERROR: GPKG driver not available", file=sys.stderr)
    print("   Install GDAL Python bindings: apt install python3-gdal", file=sys.stderr)
    sys.exit(1)

# Create GeoPackage dataset
ds = driver.CreateDataSource(output_path)
if ds is None:
    print(f"❌ ERROR: Could not create GeoPackage: {output_path}", file=sys.stderr)
    sys.exit(1)

# Create WGS84 spatial reference
srs = osr.SpatialReference()
srs.ImportFromEPSG(4326)

# ============================================================================
# Layer 1: "pois" - 5 Points
# ============================================================================
# Use wkbPoint (2D) instead of wkbPoint25D (3D)
pois_layer = ds.CreateLayer("pois", srs, ogr.wkbPoint, options=['GEOMETRY_NAME=geom'])
pois_layer.CreateField(ogr.FieldDefn("name", ogr.OFTString))
pois_layer.CreateField(ogr.FieldDefn("Type", ogr.OFTString))

poi_data = [
    (55.13, -21.10, "Hotel A", "0x2B01"),
    (55.14, -21.11, "Restaurant B", "0x2A00"),
    (55.15, -21.12, "Shop C", "0x2E00"),
    (55.16, -21.13, "Museum D", "0x2C02"),
    (55.17, -21.14, "Park E", "0x6600"),
]

for lon, lat, name, type_val in poi_data:
    feature = ogr.Feature(pois_layer.GetLayerDefn())
    feature.SetField("name", name)
    feature.SetField("Type", type_val)
    point = ogr.Geometry(ogr.wkbPoint)
    point.AddPoint(lon, lat)
    point.FlattenTo2D()  # Force 2D geometry
    feature.SetGeometry(point)
    pois_layer.CreateFeature(feature)
    feature = None

# ============================================================================
# Layer 2: "roads" - 10 LineStrings
# ============================================================================
roads_layer = ds.CreateLayer("roads", srs, ogr.wkbLineString, options=['GEOMETRY_NAME=geom'])
roads_layer.CreateField(ogr.FieldDefn("name", ogr.OFTString))
roads_layer.CreateField(ogr.FieldDefn("Type", ogr.OFTString))

road_data = [
    ([(55.10, -21.10), (55.11, -21.11)], "Rue A", "0x01"),
    ([(55.11, -21.11), (55.12, -21.12)], "Rue B", "0x02"),
    ([(55.12, -21.12), (55.13, -21.13)], "Avenue C", "0x03"),
    ([(55.13, -21.13), (55.14, -21.14)], "Boulevard D", "0x04"),
    ([(55.14, -21.14), (55.15, -21.15)], "Route E", "0x05"),
    ([(55.15, -21.15), (55.16, -21.16)], "Chemin F", "0x06"),
    ([(55.16, -21.16), (55.17, -21.17)], "Allée G", "0x07"),
    ([(55.17, -21.17), (55.18, -21.18)], "Impasse H", "0x08"),
    ([(55.18, -21.18), (55.19, -21.19)], "Sentier I", "0x09"),
    ([(55.19, -21.19), (55.20, -21.20)], "Passage J", "0x0a"),
]

for coords, name, type_val in road_data:
    feature = ogr.Feature(roads_layer.GetLayerDefn())
    feature.SetField("name", name)
    feature.SetField("Type", type_val)
    line = ogr.Geometry(ogr.wkbLineString)
    for lon, lat in coords:
        line.AddPoint(lon, lat)
    line.FlattenTo2D()  # Force 2D geometry
    feature.SetGeometry(line)
    roads_layer.CreateFeature(feature)
    feature = None

# ============================================================================
# Layer 3: "buildings" - 8 Polygons
# ============================================================================
buildings_layer = ds.CreateLayer("buildings", srs, ogr.wkbPolygon, options=['GEOMETRY_NAME=geom'])
buildings_layer.CreateField(ogr.FieldDefn("name", ogr.OFTString))
buildings_layer.CreateField(ogr.FieldDefn("Type", ogr.OFTString))

# Helper to create a small square polygon
def create_square(lon, lat, size=0.001):
    return [
        (lon, lat),
        (lon + size, lat),
        (lon + size, lat + size),
        (lon, lat + size),
        (lon, lat),  # Close the ring
    ]

building_data = [
    (create_square(55.10, -21.10), "Building A", "0x13"),
    (create_square(55.11, -21.11), "Building B", "0x13"),
    (create_square(55.12, -21.12), "Building C", "0x13"),
    (create_square(55.13, -21.13), "Building D", "0x13"),
    (create_square(55.14, -21.14), "Building E", "0x13"),
    (create_square(55.15, -21.15), "Building F", "0x13"),
    (create_square(55.16, -21.16), "Building G", "0x13"),
    (create_square(55.17, -21.17), "Building H", "0x13"),
]

for coords, name, type_val in building_data:
    feature = ogr.Feature(buildings_layer.GetLayerDefn())
    feature.SetField("name", name)
    feature.SetField("Type", type_val)
    ring = ogr.Geometry(ogr.wkbLinearRing)
    for lon, lat in coords:
        ring.AddPoint(lon, lat)
    polygon = ogr.Geometry(ogr.wkbPolygon)
    polygon.AddGeometry(ring)
    polygon.FlattenTo2D()  # Force 2D geometry
    feature.SetGeometry(polygon)
    buildings_layer.CreateFeature(feature)
    feature = None

# Close dataset
ds = None

print(f"✅ Created GeoPackage: {output_path}")
print(f"   - Layer 'pois': 5 points")
print(f"   - Layer 'roads': 10 linestrings")
print(f"   - Layer 'buildings': 8 polygons")
print(f"   - Total: 23 features")
