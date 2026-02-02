/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Implementation of OGRPolishMapLayer class
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

#include "ogrpolishmaplayer.h"
#include "polishmapparser.h"
#include "polishmapwriter.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include <cassert>
#include <cmath>

/************************************************************************/
/*                        OGRPolishMapLayer()                           */
/*                                                                      */
/* Story 1.3: Constructor without parser (for testing/legacy use).      */
/************************************************************************/

OGRPolishMapLayer::OGRPolishMapLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType)
    : m_poFeatureDefn(nullptr), m_poSRS(nullptr), m_nNextFID(1),
      m_poParser(nullptr), m_osLayerType(pszLayerName), m_bEOF(false),
      m_bReaderInitialized(false), m_bWriteMode(false), m_poWriter(nullptr) {
    InitializeLayerDefn(pszLayerName, eGeomType);
}

/************************************************************************/
/*                     OGRPolishMapLayer() with parser                  */
/*                                                                      */
/* Story 1.4: Constructor that accepts parser for feature reading.      */
/************************************************************************/

OGRPolishMapLayer::OGRPolishMapLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType,
                                     PolishMapParser* poParser)
    : m_poFeatureDefn(nullptr), m_poSRS(nullptr), m_nNextFID(1),
      m_poParser(poParser), m_osLayerType(pszLayerName), m_bEOF(false),
      m_bReaderInitialized(false), m_bWriteMode(false), m_poWriter(nullptr) {
    InitializeLayerDefn(pszLayerName, eGeomType);
}

/************************************************************************/
/*                       InitializeLayerDefn()                          */
/*                                                                      */
/* Common initialization for feature definition, SRS, and fields.       */
/* Called by both constructors to avoid code duplication.               */
/*                                                                      */
/* Note: GDAL convention prohibits exceptions. Memory allocations       */
/* with 'new' will terminate on failure (std::terminate). This is       */
/* acceptable per GDAL driver conventions - no exception safety needed. */
/************************************************************************/

void OGRPolishMapLayer::InitializeLayerDefn(const char* pszLayerName,
                                            OGRwkbGeometryType eGeomType) {
    // Set layer description
    SetDescription(pszLayerName);

    // Create feature definition with layer name
    m_poFeatureDefn = new OGRFeatureDefn(pszLayerName);
    m_poFeatureDefn->Reference();  // MANDATORY ref count increment

    // Set geometry type
    m_poFeatureDefn->SetGeomType(eGeomType);

    // Create and assign WGS84 spatial reference
    m_poSRS = new OGRSpatialReference();
    if (m_poSRS->SetWellKnownGeogCS("WGS84") != OGRERR_NONE) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "Failed to set WGS84 coordinate system");
    }
    // GDAL 3.x: Traditional GIS order (lon, lat)
    m_poSRS->SetAxisMappingStrategy(OAMS_TRADITIONAL_GIS_ORDER);
    m_poFeatureDefn->GetGeomFieldDefn(0)->SetSpatialRef(m_poSRS);

    // Add standard field definitions (FR38)
    // Type: Garmin type code (e.g., "0x2C00")
    OGRFieldDefn oFieldType("Type", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldType);

    // Label: Feature name/label
    OGRFieldDefn oFieldLabel("Label", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldLabel);

    // Data0: Numeric data field (unused for POI, coordinates in geometry)
    OGRFieldDefn oFieldData0("Data0", OFTInteger);
    m_poFeatureDefn->AddFieldDefn(&oFieldData0);

    // EndLevel: Maximum zoom level (0-9)
    OGRFieldDefn oFieldEndLevel("EndLevel", OFTInteger);
    m_poFeatureDefn->AddFieldDefn(&oFieldEndLevel);

    // Levels: Level range string (e.g., "0-3")
    OGRFieldDefn oFieldLevels("Levels", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldLevels);
}

/************************************************************************/
/*                       ~OGRPolishMapLayer()                           */
/*                                                                      */
/* Story 1.3: Proper cleanup with Release() for ref-counted objects.    */
/************************************************************************/

OGRPolishMapLayer::~OGRPolishMapLayer() {
    // Task 1.5: Release OGRFeatureDefn (decrements ref count, frees if 0)
    if (m_poFeatureDefn != nullptr) {
        m_poFeatureDefn->Release();
    }
    // Task 1.6: Release spatial reference
    if (m_poSRS != nullptr) {
        m_poSRS->Release();
    }
}

