#!/usr/bin/env python3
"""
Story 3.3: Generate Valid-Complex Test Corpus

Generates 100-200 .mp files for the valid-complex test corpus with:
- POI-only files (varied types, labels, positions)
- POLYLINE-only files (varied lengths, types)
- POLYGON-only files (varied sizes, types)
- Mixed files (all geometry types combined)
- Real-world simulated files (100+ features, complex)

Usage:
    python3 generate_valid_complex_corpus.py [output_dir]

Default output: test/data/valid-complex/
"""

import os
import sys
import random
import string
from datetime import datetime

# =============================================================================
# Configuration
# =============================================================================

# POI types (Garmin codes)
POI_TYPES = [
    0x0001,  # City (Large)
    0x0002,  # City (Medium)
    0x0003,  # City (Small)
    0x0100,  # Restaurant
    0x0200,  # Hotel
    0x0400,  # Attraction
    0x0500,  # Parking
    0x0600,  # Fuel Station
    0x0700,  # Shopping Center
    0x0D00,  # Information
    0x1100,  # Hospital
    0x2000,  # Exit
    0x2C00,  # Restaurant (alt)
    0x2A00,  # Hotel (alt)
    0x2B00,  # Shop
    0x6401,  # Bridge
    0x6402,  # Building
    0x6403,  # Cemetery
]

# POLYLINE types (Garmin road codes)
POLYLINE_TYPES = [
    0x0001,  # Major Highway
    0x0002,  # Principal Highway
    0x0003,  # Other Highway
    0x0004,  # Arterial Road
    0x0005,  # Collector Road
    0x0006,  # Residential Street
    0x0007,  # Alley/Private Road
    0x0008,  # Ramp (Low Speed)
    0x0009,  # Ramp (High Speed)
    0x000A,  # Unpaved Road
    0x0014,  # Railroad
    0x0015,  # Shoreline
    0x0016,  # Trail
    0x0018,  # Stream
    0x001F,  # River
]

# POLYGON types (Garmin zone codes)
POLYGON_TYPES = [
    0x0001,  # City/Large Urban Area
    0x0002,  # City/Small Urban Area
    0x0003,  # Water (Ocean/Sea)
    0x0004,  # Water (Lake)
    0x0005,  # Woods (Forest)
    0x0006,  # Park
    0x0007,  # Airport
    0x0008,  # Shopping Center
    0x0009,  # Marina
    0x000A,  # University
    0x000B,  # Hospital
    0x000C,  # Industrial
    0x0013,  # Man-Made Area
    0x004C,  # Forest (alt)
    0x0050,  # Woods (alt)
]

# Geographic regions (WGS84 bounding boxes)
REGIONS = {
    'france': {'lat_min': 41.0, 'lat_max': 51.5, 'lon_min': -5.0, 'lon_max': 10.0},
    'europe': {'lat_min': 35.0, 'lat_max': 60.0, 'lon_min': -10.0, 'lon_max': 30.0},
    'world': {'lat_min': -60.0, 'lat_max': 80.0, 'lon_min': -180.0, 'lon_max': 180.0},
}

# Label templates
POI_LABELS = [
    "Restaurant {}", "Hotel {}", "Parking {}", "Station {}", "Shop {}",
    "Museum {}", "Park {}", "School {}", "Hospital {}", "Bank {}",
    "Cinema {}", "Theater {}", "Library {}", "Church {}", "Mosque {}",
    "Temple {}", "Castle {}", "Tower {}", "Bridge {}", "Monument {}",
    "Cafe {}", "Bar {}", "Club {}", "Gym {}", "Pool {}",
]

POLYLINE_LABELS = [
    "Route {}", "Rue {}", "Avenue {}", "Boulevard {}", "Chemin {}",
    "Autoroute {}", "Nationale {}", "Departementale {}", "Voie {}",
    "Sentier {}", "Piste {}", "Rail {}", "Riviere {}", "Fleuve {}",
]

POLYGON_LABELS = [
    "Foret {}", "Lac {}", "Parc {}", "Zone {}", "Quartier {}",
    "Industrial {}", "Commercial {}", "Residential {}", "Campus {}",
    "Airport {}", "Port {}", "Stadium {}", "Golf {}", "Cemetery {}",
]


# =============================================================================
# Helper Functions
# =============================================================================

def random_coord(region='france'):
    """Generate a random coordinate within a region."""
    r = REGIONS[region]
    lat = random.uniform(r['lat_min'], r['lat_max'])
    lon = random.uniform(r['lon_min'], r['lon_max'])
    # Round to 6 decimals (GPS precision)
    return round(lat, 6), round(lon, 6)


