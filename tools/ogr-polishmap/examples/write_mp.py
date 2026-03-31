#!/usr/bin/env python3
"""
Example: Creating a Polish Map (.mp) file with GDAL Python bindings

This example demonstrates how to:
- Create a new Polish Map file
- Access the predefined layers (POI, POLYLINE, POLYGON)
- Create features with attributes and geometries
- Write Point, LineString, and Polygon features

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed (GDAL_DRIVER_PATH set if needed)

Usage:
    python3 write_mp.py <output.mp>
    python3 write_mp.py my_map.mp
"""

import sys
from osgeo import ogr, gdal

# Enable GDAL exceptions for better error handling
gdal.UseExceptions()


def create_polish_map(filename):
    """Create a Polish Map file with sample features."""

    print(f"Creating: {filename}")
    print("=" * 60)

    # Get the PolishMap driver
    driver = ogr.GetDriverByName("PolishMap")
    if driver is None:
        print("ERROR: PolishMap driver not available")
        return False

    # Create the datasource
    ds = driver.CreateDataSource(filename)
    if ds is None:
        print(f"ERROR: Could not create {filename}")
        return False

    print(f"Created datasource with {ds.GetLayerCount()} layers")

    # === Write POI features (Layer 0) ===
    poi_layer = ds.GetLayer(0)  # POI layer
    print(f"\nWriting to layer: {poi_layer.GetName()}")

    # POI 1: Restaurant
    feature = ogr.Feature(poi_layer.GetLayerDefn())
    feature.SetField("Type", "0x2C00")  # Restaurant type code
    feature.SetField("Label", "Le Bon Restaurant")

    point = ogr.Geometry(ogr.wkbPoint)
    point.AddPoint(2.3522, 48.8566)  # Paris coordinates (lon, lat)
    feature.SetGeometry(point)

    err = poi_layer.CreateFeature(feature)
    if err != ogr.OGRERR_NONE:
        print(f"ERROR: Failed to create POI feature: {err}")
        return False
    print("  Created POI: Restaurant at (2.3522, 48.8566)")

    # POI 2: Gas Station
    feature = ogr.Feature(poi_layer.GetLayerDefn())
    feature.SetField("Type", "0x2F01")  # Gas station type code
    feature.SetField("Label", "Station Service")

    point = ogr.Geometry(ogr.wkbPoint)
    point.AddPoint(2.3600, 48.8600)
    feature.SetGeometry(point)
    poi_layer.CreateFeature(feature)
    print("  Created POI: Gas Station at (2.3600, 48.8600)")

    # === Write POLYLINE features (Layer 1) ===
    polyline_layer = ds.GetLayer(1)  # POLYLINE layer
    print(f"\nWriting to layer: {polyline_layer.GetName()}")

    # POLYLINE 1: Trail
    feature = ogr.Feature(polyline_layer.GetLayerDefn())
    feature.SetField("Type", "0x0016")  # Trail type code
    feature.SetField("Label", "Sentier de la Foret")
    feature.SetField("EndLevel", "3")

    line = ogr.Geometry(ogr.wkbLineString)
    line.AddPoint(2.3500, 48.8500)
    line.AddPoint(2.3550, 48.8550)
    line.AddPoint(2.3600, 48.8530)
    line.AddPoint(2.3650, 48.8580)
    feature.SetGeometry(line)
    polyline_layer.CreateFeature(feature)
    print("  Created POLYLINE: Trail with 4 points")

    # POLYLINE 2: Road
    feature = ogr.Feature(polyline_layer.GetLayerDefn())
    feature.SetField("Type", "0x0001")  # Major road type code
    feature.SetField("Label", "Route Principale")

    line = ogr.Geometry(ogr.wkbLineString)
    line.AddPoint(2.3400, 48.8450)
    line.AddPoint(2.3500, 48.8500)
    line.AddPoint(2.3600, 48.8480)
    feature.SetGeometry(line)
    polyline_layer.CreateFeature(feature)
    print("  Created POLYLINE: Road with 3 points")

    # === Write POLYGON features (Layer 2) ===
    polygon_layer = ds.GetLayer(2)  # POLYGON layer
    print(f"\nWriting to layer: {polygon_layer.GetName()}")

    # POLYGON 1: Forest
    feature = ogr.Feature(polygon_layer.GetLayerDefn())
    feature.SetField("Type", "0x004C")  # Forest type code
    feature.SetField("Label", "Zone Forestiere")

    ring = ogr.Geometry(ogr.wkbLinearRing)
    ring.AddPoint(2.3400, 48.8400)
    ring.AddPoint(2.3500, 48.8450)
    ring.AddPoint(2.3550, 48.8400)
    ring.AddPoint(2.3500, 48.8350)
    ring.AddPoint(2.3400, 48.8400)  # Close the ring

    polygon = ogr.Geometry(ogr.wkbPolygon)
    polygon.AddGeometry(ring)
    feature.SetGeometry(polygon)
    polygon_layer.CreateFeature(feature)
    print("  Created POLYGON: Forest with 5 vertices")

    # Close the datasource (triggers file write)
    ds = None

    print(f"\nSuccessfully created: {filename}")
    return True


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 1

    filename = sys.argv[1]

    # Check if PolishMap driver is available
    driver = ogr.GetDriverByName("PolishMap")
    if driver is None:
        print("ERROR: PolishMap driver not available")
        print("Make sure GDAL_DRIVER_PATH is set to include the plugin")
        return 1

    if not create_polish_map(filename):
        return 1

    print("\nTip: Use ogrinfo or read_mp.py to verify the created file")
    return 0


if __name__ == "__main__":
    sys.exit(main())
