/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  DataSource for Garmin IMG format (read + write)
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

#ifndef OGRGARMINIMGDATASOURCE_H_INCLUDED
#define OGRGARMINIMGDATASOURCE_H_INCLUDED

#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "ogrgarminimg_compat.h"
#include "ogrgarminimglayer.h"
#include "garminimgfilesystem.h"
#include "garminimgtreparser.h"
#include "garminimgrgnparser.h"
#include "garminimglblparser.h"
#include "garminimgnetparser.h"
#include "garminimgnodparser.h"
#include "garminimgtypparser.h"

#include <memory>
#include <string>
#include <vector>

/************************************************************************/
/*                     TileParserSet                                    */
/************************************************************************/

struct TileParserSet {
    std::string osTileName;
    std::unique_ptr<GarminIMGTREParser> poTRE;
    std::unique_ptr<GarminIMGRGNParser> poRGN;
    std::unique_ptr<GarminIMGLBLParser> poLBL;
    std::unique_ptr<GarminIMGNETParser> poNET;
    std::unique_ptr<GarminIMGNODParser> poNOD;
};

/************************************************************************/
/*                     OGRGarminIMGDataSource                           */
/************************************************************************/

class OGRGarminIMGDataSource final : public GDALDataset {
public:
    OGRGarminIMGDataSource();
    ~OGRGarminIMGDataSource() override;

    bool Open(GDALOpenInfo* poOpenInfo);
    static OGRGarminIMGDataSource* Create(const char* pszFilename,
                                          char** papszOptions);

    int GetLayerCount() OGRGARMINIMG_CONST override;
    OGRLayer* GetLayer(int nLayer) OGRGARMINIMG_CONST override;
    int TestCapability(const char* pszCap) OGRGARMINIMG_CONST override;

    OGRLayer* ICreateLayer(const char* pszName,
                           const OGRGeomFieldDefn* poGeomFieldDefn,
                           CSLConstList papszOptions) override;

    bool IsUpdateMode() const { return m_bUpdate; }

    // Access for layers
    const std::vector<TileParserSet>& GetTiles() const { return m_aoTiles; }
    GarminIMGTYPParser* GetTYPParser() const { return m_poTYPParser.get(); }
    int GetTileCount() const { return static_cast<int>(m_aoTiles.size()); }

private:
    std::unique_ptr<GarminIMGFilesystem> m_poFilesystem;
    std::vector<TileParserSet> m_aoTiles;
    std::unique_ptr<GarminIMGTYPParser> m_poTYPParser;

    std::vector<std::unique_ptr<OGRGarminIMGLayer>> m_apoLayers;
    bool m_bUpdate = false;
    bool m_bHasRouting = false;

    void CreateReadLayers();
};

#endif /* OGRGARMINIMGDATASOURCE_H_INCLUDED */
