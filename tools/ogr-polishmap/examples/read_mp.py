#!/usr/bin/env python3
"""
Example: Reading a Polish Map (.mp) file with GDAL Python bindings

This example demonstrates how to:
- Open a Polish Map file
- Iterate through layers (POI, POLYLINE, POLYGON)
- Read features and their attributes
- Access geometry data

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed (GDAL_DRIVER_PATH set if needed)

Usage:
    python3 read_mp.py <input.mp>
    python3 read_mp.py ../test/data/valid-minimal/poi-multiple.mp
"""

import sys
from osgeo import ogr, gdal

# Enable GDAL exceptions for better error handling
gdal.UseExceptions()


def read_polish_map(filename):
    """Read and display contents of a Polish Map file."""

    print(f"Opening: {filename}")
    print("=" * 60)

    # Open the file
    ds = ogr.Open(filename)
    if ds is None:
        print(f"ERROR: Could not open {filename}")
        return False

    # Display file metadata
    print(f"Driver: {ds.GetDriver().GetName()}")
    print(f"Layer count: {ds.GetLayerCount()}")
    print()

    # Iterate through layers
    for i in range(ds.GetLayerCount()):
        layer = ds.GetLayer(i)
        layer_name = layer.GetName()
        feature_count = layer.GetFeatureCount()

        print(f"Layer {i + 1}: {layer_name}")
        print(f"  Feature count: {feature_count}")

        # Get layer definition and show fields
        layer_defn = layer.GetLayerDefn()
        field_count = layer_defn.GetFieldCount()
        print(f"  Fields ({field_count}):")
        for j in range(field_count):
            field_defn = layer_defn.GetFieldDefn(j)
            print(f"    - {field_defn.GetName()}: {field_defn.GetTypeName()}")

        # Display features
        if feature_count > 0:
            print(f"  Features:")
            layer.ResetReading()
            feature = layer.GetNextFeature()
            displayed = 0
            max_display = 5  # Limit display for large files

            while feature and displayed < max_display:
                fid = feature.GetFID()

                # Get attributes
                type_val = feature.GetField("Type") or ""
                label_val = feature.GetField("Label") or ""

                # Get geometry info
                geom = feature.GetGeometryRef()
                if geom:
                    geom_type = ogr.GeometryTypeToName(geom.GetGeometryType())

                    if geom.GetGeometryType() == ogr.wkbPoint:
                        coords = f"({geom.GetX():.6f}, {geom.GetY():.6f})"
                    elif geom.GetGeometryType() == ogr.wkbLineString:
                        coords = f"{geom.GetPointCount()} points"
                    elif geom.GetGeometryType() == ogr.wkbPolygon:
                        ring = geom.GetGeometryRef(0)
                        coords = f"{ring.GetPointCount()} vertices" if ring else "empty"
                    else:
                        coords = "complex"
                else:
                    geom_type = "None"
                    coords = ""

                print(f"    FID {fid}: Type={type_val}, Label=\"{label_val}\", "
                      f"Geom={geom_type} {coords}")

                displayed += 1
                feature = layer.GetNextFeature()

            if feature_count > max_display:
                print(f"    ... and {feature_count - max_display} more features")

        print()

    # Clean up
    ds = None
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

    if not read_polish_map(filename):
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
