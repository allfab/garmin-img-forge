.. _vector.polishmap:

Polish Map - MP
===============

.. shortname:: PolishMap

.. built_in_by_default::

This driver reads and writes Polish Map (.mp) files, a text-based vector format
used for creating Garmin GPS maps with the cGPSmapper tool.

Description
-----------

The Polish Map format is a human-readable text format that defines geographic
features for Garmin GPS devices. It supports three main geometry types:

- **POI (Points of Interest)**: Point features such as restaurants, gas stations,
  hotels, and landmarks
- **POLYLINE**: Linear features such as roads, trails, rivers, and boundaries
- **POLYGON**: Area features such as forests, lakes, urban areas, and parks

The format uses a simple INI-like structure with bracketed section headers
(e.g., ``[POI]``, ``[POLYLINE]``, ``[POLYGON]``) and key-value pairs for
attributes. Coordinates are stored in WGS84 (EPSG:4326) with latitude/longitude
pairs.

**File Extension**: ``.mp``

**MIME Type**: ``text/plain``

**Encoding**: Codepage 1252 (Windows Western European) by default

**Key Features**:

- Text-based format, editable with any text editor
- Supports Garmin type codes for symbology (0x0000-0xFFFF)
- Multi-level display support (zoom levels 0-9)
- UTF-8 to CP1252 automatic encoding conversion on write
- Preserves Label, Type, EndLevel, and Levels attributes

Driver Capabilities
-------------------

.. supports_create::

.. supports_georeferencing::

.. supports_virtualio::

**Supported Geometry Types**:

+------------------+---------------------+-----------------------------------+
| OGR Geometry     | Polish Map Section  | Description                       |
+==================+=====================+===================================+
| wkbPoint         | [POI]               | Point of Interest features        |
+------------------+---------------------+-----------------------------------+
| wkbLineString    | [POLYLINE]          | Linear features (roads, trails)   |
+------------------+---------------------+-----------------------------------+
| wkbPolygon       | [POLYGON]           | Area features (forests, lakes)    |
+------------------+---------------------+-----------------------------------+

**Supported Field Types**:

+------------------+---------------------+-----------------------------------+
| OGR Field Type   | Polish Map Field    | Description                       |
+==================+=====================+===================================+
| OFTString        | Type                | Garmin type code (e.g., "0x2C00") |
+------------------+---------------------+-----------------------------------+
| OFTString        | Label               | Feature label/name (UTF-8)        |
+------------------+---------------------+-----------------------------------+
| OFTString        | Data0               | Primary coordinate data           |
+------------------+---------------------+-----------------------------------+
| OFTInteger       | EndLevel            | Maximum display zoom level (0-9)  |
+------------------+---------------------+-----------------------------------+
| OFTString        | Levels              | Display zoom range (e.g., "0-3")  |
+------------------+---------------------+-----------------------------------+

**Driver Capabilities Table**:

+-------------------------+--------+
| Capability              | Value  |
+=========================+========+
| OLCRandomRead           | No     |
+-------------------------+--------+
| OLCSequentialWrite      | Yes    |
+-------------------------+--------+
| OLCRandomWrite          | No     |
+-------------------------+--------+
| OLCFastSpatialFilter    | No     |
+-------------------------+--------+
| OLCFastFeatureCount     | No     |
+-------------------------+--------+
| OLCFastGetExtent        | No     |
+-------------------------+--------+
| OLCCreateField          | Yes*   |
+-------------------------+--------+
| OLCDeleteField          | No     |
+-------------------------+--------+
| OLCReorderFields        | No     |
+-------------------------+--------+
| OLCAlterFieldDefn       | No     |
+-------------------------+--------+
| OLCDeleteFeature        | No     |
+-------------------------+--------+
| OLCStringsAsUTF8        | Yes    |
+-------------------------+--------+

\* OLCCreateField is only supported in write mode.

Header Section ([IMG ID])
--------------------------

The driver supports reading and writing the ``[IMG ID]`` header section with
extended metadata fields. The header contains map-level metadata that is used
by cGPSmapper and Garmin devices for proper map compilation and display.

**Story 1.2/2.2 Extension**: The driver now supports 15 standard header fields
plus preservation of custom/unknown fields for round-trip compatibility.

**Supported Header Fields**:

