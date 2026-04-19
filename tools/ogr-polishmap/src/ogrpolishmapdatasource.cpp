/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Implementation of OGRPolishMapDataSource class
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

#include "ogrpolishmapdatasource.h"
#include "ogrpolishmaplayer.h"
#include "polishmapwriter.h"
#include "cpl_conv.h"
#include "cpl_error.h"

/************************************************************************/
/*                      OGRPolishMapDataSource()                        */
/*                                                                      */
/* Story 1.3: Constructor creates the 3 empty layers.                   */
/************************************************************************/

OGRPolishMapDataSource::OGRPolishMapDataSource()
    : m_bUpdate(false)
    , m_fpOutput(nullptr)
{
    // Story 1.4: Don't create layers yet - wait for parser to be set
    // Layers will be created in SetParser() after parser is available
    // Story 2.1: Or in CreateLayersForWriteMode() for write mode
}

/************************************************************************/
/*                     ~OGRPolishMapDataSource()                        */
/*                                                                      */
/* Story 1.3 Task 7: Layers are automatically cleaned up by unique_ptr. */
/************************************************************************/

OGRPolishMapDataSource::~OGRPolishMapDataSource() {
    // Story 2.1/2.2: Write header on close if in write mode
    if (m_bUpdate && m_fpOutput != nullptr) {
        // Writer should already exist (created in Create())
        // This is a defensive check - if somehow writer is null, we can't write
        if (m_poWriter == nullptr) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "Writer not initialized in write mode - cannot write header");
        } else {
            // Write header if not already written
            // Story 2.2: Use metadata from dataset
            if (!m_poWriter->IsHeaderWritten()) {
                if (m_aoMetadata.empty()) {
                    // No metadata set - use defaults
                    m_aoMetadata["Name"] = "Untitled";
                    m_aoMetadata["CodePage"] = "1252";
                }
                m_poWriter->WriteHeader(m_aoMetadata);
            }

            // Flush pending writes
            m_poWriter->Flush();
        }

        // Close output file
        VSIFCloseL(m_fpOutput);
        m_fpOutput = nullptr;

        CPLDebug("OGR_POLISHMAP", "Closed write-mode dataset");
    }

    // Layers cleanup: unique_ptr automatically deletes layers (RAII)
    // No manual cleanup needed - m_apoLayers destructor handles it
}

/************************************************************************/
/*                          CreateLayers()                              */
/*                                                                      */
/* Story 1.3 Task 2: Create the 3 standard Polish Map layers.           */
/************************************************************************/

void OGRPolishMapDataSource::CreateLayers() {
    // Story 1.4: Pass parser to layers for feature reading
    PolishMapParser* poParser = m_poParser.get();

    // Task 2.2: Create POI layer with wkbPoint geometry type (index 0).
    // Tech-spec #2 Task 4: POI stays mono-geom (MP spec §4.4.3.1).
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POI", wkbPoint, poParser));

    // Task 2.3: Create POLYLINE layer with wkbLineString geometry type (index 1)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYLINE", wkbLineString, poParser,
                                            m_bMultiGeomFields, m_nMaxDataLevel));

    // Task 2.4: Create POLYGON layer with wkbPolygon geometry type (index 2)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYGON", wkbPolygon, poParser,
                                            m_bMultiGeomFields, m_nMaxDataLevel));

    // Task 2.5: Layers stored in m_apoLayers vector (3 layers total)
    CPLDebug("OGR_POLISHMAP", "Created %d layers: POI, POLYLINE, POLYGON",
             static_cast<int>(m_apoLayers.size()));
}

/************************************************************************/
/*                           GetLayerCount()                            */
/*                                                                      */
/* Story 1.3 Task 3: Return the number of layers (should be 3).         */
/************************************************************************/

int OGRPolishMapDataSource::GetLayerCount() OGRPOLISHMAP_CONST {
    // Task 3.1: Return size of m_apoLayers vector
    return static_cast<int>(m_apoLayers.size());
}

/************************************************************************/
/*                             GetLayer()                               */
/*                                                                      */
/* Story 1.3 Task 4: Return layer by index with bounds checking (FR24). */
/************************************************************************/

OGRLayer* OGRPolishMapDataSource::GetLayer(int nLayer) OGRPOLISHMAP_CONST {
    // Task 4.1, 4.2: Bounds checking (FR24)
    if (nLayer < 0 || nLayer >= static_cast<int>(m_apoLayers.size())) {
        return nullptr;  // Return NULL for invalid index
    }
    // Task 4.3: Return valid OGRLayer* pointer
    return m_apoLayers[nLayer].get();
}

