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
#include "cpl_conv.h"

/************************************************************************/
/*                        OGRPolishMapLayer()                           */
/************************************************************************/

OGRPolishMapLayer::OGRPolishMapLayer(const char* pszLayerName)
    : m_poFeatureDefn(new OGRFeatureDefn(pszLayerName)), m_nNextFID(1) {
    SetDescription(pszLayerName);
    m_poFeatureDefn->Reference();
}

/************************************************************************/
/*                       ~OGRPolishMapLayer()                           */
/************************************************************************/

OGRPolishMapLayer::~OGRPolishMapLayer() {
    if (m_poFeatureDefn != nullptr) {
        m_poFeatureDefn->Release();
    }
}

/************************************************************************/
/*                          ResetReading()                              */
/************************************************************************/

void OGRPolishMapLayer::ResetReading() {
    // Reset feature ID counter
    // Full implementation will be added when reading features (future story)
    m_nNextFID = 0;
}

/************************************************************************/
/*                         GetNextFeature()                             */
/************************************************************************/

OGRFeature* OGRPolishMapLayer::GetNextFeature() {
    // Stub implementation - returns nullptr
    // Feature reading will be implemented in subsequent stories
    return nullptr;
}

/************************************************************************/
/*                          GetLayerDefn()                              */
/************************************************************************/

OGRFeatureDefn* OGRPolishMapLayer::GetLayerDefn() {
    return m_poFeatureDefn;
}

/************************************************************************/
/*                         TestCapability()                             */
/************************************************************************/

int OGRPolishMapLayer::TestCapability(const char* pszCap) {
    // For now, report no capabilities
    // Will be implemented in subsequent stories
    CPL_IGNORE_RET_VAL(pszCap);
    return FALSE;
}
