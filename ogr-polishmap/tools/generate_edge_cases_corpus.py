#!/usr/bin/env python3
"""
Story 3.4: Generate Edge-Cases Test Corpus

Generates 50-100 .mp files for the edge-cases test corpus with:
- Label edge cases (empty, long, special chars, unicode, encoding)
- Coordinate edge cases (WGS84 limits, precision, extreme values)
- Data fields edge cases (Data0-Data10, sparse, out of order)
- Feature count edge cases (single, many, mixed types)
- Encoding edge cases (CP1252, CRLF, LF, mixed)

Usage:
    python3 generate_edge_cases_corpus.py [output_dir]

Default output: test/data/edge-cases/
"""

import os
import sys
import random
import string
import math


# =============================================================================
# Configuration
# =============================================================================

# POI types for edge case testing
POI_TYPES = [0x0001, 0x0100, 0x0500, 0x0600, 0x2C00]

# POLYLINE types for edge case testing
POLYLINE_TYPES = [0x0001, 0x0004, 0x0006, 0x0014, 0x001F]

# POLYGON types for edge case testing
POLYGON_TYPES = [0x0001, 0x0003, 0x0005, 0x0006, 0x000C]

# CP1252 characters (extended Latin)
CP1252_CHARS = "éèêëàâäùûüôöîïçñáéíóúÀÂÄÈÉÊËÎÏÔÖÙÛÜ"
CP1252_FRENCH = "éèêëàâäùûüôöîïç"
CP1252_GERMAN = "äöüß"
CP1252_SPANISH = "ñáéíóú¿¡"
CP1252_SYMBOLS = "€£©®™"
CP1252_MATH = "±×÷°"


# =============================================================================
# Helper Functions
# =============================================================================

def format_header(name, map_id=None, codepage=1252):
    """Format the IMG ID header section."""
    if map_id is None:
        map_id = random.randint(10000000, 99999999)
    return f"""[IMG ID]
Name={name}
ID={map_id}
CodePage={codepage}
Datum=WGS 84
[END-IMG ID]
"""


def format_poi(poi_type, label, lat, lon, end_level=None, data_fields=None):
    """Format a POI section with optional extra data fields."""
    lines = [
        "[POI]",
        f"Type=0x{poi_type:04X}",
        f"Label={label}",
        f"Data0=({lon},{lat})",  # Note: Polish Map format is (lon,lat)
    ]
    if data_fields:
        for key, value in data_fields.items():
            if key != "Data0":
                lines.append(f"{key}={value}")
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def format_polyline(line_type, label, coords, end_level=None, data_fields=None):
    """Format a POLYLINE section."""
    # coords is list of (lat, lon) tuples
    coord_str = ",".join(f"({lon},{lat})" for lat, lon in coords)
    lines = [
        "[POLYLINE]",
        f"Type=0x{line_type:04X}",
        f"Label={label}",
        f"Data0={coord_str}",
    ]
    if data_fields:
        for key, value in data_fields.items():
            if key != "Data0":
                lines.append(f"{key}={value}")
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def format_polygon(poly_type, label, coords, end_level=None, data_fields=None):
    """Format a POLYGON section."""
    # coords is list of (lat, lon) tuples, must be closed ring
    coord_str = ",".join(f"({lon},{lat})" for lat, lon in coords)
    lines = [
        "[POLYGON]",
        f"Type=0x{poly_type:04X}",
        f"Label={label}",
        f"Data0={coord_str}",
    ]
    if data_fields:
        for key, value in data_fields.items():
            if key != "Data0":
                lines.append(f"{key}={value}")
    if end_level is not None:
        lines.append(f"EndLevel={end_level}")
    lines.append("[END]")
    return "\n".join(lines)


def write_mp_file(filepath, content, line_ending='\n'):
    """Write a .mp file with specified line endings."""
    if line_ending == '\r\n':
        content = content.replace('\n', '\r\n')
    with open(filepath, 'w', encoding='cp1252', newline='') as f:
        f.write(content)


def generate_polyline_coords(point_count, start_lat=48.8566, start_lon=2.3522):
    """Generate a polyline as a list of connected coordinates."""
    coords = [(start_lat, start_lon)]
    for _ in range(point_count - 1):
        lat_offset = random.uniform(-0.001, 0.001)
        lon_offset = random.uniform(-0.001, 0.001)
        new_lat = round(coords[-1][0] + lat_offset, 6)
        new_lon = round(coords[-1][1] + lon_offset, 6)
        coords.append((new_lat, new_lon))
    return coords