/************************************************************************/
/*                          TestCapability()                            */
/*                                                                      */
/* Story 1.3 Task 5: Report dataset capabilities (FR25).                */
/************************************************************************/

int OGRPolishMapDataSource::TestCapability(const char* pszCap) OGRPOLISHMAP_CONST {
    // Task 5.1: Support ODsCRandomLayerRead capability
    if (EQUAL(pszCap, ODsCRandomLayerRead)) {
        return TRUE;  // Can read layers in random order
    }
    // Story 2.6: ODsCCreateLayer - TRUE in write mode for ogr2ogr compatibility
    // Polish Map format has fixed layers (POI, POLYLINE, POLYGON)
    // ICreateLayer() maps geometry type to the appropriate fixed layer
    if (EQUAL(pszCap, ODsCCreateLayer)) {
        return m_bUpdate ? TRUE : FALSE;
    }
    // ODsCDeleteLayer - Not implemented
    if (EQUAL(pszCap, ODsCDeleteLayer)) {
        return FALSE;
    }
    // Task 5.2: Default - capability not supported
    return FALSE;
}

/************************************************************************/
/*                          ICreateLayer()                              */
/*                                                                      */
/* Story 2.6: Map geometry type to fixed layers for ogr2ogr support.    */
/* Polish Map has fixed layers: POI (Point), POLYLINE (LineString),     */
/* POLYGON (Polygon). This method returns the appropriate layer based   */
/* on the requested geometry type, ignoring the layer name parameter.   */
/************************************************************************/

OGRLayer* OGRPolishMapDataSource::ICreateLayer(
    const char* /* pszName */,
    const OGRGeomFieldDefn* poGeomFieldDefn,
    CSLConstList /* papszOptions */) {

    // Only available in write mode
    if (!m_bUpdate) {
        CPLError(CE_Failure, CPLE_NotSupported,
                 "Cannot create layer in read-only mode");
        return nullptr;
    }

    // Get geometry type (default to wkbUnknown if no field definition)
    OGRwkbGeometryType eGType = wkbUnknown;
    if (poGeomFieldDefn != nullptr) {
        eGType = poGeomFieldDefn->GetType();
    }

    // Map geometry type to fixed layer
    // wkbFlatten removes 25D variants (wkbPoint25D -> wkbPoint)
    OGRwkbGeometryType eFlatType = wkbFlatten(eGType);

    int nLayerIndex = -1;
    switch (eFlatType) {
        case wkbPoint:
        case wkbMultiPoint:
            nLayerIndex = 0;  // POI layer
            CPLDebug("OGR_POLISHMAP", "ICreateLayer: Point geometry -> POI layer");
            break;

        case wkbLineString:
        case wkbMultiLineString:
            nLayerIndex = 1;  // POLYLINE layer
            CPLDebug("OGR_POLISHMAP", "ICreateLayer: LineString geometry -> POLYLINE layer");
            break;

        case wkbPolygon:
        case wkbMultiPolygon:
            nLayerIndex = 2;  // POLYGON layer
            CPLDebug("OGR_POLISHMAP", "ICreateLayer: Polygon geometry -> POLYGON layer");
            break;

        case wkbUnknown:
        case wkbGeometryCollection:
            // For unknown/mixed geometry, return POI layer as default
            // ogr2ogr will dispatch features by geometry type via CreateFeature
            nLayerIndex = 0;
            CPLDebug("OGR_POLISHMAP", "ICreateLayer: Unknown/mixed geometry -> POI layer (default)");
            break;

        default:
            CPLError(CE_Failure, CPLE_NotSupported,
                     "Geometry type %s not supported by PolishMap driver",
                     OGRGeometryTypeToName(eGType));
            return nullptr;
    }

    // Return the appropriate layer
    if (nLayerIndex >= 0 && nLayerIndex < GetLayerCount()) {
        return GetLayer(nLayerIndex);
    }

    return nullptr;
}

/************************************************************************/
/*                            SetParser()                               */
/*                                                                      */
/* Story 1.4: Set parser and create layers (must be done in this order)*/
/************************************************************************/

void OGRPolishMapDataSource::SetParser(std::unique_ptr<PolishMapParser> poParser) {
    m_poParser = std::move(poParser);
    // Now that parser is set, create the layers
    CreateLayers();
}

/************************************************************************/
/*                     CreateLayersForWriteMode()                        */
/*                                                                      */
/* Story 2.1 Task 2.3: Create 3 empty layers for write mode.            */
/* Similar to CreateLayers() but without parser dependency.              */
/************************************************************************/

