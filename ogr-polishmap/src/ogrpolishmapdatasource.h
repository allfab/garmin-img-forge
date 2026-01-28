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
#include <vector>

class OGRPolishMapLayer;

/************************************************************************/
/*                       OGRPolishMapDataSource                         */
/************************************************************************/

class OGRPolishMapDataSource final : public GDALDataset {
private:
    std::vector<std::unique_ptr<OGRPolishMapLayer>> m_apoLayers;
    CPLString m_osFilename;

public:
    OGRPolishMapDataSource();
    ~OGRPolishMapDataSource() override;

    // GDALDataset pure virtual methods
    int GetLayerCount() override;
    OGRLayer* GetLayer(int nLayer) override;

    int TestCapability(const char* pszCap) override;
};

#endif /* OGRPOLISHMAPDATASOURCE_H_INCLUDED */