def generate_polygon_coords(point_count, center_lat=48.8566, center_lon=2.3522):
    """Generate a closed polygon as a list of coordinates."""
    coords = []
    radius = 0.001
    for i in range(point_count - 1):
        angle = 2 * math.pi * i / (point_count - 1)
        lat = round(center_lat + radius * math.sin(angle), 6)
        lon = round(center_lon + radius * math.cos(angle), 6)
        coords.append((lat, lon))
    coords.append(coords[0])  # Close the polygon
    return coords


# =============================================================================
# Label Edge Cases (15 files)
# =============================================================================

def generate_label_edge_cases(output_dir):
    """Generate label edge case files."""
    files_created = []

    # 1. Empty Label
    content = format_header("Edge Empty Label")
    content += "\n" + format_poi(0x0100, "", 48.8566, 2.3522)
    content += "\n\n" + format_poi(0x0100, "Normal Label", 48.8600, 2.3600)
    write_mp_file(os.path.join(output_dir, "edge-empty-label.mp"), content)
    files_created.append("edge-empty-label.mp")

    # 2. Very Long Label (500+ characters)
    long_label = "A" * 500 + " Long Label Test"
    content = format_header("Edge Label Very Long")
    content += "\n" + format_poi(0x0100, long_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-very-long.mp"), content)
    files_created.append("edge-label-very-long.mp")

    # 3. Special Characters in Label
    special_label = "Test <>&\"' Special Chars"
    content = format_header("Edge Label Special Chars")
    content += "\n" + format_poi(0x0100, special_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-special-chars.mp"), content)
    files_created.append("edge-label-special-chars.mp")

    # 4. Unicode Accents (French/European)
    unicode_label = "Café René à l'église"
    content = format_header("Edge Label Unicode Accents")
    content += "\n" + format_poi(0x0100, unicode_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-unicode-accents.mp"), content)
    files_created.append("edge-label-unicode-accents.mp")

    # 5. Numbers Only Label
    content = format_header("Edge Label Numbers Only")
    content += "\n" + format_poi(0x0100, "12345", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-numbers-only.mp"), content)
    files_created.append("edge-label-numbers-only.mp")

    # 6. Label with Spaces
    content = format_header("Edge Label Spaces")
    content += "\n" + format_poi(0x0100, "  leading and trailing spaces  ", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-spaces.mp"), content)
    files_created.append("edge-label-spaces.mp")

    # 7. Label with Equals Sign
    content = format_header("Edge Label Equals Sign")
    content += "\n" + format_poi(0x0100, "Name=Value", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-equals-sign.mp"), content)
    files_created.append("edge-label-equals-sign.mp")

    # 8. Label with Brackets
    content = format_header("Edge Label Brackets")
    content += "\n" + format_poi(0x0100, "[Section] Label", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-brackets.mp"), content)
    files_created.append("edge-label-brackets.mp")

    # 9. Label with Semicolon
    content = format_header("Edge Label Semicolon")
    content += "\n" + format_poi(0x0100, "Part A; Part B", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-semicolon.mp"), content)
    files_created.append("edge-label-semicolon.mp")

    # 10. CP1252 Extended Characters
    cp1252_label = f"Test {CP1252_FRENCH}"
    content = format_header("Edge Label CP1252 Extended")
    content += "\n" + format_poi(0x0100, cp1252_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-cp1252-extended.mp"), content)
    files_created.append("edge-label-cp1252-extended.mp")

    # 11. Mixed Encoding Characters
    mixed_label = f"éèê {CP1252_SYMBOLS} àâä"
    content = format_header("Edge Label Mixed Encoding")
    content += "\n" + format_poi(0x0100, mixed_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-mixed-encoding.mp"), content)
    files_created.append("edge-label-mixed-encoding.mp")

    # 12. Whitespace Only Label
    content = format_header("Edge Label Whitespace Only")
    content += "\n" + format_poi(0x0100, "   ", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-whitespace-only.mp"), content)
    files_created.append("edge-label-whitespace-only.mp")

    # 13. Tab Characters in Label
    content = format_header("Edge Label Tab Chars")
    content += "\n" + format_poi(0x0100, "Label\twith\ttabs", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-tab-chars.mp"), content)
    files_created.append("edge-label-tab-chars.mp")

    # 14. German Characters
    german_label = f"Straße München {CP1252_GERMAN}"
    content = format_header("Edge Label German")
    content += "\n" + format_poi(0x0100, german_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-german.mp"), content)
    files_created.append("edge-label-german.mp")

    # 15. Spanish Characters
    spanish_label = f"España Ñoño {CP1252_SPANISH}"
    content = format_header("Edge Label Spanish")
    content += "\n" + format_poi(0x0100, spanish_label, 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-label-spanish.mp"), content)
    files_created.append("edge-label-spanish.mp")

    return files_created


# =============================================================================
# Coordinate Edge Cases (15 files)
# =============================================================================

def generate_coord_edge_cases(output_dir):
    """Generate coordinate edge case files."""
    files_created = []

    # 1. Maximum Latitude (North Pole)
    content = format_header("Edge Coords Max Lat")
    content += "\n" + format_poi(0x0100, "North Pole Area", 89.9999, 0.0)
    write_mp_file(os.path.join(output_dir, "edge-coords-max-lat.mp"), content)
    files_created.append("edge-coords-max-lat.mp")

    # 2. Minimum Latitude (South Pole)
    content = format_header("Edge Coords Min Lat")
    content += "\n" + format_poi(0x0100, "South Pole Area", -89.9999, 0.0)
    write_mp_file(os.path.join(output_dir, "edge-coords-min-lat.mp"), content)
    files_created.append("edge-coords-min-lat.mp")

    # 3. Maximum Longitude (Dateline East)
    content = format_header("Edge Coords Max Lon")
    content += "\n" + format_poi(0x0100, "Date Line East", 0.0, 179.9999)
    write_mp_file(os.path.join(output_dir, "edge-coords-max-lon.mp"), content)
    files_created.append("edge-coords-max-lon.mp")

    # 4. Minimum Longitude (Dateline West)
    content = format_header("Edge Coords Min Lon")
    content += "\n" + format_poi(0x0100, "Date Line West", 0.0, -179.9999)
    write_mp_file(os.path.join(output_dir, "edge-coords-min-lon.mp"), content)
    files_created.append("edge-coords-min-lon.mp")

    # 5. Antimeridian Crossing
    content = format_header("Edge Coords Antimeridian")
    coords = [(0.0, 179.9), (0.0, -179.9)]  # Crosses dateline
    content += "\n" + format_polyline(0x0001, "Antimeridian Line", coords)
    write_mp_file(os.path.join(output_dir, "edge-coords-antimeridian.mp"), content)
    files_created.append("edge-coords-antimeridian.mp")

    # 6. Extreme Precision (15 decimals stored as 6)
    content = format_header("Edge Coords Extreme Precision")
    content += "\n" + format_poi(0x0100, "Extreme Precision", 48.856614012345, 2.352221987654)
    write_mp_file(os.path.join(output_dir, "edge-coords-extreme-precision.mp"), content)
    files_created.append("edge-coords-extreme-precision.mp")

    # 7. GPS Precision (6 decimals exactly)
    content = format_header("Edge Coords GPS Precision")
    content += "\n" + format_poi(0x0100, "GPS Precision", 48.856614, 2.352222)
    write_mp_file(os.path.join(output_dir, "edge-coords-gps-precision.mp"), content)
    files_created.append("edge-coords-gps-precision.mp")

    # 8. Near Zero Coordinates
    content = format_header("Edge Coords Near Zero")
    content += "\n" + format_poi(0x0100, "Null Island Area", 0.000001, 0.000001)
    write_mp_file(os.path.join(output_dir, "edge-coords-near-zero.mp"), content)
    files_created.append("edge-coords-near-zero.mp")

    # 9. Negative Coordinates (Southwest)
    content = format_header("Edge Coords Negative")
    content += "\n" + format_poi(0x0100, "Southwest Point", -45.5, -120.5)
    write_mp_file(os.path.join(output_dir, "edge-coords-negative.mp"), content)
    files_created.append("edge-coords-negative.mp")

    # 10. Europe Bounds
    content = format_header("Edge Coords Europe Bounds")
    content += "\n" + format_poi(0x0100, "Europe North", 71.0, 25.0)
    content += "\n\n" + format_poi(0x0100, "Europe South", 35.0, 15.0)
    content += "\n\n" + format_poi(0x0100, "Europe West", 50.0, -10.0)
    content += "\n\n" + format_poi(0x0100, "Europe East", 55.0, 40.0)
    write_mp_file(os.path.join(output_dir, "edge-coords-europe-bounds.mp"), content)
    files_created.append("edge-coords-europe-bounds.mp")

    # 11. France Extreme Points
    content = format_header("Edge Coords France Extreme")
    content += "\n" + format_poi(0x0100, "Dunkerque", 51.034, 2.377)
    content += "\n\n" + format_poi(0x0100, "Cerbere", 42.443, 3.169)
    content += "\n\n" + format_poi(0x0100, "Pointe de Corsen", 48.416, -4.765)
    content += "\n\n" + format_poi(0x0100, "Lauterbourg", 48.973, 8.230)
    write_mp_file(os.path.join(output_dir, "edge-coords-france-extreme.mp"), content)
    files_created.append("edge-coords-france-extreme.mp")

    # 12. Integer Coordinates
    content = format_header("Edge Coords Integer")
    content += "\n" + format_poi(0x0100, "Integer Coords", 48.0, 2.0)
    write_mp_file(os.path.join(output_dir, "edge-coords-integer.mp"), content)
    files_created.append("edge-coords-integer.mp")

    # 13. Very Close Points
    content = format_header("Edge Coords Very Close")
    content += "\n" + format_poi(0x0100, "Point A", 48.856614, 2.352222)
    content += "\n\n" + format_poi(0x0100, "Point B", 48.856615, 2.352223)
    write_mp_file(os.path.join(output_dir, "edge-coords-very-close.mp"), content)
    files_created.append("edge-coords-very-close.mp")

    # 14. Duplicate Points
    content = format_header("Edge Coords Duplicate Points")
    content += "\n" + format_poi(0x0100, "Point 1", 48.8566, 2.3522)
    content += "\n\n" + format_poi(0x0100, "Point 2", 48.8566, 2.3522)
    content += "\n\n" + format_poi(0x0100, "Point 3", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-coords-duplicate-points.mp"), content)
    files_created.append("edge-coords-duplicate-points.mp")

    # 15. World Corners
    content = format_header("Edge Coords World Corners")
    content += "\n" + format_poi(0x0100, "NW Corner", 85.0, -170.0)
    content += "\n\n" + format_poi(0x0100, "NE Corner", 85.0, 170.0)
    content += "\n\n" + format_poi(0x0100, "SW Corner", -85.0, -170.0)
    content += "\n\n" + format_poi(0x0100, "SE Corner", -85.0, 170.0)
    write_mp_file(os.path.join(output_dir, "edge-coords-world-corners.mp"), content)
    files_created.append("edge-coords-world-corners.mp")

    return files_created


# =============================================================================
# Data Fields Edge Cases (10 files)
# =============================================================================

def generate_data_field_edge_cases(output_dir):
    """Generate data field edge case files."""
    files_created = []

    # 1. All Data Fields (Data0 to Data10)
    content = format_header("Edge Data All Fields")
    data_fields = {}
    for i in range(1, 11):  # Data1 to Data10 (Data0 is coordinates)
        coord_offset = i * 0.001
        data_fields[f"Data{i}"] = f"({2.3522 + coord_offset},{48.8566 + coord_offset})"
    content += "\n" + format_poi(0x0100, "All Data Fields", 48.8566, 2.3522, data_fields=data_fields)
    write_mp_file(os.path.join(output_dir, "edge-data-all-fields.mp"), content)
    files_created.append("edge-data-all-fields.mp")

    # 2. Sparse Data Fields (Data0, Data5, Data10 only)
    content = format_header("Edge Data Sparse")
    data_fields = {
        "Data5": "(2.355,48.859)",
        "Data10": "(2.360,48.862)"
    }
    content += "\n" + format_poi(0x0100, "Sparse Data", 48.8566, 2.3522, data_fields=data_fields)
    write_mp_file(os.path.join(output_dir, "edge-data-sparse.mp"), content)
    files_created.append("edge-data-sparse.mp")

    # 3. Out of Order Data Fields (Data5 before Data1)
    content = format_header("Edge Data Out of Order")
    # Write manually to control order
    lines = [
        "[POI]",
        "Type=0x0100",
        "Label=Out of Order Data",
        "Data5=(2.355,48.859)",
        "Data0=(2.3522,48.8566)",
        "Data1=(2.353,48.857)",
        "[END]"
    ]
    content += "\n" + "\n".join(lines)
    write_mp_file(os.path.join(output_dir, "edge-data-out-of-order.mp"), content)
    files_created.append("edge-data-out-of-order.mp")

    # 4. Maximum Integer Values
    content = format_header("Edge Data Max Value")
    # Type uses max hex value
    lines = [
        "[POI]",
        "Type=0x7FFF",
        "Label=Max Type Value",
        "Data0=(2.3522,48.8566)",
        "EndLevel=4",
        "[END]"
    ]
    content += "\n" + "\n".join(lines)
    write_mp_file(os.path.join(output_dir, "edge-data-max-value.mp"), content)
    files_created.append("edge-data-max-value.mp")

    # 5. Zero Values
    content = format_header("Edge Data Zero")
    lines = [
        "[POI]",
        "Type=0x0000",
        "Label=Zero Type",
        "Data0=(0.0,0.0)",
        "EndLevel=0",
        "[END]"
    ]
    content += "\n" + "\n".join(lines)
    write_mp_file(os.path.join(output_dir, "edge-data-zero.mp"), content)
    files_created.append("edge-data-zero.mp")

    # 6. Polyline with Many Coordinates (1000 points)
    content = format_header("Edge Data Many Coords")
    coords = generate_polyline_coords(1000, 48.8566, 2.3522)
    content += "\n" + format_polyline(0x0001, "1000 Point Polyline", coords)
    write_mp_file(os.path.join(output_dir, "edge-data-many-coords.mp"), content)
    files_created.append("edge-data-many-coords.mp")

    # 7. Polygon with Many Vertices (500 points)
    content = format_header("Edge Data Many Vertices")
    coords = generate_polygon_coords(500, 48.8566, 2.3522)
    content += "\n" + format_polygon(0x0001, "500 Vertex Polygon", coords)
    write_mp_file(os.path.join(output_dir, "edge-data-many-vertices.mp"), content)
    files_created.append("edge-data-many-vertices.mp")

    # 8. Multiple Data lines for same index (should use last)
    content = format_header("Edge Data Duplicate Index")
    lines = [
        "[POI]",
        "Type=0x0100",
        "Label=Duplicate Data Index",
        "Data0=(2.0,48.0)",
        "Data0=(2.5,48.5)",  # Duplicate - should override
        "[END]"
    ]
    content += "\n" + "\n".join(lines)
    write_mp_file(os.path.join(output_dir, "edge-data-duplicate-index.mp"), content)
    files_created.append("edge-data-duplicate-index.mp")

    # 9. EndLevel variations
    content = format_header("Edge Data EndLevel Variations")
    content += "\n" + format_poi(0x0100, "Level 0", 48.8566, 2.3522, end_level=0)
    content += "\n\n" + format_poi(0x0100, "Level 1", 48.8600, 2.3600, end_level=1)
    content += "\n\n" + format_poi(0x0100, "Level 4", 48.8700, 2.3700, end_level=4)
    write_mp_file(os.path.join(output_dir, "edge-data-endlevel-variations.mp"), content)
    files_created.append("edge-data-endlevel-variations.mp")

    # 10. Mixed geometry types with data fields
    content = format_header("Edge Data Mixed Types")
    content += "\n" + format_poi(0x0100, "POI with Data", 48.8566, 2.3522,
                                 data_fields={"Data1": "(2.353,48.857)"})
    coords = generate_polyline_coords(5, 48.86, 2.36)
    content += "\n\n" + format_polyline(0x0001, "Line with Data", coords,
                                        data_fields={"Data1": "(2.37,48.87),(2.38,48.88)"})
    coords = generate_polygon_coords(6, 48.87, 2.37)
    content += "\n\n" + format_polygon(0x0001, "Poly with Data", coords,
                                       data_fields={"Data1": "(2.38,48.88),(2.39,48.89),(2.40,48.90),(2.38,48.88)"})
    write_mp_file(os.path.join(output_dir, "edge-data-mixed-types.mp"), content)
    files_created.append("edge-data-mixed-types.mp")

    return files_created


# =============================================================================
# Feature Count Edge Cases (10 files)
# =============================================================================

def generate_feature_count_edge_cases(output_dir):
    """Generate feature count edge case files."""
    files_created = []

    # 1. Single Feature
    content = format_header("Edge Features Single")
    content += "\n" + format_poi(0x0100, "Only One", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-features-single.mp"), content)
    files_created.append("edge-features-single.mp")

    # 2. 100 Features
    content = format_header("Edge Features 100")
    for i in range(100):
        lat = 48.0 + (i // 10) * 0.01
        lon = 2.0 + (i % 10) * 0.01
        content += "\n" + format_poi(0x0100, f"POI {i+1}", lat, lon) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-100.mp"), content)
    files_created.append("edge-features-100.mp")

    # 3. 1000 Features
    content = format_header("Edge Features 1000")
    for i in range(1000):
        lat = 48.0 + (i // 100) * 0.001
        lon = 2.0 + (i % 100) * 0.001
        content += "\n" + format_poi(random.choice(POI_TYPES), f"POI {i+1}", lat, lon) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-1000.mp"), content)
    files_created.append("edge-features-1000.mp")

    # 4. Mixed Types
    content = format_header("Edge Features Mixed Types")
    # 10 POIs
    for i in range(10):
        content += "\n" + format_poi(0x0100, f"POI {i+1}", 48.8 + i*0.01, 2.3 + i*0.01) + "\n"
    # 10 Polylines
    for i in range(10):
        coords = generate_polyline_coords(5, 48.9 + i*0.01, 2.4 + i*0.01)
        content += "\n" + format_polyline(0x0001, f"Line {i+1}", coords) + "\n"
    # 10 Polygons
    for i in range(10):
        coords = generate_polygon_coords(6, 49.0 + i*0.01, 2.5 + i*0.01)
        content += "\n" + format_polygon(0x0001, f"Poly {i+1}", coords) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-mixed-types.mp"), content)
    files_created.append("edge-features-mixed-types.mp")

    # 5. Same Location (all features at same point)
    content = format_header("Edge Features Same Location")
    for i in range(10):
        content += "\n" + format_poi(0x0100, f"Same Loc {i+1}", 48.8566, 2.3522) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-same-location.mp"), content)
    files_created.append("edge-features-same-location.mp")

    # 6. Ordered Features (geographic order)
    content = format_header("Edge Features Ordered")
    for i in range(20):
        lat = 48.0 + i * 0.05  # North progression
        lon = 2.0 + i * 0.02   # East progression
        content += "\n" + format_poi(0x0100, f"Ordered {i+1}", lat, lon) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-ordered.mp"), content)
    files_created.append("edge-features-ordered.mp")

    # 7. Random Order Sections
    content = format_header("Edge Features Random Order")
    # Deliberately interleave section types
    content += "\n" + format_poi(0x0100, "POI 1", 48.8, 2.3)
    coords = generate_polyline_coords(3, 48.85, 2.35)
    content += "\n\n" + format_polyline(0x0001, "Line 1", coords)
    content += "\n\n" + format_poi(0x0100, "POI 2", 48.81, 2.31)
    coords = generate_polygon_coords(5, 48.9, 2.4)
    content += "\n\n" + format_polygon(0x0001, "Poly 1", coords)
    coords = generate_polyline_coords(3, 48.86, 2.36)
    content += "\n\n" + format_polyline(0x0001, "Line 2", coords)
    content += "\n\n" + format_poi(0x0100, "POI 3", 48.82, 2.32)
    write_mp_file(os.path.join(output_dir, "edge-features-random-order.mp"), content)
    files_created.append("edge-features-random-order.mp")

    # 8. Polyline with Many Points (500)
    content = format_header("Edge Features Polyline Many Points")
    coords = generate_polyline_coords(500, 48.8, 2.3)
    content += "\n" + format_polyline(0x0001, "500 Point Line", coords)
    write_mp_file(os.path.join(output_dir, "edge-features-polyline-many-points.mp"), content)
    files_created.append("edge-features-polyline-many-points.mp")

    # 9. Polygon Complex (100 vertices)
    content = format_header("Edge Features Polygon Complex")
    coords = generate_polygon_coords(100, 48.85, 2.35)
    content += "\n" + format_polygon(0x0001, "100 Vertex Polygon", coords)
    write_mp_file(os.path.join(output_dir, "edge-features-polygon-complex.mp"), content)
    files_created.append("edge-features-polygon-complex.mp")

    # 10. All Types Each (one of each type from each category)
    content = format_header("Edge Features All Types Each")
    for poi_type in POI_TYPES[:5]:
        content += "\n" + format_poi(poi_type, f"Type 0x{poi_type:04X}", 48.8 + random.random()*0.1, 2.3 + random.random()*0.1) + "\n"
    for line_type in POLYLINE_TYPES[:5]:
        coords = generate_polyline_coords(4, 48.9 + random.random()*0.1, 2.4 + random.random()*0.1)
        content += "\n" + format_polyline(line_type, f"Type 0x{line_type:04X}", coords) + "\n"
    for poly_type in POLYGON_TYPES[:5]:
        coords = generate_polygon_coords(5, 49.0 + random.random()*0.1, 2.5 + random.random()*0.1)
        content += "\n" + format_polygon(poly_type, f"Type 0x{poly_type:04X}", coords) + "\n"
    write_mp_file(os.path.join(output_dir, "edge-features-all-types-each.mp"), content)
    files_created.append("edge-features-all-types-each.mp")

    return files_created


# =============================================================================
# Encoding Edge Cases (10 files)
# =============================================================================

def generate_encoding_edge_cases(output_dir):
    """Generate encoding edge case files."""
    files_created = []

    # 1. CP1252 All Valid Characters
    content = format_header("Edge Encoding CP1252 All")
    content += "\n" + format_poi(0x0100, f"All: {CP1252_CHARS}", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-all.mp"), content)
    files_created.append("edge-encoding-cp1252-all.mp")

    # 2. CP1252 French Characters
    content = format_header("Edge Encoding CP1252 French")
    content += "\n" + format_poi(0x0100, f"French: {CP1252_FRENCH}", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-french.mp"), content)
    files_created.append("edge-encoding-cp1252-french.mp")

    # 3. CP1252 German Characters
    content = format_header("Edge Encoding CP1252 German")
    content += "\n" + format_poi(0x0100, f"German: {CP1252_GERMAN}", 52.52, 13.405)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-german.mp"), content)
    files_created.append("edge-encoding-cp1252-german.mp")

    # 4. CP1252 Spanish Characters
    content = format_header("Edge Encoding CP1252 Spanish")
    content += "\n" + format_poi(0x0100, f"Spanish: {CP1252_SPANISH}", 40.4168, -3.7038)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-spanish.mp"), content)
    files_created.append("edge-encoding-cp1252-spanish.mp")

    # 5. CP1252 Symbols
    content = format_header("Edge Encoding CP1252 Symbols")
    content += "\n" + format_poi(0x0100, f"Symbols: {CP1252_SYMBOLS}", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-symbols.mp"), content)
    files_created.append("edge-encoding-cp1252-symbols.mp")

    # 6. CP1252 Math Characters
    content = format_header("Edge Encoding CP1252 Math")
    content += "\n" + format_poi(0x0100, f"Math: {CP1252_MATH}", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-cp1252-math.mp"), content)
    files_created.append("edge-encoding-cp1252-math.mp")

    # 7. Windows Line Endings (CRLF)
    content = format_header("Edge Encoding CRLF")
    content += "\n" + format_poi(0x0100, "CRLF Line Ending", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-crlf.mp"), content, line_ending='\r\n')
    files_created.append("edge-encoding-crlf.mp")

    # 8. Unix Line Endings (LF)
    content = format_header("Edge Encoding LF")
    content += "\n" + format_poi(0x0100, "LF Line Ending", 48.8566, 2.3522)
    write_mp_file(os.path.join(output_dir, "edge-encoding-lf.mp"), content, line_ending='\n')
    files_created.append("edge-encoding-lf.mp")

    # 9. Mixed Line Endings (CRLF and LF in same file)
    content = format_header("Edge Encoding Mixed EOL")
    content += "\n" + format_poi(0x0100, "First POI", 48.8566, 2.3522)
    # Manually insert mixed line endings
    content = content.replace("\n[POI]", "\r\n[POI]")  # CRLF before POI
    content += "\n\n" + format_poi(0x0100, "Second POI", 48.87, 2.36)
    write_mp_file(os.path.join(output_dir, "edge-encoding-mixed-eol.mp"), content)
    files_created.append("edge-encoding-mixed-eol.mp")

    # 10. Extra Whitespace (blank lines, trailing spaces)
    content = format_header("Edge Encoding Whitespace")
    content += "\n\n\n"  # Extra blank lines
    content += format_poi(0x0100, "Whitespace Test   ", 48.8566, 2.3522)  # Trailing spaces
    content += "\n\n\n"  # More blank lines
    content += format_poi(0x0100, "  Leading Spaces", 48.87, 2.36)  # Leading spaces in label
    content += "\n\n"
    write_mp_file(os.path.join(output_dir, "edge-encoding-whitespace.mp"), content)
    files_created.append("edge-encoding-whitespace.mp")

    return files_created


# =============================================================================
# Main Generation Function
# =============================================================================

def generate_corpus(output_dir):
    """Generate the complete edge-cases corpus."""
    print(f"Generating edge-cases corpus in: {output_dir}")
    print()

    os.makedirs(output_dir, exist_ok=True)

    stats = {
        'label': 0,
        'coords': 0,
        'data': 0,
        'features': 0,
        'encoding': 0,
    }

    # Set random seed for reproducibility (Story 3.4 implementation date: 2026-02-03)
    # Using fixed seed ensures corpus is identical across regenerations
    random.seed(20260203)

    # Generate Label Edge Cases
    print("Generating label edge cases...", end=" ")
    files = generate_label_edge_cases(output_dir)
    stats['label'] = len(files)
    print(f"{len(files)} files")

    # Generate Coordinate Edge Cases
    print("Generating coordinate edge cases...", end=" ")
    files = generate_coord_edge_cases(output_dir)
    stats['coords'] = len(files)
    print(f"{len(files)} files")

    # Generate Data Field Edge Cases
    print("Generating data field edge cases...", end=" ")
    files = generate_data_field_edge_cases(output_dir)
    stats['data'] = len(files)
    print(f"{len(files)} files")

    # Generate Feature Count Edge Cases
    print("Generating feature count edge cases...", end=" ")
    files = generate_feature_count_edge_cases(output_dir)
    stats['features'] = len(files)
    print(f"{len(files)} files")

    # Generate Encoding Edge Cases
    print("Generating encoding edge cases...", end=" ")
    files = generate_encoding_edge_cases(output_dir)
    stats['encoding'] = len(files)
    print(f"{len(files)} files")

    # Summary
    total_files = sum(stats.values())

    print()
    print("=" * 50)
    print("Edge Cases Corpus Generation Complete")
    print("=" * 50)
    print(f"Total files: {total_files}")
    print()
    print("Breakdown:")
    print(f"  Label edge cases:    {stats['label']:3d} files")
    print(f"  Coordinate edge cases: {stats['coords']:3d} files")
    print(f"  Data field edge cases: {stats['data']:3d} files")
    print(f"  Feature count edge cases: {stats['features']:3d} files")
    print(f"  Encoding edge cases:   {stats['encoding']:3d} files")
    print()
    print(f"Output directory: {output_dir}")

    return total_files


def main():
    """Main entry point."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    default_output = os.path.join(script_dir, '..', 'test', 'data', 'edge-cases')

    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = default_output

    output_dir = os.path.abspath(output_dir)

    parent_dir = os.path.dirname(output_dir)
    if not os.path.isdir(parent_dir):
        print(f"ERROR: Parent directory does not exist: {parent_dir}")
        return 1

    try:
        total_files = generate_corpus(output_dir)
    except OSError as e:
        print(f"\nERROR: Failed to generate corpus: {e}")
        return 1
    except Exception as e:
        print(f"\nERROR: Unexpected error during generation: {e}")
        import traceback
        traceback.print_exc()
        return 1

    if total_files >= 50:
        print(f"\nCorpus meets AC1 requirement (50-100 files): {total_files} files generated")
        return 0
    else:
        print(f"\nWarning: Only {total_files} files generated, expected >= 50")
        return 1


if __name__ == '__main__':
    sys.exit(main())