/************************************************************************/
/*                          ResetReading()                              */
/*                                                                      */
/* Story 1.4: Reset file position and FID counter for POI.              */
/* Story 1.5: Add POLYLINE support.                                     */
/* Note: All layer types use lazy initialization via m_bReaderInitialized*/
/************************************************************************/

void OGRPolishMapLayer::ResetReading() {
    // Reset feature ID counter to 1 (FID starts at 1 per architecture)
    m_nNextFID = 1;

    // Reset EOF flag and force re-initialization on next read
    m_bEOF = false;
    m_bReaderInitialized = false;  // Force re-seek on next GetNextFeature()

    // Note: Actual seek happens lazily in GetNextPOIFeature(), GetNextPolylineFeature(),
    // or GetNextPolygonFeature() when m_bReaderInitialized is false. This avoids redundant seeks.
}

/************************************************************************/
/*                         GetNextFeature()                             */
/*                                                                      */
/* Story 1.4: Read POI features from parser.                            */
/* Story 1.5: Add POLYLINE support with dispatch pattern.               */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextFeature() {
    // Check preconditions
    if (m_poParser == nullptr || m_bEOF) {
        return nullptr;
    }

    // Dispatch based on layer type
    if (m_osLayerType == "POI") {
        return GetNextPOIFeature();
    } else if (m_osLayerType == "POLYLINE") {
        return GetNextPolylineFeature();
    } else if (m_osLayerType == "POLYGON") {
        return GetNextPolygonFeature();
    }

    return nullptr;
}

/************************************************************************/
/*                       GetNextPOIFeature()                            */
/*                                                                      */
/* Story 1.4: Read POI features from parser.                            */
/* Story 1.5: Extracted from GetNextFeature() for dispatch pattern.     */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextPOIFeature() {
    // LEÇON 1.4: Assert parser is valid before use
    assert(m_poParser != nullptr);

    // First call: reset parser to start of POI sections
    if (!m_bReaderInitialized) {
        m_poParser->ResetPOIReading();
        m_bReaderInitialized = true;
    }

    PolishMapPOISection oSection;
    while (m_poParser->ParseNextPOI(oSection)) {
        // Create feature from section
        OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

        // Set FID (sequential, starts at 1)
        poFeature->SetFID(m_nNextFID++);

        // Create point geometry (lon = X, lat = Y)
        OGRPoint* poPoint = new OGRPoint(oSection.oCoords.second,  // lon = X
                                          oSection.oCoords.first);   // lat = Y
        poPoint->assignSpatialReference(m_poSRS);
        poFeature->SetGeometryDirectly(poPoint);

        // Set fields
        poFeature->SetField("Type", oSection.osType.c_str());
        poFeature->SetField("Label", oSection.osLabel.c_str());

        // Data0: For POI, coordinates are in geometry, not this field.
        // Field kept for schema consistency across all layer types.
        poFeature->SetField("Data0", 0);

        if (oSection.nEndLevel >= 0) {
            poFeature->SetField("EndLevel", oSection.nEndLevel);
        }

        if (!oSection.osLevels.empty()) {
            poFeature->SetField("Levels", oSection.osLevels.c_str());
        }

        // Apply spatial and attribute filters (inherited from OGRLayer)
        if ((m_poFilterGeom == nullptr || FilterGeometry(poFeature->GetGeomFieldRef(0))) &&
            (m_poAttrQuery == nullptr || m_poAttrQuery->Evaluate(poFeature))) {
            return poFeature;  // Ownership transferred to caller
        }

        // Feature filtered out, delete and try next
        delete poFeature;
    }

    m_bEOF = true;
    return nullptr;
}