+------------------+-------------+-----------------------------------------------+
| Field Name       | Default     | Description                                   |
+==================+=============+===============================================+
| **ID**           | "1"         | Map identifier (REQUIRED by cGPSmapper spec)  |
+------------------+-------------+-----------------------------------------------+
| **Name**         | "Untitled"  | Human-readable map name                       |
+------------------+-------------+-----------------------------------------------+
| **CodePage**     | "1252"      | Character encoding (CP1252 = Western Europe)  |
+------------------+-------------+-----------------------------------------------+
| **Datum**        | "W84"       | Coordinate system (W84 = WGS 84)              |
+------------------+-------------+-----------------------------------------------+
| **Elevation**    | (none)      | Elevation unit: M (meters) or F (feet)        |
+------------------+-------------+-----------------------------------------------+
| **LBLcoding**    | "9"         | Label encoding: 6/9/10 (9 = 8-bit, smallest)  |
+------------------+-------------+-----------------------------------------------+
| **Preprocess**   | "F"         | Preprocessing: G/F/P/N (F = full, best compat)|
+------------------+-------------+-----------------------------------------------+
| **Levels**       | (none)      | Number of zoom levels: 1-10                   |
+------------------+-------------+-----------------------------------------------+
| **Level0-N**     | (none)      | Zoom level definitions (e.g., Level0=24)      |
+------------------+-------------+-----------------------------------------------+
| **TreeSize**     | "3000"      | Map tree size: 100-15000 (3000 = countryside) |
+------------------+-------------+-----------------------------------------------+
| **RgnLimit**     | "1024"      | Region element limit: 50-1024 (1024 = max)    |
+------------------+-------------+-----------------------------------------------+
| **Transparent**  | "N"         | Transparency: Y (yes) / N (no) / S (semi)     |
+------------------+-------------+-----------------------------------------------+
| **SimplifyLevel**| "2"         | Douglas-Peucker simplification: 0-4           |
+------------------+-------------+-----------------------------------------------+
| **Marine**       | "N"         | Marine map: Y (yes) / N (no)                  |
+------------------+-------------+-----------------------------------------------+
| **LeftSideTraffic** | "N"      | Left-side traffic: Y (yes) / N (no)           |
+------------------+-------------+-----------------------------------------------+

**Intelligent Defaults**: When creating a new Polish Map file, the driver
applies intelligent default values based on cGPSmapper best practices. These
defaults ensure proper compilation and compatibility with Garmin devices.

**Round-Trip Preservation**: The driver preserves all header fields during
read-write operations, including unrecognized custom fields. This ensures
100% metadata preservation for round-trip conversions.

**Setting Header Metadata**: Use GDAL ``SetMetadataItem()`` API to set header
fields programmatically, or use dataset creation options (NAME, ID, CODEPAGE).

**Example - Reading header metadata**::

    from osgeo import gdal
    ds = gdal.Open("input.mp")
    print(ds.GetMetadataItem("ID"))        # Map ID
    print(ds.GetMetadataItem("Name"))      # Map name
    print(ds.GetMetadataItem("LBLcoding")) # Label encoding

**Example - Writing header metadata**::

    from osgeo import ogr
    driver = ogr.GetDriverByName("PolishMap")
    ds = driver.CreateDataSource("output.mp")
    ds.SetMetadataItem("Name", "My Custom Map")
    ds.SetMetadataItem("ID", "12345")
    ds.SetMetadataItem("LBLcoding", "9")
    ds.SetMetadataItem("Preprocess", "F")
    # ... add features ...
    ds = None  # Close and write

Field Mapping (CreateField Behavior)
------------------------------------

When converting from other formats (such as Shapefile or GeoJSON) using
``ogr2ogr``, the driver implements an **accept-and-map** pattern for field
handling:

- **All fields are accepted**: The driver returns ``OGRERR_NONE`` for any
  ``CreateField()`` call, ensuring compatibility with any source format.
- **Known fields are mapped**: Fields matching the Polish Map schema are
  tracked and their values are written to the output file.
- **Unknown fields are silently ignored**: Fields that don't match the
  Polish Map schema are accepted but their values are not written.

This approach enables seamless conversion from any source format without
errors, while preserving only the attributes meaningful for Polish Map files.

**Known Polish Map Fields** (case-insensitive matching):

