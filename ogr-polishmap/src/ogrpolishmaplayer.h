/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Declaration of OGRPolishMapLayer class
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 ****************************************************************************/

#ifndef OGRPOLISHMAPLAYER_H_INCLUDED
#define OGRPOLISHMAPLAYER_H_INCLUDED

#include "ogrsf_frmts.h"
#include "ogr_spatialref.h"
#include <set>
#include <string>

// Forward declarations
class PolishMapParser;
class PolishMapWriter;

/************************************************************************/
/*                         OGRPolishMapLayer                            */
/************************************************************************/

/**
 * @class OGRPolishMapLayer
 * @brief Layer class for Polish Map format features.
 *
 * Represents one of the three layer types in a Polish Map file:
 * - POI: Point of Interest features (wkbPoint)
 * - POLYLINE: Linear features like roads and trails (wkbLineString)
 * - POLYGON: Area features like forests and lakes (wkbPolygon)
 *
 * @section fields Attribute Fields
 * Each layer supports the following fields:
 * - **Type** (OFTString): Garmin type code (e.g., "0x2C00" for restaurant)
 * - **Label** (OFTString): Feature name/label (UTF-8 encoded)
 * - **Data0** (OFTString): Raw coordinate data string
 * - **EndLevel** (OFTInteger): Maximum display zoom level (0-9)
 * - **Levels** (OFTString): Display zoom range (e.g., "0-3")
 *
 * @section srs Spatial Reference
 * All features use WGS84 (EPSG:4326) coordinate system with
 * latitude/longitude coordinates.
 *
 * @section modes Operating Modes
 * - Read mode: Features are read from file via PolishMapParser
 * - Write mode: Features are written to file via PolishMapWriter
 *
 * @see OGRPolishMapDataSource
 * @see PolishMapParser
 * @see PolishMapWriter
 */
class OGRPolishMapLayer final : public OGRLayer {
private:
    // Ring closure tolerance: 1e-9 degrees (~0.1mm at Earth surface)
    // Used for comparing first/last point to determine if POLYGON ring needs auto-closure
    static constexpr double RING_CLOSURE_TOLERANCE = 1e-9;

    OGRFeatureDefn* m_poFeatureDefn;
    OGRSpatialReference* m_poSRS;
    GIntBig m_nNextFID;

    // Story 1.4: Parser and layer type for feature reading
    PolishMapParser* m_poParser;       // Non-owning pointer to parser
    CPLString m_osLayerType;           // "POI", "POLYLINE", or "POLYGON"
    bool m_bEOF;                       // End of file reached
    bool m_bReaderInitialized;        // Parser position initialized

    // Story 2.3: Write mode support
    bool m_bWriteMode;                 // true if layer is in write mode
    PolishMapWriter* m_poWriter;       // Non-owning pointer to writer (write mode only)

    /**
     * @brief Set of field names mapped to Polish Map schema.
     *
     * Story 4.1: Tracks which source fields have been mapped to Polish Map
     * attributes via CreateField(). Used for accept-and-map pattern to enable
     * ogr2ogr compatibility with any source format.
     *
     * @see CreateField()
     */
    std::set<std::string> m_oMappedFields;

public:
    /**
     * @brief Construct a layer for write mode (no parser).
     *
     * Creates a layer with the specified name and geometry type.
     * Used when creating new Polish Map files for writing.
     *
     * @param pszLayerName Layer name ("POI", "POLYLINE", or "POLYGON").
     * @param eGeomType Geometry type (wkbPoint, wkbLineString, or wkbPolygon).
     */
    OGRPolishMapLayer(const char* pszLayerName, OGRwkbGeometryType eGeomType);

    /**
     * @brief Construct a layer for read mode with parser.
     *
     * Creates a layer connected to a parser for reading features from file.
     *
     * @param pszLayerName Layer name ("POI", "POLYLINE", or "POLYGON").
     * @param eGeomType Geometry type (wkbPoint, wkbLineString, or wkbPolygon).
     * @param poParser Non-owning pointer to the file parser.
     *
     * @note The parser must remain valid for the lifetime of the layer.
     */
    OGRPolishMapLayer(const char* pszLayerName, OGRwkbGeometryType eGeomType,
                      PolishMapParser* poParser);

    /**
     * @brief Destructor.
     *
     * Releases the feature definition and spatial reference.
     */
    ~OGRPolishMapLayer() override;

    /** @brief Copy constructor (deleted). */
    OGRPolishMapLayer(const OGRPolishMapLayer&) = delete;

    /** @brief Copy assignment operator (deleted). */
    OGRPolishMapLayer& operator=(const OGRPolishMapLayer&) = delete;

    /**
     * @brief Reset feature reading to the beginning of the layer.
     *
     * Resets the internal reading position so that the next call to
     * GetNextFeature() returns the first feature in the layer.
     *
     * @note In read mode, this resets the parser position.
     * @note In write mode, this has no effect.
     */
    void ResetReading() override;