/************************************************************************/
/*                      GetNextPolylineFeature()                        */
/*                                                                      */
/* Story 1.5: Read POLYLINE features from parser.                       */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextPolylineFeature() {
    // LEÇON 1.4: Assert parser is valid before use
    assert(m_poParser != nullptr);

    // First call: reset parser to start of POLYLINE sections
    if (!m_bReaderInitialized) {
        m_poParser->ResetPolylineReading();
        m_bReaderInitialized = true;
    }

    PolishMapPolylineSection oSection;
    while (m_poParser->ParseNextPolyline(oSection)) {
        // Create feature from section
        OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

        // Set FID (sequential, starts at 1)
        poFeature->SetFID(m_nNextFID++);

        // Create LineString geometry with N points
        OGRLineString* poLine = new OGRLineString();
        for (const auto& coord : oSection.aoCoords) {
            // CRITICAL: OGR uses (X=lon, Y=lat) order, NOT (lat, lon)!
            poLine->addPoint(coord.second, coord.first);  // lon, lat
        }
        poLine->assignSpatialReference(m_poSRS);
        poFeature->SetGeometryDirectly(poLine);

        // Set fields
        poFeature->SetField("Type", oSection.osType.c_str());
        poFeature->SetField("Label", oSection.osLabel.c_str());

        // Data0-N: For POLYLINE, coordinates from Data0..DataN are stored in
        // the LineString geometry, not in individual fields. Field Data0 kept
        // at 0 for schema consistency across all layer types.
        poFeature->SetField("Data0", 0);

        if (oSection.nEndLevel >= 0) {
            poFeature->SetField("EndLevel", oSection.nEndLevel);
        }

        if (!oSection.osLevels.empty()) {
            poFeature->SetField("Levels", oSection.osLevels.c_str());
        }

        // Apply spatial and attribute filters (inherited from OGRLayer)
        if ((m_poFilterGeom == nullptr || FilterGeometry(poFeature->GetGeomFieldRef(0))) &&
            (m_poAttrQuery == nullptr || m_poAttrQuery->Evaluate(poFeature))) {
            return poFeature;  // Ownership transferred to caller
        }

        // Feature filtered out, delete and try next
        delete poFeature;
    }

    m_bEOF = true;
    return nullptr;
}

/************************************************************************/
/*                      GetNextPolygonFeature()                         */
/*                                                                      */
/* Story 1.6: Read POLYGON features from parser.                        */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextPolygonFeature() {
    // LEÇON 1.4: Assert parser is valid before use
    assert(m_poParser != nullptr);

    // First call: reset parser to start of POLYGON sections
    if (!m_bReaderInitialized) {
        m_poParser->ResetPolygonReading();
        m_bReaderInitialized = true;
    }

    PolishMapPolygonSection oSection;
    while (m_poParser->ParseNextPolygon(oSection)) {
        // Create feature from section
        OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

        // Set FID (sequential, starts at 1)
        poFeature->SetFID(m_nNextFID++);

        // Create Polygon geometry with exterior ring
        OGRPolygon* poPolygon = new OGRPolygon();
        OGRLinearRing* poRing = new OGRLinearRing();

        for (const auto& coord : oSection.aoCoords) {
            // CRITICAL: OGR uses (X=lon, Y=lat) order, NOT (lat, lon)!
            poRing->addPoint(coord.second, coord.first);  // lon, lat
        }

        // Check if ring is closed (AC4: auto-close open ring)
        // Use class constant RING_CLOSURE_TOLERANCE for comparison
        if (oSection.aoCoords.size() >= 3) {
            const auto& firstPt = oSection.aoCoords.front();
            const auto& lastPt = oSection.aoCoords.back();

            if (std::abs(firstPt.first - lastPt.first) > RING_CLOSURE_TOLERANCE ||
                std::abs(firstPt.second - lastPt.second) > RING_CLOSURE_TOLERANCE) {
                // Ring is open - auto-close with debug log (Minor Issue per Architecture)
                CPLDebug("OGR_POLISHMAP", "Auto-closing POLYGON ring");
                poRing->addPoint(firstPt.second, firstPt.first);  // lon, lat
            }
        }

        // Add ring to polygon (no closeRings() call - we handle closure manually above)
        poPolygon->addRingDirectly(poRing);
        poPolygon->assignSpatialReference(m_poSRS);
        poFeature->SetGeometryDirectly(poPolygon);

        // Set fields
        poFeature->SetField("Type", oSection.osType.c_str());
        poFeature->SetField("Label", oSection.osLabel.c_str());

        // Data0-N: For POLYGON, coordinates from Data0..DataN are stored in
        // the Polygon geometry ring, not in individual fields. Field Data0 kept
        // at 0 for schema consistency across all layer types.
        poFeature->SetField("Data0", 0);

        if (oSection.nEndLevel >= 0) {
            poFeature->SetField("EndLevel", oSection.nEndLevel);
        }

        if (!oSection.osLevels.empty()) {
            poFeature->SetField("Levels", oSection.osLevels.c_str());
        }

        // Apply spatial and attribute filters (inherited from OGRLayer)
        if ((m_poFilterGeom == nullptr || FilterGeometry(poFeature->GetGeomFieldRef(0))) &&
            (m_poAttrQuery == nullptr || m_poAttrQuery->Evaluate(poFeature))) {
            return poFeature;  // Ownership transferred to caller
        }

        // Feature filtered out, delete and try next
        delete poFeature;
    }

    m_bEOF = true;
    return nullptr;
}