+------------------+-----------------------------------+
| Field Name       | Description                       |
+==================+===================================+
| Type             | Garmin type code (e.g., "0x2C00") |
+------------------+-----------------------------------+
| Label            | Feature label/name                |
+------------------+-----------------------------------+
| Data0..Data9     | Coordinate data strings           |
+------------------+-----------------------------------+
| EndLevel         | Maximum display zoom level (0-9)  |
+------------------+-----------------------------------+
| Levels           | Display zoom range (e.g., "0-3")  |
+------------------+-----------------------------------+

**Example: Converting Shapefile to Polish Map**::

    # Convert roads shapefile - unknown fields like "ROAD_CLASS" are ignored
    ogr2ogr -f "PolishMap" roads.mp roads.shp

    # Convert with SQL to map source fields to Polish Map fields
    ogr2ogr -f "PolishMap" output.mp input.shp \
        -sql "SELECT NAME AS Label, TYPE_CODE AS Type FROM input"

Dataset Creation Options
------------------------

The following dataset creation options are available:

- **NAME** (string): Map name to be written in the [IMG ID] header section.
  Default: filename without extension.

- **CODEPAGE** (string): Character encoding for the output file.
  Default: ``1252`` (Windows CP1252).
  Common values: ``1252`` (Western European), ``1250`` (Central European),
  ``1251`` (Cyrillic), ``65001`` (UTF-8).

- **ID** (string): Map identifier for the [IMG ID] header section.
  Default: auto-generated.

- **FIELD_MAPPING** (string): Path to a YAML configuration file that defines
  custom field name mappings from source fields to Polish Map fields.
  Default: None (uses hardcoded aliases).

  This option enables automatic mapping of fields during ogr2ogr conversion.
  For example, you can map IGN BDTOPO field names or OpenStreetMap tags to
  Polish Map standard fields without using SQL SELECT statements.

  **YAML file format**::

      field_mapping:
        SOURCE_FIELD: TARGET_FIELD
        ANOTHER_SOURCE: ANOTHER_TARGET

  **Available target fields**: Type, Label, EndLevel, Levels, Data0-Data9,
  SubType, Marine, City, StreetDesc, HouseNumber, PhoneNumber, Highway,
  CityName, RegionName, CountryName, Zip, DirIndicator, RouteParam

  See ``examples/bdtopo-mapping.yaml`` for a complete example and
  ``docs/field-mapping-guide.md`` for detailed documentation.

**Example with creation options**::

    ogr2ogr -f "PolishMap" output.mp input.geojson \
        -dsco NAME="My Custom Map" \
        -dsco CODEPAGE=1252

Layer Creation Options
----------------------

The PolishMap driver uses three predefined layers (POI, POLYLINE, POLYGON)
that are automatically created when opening a file for writing. Layer creation
options are not applicable as layers are fixed.

When using ``ogr2ogr`` or the ``ICreateLayer()`` API, the driver automatically
routes features to the appropriate layer based on geometry type:

- Point geometries → POI layer
- LineString geometries → POLYLINE layer
- Polygon geometries → POLYGON layer

Examples
--------

**Reading a Polish Map file with ogrinfo**::

    # Display file summary
    ogrinfo sample.mp

    # Display all features in the POI layer
    ogrinfo -al sample.mp POI

    # Display features with specific attributes
    ogrinfo -al -where "Type='0x2C00'" sample.mp

**Converting with ogr2ogr**::

    # Convert Polish Map to GeoJSON
    ogr2ogr -f "GeoJSON" output.geojson input.mp

    # Convert GeoJSON to Polish Map
    ogr2ogr -f "PolishMap" output.mp input.geojson

    # Convert Shapefile to Polish Map
    ogr2ogr -f "PolishMap" output.mp roads.shp

    # Convert specific layer
    ogr2ogr -f "GeoJSON" pois.geojson input.mp POI

    # Apply spatial filter during conversion
    ogr2ogr -f "GeoJSON" paris.geojson input.mp -spat 2.2 48.8 2.4 49.0

    # Apply attribute filter
    ogr2ogr -f "GeoJSON" restaurants.geojson input.mp -where "Type='0x2C00'"

**Converting with Field Mapping Configuration**::

    # Convert IGN BDTOPO with custom field mapping
    ogr2ogr -f "PolishMap" communes.mp COMMUNE.shp \
        -dsco FIELD_MAPPING=bdtopo-mapping.yaml

    # Convert OpenStreetMap data with field mapping
    ogr2ogr -f "PolishMap" buildings.mp buildings.geojson \
        -dsco FIELD_MAPPING=osm-mapping.yaml

    # Field mapping with filters
    ogr2ogr -f "PolishMap" grandes_villes.mp COMMUNE.shp \
        -dsco FIELD_MAPPING=bdtopo-mapping.yaml \
        -where "POPULATION > 10000"

    # Without field mapping (uses hardcoded aliases)
    ogr2ogr -f "PolishMap" output.mp input.shp

