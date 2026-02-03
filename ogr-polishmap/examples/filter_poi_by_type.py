#!/usr/bin/env python3
"""
Example: Filtering POI features by Type code using GDAL Python bindings

This example demonstrates how to:
- Open a Polish Map file and access the POI layer
- Apply attribute filters to select specific POI types
- Apply spatial filters to limit features by bounding box
- Combine attribute and spatial filters

Garmin POI Type Codes Reference:
- 0x2C00-0x2CFF: Food and drink (restaurants, cafes, bars)
- 0x2F00-0x2FFF: Auto services (gas stations, parking)
- 0x2E00-0x2EFF: Shopping (stores, malls)
- 0x2D00-0x2DFF: Lodging (hotels, campgrounds)
- 0x6400-0x64FF: Landmarks and attractions

Requirements:
- Python GDAL bindings (python3-gdal or osgeo package)
- PolishMap driver must be installed (GDAL_DRIVER_PATH set if needed)

Usage:
    python3 filter_poi_by_type.py <input.mp> [type_code]
    python3 filter_poi_by_type.py ../test/data/valid-minimal/poi-multiple.mp
    python3 filter_poi_by_type.py ../test/data/valid-minimal/poi-multiple.mp 0x2C00
"""

import sys
from osgeo import ogr, gdal

# Enable GDAL exceptions for better error handling
gdal.UseExceptions()

# Common Garmin POI type codes
POI_TYPES = {
    # Food and Drink
    "0x2C00": "Restaurant (Other)",
    "0x2C01": "Restaurant (American)",
    "0x2C02": "Restaurant (Asian)",
    "0x2C03": "Restaurant (Barbecue)",
    "0x2C04": "Restaurant (Chinese)",
    "0x2C05": "Restaurant (Deli/Bakery)",
    "0x2C06": "Restaurant (International)",
    "0x2C07": "Restaurant (Fast Food)",
    "0x2C08": "Restaurant (Italian)",
    "0x2C09": "Restaurant (Mexican)",
    "0x2C0A": "Restaurant (Pizza)",
    "0x2C0B": "Restaurant (Seafood)",
    "0x2C0C": "Restaurant (Steak/Grill)",
    "0x2C0D": "Restaurant (Bagel/Donut)",
    "0x2C0E": "Restaurant (Cafe/Diner)",
    "0x2C0F": "Restaurant (French)",
    "0x2C10": "Restaurant (German)",
    "0x2C11": "Restaurant (British)",
    # Auto Services
    "0x2F00": "Gas Station (Other)",
    "0x2F01": "Gas Station",
    "0x2F02": "Auto Rental",
    "0x2F03": "Auto Repair",
    "0x2F04": "Airport",
    "0x2F05": "Post Office",
    "0x2F06": "Bank/ATM",
    "0x2F07": "Auto Dealer",
    "0x2F08": "Ground Transportation",
    "0x2F09": "Marina",
    "0x2F0A": "Wrecker Service",
    "0x2F0B": "Parking",
    "0x2F0C": "Rest Area/Tourist Info",
    # Shopping
    "0x2E00": "Shopping (Other)",
    "0x2E01": "Department Store",
    "0x2E02": "Grocery Store",
    "0x2E03": "General Merchandise",
    "0x2E04": "Shopping Center",
    "0x2E05": "Pharmacy",
    "0x2E06": "Convenience Store",
    "0x2E07": "Clothing Store",
    "0x2E08": "Home/Garden Store",
    "0x2E09": "Home Furnishings",
    "0x2E0A": "Specialty Retail",
    "0x2E0B": "Computer/Software",
    # Lodging
    "0x2D00": "Lodging (Other)",
    "0x2D01": "Hotel/Motel",
    "0x2D02": "Bed & Breakfast",
    "0x2D03": "Campground/RV Park",
    "0x2D04": "Resort",
    "0x2D05": "Trailer Park",
}


def describe_type(type_code):
    """Get human-readable description for a type code."""
    type_upper = type_code.upper() if type_code else ""
    return POI_TYPES.get(type_upper, f"Unknown ({type_code})")


