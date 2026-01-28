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
/************************************************************************/

OGRPolishMapDataSource::OGRPolishMapDataSource() {
    // Empty constructor - initialization happens in Open() (future story)
}

/************************************************************************/
/*                     ~OGRPolishMapDataSource()                        */
/************************************************************************/

OGRPolishMapDataSource::~OGRPolishMapDataSource() {
    // Layers are automatically cleaned up by unique_ptr
}

/************************************************************************/
/*                           GetLayerCount()                            */
/************************************************************************/

int OGRPolishMapDataSource::GetLayerCount() {
    return static_cast<int>(m_apoLayers.size());
}

/************************************************************************/
/*                             GetLayer()                               */
/************************************************************************/

OGRLayer* OGRPolishMapDataSource::GetLayer(int nLayer) {
    if (nLayer < 0 || nLayer >= GetLayerCount()) {
        return nullptr;
    }

    return m_apoLayers[nLayer].get();
}

/************************************************************************/
/*                          TestCapability()                            */
/************************************************************************/

int OGRPolishMapDataSource::TestCapability(const char* pszCap) {
    // For now, report no capabilities
    // Will be implemented in subsequent stories
    CPL_IGNORE_RET_VAL(pszCap);
    return FALSE;
}
