# Polish Map Format Specification

This document describes the Polish Map (.mp) format used for creating Garmin GPS maps with the cGPSmapper tool.

## Overview

Polish Map is a text-based vector format that defines geographic features for Garmin GPS devices. The format uses:

- **File extension**: `.mp`
- **Encoding**: Windows CP1252 (Western European) by default
- **Coordinate system**: WGS84 (EPSG:4326) with latitude/longitude
- **Structure**: INI-like sections with key=value pairs

## File Structure

A Polish Map file consists of:

1. **Header Section** (`[IMG ID]`): File-level metadata
2. **Feature Sections**: One of `[POI]`, `[POLYLINE]`, or `[POLYGON]`
3. **Section End Marker** (`[END]`): Marks end of each section

```
[IMG ID]
Name=My Map
CodePage=1252
ID=12345678
[END]

[POI]
Type=0x2C00
Label=Restaurant
Data0=(48.8566,2.3522)
[END]

[POLYLINE]
Type=0x0001
Label=Main Road
Data0=(48.8500,2.3400),(48.8550,2.3500),(48.8600,2.3450)
[END]

[POLYGON]
Type=0x004C
Label=Forest
Data0=(48.8400,2.3300),(48.8450,2.3400),(48.8400,2.3500),(48.8400,2.3300)
[END]
```

## Header Section ([IMG ID])

The header section defines file-level metadata. It must appear at the beginning of the file.

### Required Fields

| Field | Description | Example |
|-------|-------------|---------|
| `CodePage` | Character encoding code page | `1252` |

### Optional Fields

| Field | Description | Example |
|-------|-------------|---------|
| `Name` | Map name/title | `My Custom Map` |
| `ID` | Unique map identifier (8 digits) | `12345678` |
| `Datum` | Coordinate datum | `WGS 84` |
| `Elevation` | Elevation unit (M=meters, F=feet) | `M` |
| `Copyright` | Copyright notice | `(c) 2026 Author` |
| `PreProcess` | Preprocessing flags | `F` |
| `LblCoding` | Label encoding | `9` |
| `TreSize` | TRE file size hint | `1000` |
| `RgnLimit` | Region limit | `1024` |

### Example Header

```
[IMG ID]
Name=France POI
ID=12345678
CodePage=1252
Datum=WGS 84
Elevation=M
Copyright=(c) 2026 Open Data Contributors
[END]
```

## POI Section ([POI])

Point of Interest features represent single locations like restaurants, gas stations, hotels, etc.

### Fields

| Field | Required | Description | Example |
|-------|----------|-------------|---------|
| `Type` | Yes | Garmin POI type code (hex) | `0x2C00` |
| `Label` | No | Feature name/label | `Le Restaurant` |
| `Data0` | Yes | Coordinates as (lat,lon) | `(48.8566,2.3522)` |
| `EndLevel` | No | Max zoom level (0-9) | `3` |
| `Levels` | No | Display level range | `0-3` |

### Coordinate Format

POI coordinates use the format `(latitude,longitude)` with decimal degrees:

```
Data0=(48.856614,2.352222)
```

- Latitude: -90 to +90 (positive = North)
- Longitude: -180 to +180 (positive = East)
- Precision: typically 6 decimal places (~0.1m accuracy)

### Example POI

```
[POI]
Type=0x2C00
Label=Tour Eiffel
Data0=(48.858370,2.294481)
EndLevel=5
[END]
```

## POLYLINE Section ([POLYLINE])

Linear features representing roads, trails, rivers, boundaries, etc.

### Fields

| Field | Required | Description | Example |
|-------|----------|-------------|---------|
| `Type` | Yes | Garmin polyline type code | `0x0001` |
| `Label` | No | Feature name/label | `Rue de Rivoli` |
| `Data0` | Yes | Coordinate list | `(lat1,lon1),(lat2,lon2),...` |
| `EndLevel` | No | Max zoom level (0-9) | `3` |
| `Levels` | No | Display level range | `0-3` |
| `DirIndicator` | No | Direction indicator | `1` |
| `RouteParam` | No | Routing parameters | `3,0,0,0,0,0,0,0,0,0,0,0` |

### Coordinate Format

POLYLINE coordinates are a comma-separated list of (lat,lon) pairs:

```
Data0=(48.8500,2.3400),(48.8550,2.3500),(48.8600,2.3450)
```

Minimum: 2 points required for a valid line.

### Alternative Coordinate Formats

Some files use simpler formats:

```
; Standard format
Data0=(48.850,2.340),(48.855,2.350)

; Compact format (no parentheses)
Data0=48.850,2.340,48.855,2.350
```