def random_label(templates):
    """Generate a random label from templates."""
    template = random.choice(templates)
    suffix = ''.join(random.choices(string.ascii_uppercase + string.digits, k=3))
    return template.format(suffix)


def generate_poi_coords(count, region='france'):
    """Generate a list of POI coordinates."""
    return [random_coord(region) for _ in range(count)]


def generate_polyline_coords(point_count, region='france'):
    """Generate a polyline as a list of connected coordinates."""
    # Start point
    start_lat, start_lon = random_coord(region)
    coords = [(start_lat, start_lon)]

    # Generate connected points with small offsets
    for _ in range(point_count - 1):
        lat_offset = random.uniform(-0.01, 0.01)
        lon_offset = random.uniform(-0.01, 0.01)
        new_lat = round(coords[-1][0] + lat_offset, 6)
        new_lon = round(coords[-1][1] + lon_offset, 6)
        coords.append((new_lat, new_lon))

    return coords


def generate_polygon_coords(point_count, region='france'):
    """Generate a closed polygon as a list of coordinates."""
    center_lat, center_lon = random_coord(region)
    coords = []

    # Generate points in a rough circle around center
    import math
    radius = random.uniform(0.001, 0.01)  # ~100m to ~1km

    for i in range(point_count - 1):  # -1 because we'll close the ring
        angle = 2 * math.pi * i / (point_count - 1)
        lat = round(center_lat + radius * math.sin(angle), 6)
        lon = round(center_lon + radius * math.cos(angle), 6)
        coords.append((lat, lon))

    # Close the polygon
    coords.append(coords[0])

    return coords


def format_poi(poi_type, label, lat, lon, end_level=None):
    """Format a POI section."""
    lines = [
        "[POI]",
        f"Type=0x{poi_type:04X}",
        f"Label={label}",
        f"Data0=({lat},{lon})",
    ]
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def format_polyline(line_type, label, coords, end_level=None):
    """Format a POLYLINE section."""
    coord_str = ",".join(f"({lat},{lon})" for lat, lon in coords)
    lines = [
        "[POLYLINE]",
        f"Type=0x{line_type:04X}",
        f"Label={label}",
        f"Data0={coord_str}",
    ]
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def format_polygon(poly_type, label, coords, end_level=None):
    """Format a POLYGON section."""
    coord_str = ",".join(f"({lat},{lon})" for lat, lon in coords)
    lines = [
        "[POLYGON]",
        f"Type=0x{poly_type:04X}",
        f"Label={label}",
        f"Data0={coord_str}",
    ]
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def format_header(name, map_id=None):
    """Format the IMG ID header section."""
    if map_id is None:
        map_id = random.randint(10000000, 99999999)
    return f"""[IMG ID]
Name={name}
ID={map_id}
CodePage=1252
Datum=WGS 84
[END-IMG ID]
"""


def write_mp_file(filepath, name, sections):
    """Write a complete .mp file."""
    content = format_header(name) + "\n" + "\n\n".join(sections) + "\n"
    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(content)


# =============================================================================
# File Generators
# =============================================================================

def generate_poi_file(output_dir, index, feature_count=None, region='france'):
    """Generate a POI-only file."""
    if feature_count is None:
        feature_count = random.randint(1, 20)

    filename = f"poi-varied-{index:03d}.mp"
    filepath = os.path.join(output_dir, filename)
    name = f"POI Test {index}"

    sections = []
    for _ in range(feature_count):
        poi_type = random.choice(POI_TYPES)
        label = random_label(POI_LABELS)
        lat, lon = random_coord(region)
        end_level = random.choice([None, 1, 2, 3, 4]) if random.random() > 0.5 else None
        sections.append(format_poi(poi_type, label, lat, lon, end_level))

    write_mp_file(filepath, name, sections)
    return filepath, feature_count


def generate_polyline_file(output_dir, index, feature_count=None, region='france'):
    """Generate a POLYLINE-only file."""
    if feature_count is None:
        feature_count = random.randint(1, 15)

    filename = f"polyline-varied-{index:03d}.mp"
    filepath = os.path.join(output_dir, filename)
    name = f"POLYLINE Test {index}"

    sections = []
    for _ in range(feature_count):
        line_type = random.choice(POLYLINE_TYPES)
        label = random_label(POLYLINE_LABELS)
        point_count = random.randint(2, 50)
        coords = generate_polyline_coords(point_count, region)
        end_level = random.choice([None, 1, 2, 3]) if random.random() > 0.5 else None
        sections.append(format_polyline(line_type, label, coords, end_level))

    write_mp_file(filepath, name, sections)
    return filepath, feature_count


