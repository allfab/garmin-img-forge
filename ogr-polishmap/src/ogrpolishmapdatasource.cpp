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
    // Story 2.1: Write minimal header on close if in write mode
    if (m_bUpdate && m_fpOutput != nullptr) {
        // Writer should already exist (created in Create())
        // This is a defensive check - if somehow writer is null, we can't write
        if (m_poWriter == nullptr) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "Writer not initialized in write mode - cannot write header");
        } else {
            // Write header if not already written
            // TODO Story 2.2: Use metadata from dataset instead of hardcoded "Untitled"
            if (!m_poWriter->IsHeaderWritten()) {
                m_poWriter->WriteHeader("Untitled", "1252");
            }

            // Flush pending writes
            m_poWriter->Flush();
        }

        // Close output file
        VSIFCloseL(m_fpOutput);
        m_fpOutput = nullptr;

        CPLDebug("OGR_POLISHMAP", "Closed write-mode dataset");
    }

    // Task 7.1: unique_ptr automatically deletes layers (RAII)
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

    // Task 2.2: Create POI layer with wkbPoint geometry type (index 0)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POI", wkbPoint, poParser));

    // Task 2.3: Create POLYLINE layer with wkbLineString geometry type (index 1)
    // Note: POLYLINE not yet implemented - will be Story 1.5
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYLINE", wkbLineString, poParser));

    // Task 2.4: Create POLYGON layer with wkbPolygon geometry type (index 2)
    // Note: POLYGON not yet implemented - will be Story 1.6
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYGON", wkbPolygon, poParser));

    // Task 2.5: Layers stored in m_apoLayers vector (3 layers total)
    CPLDebug("OGR_POLISHMAP", "Created %d layers: POI, POLYLINE, POLYGON",
             static_cast<int>(m_apoLayers.size()));
}

/************************************************************************/
/*                           GetLayerCount()                            */
/*                                                                      */
/* Story 1.3 Task 3: Return the number of layers (should be 3).         */
/************************************************************************/

int OGRPolishMapDataSource::GetLayerCount() {
    // Task 3.1: Return size of m_apoLayers vector
    return static_cast<int>(m_apoLayers.size());
}

/************************************************************************/
/*                             GetLayer()                               */
/*                                                                      */
/* Story 1.3 Task 4: Return layer by index with bounds checking (FR24). */
/************************************************************************/

OGRLayer* OGRPolishMapDataSource::GetLayer(int nLayer) {
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

int OGRPolishMapDataSource::TestCapability(const char* pszCap) {
    // Task 5.1: Support ODsCRandomLayerRead capability
    if (EQUAL(pszCap, ODsCRandomLayerRead)) {
        return TRUE;  // Can read layers in random order
    }
    // Story 2.1: ODsCCreateLayer - FALSE always
    // Polish Map format has fixed layers (POI, POLYLINE, POLYGON)
    // created automatically by Create(). User cannot create custom layers.
    if (EQUAL(pszCap, ODsCCreateLayer)) {
        return FALSE;
    }
    // ODsCDeleteLayer - Not implemented
    if (EQUAL(pszCap, ODsCDeleteLayer)) {
        return FALSE;
    }
    // Task 5.2: Default - capability not supported
    return FALSE;
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
    // Layers will be write-only until Story 2.3-2.5 implement CreateFeature

    // Task 2.2: Create POI layer with wkbPoint geometry type (index 0)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POI", wkbPoint, nullptr));

    // Task 2.3: Create POLYLINE layer with wkbLineString geometry type (index 1)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYLINE", wkbLineString, nullptr));

    // Task 2.4: Create POLYGON layer with wkbPolygon geometry type (index 2)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYGON", wkbPolygon, nullptr));

    CPLDebug("OGR_POLISHMAP", "Created %d layers for write mode: POI, POLYLINE, POLYGON",
             static_cast<int>(m_apoLayers.size()));
}

/************************************************************************/
/*                              Create()                                 */
/*                                                                      */
/* Story 2.1: Factory method for creating new Polish Map dataset.       */
/* Opens file for writing, creates 3 empty layers.                       */
/************************************************************************/

OGRPolishMapDataSource* OGRPolishMapDataSource::Create(const char* pszFilename) {
    // Validate input - NULL path is invalid
    if (pszFilename == nullptr) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Cannot create Polish Map file: filename is NULL");
        return nullptr;
    }

    // Task 1.3: Create file with VSIFOpenL() in mode "w" (overwrites if exists)
    VSILFILE* fp = VSIFOpenL(pszFilename, "w");
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

    // Task 2.3: Create 3 empty layers for write mode
    poDS->CreateLayersForWriteMode();

    // Create writer instance (will be used on close or when writing features)
    poDS->m_poWriter = std::make_unique<PolishMapWriter>(fp);

    CPLDebug("OGR_POLISHMAP", "Created new Polish Map dataset for writing: %s",
             pszFilename);

    return poDS;
}