    /**
     * @brief Get the next feature in the layer.
     *
     * Returns the next feature from the layer, applying any active
     * spatial or attribute filters. Returns nullptr when no more
     * features are available.
     *
     * @return Pointer to the next OGRFeature, or nullptr if no more features.
     *         Caller takes ownership and must destroy with OGRFeature::DestroyFeature().
     *
     * @note The returned feature includes geometry and all attribute fields.
     * @note Filters set via SetSpatialFilter() and SetAttributeFilter() are applied.
     */
    OGRFeature* GetNextFeature() override;

    /**
     * @brief Get the layer's feature definition.
     *
     * Returns the schema definition for features in this layer, including
     * geometry type and field definitions (Type, Label, Data0, EndLevel, Levels).
     *
     * @return Pointer to OGRFeatureDefn. Ownership remains with the layer.
     */
    OGRFeatureDefn* GetLayerDefn() override;

    /**
     * @brief Test if the layer supports a specific capability.
     *
     * @param pszCap Capability name string.
     * @return TRUE if capability is supported, FALSE otherwise.
     *
     * @note Supported capabilities:
     *       - OLCSequentialWrite: Yes (write mode only)
     *       - OLCStringsAsUTF8: Yes
     *       - OLCFastFeatureCount: No
     *       - OLCFastGetExtent: No
     *       - OLCRandomRead: No
     *       - OLCRandomWrite: No
     */
    int TestCapability(const char* pszCap) override;

    /**
     * @brief Set the writer for this layer (enables write mode).
     *
     * Associates a PolishMapWriter with this layer, enabling feature
     * writing via CreateFeature()/ICreateFeature().
     *
     * @param poWriter Non-owning pointer to PolishMapWriter instance.
     *
     * @note The writer must remain valid for the lifetime of the layer.
     * @note Call this before attempting to write features.
     */
    void SetWriter(PolishMapWriter* poWriter);

protected:
    /**
     * @brief Create and write a feature to the Polish Map file.
     *
     * Writes the feature to the file using the associated PolishMapWriter.
     * The feature must have valid geometry matching the layer's geometry type.
     *
     * @param poFeature Feature to write. Must have valid geometry.
     * @return OGRERR_NONE on success, or error code on failure.
     *
     * @note The feature's FID is assigned by the layer after successful write.
     * @note Required fields: geometry. Optional: Type, Label, EndLevel, Levels.
     * @note This is the protected implementation called by CreateFeature().
     */
    OGRErr ICreateFeature(OGRFeature* poFeature) override;

    /**
     * @brief Accept and map field definitions from source layer.
     *
     * Implements the "accept-and-map" pattern for ogr2ogr compatibility.
     * All fields are accepted (returns OGRERR_NONE), but only fields matching
     * the Polish Map schema are tracked for output:
     * - Type: Feature type code
     * - Label: Feature label/name
     * - Data[0-9]+: Data fields
     * - EndLevel: Maximum zoom level
     * - Levels: Zoom level range
     *
     * Unknown fields are silently ignored to allow seamless conversion
     * from any source format.
     *
     * @param poField Field definition to process.
     * @param bApproxOK Approximate match flag (unused, always accepts).
     * @return OGRERR_NONE always (accept-and-ignore pattern).
     *
     * @note Story 4.1: Enables ogr2ogr conversion from Shapefile to Polish Map.
     * @see TestCapability(OLCCreateField)
     */
    OGRErr CreateField(const OGRFieldDefn* poField, int bApproxOK = TRUE) override;

private:
    /**
     * @brief Initialize the feature definition, SRS, and field definitions.
     *
     * Sets up the layer's schema with geometry type, WGS84 SRS, and
     * standard Polish Map fields (Type, Label, Data0, EndLevel, Levels).
     *
     * @param pszLayerName Layer name for the feature definition.
     * @param eGeomType Geometry type for the layer.
     */
    void InitializeLayerDefn(const char* pszLayerName, OGRwkbGeometryType eGeomType);

    /**
     * @brief Read and return the next POI feature from the parser.
     *
     * Parses the next [POI] section and converts it to an OGRFeature with
     * Point geometry.
     *
     * @return Pointer to new OGRFeature, or nullptr if no more POIs.
     */
    OGRFeature* GetNextPOIFeature();

    /**
     * @brief Read and return the next POLYLINE feature from the parser.
     *
     * Parses the next [POLYLINE] section and converts it to an OGRFeature
     * with LineString geometry.
     *
     * @return Pointer to new OGRFeature, or nullptr if no more POLYLINEs.
     */
    OGRFeature* GetNextPolylineFeature();

    /**
     * @brief Read and return the next POLYGON feature from the parser.
     *
     * Parses the next [POLYGON] section and converts it to an OGRFeature
     * with Polygon geometry.
     *
     * @return Pointer to new OGRFeature, or nullptr if no more POLYGONs.
     */
    OGRFeature* GetNextPolygonFeature();
};

#endif /* OGRPOLISHMAPLAYER_H_INCLUDED */
