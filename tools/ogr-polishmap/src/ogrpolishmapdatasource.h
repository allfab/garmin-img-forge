/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Declaration of OGRPolishMapDataSource class
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

#ifndef OGRPOLISHMAPDATASOURCE_H_INCLUDED
#define OGRPOLISHMAPDATASOURCE_H_INCLUDED

#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "ogrpolishmap_compat.h"
#include "polishmapparser.h"
#include "polishmapfieldmapper.h"
#include <vector>
#include <memory>
#include <map>
#include <string>

class OGRPolishMapLayer;
class PolishMapWriter;

/************************************************************************/
/*                       OGRPolishMapDataSource                         */
/************************************************************************/

/**
 * @class OGRPolishMapDataSource
 * @brief Dataset class for Polish Map (.mp) files.
 *
 * This class represents an opened Polish Map file and provides access to
 * its three fixed layers: POI (Point), POLYLINE (LineString), and POLYGON.
 * It manages file I/O, header metadata, and layer access for both reading
 * and writing operations.
 *
 * @section layers Layer Structure
 * Polish Map files always have exactly three layers:
 * - Layer 0: POI (wkbPoint) - Points of Interest
 * - Layer 1: POLYLINE (wkbLineString) - Roads, trails, boundaries
 * - Layer 2: POLYGON (wkbPolygon) - Areas, forests, water bodies
 *
 * @section metadata Header Metadata
 * The [IMG ID] section metadata can be accessed via GetMetadataItem():
 * - "Name": Map name
 * - "ID": Map identifier
 * - "CodePage": Character encoding (default: 1252)
 * - "Datum": Coordinate system (default: WGS 84)
 *
 * @see OGRPolishMapDriver
 * @see OGRPolishMapLayer
 * @see PolishMapParser
 * @see PolishMapWriter
 */
class OGRPolishMapDataSource final : public GDALDataset {
private:
    std::vector<std::unique_ptr<OGRPolishMapLayer>> m_apoLayers;
    CPLString m_osFilename;
    PolishMapHeaderData m_oHeaderData;  // Story 1.2: Header metadata storage
    std::unique_ptr<PolishMapParser> m_poParser;  // Story 1.4: Parser for feature reading

    // Story 2.1: Write mode support
    bool m_bUpdate;                              // false = read (Open), true = write (Create)
    VSILFILE* m_fpOutput;                        // Output file handle (write mode only)
    std::unique_ptr<PolishMapWriter> m_poWriter; // Writer instance (write mode only)

    // Story 2.2: Metadata storage for Polish Map header fields
    std::map<std::string, std::string> m_aoMetadata;

    // Story 4.4: Field mapping support
    std::unique_ptr<PolishMapFieldMapper> m_poFieldMapper;  // Field mapper (write mode only)

    // Tech-spec #2 Task 2: Multi-geometry fields (Data1=, Data2=, ...)
    // Activated via CSL option MULTI_GEOM_FIELDS=YES + MAX_DATA_LEVEL=K (K in [1, 9]).
    // POI layer is always mono-geom (MP spec §4.4.3.1).
    bool m_bMultiGeomFields = false;
    int  m_nMaxDataLevel = 0;  // 0 = disabled; otherwise K such that N-1 extra geom fields are added for N=K+1

    // Story 1.3: Initialize the 3 layers (POI, POLYLINE, POLYGON)
    void CreateLayers();

    // Story 2.1: Create layers for write mode (no parser needed)
    void CreateLayersForWriteMode();

public:
    /**
     * @brief Default constructor.
     *
     * Initializes an empty datasource in read mode. Use Open() via the driver
     * to open an existing file, or Create() to create a new file for writing.
     */
    OGRPolishMapDataSource();

    /**
     * @brief Destructor.
     *
     * Closes the file handle and frees all layer resources.
     * In write mode, ensures all pending data is flushed to disk.
     */
    ~OGRPolishMapDataSource() override;

    /**
     * @brief Get the number of layers in the dataset.
     *
     * Polish Map files always have exactly 3 layers (POI, POLYLINE, POLYGON),
     * though some layers may be empty if the file contains no features of that type.
     *
     * @return Always returns 3.
     */
    int GetLayerCount() OGRPOLISHMAP_CONST override;

    /**
     * @brief Get a layer by index.
     *
     * @param nLayer Layer index (0=POI, 1=POLYLINE, 2=POLYGON).
     * @return Pointer to the requested OGRLayer, or nullptr if index is out of range.
     *
     * @note Ownership remains with the dataset. Caller must NOT delete the returned pointer.
     * @note The returned layer is valid as long as the dataset is open.
     */
    OGRLayer* GetLayer(int nLayer) OGRPOLISHMAP_CONST override;

    /**
     * @brief Test if the dataset supports a specific capability.
     *
     * @param pszCap Capability name string (ODsCCreateLayer, ODsCDeleteLayer, etc.).
     * @return TRUE if capability is supported, FALSE otherwise.
     *
     * @note Supported capabilities:
     *       - ODsCCreateLayer: Yes (routes to appropriate fixed layer)
     *       - ODsCRandomLayerRead: No
     *       - ODsCSingleLayerZip: No
     */
    int TestCapability(const char* pszCap) OGRPOLISHMAP_CONST override;

