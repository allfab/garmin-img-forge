#!/usr/bin/env python3
"""
Example: Converting Polish Map (.mp) to GeoJSON format using GDAL Python bindings

This example demonstrates how to:
- Open a Polish Map file with OGR
- Create a new GeoJSON file
- Convert features between formats
- Preserve attributes during conversion

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed (GDAL_DRIVER_PATH set if needed)

Usage:
    python3 convert_mp_to_geojson.py <input.mp> <output.geojson>
    python3 convert_mp_to_geojson.py ../test/data/valid-minimal/poi-multiple.mp output.geojson
"""

import sys
import os
from osgeo import ogr, gdal

# Enable GDAL exceptions for better error handling
gdal.UseExceptions()


def convert_mp_to_geojson(input_file, output_file):
    """Convert a Polish Map file to GeoJSON format."""

    print(f"Converting: {input_file} -> {output_file}")
    print("=" * 60)

    # Open source Polish Map file
    src_ds = ogr.Open(input_file)
    if src_ds is None:
        print(f"ERROR: Could not open {input_file}")
        return False

    print(f"Source: {src_ds.GetDriver().GetName()}")
    print(f"Source layers: {src_ds.GetLayerCount()}")

    # Delete output file if it exists
    if os.path.exists(output_file):
        os.remove(output_file)

    # Create destination GeoJSON file
    driver = ogr.GetDriverByName("GeoJSON")
    if driver is None:
        print("ERROR: GeoJSON driver not available")
        return False

    dst_ds = driver.CreateDataSource(output_file)
    if dst_ds is None:
        print(f"ERROR: Could not create {output_file}")
        return False

    print(f"Destination: {dst_ds.GetDriver().GetName()}")
    print()

    # Statistics
    total_features = 0

    # Process each source layer (POI, POLYLINE, POLYGON)
    for src_layer_idx in range(src_ds.GetLayerCount()):
        src_layer = src_ds.GetLayer(src_layer_idx)
        src_layer_name = src_layer.GetName()
        feature_count = src_layer.GetFeatureCount()

        if feature_count == 0:
            print(f"Skipping empty layer: {src_layer_name}")
            continue

        print(f"Processing layer: {src_layer_name} ({feature_count} features)")

        # Create corresponding layer in GeoJSON
        src_defn = src_layer.GetLayerDefn()
        geom_type = src_defn.GetGeomType()
        srs = src_layer.GetSpatialRef()

        dst_layer = dst_ds.CreateLayer(src_layer_name, srs, geom_type)
        if dst_layer is None:
            print(f"  ERROR: Could not create layer {src_layer_name}")
            continue

        # Copy field definitions
        for i in range(src_defn.GetFieldCount()):
            field_defn = src_defn.GetFieldDefn(i)
            dst_layer.CreateField(field_defn)

        # Copy features
        src_layer.ResetReading()
        layer_features = 0

        for src_feature in src_layer:
            dst_feature = ogr.Feature(dst_layer.GetLayerDefn())

            # Copy geometry
            geom = src_feature.GetGeometryRef()
            if geom:
                dst_feature.SetGeometry(geom.Clone())

            # Copy all fields
            for i in range(src_defn.GetFieldCount()):
                value = src_feature.GetField(i)
                if value is not None:
                    dst_feature.SetField(i, value)

            # Create feature in destination
            err = dst_layer.CreateFeature(dst_feature)
            if err == ogr.OGRERR_NONE:
                layer_features += 1

        print(f"  Converted {layer_features} features")
        total_features += layer_features

    # Close datasets
    src_ds = None
    dst_ds = None

    # Print summary
    print()
    print("Conversion Summary:")
    print(f"  Total features converted: {total_features}")
    print(f"  Output file: {output_file}")

    # Show output file size
    if os.path.exists(output_file):
        size = os.path.getsize(output_file)
        if size < 1024:
            print(f"  File size: {size} bytes")
        elif size < 1024 * 1024:
            print(f"  File size: {size / 1024:.1f} KB")
        else:
            print(f"  File size: {size / (1024 * 1024):.1f} MB")

    return True


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 1

    input_file = sys.argv[1]
    output_file = sys.argv[2]

    # Check if PolishMap driver is available
    driver = ogr.GetDriverByName("PolishMap")
    if driver is None:
        print("ERROR: PolishMap driver not available")
        print("Make sure GDAL_DRIVER_PATH is set to include the plugin")
        return 1

    if not convert_mp_to_geojson(input_file, output_file):
        return 1

    print("\nTip: Use ogrinfo to verify the converted file:")
    print(f"  ogrinfo -al {output_file}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
