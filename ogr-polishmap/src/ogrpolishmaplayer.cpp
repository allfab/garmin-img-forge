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
#include "cpl_conv.h"
#include "cpl_error.h"
#include <cassert>

/************************************************************************/
/*                        OGRPolishMapLayer()                           */
/*                                                                      */
/* Story 1.3: Full constructor with geometry type, field definitions,   */
/* and WGS84 spatial reference.                                         */
/*                                                                      */
/* M1 Note: GDAL convention prohibits exceptions. Memory allocations    */
/* with 'new' will terminate on failure (std::terminate). This is       */
/* acceptable per GDAL driver conventions - no exception safety needed. */
/************************************************************************/

OGRPolishMapLayer::OGRPolishMapLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType)
    : m_poFeatureDefn(nullptr), m_poSRS(nullptr), m_nNextFID(1),
      m_poParser(nullptr), m_osLayerType(pszLayerName), m_bEOF(false),
      m_bReaderInitialized(false) {

    // Set layer description
    SetDescription(pszLayerName);

    // Create feature definition with layer name (Task 1.2)
    m_poFeatureDefn = new OGRFeatureDefn(pszLayerName);
    m_poFeatureDefn->Reference();  // Task 1.5: MANDATORY ref count increment

    // Set geometry type (Task 1.3)
    m_poFeatureDefn->SetGeomType(eGeomType);

    // Create and assign WGS84 spatial reference (Task 1.6, FR40)
    m_poSRS = new OGRSpatialReference();
    // M2 Fix: Check return value of SetWellKnownGeogCS
    if (m_poSRS->SetWellKnownGeogCS("WGS84") != OGRERR_NONE) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "Failed to set WGS84 coordinate system");
    }
    // GDAL 3.x: Traditional GIS order (lon, lat)
    m_poSRS->SetAxisMappingStrategy(OAMS_TRADITIONAL_GIS_ORDER);
    // Note: SRS is set on GeomFieldDefn. When creating features (Story 1.4+),
    // call OGRGeometry::assignSpatialReference(m_poSRS) on each geometry.
    m_poFeatureDefn->GetGeomFieldDefn(0)->SetSpatialRef(m_poSRS);

    // Add standard field definitions (Task 1.4, FR38)
    // Type: Garmin type code (e.g., "0x2C00")
    OGRFieldDefn oFieldType("Type", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldType);

    // Label: Feature name/label
    OGRFieldDefn oFieldLabel("Label", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldLabel);

    // Data0: Numeric data field
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
/*                     OGRPolishMapLayer() with parser                  */
/*                                                                      */
/* Story 1.4: Constructor that accepts parser for feature reading.      */
/************************************************************************/

OGRPolishMapLayer::OGRPolishMapLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType,
                                     PolishMapParser* poParser)
    : m_poFeatureDefn(nullptr), m_poSRS(nullptr), m_nNextFID(1),
      m_poParser(poParser), m_osLayerType(pszLayerName), m_bEOF(false),
      m_bReaderInitialized(false) {

    // Set layer description
    SetDescription(pszLayerName);

    // Create feature definition with layer name
    m_poFeatureDefn = new OGRFeatureDefn(pszLayerName);
    m_poFeatureDefn->Reference();

    // Set geometry type
    m_poFeatureDefn->SetGeomType(eGeomType);

    // Create and assign WGS84 spatial reference
    m_poSRS = new OGRSpatialReference();
    if (m_poSRS->SetWellKnownGeogCS("WGS84") != OGRERR_NONE) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "Failed to set WGS84 coordinate system");
    }
    m_poSRS->SetAxisMappingStrategy(OAMS_TRADITIONAL_GIS_ORDER);
    m_poFeatureDefn->GetGeomFieldDefn(0)->SetSpatialRef(m_poSRS);

    // Add standard field definitions
    OGRFieldDefn oFieldType("Type", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldType);

    OGRFieldDefn oFieldLabel("Label", OFTString);
    m_poFeatureDefn->AddFieldDefn(&oFieldLabel);

    OGRFieldDefn oFieldData0("Data0", OFTInteger);
    m_poFeatureDefn->AddFieldDefn(&oFieldData0);

    OGRFieldDefn oFieldEndLevel("EndLevel", OFTInteger);
    m_poFeatureDefn->AddFieldDefn(&oFieldEndLevel);

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
/* Story 1.4: Reset file position and FID counter.                      */
/************************************************************************/

void OGRPolishMapLayer::ResetReading() {
    // Reset feature ID counter to 1 (FID starts at 1 per architecture)
    m_nNextFID = 1;

    // Story 1.4: Reset EOF flag and parser position
    m_bEOF = false;
    m_bReaderInitialized = false;  // Force re-initialization on next read
    if (m_poParser != nullptr && m_osLayerType == "POI") {
        m_poParser->ResetPOIReading();
    }
}

/************************************************************************/
/*                         GetNextFeature()                             */
/*                                                                      */
/* Story 1.4: Read POI features from parser.                            */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextFeature() {
    // Story 1.4: Only POI layer is implemented
    if (m_osLayerType != "POI" || m_poParser == nullptr || m_bEOF) {
        return nullptr;
    }

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

        // Data0 is not really used for POI (coordinates are in geometry)
        // but keep it for compatibility
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
    // Task 6.2: OLCSequentialWrite - CreateFeature() not implemented (Story 2.x)
    if (EQUAL(pszCap, OLCSequentialWrite)) {
        return FALSE;
    }
    // Task 6.3: OLCFastFeatureCount - No optimization yet
    if (EQUAL(pszCap, OLCFastFeatureCount)) {
        return FALSE;
    }
    // Default: capability not supported
    return FALSE;
}