    /**
     * @brief Create a layer (routes to appropriate fixed layer).
     *
     * Polish Map format has fixed layers (POI, POLYLINE, POLYGON). This method
     * does not create new layers but instead routes to the appropriate existing
     * layer based on the requested geometry type:
     * - wkbPoint, wkbMultiPoint → POI layer (index 0)
     * - wkbLineString, wkbMultiLineString → POLYLINE layer (index 1)
     * - wkbPolygon, wkbMultiPolygon → POLYGON layer (index 2)
     *
     * This enables ogr2ogr compatibility where -nlt or source geometry type
     * determines which Polish Map layer receives the features.
     *
     * @param pszName Requested layer name (ignored, routing is by geometry type).
     * @param poGeomFieldDefn Geometry field definition containing geometry type.
     * @param papszOptions Layer creation options (currently unused).
     * @return Pointer to the appropriate existing layer, or nullptr on error.
     *
     * @note The returned layer is owned by the dataset; do not delete it.
     */
    OGRLayer* ICreateLayer(const char* pszName,
                           const OGRGeomFieldDefn* poGeomFieldDefn,
                           CSLConstList papszOptions) override;

    /**
     * @brief Set the header data from the parser.
     *
     * @param oData Parsed header data structure.
     *
     * @note Used internally during file opening.
     */
    void SetHeaderData(const PolishMapHeaderData& oData) { m_oHeaderData = oData; }

    /**
     * @brief Get the header metadata.
     *
     * @return Const reference to the header data structure containing
     *         Name, ID, CodePage, Datum, and other fields.
     */
    const PolishMapHeaderData& GetHeaderData() const { return m_oHeaderData; }

    /**
     * @brief Tech-spec #2 Task 4: enable multi-geom exposure on read path.
     *        Must be called BEFORE SetParser() (which creates the layers).
     */
    void SetMultiGeomFields(bool bEnabled, int nMaxDataLevel) {
        m_bMultiGeomFields = bEnabled;
        m_nMaxDataLevel = nMaxDataLevel;
    }

    /**
     * @brief Set the parser instance for reading.
     *
     * @param poParser Unique pointer to a PolishMapParser instance.
     *                 Ownership is transferred to the datasource.
     *
     * @note Used internally during file opening.
     */
    void SetParser(std::unique_ptr<PolishMapParser> poParser);

    /**
     * @brief Get the parser instance.
     *
     * @return Raw pointer to the parser, or nullptr if not set.
     *
     * @note The parser is owned by the datasource; do not delete.
     */
    PolishMapParser* GetParser() { return m_poParser.get(); }

    /**
     * @brief Factory method to create a new Polish Map file for writing.
     *
     * Creates a new file and initializes it with empty POI, POLYLINE, and
     * POLYGON layers ready for feature writing.
     *
     * @param pszFilename Output file path.
     * @param papszOptions Creation options (e.g., FIELD_MAPPING=path.yaml).
     * @return Pointer to new datasource on success, nullptr on failure.
     *         Caller takes ownership and must delete with delete operator or GDALClose().
     *
     * @note On failure, CPLError() is called with appropriate error message.
     * @note Supported options:
     *       - FIELD_MAPPING=path: Path to YAML field mapping config (Story 4.4)
     */
    static OGRPolishMapDataSource* Create(const char* pszFilename,
                                           char** papszOptions = nullptr);

    /**
     * @brief Check if the datasource is in write/update mode.
     *
     * @return true if opened for writing (via Create()), false if read-only (via Open()).
     */
    bool IsUpdateMode() const { return m_bUpdate; }

    /**
     * @brief Set a metadata item for the Polish Map header.
     *
     * Sets a key-value pair that will be written to the [IMG ID] section.
     * Standard keys include: Name, ID, CodePage, Datum, Elevation.
     *
     * @param pszName Metadata key name.
     * @param pszValue Metadata value.
     * @param pszDomain Metadata domain (ignored, always uses default domain).
     * @return CE_None on success, CE_Failure on error.
     */
    CPLErr SetMetadataItem(const char* pszName, const char* pszValue,
                           const char* pszDomain = nullptr) override;

    /**
     * @brief Get a metadata item from the Polish Map header.
     *
     * Retrieves a value from the [IMG ID] section metadata.
     *
     * @param pszName Metadata key name.
     * @param pszDomain Metadata domain (ignored, always uses default domain).
     * @return Value string, or nullptr if key not found.
     *
     * @note The returned string is owned by the dataset; do not free.
     */
    const char* GetMetadataItem(const char* pszName,
                                const char* pszDomain = nullptr) override;

    /**
     * @brief Get the raw metadata map for the Polish Map header.
     *
     * Provides direct access to the internal metadata storage, used by
     * PolishMapWriter to generate the [IMG ID] section during file write.
     *
     * @return Const reference to the metadata map (key → value pairs).
     *
     * @note Used internally by PolishMapWriter.
     */
    const std::map<std::string, std::string>& GetPolishMapMetadata() const { return m_aoMetadata; }

    /**
     * @brief Tech-spec #2: Is multi-geometry fields mode active?
     */
    bool IsMultiGeomFieldsEnabled() const { return m_bMultiGeomFields; }

    /**
     * @brief Tech-spec #2: Maximum Data index supported when multi-geom is active.
     *        Layer defn adds Data1..DataK as additional OGRGeomFieldDefn.
     */
    int GetMaxDataLevel() const { return m_nMaxDataLevel; }
};

#endif /* OGRPOLISHMAPDATASOURCE_H_INCLUDED */