See ``docs/field-mapping-guide.md`` for comprehensive documentation on creating
custom field mapping configurations.

**Python example - Reading**:

.. code-block:: python

    from osgeo import ogr, gdal

    gdal.UseExceptions()

    # Open the Polish Map file
    ds = ogr.Open("sample.mp")
    if ds is None:
        raise Exception("Could not open file")

    print(f"Driver: {ds.GetDriver().GetName()}")
    print(f"Layers: {ds.GetLayerCount()}")

    # Iterate through layers
    for i in range(ds.GetLayerCount()):
        layer = ds.GetLayer(i)
        print(f"\nLayer: {layer.GetName()}")
        print(f"Feature count: {layer.GetFeatureCount()}")

        # Read features
        for feature in layer:
            fid = feature.GetFID()
            type_val = feature.GetField("Type")
            label = feature.GetField("Label")
            geom = feature.GetGeometryRef()

            print(f"  FID {fid}: Type={type_val}, Label={label}")
            if geom:
                print(f"    Geometry: {geom.ExportToWkt()[:50]}...")

    # Close dataset (setting to None releases the handle)
    ds = None

**Python example - Writing**:

.. code-block:: python

    from osgeo import ogr, gdal

    gdal.UseExceptions()

    # Get the PolishMap driver
    driver = ogr.GetDriverByName("PolishMap")

    # Create new file
    ds = driver.CreateDataSource("output.mp")

    # Get the POI layer (index 0)
    poi_layer = ds.GetLayer(0)

    # Create a POI feature
    feature = ogr.Feature(poi_layer.GetLayerDefn())
    feature.SetField("Type", "0x2C00")  # Restaurant
    feature.SetField("Label", "My Restaurant")

    point = ogr.Geometry(ogr.wkbPoint)
    point.AddPoint(2.3522, 48.8566)  # lon, lat
    feature.SetGeometry(point)

    poi_layer.CreateFeature(feature)

    # Close and save (setting to None flushes writes and releases handle)
    ds = None

**Python example - Format Conversion**:

.. code-block:: python

    from osgeo import ogr, gdal

    gdal.UseExceptions()

    # Open source GeoJSON
    src_ds = ogr.Open("input.geojson")

    # Create destination Polish Map
    driver = ogr.GetDriverByName("PolishMap")
    dst_ds = driver.CreateDataSource("output.mp")

    # Copy features (simplified)
    src_layer = src_ds.GetLayer(0)
    for src_feature in src_layer:
        geom = src_feature.GetGeometryRef()
        if geom.GetGeometryType() == ogr.wkbPoint:
            dst_layer = dst_ds.GetLayer(0)  # POI
        elif geom.GetGeometryType() == ogr.wkbLineString:
            dst_layer = dst_ds.GetLayer(1)  # POLYLINE
        elif geom.GetGeometryType() == ogr.wkbPolygon:
            dst_layer = dst_ds.GetLayer(2)  # POLYGON
        else:
            continue

        dst_feature = ogr.Feature(dst_layer.GetLayerDefn())
        dst_feature.SetGeometry(geom)

        # Copy Type field if present
        type_idx = src_feature.GetFieldIndex("Type")
        if type_idx >= 0:
            dst_feature.SetField("Type", src_feature.GetField(type_idx))

        dst_layer.CreateFeature(dst_feature)

    # Close datasets (releases handles and flushes pending writes)
    src_ds = None
    dst_ds = None

See Also
--------

- `cGPSmapper Manual <http://www.cgpsmapper.com/manual.htm>`__ - Complete Polish Map format specification
- `Garmin Type Codes Reference <https://wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/POI_Types>`__ - Type code reference for POI, polyline, and polygon features
- :ref:`GeoJSON driver <vector.geojson>` - Common conversion source/target format
- :ref:`Shapefile driver <vector.shapefile>` - Common conversion source/target format
- :ref:`GPX driver <vector.gpx>` - Related GPS data format
- `GDAL Vector Driver Tutorial <https://gdal.org/tutorials/vector_driver_tut.html>`__ - Creating custom OGR drivers
