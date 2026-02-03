#!/usr/bin/env python3
"""
Story 3.4: Generate Additional Error-Recovery Test Corpus

Extends the existing 19 error-recovery files with 31+ additional malformed files:
- Header Errors (5 files)
- Section Errors (8 files)
- Geometry Errors (8 files)
- Attribute Errors (5 files)
- Mixed Errors (5 files)

Usage:
    python3 generate_error_recovery_corpus.py [output_dir]

Default output: test/data/error-recovery/
"""

import os
import sys
import random


# =============================================================================
# Helper Functions
# =============================================================================

def write_mp_file(filepath, content):
    """Write a .mp file."""
    with open(filepath, 'w', encoding='cp1252', newline='') as f:
        f.write(content)


def format_valid_header(name, map_id=12345678):
    """Format a valid IMG ID header section."""
    return f"""[IMG ID]
Name={name}
ID={map_id}
CodePage=1252
Datum=WGS 84
[END-IMG ID]
"""


def format_valid_poi(label, lat, lon, poi_type=0x0100):
    """Format a valid POI section."""
    return f"""[POI]
Type=0x{poi_type:04X}
Label={label}
Data0=({lon},{lat})
[END]
"""


def format_valid_polyline(label, coords):
    """Format a valid POLYLINE section."""
    coord_str = ",".join(f"({lon},{lat})" for lat, lon in coords)
    return f"""[POLYLINE]
Type=0x0001
Label={label}
Data0={coord_str}
[END]
"""


def format_valid_polygon(label, coords):
    """Format a valid POLYGON section."""
    coord_str = ",".join(f"({lon},{lat})" for lat, lon in coords)
    return f"""[POLYGON]
Type=0x0001
Label={label}
Data0={coord_str}
[END]
"""


# =============================================================================
# Header Errors (5 files)
# =============================================================================

def generate_header_errors(output_dir):
    """Generate header error files."""
    files_created = []

    # 1. Duplicate Header - Two [IMG ID] sections
    content = """[IMG ID]
Name=First Header
ID=11111111
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[IMG ID]
Name=Duplicate Header
ID=22222222
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x0100
Label=Test POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-header-duplicate.mp"), content)
    files_created.append("err-header-duplicate.mp")

    # 2. Malformed ID (non-numeric)
    content = """[IMG ID]
Name=Malformed ID
ID=NotANumber
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x0100
Label=Test POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-header-malformed-id.mp"), content)
    files_created.append("err-header-malformed-id.mp")

    # 3. Missing Name in header
    content = """[IMG ID]
ID=12345678
CodePage=1252
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x0100
Label=Test POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-header-missing-name.mp"), content)
    files_created.append("err-header-missing-name.mp")

    # 4. Invalid Codepage
    content = """[IMG ID]
Name=Invalid Codepage
ID=12345678
CodePage=99999
Datum=WGS 84
[END-IMG ID]

[POI]
Type=0x0100
Label=Test POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-header-invalid-codepage.mp"), content)
    files_created.append("err-header-invalid-codepage.mp")

    # 5. Empty header values
    content = """[IMG ID]
Name=
ID=
CodePage=
Datum=
[END-IMG ID]

[POI]
Type=0x0100
Label=Test POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-header-empty-values.mp"), content)
    files_created.append("err-header-empty-values.mp")

    return files_created


# =============================================================================
# Section Errors (8 files)
# =============================================================================

def generate_section_errors(output_dir):
    """Generate section error files."""
    files_created = []

    # 1. Unknown section type
    content = format_valid_header("Unknown Section")
    content += """
[UNKNOWN]
Type=0x0100
Label=Unknown Type
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI After Unknown
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-unknown-type.mp"), content)
    files_created.append("err-section-unknown-type.mp")

    # 2. Duplicate POI (same feature twice)
    content = format_valid_header("Duplicate POI")
    content += """
[POI]
Type=0x0100
Label=Same POI
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Same POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-duplicate-poi.mp"), content)
    files_created.append("err-section-duplicate-poi.mp")

    # 3. Incomplete section (no [END])
    content = format_valid_header("Incomplete Section")
    content += """