def generate_polygon_file(output_dir, index, feature_count=None, region='france'):
    """Generate a POLYGON-only file."""
    if feature_count is None:
        feature_count = random.randint(1, 10)

    filename = f"polygon-varied-{index:03d}.mp"
    filepath = os.path.join(output_dir, filename)
    name = f"POLYGON Test {index}"

    sections = []
    for _ in range(feature_count):
        poly_type = random.choice(POLYGON_TYPES)
        label = random_label(POLYGON_LABELS)
        point_count = random.randint(4, 30)  # Minimum 4 for closed polygon
        coords = generate_polygon_coords(point_count, region)
        end_level = random.choice([None, 1, 2, 3]) if random.random() > 0.5 else None
        sections.append(format_polygon(poly_type, label, coords, end_level))

    write_mp_file(filepath, name, sections)
    return filepath, feature_count


def generate_mixed_file(output_dir, index, region='france'):
    """Generate a mixed file with POI, POLYLINE, and POLYGON features."""
    poi_count = random.randint(2, 10)
    polyline_count = random.randint(2, 8)
    polygon_count = random.randint(1, 5)

    filename = f"mixed-varied-{index:03d}.mp"
    filepath = os.path.join(output_dir, filename)
    name = f"Mixed Test {index}"

    sections = []

    # POIs
    for _ in range(poi_count):
        poi_type = random.choice(POI_TYPES)
        label = random_label(POI_LABELS)
        lat, lon = random_coord(region)
        sections.append(format_poi(poi_type, label, lat, lon))

    # POLYLINEs
    for _ in range(polyline_count):
        line_type = random.choice(POLYLINE_TYPES)
        label = random_label(POLYLINE_LABELS)
        point_count = random.randint(2, 30)
        coords = generate_polyline_coords(point_count, region)
        sections.append(format_polyline(line_type, label, coords))

    # POLYGONs
    for _ in range(polygon_count):
        poly_type = random.choice(POLYGON_TYPES)
        label = random_label(POLYGON_LABELS)
        point_count = random.randint(4, 20)
        coords = generate_polygon_coords(point_count, region)
        sections.append(format_polygon(poly_type, label, coords))

    write_mp_file(filepath, name, sections)
    return filepath, poi_count + polyline_count + polygon_count


def generate_realworld_file(output_dir, index, region='france'):
    """Generate a real-world simulated file with 100+ features."""
    poi_count = random.randint(40, 60)
    polyline_count = random.randint(30, 50)
    polygon_count = random.randint(20, 30)

    filename = f"real-world-{index:03d}.mp"
    filepath = os.path.join(output_dir, filename)
    name = f"Real World Map {index}"

    sections = []

    # POIs - varied types simulating a real map
    for i in range(poi_count):
        poi_type = random.choice(POI_TYPES)
        label = random_label(POI_LABELS)
        lat, lon = random_coord(region)
        end_level = random.randint(1, 4) if i % 3 == 0 else None
        sections.append(format_poi(poi_type, label, lat, lon, end_level))

    # POLYLINEs - roads and water features
    for i in range(polyline_count):
        line_type = random.choice(POLYLINE_TYPES)
        label = random_label(POLYLINE_LABELS)
        # Real roads have more points
        point_count = random.randint(5, 100)
        coords = generate_polyline_coords(point_count, region)
        end_level = random.randint(1, 3) if i % 4 == 0 else None
        sections.append(format_polyline(line_type, label, coords, end_level))

    # POLYGONs - areas
    for i in range(polygon_count):
        poly_type = random.choice(POLYGON_TYPES)
        label = random_label(POLYGON_LABELS)
        # Real areas have more points
        point_count = random.randint(8, 50)
        coords = generate_polygon_coords(point_count, region)
        end_level = random.randint(1, 3) if i % 3 == 0 else None
        sections.append(format_polygon(poly_type, label, coords, end_level))

    write_mp_file(filepath, name, sections)
    return filepath, poi_count + polyline_count + polygon_count


# =============================================================================
# Main Generation Function
# =============================================================================

