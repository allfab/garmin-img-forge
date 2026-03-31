#!/usr/bin/env python3
"""
Story 3.1: Performance Test Data Generator (Task 4)

Generates Polish Map format test files of specified sizes for performance benchmarking.
Target sizes:
- 1 MB (~5,000 mixed features)
- 10 MB (~50,000 mixed features)
- 50 MB (~250,000 mixed features)
- 100 MB (~500,000 mixed features)

Architecture: NFR1 (parsing < 2s for 10 MB), NFR2 (writing < 3s for 10 MB)
"""

import argparse
import os
import random
import math
import sys

# Feature type codes (from Garmin specification)
POI_TYPES = ["0x2C00", "0x2A00", "0x2B00", "0x2D00", "0x6401"]
POLYLINE_TYPES = ["0x0001", "0x0006", "0x0016", "0x001A", "0x001B"]
POLYGON_TYPES = ["0x0001", "0x0019", "0x003C", "0x004C", "0x0050"]

# Sample labels
LABELS = [
    "Restaurant", "Hotel", "Cafe", "Shop", "Bank",
    "Main Street", "Highway", "River", "Park Avenue",
    "Forest", "Lake", "Building", "Industrial Zone"
]

def generate_header(name):
    """Generate [IMG ID] header section."""
    return f"""[IMG ID]
Name={name}
ID=12345678
CodePage=1252
Datum=WGS 84
[END]

"""

def generate_poi(feature_id, lat, lon):
    """Generate a POI feature."""
    poi_type = random.choice(POI_TYPES)
    label = f"{random.choice(LABELS)} {feature_id}"
    end_level = random.randint(0, 9)

    return f"""[POI]
Type={poi_type}
Label={label}
Data0=({lat:.6f},{lon:.6f})
EndLevel={end_level}
[END]

"""

def generate_polyline(feature_id, base_lat, base_lon, num_points=10):
    """Generate a POLYLINE feature with multiple points."""
    line_type = random.choice(POLYLINE_TYPES)
    label = f"{random.choice(LABELS)} {feature_id}"

    # Generate connected points
    coords = []
    lat, lon = base_lat, base_lon
    for i in range(num_points):
        coords.append(f"({lat:.6f},{lon:.6f})")
        # Small random walk
        lat += random.uniform(-0.001, 0.001)
        lon += random.uniform(-0.001, 0.001)

    data0 = ",".join(coords)

    return f"""[POLYLINE]
Type={line_type}
Label={label}
Data0={data0}
EndLevel={random.randint(0, 5)}
[END]

"""

def generate_polygon(feature_id, base_lat, base_lon, num_points=8):
    """Generate a POLYGON feature with closed ring."""
    poly_type = random.choice(POLYGON_TYPES)
    label = f"{random.choice(LABELS)} {feature_id}"

    # Generate a rough polygon shape
    coords = []
    angle_step = 2 * math.pi / num_points
    radius = random.uniform(0.001, 0.005)

    first_lat, first_lon = None, None
    for i in range(num_points):
        angle = i * angle_step
        lat = base_lat + radius * math.cos(angle)
        lon = base_lon + radius * math.sin(angle)
        coords.append(f"({lat:.6f},{lon:.6f})")
        if i == 0:
            first_lat, first_lon = lat, lon

    # Close the ring
    coords.append(f"({first_lat:.6f},{first_lon:.6f})")
    data0 = ",".join(coords)

    return f"""[POLYGON]
Type={poly_type}
Label={label}
Data0={data0}
EndLevel={random.randint(0, 3)}
[END]

"""