[POI]
Type=0x0100
Label=No END tag
Data0=(2.3522,48.8566)

[POI]
Type=0x0100
Label=Valid POI After
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-incomplete.mp"), content)
    files_created.append("err-section-incomplete.mp")

    # 4. [END] only (without preceding section)
    content = format_valid_header("END Only")
    content += """
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-end-only.mp"), content)
    files_created.append("err-section-end-only.mp")

    # 5. Deeply nested sections (3+ levels)
    content = format_valid_header("Nested Deep")
    content += """
[POI]
Type=0x0100
Label=Outer POI
Data0=(2.3522,48.8566)
[POI]
Type=0x0100
Label=Nested POI
Data0=(2.35,48.85)
[POI]
Type=0x0100
Label=Deep Nested
Data0=(2.34,48.84)
[END]
[END]
[END]

[POI]
Type=0x0100
Label=Valid After Nested
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-nested-deep.mp"), content)
    files_created.append("err-section-nested-deep.mp")

    # 6. Empty section ([POI] immediately followed by [END])
    content = format_valid_header("Empty Section")
    content += """
[POI]
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-empty.mp"), content)
    files_created.append("err-section-empty.mp")

    # 7. Wrong END tag
    content = format_valid_header("Wrong END")
    content += """
[POLYLINE]
Type=0x0001
Label=Polyline
Data0=(2.3522,48.8566),(2.36,48.87)
[END-POI]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.37,48.88)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-wrong-end.mp"), content)
    files_created.append("err-section-wrong-end.mp")

    # 8. Lowercase section names
    content = format_valid_header("Case Mismatch")
    content += """
[poi]
Type=0x0100
Label=Lowercase POI
Data0=(2.3522,48.8566)
[end]

[POI]
Type=0x0100
Label=Uppercase POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-section-case-mismatch.mp"), content)
    files_created.append("err-section-case-mismatch.mp")

    return files_created


# =============================================================================
# Geometry Errors (8 files)
# =============================================================================

def generate_geometry_errors(output_dir):
    """Generate geometry error files."""
    files_created = []

    # 1. Latitude over 90
    content = format_valid_header("Lat Over 90")
    content += """
[POI]
Type=0x0100
Label=Invalid Lat
Data0=(2.3522,91.0)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-lat-over-90.mp"), content)
    files_created.append("err-geom-lat-over-90.mp")

    # 2. Longitude over 180
    content = format_valid_header("Lon Over 180")
    content += """
[POI]
Type=0x0100
Label=Invalid Lon
Data0=(181.0,48.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-lon-over-180.mp"), content)
    files_created.append("err-geom-lon-over-180.mp")

    # 3. NaN coordinates
    content = format_valid_header("NaN Coords")
    content += """
[POI]
Type=0x0100
Label=NaN Coords
Data0=(NaN,NaN)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-nan-coords.mp"), content)
    files_created.append("err-geom-nan-coords.mp")

    # 4. Infinity coordinates
    content = format_valid_header("Inf Coords")
    content += """
[POI]
Type=0x0100
Label=Inf Coords
Data0=(Inf,-Inf)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-inf-coords.mp"), content)
    files_created.append("err-geom-inf-coords.mp")

    # 5. Empty Data0
    content = format_valid_header("Empty Data0")
    content += """
[POI]
Type=0x0100
Label=Empty Data
Data0=
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-empty-data0.mp"), content)
    files_created.append("err-geom-empty-data0.mp")

    # 6. Single parenthesis (missing closing)
    content = format_valid_header("Single Parens")
    content += """
[POI]
Type=0x0100
Label=Missing Close
Data0=(2.3522,48.8566
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-single-parens.mp"), content)
    files_created.append("err-geom-single-parens.mp")

    # 7. No comma in coordinates
    content = format_valid_header("No Comma")
    content += """