def filter_poi_by_type(filename, type_filter=None):
    """Filter and display POI features by type code."""

    print(f"Opening: {filename}")
    print("=" * 60)

    # Open the file
    ds = ogr.Open(filename)
    if ds is None:
        print(f"ERROR: Could not open {filename}")
        return False

    # Get POI layer (index 0)
    poi_layer = ds.GetLayer(0)
    if poi_layer is None or poi_layer.GetName() != "POI":
        print("ERROR: POI layer not found")
        return False

    print(f"Layer: {poi_layer.GetName()}")
    print(f"Total features: {poi_layer.GetFeatureCount()}")
    print()

    # Apply attribute filter if specified
    if type_filter:
        # SQL-style attribute filter
        filter_expr = f"Type = '{type_filter}'"
        poi_layer.SetAttributeFilter(filter_expr)
        filtered_count = poi_layer.GetFeatureCount()
        print(f"Attribute filter: {filter_expr}")
        print(f"Filtered features: {filtered_count}")
        print(f"Type description: {describe_type(type_filter)}")
    else:
        print("No type filter applied (showing all POIs)")

    print()

    # Display filtered features
    print("Filtered POI Features:")
    print("-" * 60)

    poi_layer.ResetReading()
    feature = poi_layer.GetNextFeature()
    count = 0

    while feature:
        fid = feature.GetFID()
        type_val = feature.GetField("Type") or ""
        label = feature.GetField("Label") or "(no label)"

        # Get coordinates
        geom = feature.GetGeometryRef()
        if geom and geom.GetGeometryType() == ogr.wkbPoint:
            lon = geom.GetX()
            lat = geom.GetY()
            coords = f"({lat:.6f}, {lon:.6f})"
        else:
            coords = "(no geometry)"

        # Get type description
        type_desc = describe_type(type_val)

        print(f"FID {fid}: {label}")
        print(f"  Type: {type_val} - {type_desc}")
        print(f"  Coords: {coords}")
        print()

        count += 1
        feature = poi_layer.GetNextFeature()

    print("-" * 60)
    print(f"Total displayed: {count} features")

    # Clear filter
    poi_layer.SetAttributeFilter(None)

    ds = None
    return True


def filter_poi_by_bbox(filename, minx, miny, maxx, maxy):
    """Filter POI features by spatial bounding box."""

    print(f"Opening: {filename}")
    print("=" * 60)

    ds = ogr.Open(filename)
    if ds is None:
        print(f"ERROR: Could not open {filename}")
        return False

    poi_layer = ds.GetLayer(0)
    if poi_layer is None:
        print("ERROR: POI layer not found")
        return False

    print(f"Total features: {poi_layer.GetFeatureCount()}")
    print(f"Spatial filter: BBOX({minx}, {miny}, {maxx}, {maxy})")

    # Apply spatial filter
    poi_layer.SetSpatialFilterRect(minx, miny, maxx, maxy)
    filtered_count = poi_layer.GetFeatureCount()
    print(f"Filtered features: {filtered_count}")
    print()

    # Display filtered features
    poi_layer.ResetReading()
    for feature in poi_layer:
        fid = feature.GetFID()
        label = feature.GetField("Label") or "(no label)"
        type_val = feature.GetField("Type") or ""
        print(f"FID {fid}: {label} (Type: {type_val})")

    # Clear filter
    poi_layer.SetSpatialFilter(None)

    ds = None
    return True


def list_poi_types(filename):
    """List all unique POI types in the file."""

    print(f"Analyzing POI types in: {filename}")
    print("=" * 60)

    ds = ogr.Open(filename)
    if ds is None:
        print(f"ERROR: Could not open {filename}")
        return False

    poi_layer = ds.GetLayer(0)
    if poi_layer is None:
        print("ERROR: POI layer not found")
        return False

    # Count POIs by type
    type_counts = {}
    poi_layer.ResetReading()

    for feature in poi_layer:
        type_val = feature.GetField("Type") or "(none)"
        type_counts[type_val] = type_counts.get(type_val, 0) + 1

    # Sort by count (descending)
    sorted_types = sorted(type_counts.items(), key=lambda x: x[1], reverse=True)

    print(f"Total POI count: {poi_layer.GetFeatureCount()}")
    print(f"Unique types: {len(type_counts)}")
    print()
    print("POI Types Summary:")
    print("-" * 60)
    print(f"{'Type':<12} {'Count':>6}  Description")
    print("-" * 60)

    for type_code, count in sorted_types:
        desc = describe_type(type_code)
        print(f"{type_code:<12} {count:>6}  {desc}")

    ds = None
    return True


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        print("\nExample commands:")
        print("  # List all unique POI types")
        print("  python3 filter_poi_by_type.py sample.mp")
        print()
        print("  # Filter by specific type code")
        print("  python3 filter_poi_by_type.py sample.mp 0x2C00")
        print()
        print("  # Available type codes:")
        for code, desc in list(POI_TYPES.items())[:10]:
            print(f"    {code}: {desc}")
        print("    ... and more")
        return 1

    filename = sys.argv[1]
    type_filter = sys.argv[2] if len(sys.argv) > 2 else None

    # Check if PolishMap driver is available
    driver = ogr.GetDriverByName("PolishMap")
    if driver is None:
        print("ERROR: PolishMap driver not available")
        print("Make sure GDAL_DRIVER_PATH is set to include the plugin")
        return 1

    if type_filter:
        # Filter by specific type
        if not filter_poi_by_type(filename, type_filter):
            return 1
    else:
        # List all types
        if not list_poi_types(filename):
            return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
