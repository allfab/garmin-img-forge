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
#include "cpl_conv.h"
#include "cpl_error.h"

/************************************************************************/
/*                      OGRPolishMapDataSource()                        */
/*                                                                      */
/* Story 1.3: Constructor creates the 3 empty layers.                   */
/************************************************************************/

OGRPolishMapDataSource::OGRPolishMapDataSource() {
    // Task 2.1: Create 3 layers in constructor
    CreateLayers();
}

/************************************************************************/
/*                     ~OGRPolishMapDataSource()                        */
/*                                                                      */
/* Story 1.3 Task 7: Layers are automatically cleaned up by unique_ptr. */
/************************************************************************/

OGRPolishMapDataSource::~OGRPolishMapDataSource() {
    // Task 7.1: unique_ptr automatically deletes layers (RAII)
    // No manual cleanup needed - m_apoLayers destructor handles it
}

/************************************************************************/
/*                          CreateLayers()                              */
/*                                                                      */
/* Story 1.3 Task 2: Create the 3 standard Polish Map layers.           */
/************************************************************************/

void OGRPolishMapDataSource::CreateLayers() {
    // Task 2.2: Create POI layer with wkbPoint geometry type (index 0)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POI", wkbPoint));

    // Task 2.3: Create POLYLINE layer with wkbLineString geometry type (index 1)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYLINE", wkbLineString));

    // Task 2.4: Create POLYGON layer with wkbPolygon geometry type (index 2)
    m_apoLayers.push_back(
        std::make_unique<OGRPolishMapLayer>("POLYGON", wkbPolygon));

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
    // ODsCCreateLayer - Not implemented yet (Story 2.x)
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