[POI]
Type=0x0100
Label=No Comma
Data0=(2.352248.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-no-comma.mp"), content)
    files_created.append("err-geom-no-comma.mp")

    # 8. Extra values in coordinate tuple
    content = format_valid_header("Extra Values")
    content += """
[POI]
Type=0x0100
Label=Three Values
Data0=(2.3522,48.8566,100)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-geom-extra-values.mp"), content)
    files_created.append("err-geom-extra-values.mp")

    return files_created


# =============================================================================
# Attribute Errors (5 files)
# =============================================================================

def generate_attribute_errors(output_dir):
    """Generate attribute error files."""
    files_created = []

    # 1. Type invalid (not hex)
    content = format_valid_header("Type Invalid")
    content += """
[POI]
Type=NotHex
Label=Invalid Type
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-attr-type-invalid.mp"), content)
    files_created.append("err-attr-type-invalid.mp")

    # 2. Type overflow
    content = format_valid_header("Type Overflow")
    content += """
[POI]
Type=0xFFFFFFFF
Label=Overflow Type
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-attr-type-overflow.mp"), content)
    files_created.append("err-attr-type-overflow.mp")

    # 3. EndLevel negative
    content = format_valid_header("EndLevel Negative")
    content += """
[POI]
Type=0x0100
Label=Negative Level
Data0=(2.3522,48.8566)
EndLevel=-1
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-attr-endlevel-negative.mp"), content)
    files_created.append("err-attr-endlevel-negative.mp")

    # 4. Levels invalid
    content = format_valid_header("Levels Invalid")
    content += """
[POI]
Type=0x0100
Label=Invalid Levels
Data0=(2.3522,48.8566)
Levels=abc
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-attr-levels-invalid.mp"), content)
    files_created.append("err-attr-levels-invalid.mp")

    # 5. Duplicate key in same section
    content = format_valid_header("Duplicate Key")
    content += """
[POI]
Type=0x0100
Type=0x0200
Label=Duplicate Type
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Valid POI
Data0=(2.36,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-attr-duplicate-key.mp"), content)
    files_created.append("err-attr-duplicate-key.mp")

    return files_created


# =============================================================================
# Mixed Errors (5 files)
# =============================================================================

def generate_mixed_errors(output_dir):
    """Generate mixed error files."""
    files_created = []

    # 1. Partial valid (50% valid, 50% invalid)
    content = format_valid_header("Partial Valid")
    content += """
[POI]
Type=0x0100
Label=Valid POI 1
Data0=(2.3522,48.8566)
[END]

[POI]
Type=NotValid
Label=Invalid POI
Data0=(invalid)
[END]

[POI]
Type=0x0100
Label=Valid POI 2
Data0=(2.36,48.87)
[END]

[UNKNOWN]
Invalid=Content
[END]

[POI]
Type=0x0100
Label=Valid POI 3
Data0=(2.37,48.88)
[END]

[POI]
Type=0x0100
Label=Invalid Coords
Data0=(999,999)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-mixed-partial-valid.mp"), content)
    files_created.append("err-mixed-partial-valid.mp")

    # 2. Progressive errors (errors become more severe)
    content = format_valid_header("Progressive Errors")
    content += """
[POI]
Type=0x0100
Label=Valid Start
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=
Data0=(2.35,48.85)
[END]

[POI]
Type=Invalid
Label=Bad Type
Data0=(2.36,48.87)
[END]

[UNKNOWN]
Random=Data
[END]

[POI]
Type=0x0100
Label=After Errors
Data0=(2.37,48.88)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-mixed-progressive.mp"), content)
    files_created.append("err-mixed-progressive.mp")

    # 3. Recoverable chain (multiple recoverable errors in sequence)
    content = format_valid_header("Recoverable Chain")
    content += """
[POI]
Type=0x0100
Label=POI with extra spaces
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=
Data0=(2.35,48.85)
[END]

[POI]
Type=0x0100
Label=Invalid coords ignored
Data0=(999,999)
[END]

[POI]
Type=0xFFFF
Label=Unknown type code
Data0=(2.36,48.86)
[END]

[POI]
Type=0x0100
Label=Final Valid
Data0=(2.37,48.87)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-mixed-recoverable-chain.mp"), content)
    files_created.append("err-mixed-recoverable-chain.mp")

    # 4. Critical then valid (critical error followed by valid content)
    content = """[IMG ID]
Name=Critical Then Valid
ID=
CodePage=Invalid
[END-IMG ID]

[POI]
Type=0x0100
Label=Should Not Parse
Data0=(2.3522,48.8566)
[END]
"""
    write_mp_file(os.path.join(output_dir, "err-mixed-critical-then-valid.mp"), content)
    files_created.append("err-mixed-critical-then-valid.mp")

    # 5. Valid then critical (valid content then critical error)
    content = format_valid_header("Valid Then Critical")
    content += """
[POI]
Type=0x0100
Label=Valid Before Critical
Data0=(2.3522,48.8566)
[END]

[POI]
Type=0x0100
Label=Another Valid
Data0=(2.36,48.87)
[END]
"""
    # Add binary garbage at the end to simulate file corruption
    write_mp_file(os.path.join(output_dir, "err-mixed-valid-then-critical.mp"), content)
    # Append some binary data
    with open(os.path.join(output_dir, "err-mixed-valid-then-critical.mp"), 'ab') as f:
        f.write(b'\x00\xFF\xFE\x00\x01\x02\x03\x04\x05')
    files_created.append("err-mixed-valid-then-critical.mp")

    return files_created


# =============================================================================
# Main Generation Function
# =============================================================================

def generate_corpus(output_dir):
    """Generate the additional error-recovery corpus."""
    print(f"Generating additional error-recovery corpus in: {output_dir}")
    print()

    os.makedirs(output_dir, exist_ok=True)

    stats = {
        'header': 0,
        'section': 0,
        'geometry': 0,
        'attribute': 0,
        'mixed': 0,
    }

    # Generate Header Errors
    print("Generating header error files...", end=" ")
    files = generate_header_errors(output_dir)
    stats['header'] = len(files)
    print(f"{len(files)} files")

    # Generate Section Errors
    print("Generating section error files...", end=" ")
    files = generate_section_errors(output_dir)
    stats['section'] = len(files)
    print(f"{len(files)} files")

    # Generate Geometry Errors
    print("Generating geometry error files...", end=" ")
    files = generate_geometry_errors(output_dir)
    stats['geometry'] = len(files)
    print(f"{len(files)} files")

    # Generate Attribute Errors
    print("Generating attribute error files...", end=" ")
    files = generate_attribute_errors(output_dir)
    stats['attribute'] = len(files)
    print(f"{len(files)} files")

    # Generate Mixed Errors
    print("Generating mixed error files...", end=" ")
    files = generate_mixed_errors(output_dir)
    stats['mixed'] = len(files)
    print(f"{len(files)} files")

    # Summary
    total_new = sum(stats.values())

    print()
    print("=" * 50)
    print("Error Recovery Corpus Extension Complete")
    print("=" * 50)
    print(f"New files generated: {total_new}")
    print()
    print("Breakdown:")
    print(f"  Header errors:    {stats['header']:3d} files")
    print(f"  Section errors:   {stats['section']:3d} files")
    print(f"  Geometry errors:  {stats['geometry']:3d} files")
    print(f"  Attribute errors: {stats['attribute']:3d} files")
    print(f"  Mixed errors:     {stats['mixed']:3d} files")
    print()
    print(f"Output directory: {output_dir}")

    return total_new


def main():
    """Main entry point."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    default_output = os.path.join(script_dir, '..', 'test', 'data', 'error-recovery')

    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = default_output

    output_dir = os.path.abspath(output_dir)

    if not os.path.isdir(output_dir):
        print(f"ERROR: Directory does not exist: {output_dir}")
        print("Please create the directory first or specify a valid path.")
        return 1

    try:
        total_new = generate_corpus(output_dir)
    except OSError as e:
        print(f"\nERROR: Failed to generate corpus: {e}")
        return 1
    except Exception as e:
        print(f"\nERROR: Unexpected error during generation: {e}")
        import traceback
        traceback.print_exc()
        return 1

    # Count total files in directory
    import glob
    total_files = len(glob.glob(os.path.join(output_dir, '*.mp')))
    print(f"\nTotal files in error-recovery directory: {total_files}")

    if total_files >= 50:
        print(f"Corpus meets AC6 requirement (50-100 files)")
        return 0
    else:
        print(f"Warning: Only {total_files} files, expected >= 50")
        return 1


if __name__ == '__main__':
    sys.exit(main())