def generate_corpus(output_dir):
    """Generate the complete valid-complex corpus."""
    print(f"Generating valid-complex corpus in: {output_dir}")
    print()

    # Ensure output directory exists
    os.makedirs(output_dir, exist_ok=True)

    stats = {
        'poi': {'files': 0, 'features': 0},
        'polyline': {'files': 0, 'features': 0},
        'polygon': {'files': 0, 'features': 0},
        'mixed': {'files': 0, 'features': 0},
        'realworld': {'files': 0, 'features': 0},
    }

    # Generate POI-only files (30 files)
    print("Generating POI-only files...", end=" ")
    for i in range(1, 31):
        region = random.choice(['france', 'europe', 'france', 'france'])  # Bias towards France
        _, count = generate_poi_file(output_dir, i, region=region)
        stats['poi']['files'] += 1
        stats['poi']['features'] += count
    print(f"30 files, {stats['poi']['features']} features")

    # Generate POLYLINE-only files (30 files)
    print("Generating POLYLINE-only files...", end=" ")
    for i in range(1, 31):
        region = random.choice(['france', 'europe', 'france', 'france'])
        _, count = generate_polyline_file(output_dir, i, region=region)
        stats['polyline']['files'] += 1
        stats['polyline']['features'] += count
    print(f"30 files, {stats['polyline']['features']} features")

    # Generate POLYGON-only files (30 files)
    print("Generating POLYGON-only files...", end=" ")
    for i in range(1, 31):
        region = random.choice(['france', 'europe', 'france', 'france'])
        _, count = generate_polygon_file(output_dir, i, region=region)
        stats['polygon']['files'] += 1
        stats['polygon']['features'] += count
    print(f"30 files, {stats['polygon']['features']} features")

    # Generate mixed files (50 files)
    print("Generating mixed files...", end=" ")
    for i in range(1, 51):
        region = random.choice(['france', 'europe', 'world'])
        _, count = generate_mixed_file(output_dir, i, region=region)
        stats['mixed']['files'] += 1
        stats['mixed']['features'] += count
    print(f"50 files, {stats['mixed']['features']} features")

    # Generate real-world simulated files (15 files)
    print("Generating real-world files...", end=" ")
    for i in range(1, 16):
        region = random.choice(['france', 'europe'])
        _, count = generate_realworld_file(output_dir, i, region=region)
        stats['realworld']['files'] += 1
        stats['realworld']['features'] += count
    print(f"15 files, {stats['realworld']['features']} features")

    # Summary
    total_files = sum(s['files'] for s in stats.values())
    total_features = sum(s['features'] for s in stats.values())

    print()
    print("=" * 50)
    print("Corpus Generation Complete")
    print("=" * 50)
    print(f"Total files: {total_files}")
    print(f"Total features: {total_features}")
    print()
    print("Breakdown:")
    print(f"  POI-only:      {stats['poi']['files']:3d} files, {stats['poi']['features']:5d} features")
    print(f"  POLYLINE-only: {stats['polyline']['files']:3d} files, {stats['polyline']['features']:5d} features")
    print(f"  POLYGON-only:  {stats['polygon']['files']:3d} files, {stats['polygon']['features']:5d} features")
    print(f"  Mixed:         {stats['mixed']['files']:3d} files, {stats['mixed']['features']:5d} features")
    print(f"  Real-world:    {stats['realworld']['files']:3d} files, {stats['realworld']['features']:5d} features")
    print()
    print(f"Output directory: {output_dir}")

    return total_files, total_features


def main():
    """Main entry point."""
    # Default output directory
    script_dir = os.path.dirname(os.path.abspath(__file__))
    default_output = os.path.join(script_dir, '..', 'test', 'data', 'valid-complex')

    # Allow override via command line
    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = default_output

    output_dir = os.path.abspath(output_dir)

    # Validate output directory
    parent_dir = os.path.dirname(output_dir)
    if not os.path.isdir(parent_dir):
        print(f"ERROR: Parent directory does not exist: {parent_dir}")
        print("Please create the parent directory first or specify a valid path.")
        return 1

    # Set random seed for reproducibility (can be removed for true randomness)
    random.seed(20260202)  # Story creation date

    try:
        total_files, total_features = generate_corpus(output_dir)
    except OSError as e:
        print(f"\nERROR: Failed to generate corpus: {e}")
        return 1
    except Exception as e:
        print(f"\nERROR: Unexpected error during generation: {e}")
        return 1

    # Exit with success if we generated enough files
    if total_files >= 100:
        print("\nCorpus meets AC4 requirement (100-200 files).")
        return 0
    else:
        print(f"\nWarning: Only {total_files} files generated, expected >= 100.")
        return 1


if __name__ == '__main__':
    sys.exit(main())