def generate_test_file(output_path, target_size_bytes, feature_ratio=(0.5, 0.3, 0.2)):
    """
    Generate a test file of approximately target_size_bytes.

    Args:
        output_path: Path to output .mp file
        target_size_bytes: Approximate target file size in bytes
        feature_ratio: Tuple of (POI, POLYLINE, POLYGON) ratios
    """
    poi_ratio, line_ratio, poly_ratio = feature_ratio

    # Estimate bytes per feature type
    avg_poi_size = 120      # ~120 bytes per POI
    avg_line_size = 350     # ~350 bytes per POLYLINE (10 points)
    avg_poly_size = 400     # ~400 bytes per POLYGON (8 points)

    # Calculate weighted average feature size
    avg_feature_size = (
        poi_ratio * avg_poi_size +
        line_ratio * avg_line_size +
        poly_ratio * avg_poly_size
    )

    # Estimate total features needed
    header_size = 100
    estimated_features = int((target_size_bytes - header_size) / avg_feature_size)

    # Calculate feature counts
    num_pois = int(estimated_features * poi_ratio)
    num_polylines = int(estimated_features * line_ratio)
    num_polygons = estimated_features - num_pois - num_polylines

    print(f"Generating: {output_path}")
    print(f"  Target size: {target_size_bytes / (1024*1024):.1f} MB")
    print(f"  Estimated features: {estimated_features}")
    print(f"  POIs: {num_pois}, Polylines: {num_polylines}, Polygons: {num_polygons}")

    # Generate file
    with open(output_path, 'w', encoding='cp1252') as f:
        # Write header
        name = os.path.basename(output_path).replace('.mp', '')
        f.write(generate_header(name))

        # World bounds for random coordinates
        lat_min, lat_max = 45.0, 55.0
        lon_min, lon_max = -5.0, 15.0

        feature_id = 0

        # Write POIs
        for i in range(num_pois):
            lat = random.uniform(lat_min, lat_max)
            lon = random.uniform(lon_min, lon_max)
            f.write(generate_poi(feature_id, lat, lon))
            feature_id += 1
            if feature_id % 10000 == 0:
                print(f"    Generated {feature_id} features...")

        # Write Polylines
        for i in range(num_polylines):
            lat = random.uniform(lat_min, lat_max)
            lon = random.uniform(lon_min, lon_max)
            num_points = random.randint(5, 20)
            f.write(generate_polyline(feature_id, lat, lon, num_points))
            feature_id += 1
            if feature_id % 10000 == 0:
                print(f"    Generated {feature_id} features...")

        # Write Polygons
        for i in range(num_polygons):
            lat = random.uniform(lat_min, lat_max)
            lon = random.uniform(lon_min, lon_max)
            num_points = random.randint(4, 12)
            f.write(generate_polygon(feature_id, lat, lon, num_points))
            feature_id += 1
            if feature_id % 10000 == 0:
                print(f"    Generated {feature_id} features...")

    # Report actual size
    actual_size = os.path.getsize(output_path)
    print(f"  Actual size: {actual_size / (1024*1024):.2f} MB ({feature_id} features)")
    print()

    return actual_size, feature_id

def main():
    parser = argparse.ArgumentParser(
        description='Generate Polish Map test files for performance benchmarking'
    )
    parser.add_argument(
        '--output-dir', '-o',
        default='test/data/performance',
        help='Output directory for test files'
    )
    parser.add_argument(
        '--sizes', '-s',
        nargs='+',
        type=int,
        default=[1, 10],
        help='File sizes in MB to generate (default: 1 10)'
    )
    parser.add_argument(
        '--all', '-a',
        action='store_true',
        help='Generate all standard sizes (1, 10, 50, 100 MB)'
    )
    args = parser.parse_args()

    # Create output directory
    os.makedirs(args.output_dir, exist_ok=True)

    # Determine sizes to generate
    if args.all:
        sizes_mb = [1, 10, 50, 100]
    else:
        sizes_mb = args.sizes

    print("=" * 60)
    print("Story 3.1: Performance Test Data Generator")
    print("=" * 60)
    print()

    # Set seed for reproducibility
    random.seed(42)

    results = []
    for size_mb in sizes_mb:
        output_path = os.path.join(args.output_dir, f"perf-{size_mb}mb.mp")
        target_bytes = size_mb * 1024 * 1024
        actual_size, feature_count = generate_test_file(output_path, target_bytes)
        results.append((size_mb, actual_size, feature_count))

    print("=" * 60)
    print("Summary")
    print("=" * 60)
    for size_mb, actual_size, feature_count in results:
        print(f"  perf-{size_mb}mb.mp: {actual_size / (1024*1024):.2f} MB, {feature_count} features")
    print()
    print("Done!")

if __name__ == '__main__':
    main()
