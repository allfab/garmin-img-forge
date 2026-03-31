#!/usr/bin/env python3
"""
Example: Converting GeoJSON to Polish Map (.mp) format using GDAL Python bindings

This example demonstrates how to:
- Open a GeoJSON file with OGR
- Create a new Polish Map file
- Convert features between formats
- Handle different geometry types (Point -> POI, LineString -> POLYLINE, Polygon -> POLYGON)

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed (GDAL_DRIVER_PATH set if needed)

Usage:
    python3 convert_geojson_to_mp.py <input.geojson> <output.mp>
    python3 convert_geojson_to_mp.py ../test/data/valid-minimal/integration_test.geojson output.mp
"""

import sys
from osgeo import ogr, gdal

# Enable GDAL exceptions for better error handling
gdal.UseExceptions()


def get_target_layer(ds, geom_type):
    """Get the appropriate layer based on geometry type."""
    if geom_type == ogr.wkbPoint:
        return ds.GetLayer(0)  # POI
    elif geom_type == ogr.wkbLineString:
        return ds.GetLayer(1)  # POLYLINE
    elif geom_type == ogr.wkbPolygon:
        return ds.GetLayer(2)  # POLYGON
    elif geom_type == ogr.wkbMultiPoint:
        return ds.GetLayer(0)  # POI (will need to extract points)
    elif geom_type == ogr.wkbMultiLineString:
        return ds.GetLayer(1)  # POLYLINE
    elif geom_type == ogr.wkbMultiPolygon:
        return ds.GetLayer(2)  # POLYGON
    return None


def convert_feature(src_feature, dst_layer):
    """Convert a single feature to the destination layer."""
    dst_defn = dst_layer.GetLayerDefn()
    dst_feature = ogr.Feature(dst_defn)

    # Copy geometry
    geom = src_feature.GetGeometryRef()
    if geom is None:
        return False

    # Handle multi-geometries by extracting first component
    geom_type = geom.GetGeometryType()
    if geom_type == ogr.wkbMultiPoint:
        if geom.GetGeometryCount() > 0:
            geom = geom.GetGeometryRef(0).Clone()
    elif geom_type == ogr.wkbMultiLineString:
        if geom.GetGeometryCount() > 0:
            geom = geom.GetGeometryRef(0).Clone()
    elif geom_type == ogr.wkbMultiPolygon:
        if geom.GetGeometryCount() > 0:
            geom = geom.GetGeometryRef(0).Clone()

    dst_feature.SetGeometry(geom)

    # Copy common fields
    src_defn = src_feature.GetDefnRef()
    for i in range(src_defn.GetFieldCount()):
        field_name = src_defn.GetFieldDefn(i).GetName()

        # Check if destination has this field
        dst_idx = dst_defn.GetFieldIndex(field_name)
        if dst_idx >= 0:
            value = src_feature.GetField(i)
            if value is not None:
                dst_feature.SetField(dst_idx, str(value))

    # Create the feature
    err = dst_layer.CreateFeature(dst_feature)
    return err == ogr.OGRERR_NONE


def convert_geojson_to_mp(input_file, output_file):
    """Convert a GeoJSON file to Polish Map format."""

    print(f"Converting: {input_file} -> {output_file}")
    print("=" * 60)

    # Open source file
    src_ds = ogr.Open(input_file)
    if src_ds is None:
        print(f"ERROR: Could not open {input_file}")
        return False

    print(f"Source: {src_ds.GetDriver().GetName()}")
    print(f"Source layers: {src_ds.GetLayerCount()}")

    # Create destination file
    driver = ogr.GetDriverByName("PolishMap")
    if driver is None:
        print("ERROR: PolishMap driver not available")
        return False

    dst_ds = driver.CreateDataSource(output_file)
    if dst_ds is None:
        print(f"ERROR: Could not create {output_file}")
        return False

    print(f"Destination: {dst_ds.GetDriver().GetName()}")
    print(f"Destination layers: {dst_ds.GetLayerCount()}")
    print()

    # Statistics
    stats = {"POI": 0, "POLYLINE": 0, "POLYGON": 0, "skipped": 0}

    # Process each source layer
    for src_layer_idx in range(src_ds.GetLayerCount()):
        src_layer = src_ds.GetLayer(src_layer_idx)
        src_layer_name = src_layer.GetName()
        feature_count = src_layer.GetFeatureCount()

        print(f"Processing layer: {src_layer_name} ({feature_count} features)")

        src_layer.ResetReading()
        src_feature = src_layer.GetNextFeature()

        while src_feature:
            geom = src_feature.GetGeometryRef()
            if geom:
                geom_type = geom.GetGeometryType()

                # Flatten to 2D if necessary
                flat_type = ogr.GT_Flatten(geom_type)

                # Get target layer
                dst_layer = get_target_layer(dst_ds, flat_type)

                if dst_layer:
                    if convert_feature(src_feature, dst_layer):
                        stats[dst_layer.GetName()] += 1
                    else:
                        stats["skipped"] += 1
                else:
                    print(f"  Warning: Unsupported geometry type: "
                          f"{ogr.GeometryTypeToName(geom_type)}")
                    stats["skipped"] += 1
            else:
                stats["skipped"] += 1

            src_feature = src_layer.GetNextFeature()

    # Close datasets
    src_ds = None
    dst_ds = None

    # Print summary
    print()
    print("Conversion Summary:")
    print(f"  POI features:      {stats['POI']}")
    print(f"  POLYLINE features: {stats['POLYLINE']}")
    print(f"  POLYGON features:  {stats['POLYGON']}")
    print(f"  Skipped:           {stats['skipped']}")
    print()
    print(f"Successfully created: {output_file}")

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

    if not convert_geojson_to_mp(input_file, output_file):
        return 1

    print("\nTip: Use ogrinfo or read_mp.py to verify the converted file")
    return 0


if __name__ == "__main__":
    sys.exit(main())