### Example POLYLINE

```
[POLYLINE]
Type=0x0001
Label=Avenue des Champs-Élysées
Data0=(48.8738,2.2950),(48.8697,2.3079),(48.8656,2.3208)
EndLevel=4
Levels=0-4
[END]
```

## POLYGON Section ([POLYGON])

Area features representing forests, lakes, urban areas, parks, etc.

### Fields

| Field | Required | Description | Example |
|-------|----------|-------------|---------|
| `Type` | Yes | Garmin polygon type code | `0x004C` |
| `Label` | No | Feature name/label | `Bois de Boulogne` |
| `Data0` | Yes | Coordinate ring (closed) | `(lat1,lon1),...,(lat1,lon1)` |
| `EndLevel` | No | Max zoom level (0-9) | `3` |
| `Levels` | No | Display level range | `0-3` |

### Coordinate Format

POLYGON coordinates must form a closed ring (first point = last point):

```
Data0=(48.840,2.330),(48.845,2.340),(48.840,2.350),(48.840,2.330)
```

Minimum: 3 unique points (triangle) + closing point = 4 points total.

### Example POLYGON

```
[POLYGON]
Type=0x004C
Label=Jardin des Tuileries
Data0=(48.8632,2.3277),(48.8652,2.3277),(48.8652,2.3340),(48.8632,2.3340),(48.8632,2.3277)
EndLevel=3
[END]
```

## Type Codes

Type codes are hexadecimal values that determine feature symbology on Garmin devices.

### Format

```
Type=0xNNNN
```

Where `NNNN` is a 4-digit hexadecimal number (case-insensitive).

### Type Code Ranges

| Range | Category |
|-------|----------|
| 0x0001-0x0FFF | Roads and Polylines |
| 0x2C00-0x2FFF | Points of Interest |
| 0x0001-0x004F | Polygons/Areas |

See [garmin-types.md](garmin-types.md) for the complete reference.

## EndLevel and Levels

These fields control at which zoom levels features are displayed.

### EndLevel

Maximum zoom level at which the feature is visible (0-9, where 0 is most zoomed out):

```
EndLevel=3  ; Visible at levels 0, 1, 2, 3
```

### Levels

Display level range (alternative to EndLevel):

```
Levels=0-3  ; Visible at levels 0, 1, 2, 3
```

## Character Encoding

### CodePage Values

| CodePage | Encoding | Region |
|----------|----------|--------|
| `1250` | CP1250 | Central European |
| `1251` | CP1251 | Cyrillic |
| `1252` | CP1252 | Western European (default) |
| `1253` | CP1253 | Greek |
| `1254` | CP1254 | Turkish |
| `1255` | CP1255 | Hebrew |
| `1256` | CP1256 | Arabic |
| `1257` | CP1257 | Baltic |
| `1258` | CP1258 | Vietnamese |
| `65001` | UTF-8 | Unicode |

### Special Characters

Labels containing special characters should use the appropriate code page:

```
[IMG ID]
CodePage=1252
[END]

[POI]
Type=0x2C00
Label=Café des Arts  ; é encoded as 0xE9 in CP1252
Data0=(48.8566,2.3522)
[END]
```

## Comments

Lines starting with `;` are comments and ignored by parsers:

```
; This is a comment
[POI]
Type=0x2C00  ; Restaurant type
Label=My POI
Data0=(48.8566,2.3522)
[END]
```

## Multi-Line Values

Some fields can span multiple lines using continuation:

```
[POLYLINE]
Type=0x0001
Data0=(48.850,2.340),(48.855,2.350),
Data1=(48.860,2.345),(48.865,2.355)
[END]
```

The `Data1`, `Data2`, etc. fields continue the coordinate list from `Data0`.

## Validation Rules

### General Rules

1. Every section must end with `[END]`
2. The file must start with `[IMG ID]` section
3. Type field is required for all feature sections
4. Data0 field is required for all feature sections

### Coordinate Validation

1. Latitude must be in range [-90, +90]
2. Longitude must be in range [-180, +180]
3. POI must have exactly 1 coordinate
4. POLYLINE must have at least 2 coordinates
5. POLYGON must have at least 3 unique coordinates (4 total with closing)

### Type Code Validation

1. Must be valid hexadecimal (0x prefix optional)
2. Should be in recognized range for the section type

## References

- [cGPSmapper Manual](http://www.cgpsmapper.com/manual.htm) - Complete format specification
- [GPSMapEdit Documentation](http://www.geopainting.com/en/help/) - Editor with format support
- [Garmin Type Codes](http://www.yournavigation.org/wiki/index.php/Garmin_map_type_codes) - Type reference