/************************************************************************/
/*                          GetLayerDefn()                              */
/************************************************************************/

OGRFeatureDefn* OGRPolishMapLayer::GetLayerDefn() {
    // M5 Fix: Assert that feature definition exists (should never be null)
    assert(m_poFeatureDefn != nullptr);
    return m_poFeatureDefn;
}

/************************************************************************/
/*                         TestCapability()                             */
/*                                                                      */
/* Story 1.3 Task 6: Report layer capabilities.                         */
/************************************************************************/

int OGRPolishMapLayer::TestCapability(const char* pszCap) {
    // Task 6.1: OLCRandomRead - GetFeature(FID) not implemented (Post-MVP)
    if (EQUAL(pszCap, OLCRandomRead)) {
        return FALSE;
    }
    // Story 2.3 Task 4: OLCSequentialWrite - TRUE only for POI in write mode
    // POLYLINE/POLYGON writing not yet implemented (Stories 2.4/2.5)
    if (EQUAL(pszCap, OLCSequentialWrite)) {
        if (m_bWriteMode && m_osLayerType == "POI") {
            return TRUE;
        }
        return FALSE;
    }
    // Story 2.3 Task 4.2: OLCRandomWrite - SetFeature() not supported
    if (EQUAL(pszCap, OLCRandomWrite)) {
        return FALSE;
    }
    // Task 6.3: OLCFastFeatureCount - No optimization yet
    if (EQUAL(pszCap, OLCFastFeatureCount)) {
        return FALSE;
    }
    // Story 1.7: OLCFastSpatialFilter - No spatial index, client-side linear filtering
    // Filters work via inherited OGRLayer methods but without acceleration
    if (EQUAL(pszCap, OLCFastSpatialFilter)) {
        return FALSE;
    }
    // Default: capability not supported
    return FALSE;
}

/************************************************************************/
/*                            SetWriter()                               */
/*                                                                      */
/* Story 2.3 Task 1.3: Connect writer for write mode.                   */
/************************************************************************/

void OGRPolishMapLayer::SetWriter(PolishMapWriter* poWriter) {
    m_poWriter = poWriter;
    m_bWriteMode = (poWriter != nullptr);

    CPLDebug("OGR_POLISHMAP", "Layer %s: SetWriter(%s), write mode = %s",
             m_osLayerType.c_str(),
             poWriter ? "writer" : "nullptr",
             m_bWriteMode ? "true" : "false");
}

/************************************************************************/
/*                          ICreateFeature()                             */
/*                                                                      */
/* Story 2.3 Task 1.4-1.5: Override ICreateFeature for POI writing.     */
/* GDAL convention: Override ICreateFeature(), NOT CreateFeature().      */
/************************************************************************/

OGRErr OGRPolishMapLayer::ICreateFeature(OGRFeature* poFeature) {
    // Task 1.5: Validate NULL feature
    if (poFeature == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "ICreateFeature: NULL feature pointer");
        return OGRERR_FAILURE;
    }

    // Validate write mode
    if (!m_bWriteMode || m_poWriter == nullptr) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "ICreateFeature: Layer not in write mode or writer not set");
        return OGRERR_FAILURE;
    }

    // Dispatch based on layer type
    if (m_osLayerType == "POI") {
        if (!m_poWriter->WritePOI(poFeature)) {
            return OGRERR_FAILURE;
        }
    } else if (m_osLayerType == "POLYLINE") {
        // Story 2.4: POLYLINE writing not implemented yet
        CPLError(CE_Failure, CPLE_NotSupported,
                 "ICreateFeature: POLYLINE writing not implemented yet");
        return OGRERR_UNSUPPORTED_OPERATION;
    } else if (m_osLayerType == "POLYGON") {
        // Story 2.5: POLYGON writing not implemented yet
        CPLError(CE_Failure, CPLE_NotSupported,
                 "ICreateFeature: POLYGON writing not implemented yet");
        return OGRERR_UNSUPPORTED_OPERATION;
    } else {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "ICreateFeature: Unknown layer type: %s", m_osLayerType.c_str());
        return OGRERR_FAILURE;
    }

    // Set FID for the feature (sequential)
    poFeature->SetFID(m_nNextFID++);

    return OGRERR_NONE;
}