void OGRPolishMapDataSource::CreateLayersForWriteMode() {
    // No parser in write mode - pass nullptr
    // Layers will be write-only

    // Task 2.2: Create POI layer with wkbPoint geometry type (index 0).
    // Tech-spec #2 Task 2: POI is always mono-geom, flags not propagated.
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POI", wkbPoint, nullptr));

    // Task 2.3: Create POLYLINE layer with wkbLineString geometry type (index 1)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYLINE", wkbLineString, nullptr,
                                            m_bMultiGeomFields, m_nMaxDataLevel));

    // Task 2.4: Create POLYGON layer with wkbPolygon geometry type (index 2)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYGON", wkbPolygon, nullptr,
                                            m_bMultiGeomFields, m_nMaxDataLevel));

    // Story 2.3 Task 3.1: Connect writer to layers AFTER writer is created
    // Note: Writer is created in Create() method, so layers need to be connected there
    // This method just creates the layers - connection happens in Create()

    CPLDebug("OGR_POLISHMAP", "Created %d layers for write mode: POI, POLYLINE, POLYGON",
             static_cast<int>(m_apoLayers.size()));
}

/************************************************************************/
/*                              Create()                                 */
/*                                                                      */
/* Story 2.1: Factory method for creating new Polish Map dataset.       */
/* Opens file for writing, creates 3 empty layers.                       */
/************************************************************************/

OGRPolishMapDataSource* OGRPolishMapDataSource::Create(const char* pszFilename,
                                                        char** papszOptions) {
    // Validate input - NULL path is invalid
    if (pszFilename == nullptr) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Cannot create Polish Map file: filename is NULL");
        return nullptr;
    }

    // Story 2.2.8 Task 2.2: Validate HEADER_TEMPLATE before creating dataset (AC2, AC3)
    const char* pszHeaderTemplate = CSLFetchNameValue(papszOptions, "HEADER_TEMPLATE");
    if (pszHeaderTemplate != nullptr && pszHeaderTemplate[0] != '\0') {
        // Check if template file exists
        VSIStatBufL sStat;
        if (VSIStatL(pszHeaderTemplate, &sStat) != 0) {
            CPLError(CE_Failure, CPLE_OpenFailed,
                     "HEADER_TEMPLATE file not found: %s", pszHeaderTemplate);
            return nullptr;
        }

        // Validate template has valid [IMG ID] section
        PolishMapParser oTemplateParser(pszHeaderTemplate);
        if (!oTemplateParser.IsOpen()) {
            CPLError(CE_Failure, CPLE_OpenFailed,
                     "Failed to open HEADER_TEMPLATE file: %s", pszHeaderTemplate);
            return nullptr;
        }
        if (!oTemplateParser.ParseHeader()) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "HEADER_TEMPLATE file has invalid [IMG ID] section: %s",
                     pszHeaderTemplate);
            return nullptr;
        }
        CPLDebug("OGR_POLISHMAP", "HEADER_TEMPLATE validated: %s", pszHeaderTemplate);
    }

    // Task 1.3: Create file with VSIFOpenL() in binary mode to preserve CP1252 bytes
    VSILFILE* fp = VSIFOpenL(pszFilename, "wb");
    if (fp == nullptr) {
        // Task 1.4: Return NULL + CPLError on failure
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Cannot create Polish Map file: %s", pszFilename);
        return nullptr;
    }

    // Task 1.5: Create OGRPolishMapDataSource in write mode
    OGRPolishMapDataSource* poDS = new OGRPolishMapDataSource();

    // Set write mode flag
    poDS->m_bUpdate = true;
    poDS->m_fpOutput = fp;

    // Set description (file path)
    poDS->SetDescription(pszFilename);

    // Story 2.2.8 Task 2.1: Store HEADER_TEMPLATE option in metadata (already validated above)
    if (pszHeaderTemplate != nullptr && pszHeaderTemplate[0] != '\0') {
        poDS->m_aoMetadata["HEADER_TEMPLATE"] = pszHeaderTemplate;
    }

    // Story 4.4 Task 3.2: Read FIELD_MAPPING option and initialize mapper
    const char* pszFieldMapping = CSLFetchNameValue(papszOptions, "FIELD_MAPPING");
    if (pszFieldMapping != nullptr && pszFieldMapping[0] != '\0') {
        poDS->m_poFieldMapper = std::make_unique<PolishMapFieldMapper>();
        if (!poDS->m_poFieldMapper->LoadConfig(pszFieldMapping)) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "Failed to load field mapping config: %s", pszFieldMapping);
            // Continue without field mapper (use hardcoded aliases)
            poDS->m_poFieldMapper.reset();
        } else {
            CPLDebug("OGR_POLISHMAP", "Field mapping config loaded: %s", pszFieldMapping);
        }
    }

    // Tech-spec #2 Task 2: Parse multi-geometry fields options (opt-in).
    // When enabled, POLYLINE/POLYGON layers expose additional OGRGeomFieldDefn
    // for Data1=, Data2=, ..., DataK= in the Polish Map output.
    // POI layer remains mono-geom per MP spec §4.4.3.1.
    const char* pszMultiGeom = CSLFetchNameValue(papszOptions, "MULTI_GEOM_FIELDS");
    if (pszMultiGeom != nullptr && CPLTestBool(pszMultiGeom)) {
        const char* pszMaxLevel = CSLFetchNameValue(papszOptions, "MAX_DATA_LEVEL");
        int nMaxLevel = (pszMaxLevel != nullptr) ? atoi(pszMaxLevel) : 4;
        if (nMaxLevel < 1 || nMaxLevel > 9) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "MAX_DATA_LEVEL must be in [1, 9], got %d", nMaxLevel);
            delete poDS;
            return nullptr;
        }
        poDS->m_bMultiGeomFields = true;
        poDS->m_nMaxDataLevel = nMaxLevel;
        CPLDebug("OGR_POLISHMAP",
                 "Multi-geometry fields enabled, MAX_DATA_LEVEL=%d", nMaxLevel);
    }

    // Story 2.3 Task 3.2: Create writer BEFORE layers (so we can connect them)
    poDS->m_poWriter = std::make_unique<PolishMapWriter>(fp);

    // Give writer access to datasource metadata (for HEADER_TEMPLATE support in safety net)
    poDS->m_poWriter->SetMetadata(&poDS->m_aoMetadata);

    // Task 2.3: Create 3 empty layers for write mode
    poDS->CreateLayersForWriteMode();

    // Story 2.3 Task 3.1: Connect writer to all layers
    PolishMapWriter* poWriter = poDS->m_poWriter.get();
    for (auto& poLayer : poDS->m_apoLayers) {
        poLayer->SetWriter(poWriter);

        // Story 4.4 Task 3.4: Pass field mapper to layers
        if (poDS->m_poFieldMapper) {
            poLayer->SetFieldMapper(poDS->m_poFieldMapper.get());
        }
    }

    CPLDebug("OGR_POLISHMAP", "Created new Polish Map dataset for writing: %s",
             pszFilename);

    return poDS;
}

/************************************************************************/
/*                         SetMetadataItem()                             */
/*                                                                      */
/* Story 2.2 Task 1: Store metadata for Polish Map header fields.        */
/* Overrides GDALMajorObject::SetMetadataItem().                         */
/************************************************************************/

CPLErr OGRPolishMapDataSource::SetMetadataItem(const char* pszName,
                                                const char* pszValue,
                                                const char* pszDomain) {
    // Polish Map metadata domain (nullptr, empty string, or "POLISHMAP")
    if (pszDomain == nullptr || EQUAL(pszDomain, "") || EQUAL(pszDomain, "POLISHMAP")) {
        if (pszName != nullptr && pszValue != nullptr) {
            m_aoMetadata[pszName] = pszValue;
            CPLDebug("OGR_POLISHMAP", "SetMetadataItem: %s=%s", pszName, pszValue);
            return CE_None;
        }
        return CE_Failure;
    }
    // Delegate to parent for other domains
    return GDALDataset::SetMetadataItem(pszName, pszValue, pszDomain);
}

/************************************************************************/
/*                         GetMetadataItem()                             */
/*                                                                      */
/* Story 2.2 Task 2: Retrieve metadata for Polish Map header fields.     */
/* Overrides GDALMajorObject::GetMetadataItem().                         */
/************************************************************************/

const char* OGRPolishMapDataSource::GetMetadataItem(const char* pszName,
                                                     const char* pszDomain) {
    // Polish Map metadata domain (nullptr, empty string, or "POLISHMAP")
    if (pszDomain == nullptr || EQUAL(pszDomain, "") || EQUAL(pszDomain, "POLISHMAP")) {
        if (pszName == nullptr) {
            return nullptr;
        }
        auto it = m_aoMetadata.find(pszName);
        if (it != m_aoMetadata.end()) {
            return it->second.c_str();
        }
        return nullptr;
    }
    // Delegate to parent for other domains
    return GDALDataset::GetMetadataItem(pszName, pszDomain);
}
