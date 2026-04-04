/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  DataSource implementation for Garmin IMG format
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

#include "ogrgarminimgdatasource.h"
#include "garminimgfields.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"

/************************************************************************/
/*                     OGRGarminIMGDataSource()                         */
/************************************************************************/

OGRGarminIMGDataSource::OGRGarminIMGDataSource() {
}

/************************************************************************/
/*                    ~OGRGarminIMGDataSource()                         */
/************************************************************************/

OGRGarminIMGDataSource::~OGRGarminIMGDataSource() {
}

/************************************************************************/
/*                              Open()                                  */
/************************************************************************/

bool OGRGarminIMGDataSource::Open(GDALOpenInfo* poOpenInfo) {
    SetDescription(poOpenInfo->pszFilename);

    // Parse filesystem
    m_poFilesystem = std::make_unique<GarminIMGFilesystem>();
    if (!m_poFilesystem->Parse(poOpenInfo->pszFilename)) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG: Failed to parse IMG filesystem: %s",
                 poOpenInfo->pszFilename);
        return false;
    }

    // Get tile names
    auto aosTileNames = m_poFilesystem->GetTileNames();
    if (aosTileNames.empty()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG: No tiles found in IMG file: %s",
                 poOpenInfo->pszFilename);
        return false;
    }

    // Instantiate parsers per tile
    for (const auto& osTileName : aosTileNames) {
        TileParserSet oTile;
        oTile.osTileName = osTileName;

        // TRE (required)
        const auto* pabyTRE = m_poFilesystem->GetSubfileData(osTileName + ".TRE");
        if (!pabyTRE) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Tile '%s' has no TRE subfile, skipping",
                     osTileName.c_str());
            continue;
        }
        oTile.poTRE = std::make_unique<GarminIMGTREParser>();
        if (!oTile.poTRE->Parse(pabyTRE->data(), static_cast<uint32_t>(pabyTRE->size()))) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Failed to parse TRE for tile '%s', skipping",
                     osTileName.c_str());
            continue;
        }

        // RGN (required)
        const auto* pabyRGN = m_poFilesystem->GetSubfileData(osTileName + ".RGN");
        if (!pabyRGN) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Tile '%s' has no RGN subfile, skipping",
                     osTileName.c_str());
            continue;
        }
        oTile.poRGN = std::make_unique<GarminIMGRGNParser>();
        if (!oTile.poRGN->Parse(pabyRGN->data(), static_cast<uint32_t>(pabyRGN->size()))) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Failed to parse RGN for tile '%s', skipping",
                     osTileName.c_str());
            continue;
        }

        // LBL (required)
        const auto* pabyLBL = m_poFilesystem->GetSubfileData(osTileName + ".LBL");
        if (!pabyLBL) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Tile '%s' has no LBL subfile, skipping",
                     osTileName.c_str());
            continue;
        }
        oTile.poLBL = std::make_unique<GarminIMGLBLParser>();
        if (!oTile.poLBL->Parse(pabyLBL->data(), static_cast<uint32_t>(pabyLBL->size()))) {
            CPLError(CE_Warning, CPLE_AppDefined,
                     "GarminIMG: Failed to parse LBL for tile '%s', skipping",
                     osTileName.c_str());
            continue;
        }

        // NET (optional)
        const auto* pabyNET = m_poFilesystem->GetSubfileData(osTileName + ".NET");
        if (pabyNET) {
            oTile.poNET = std::make_unique<GarminIMGNETParser>();
            if (oTile.poNET->Parse(pabyNET->data(), static_cast<uint32_t>(pabyNET->size()),
                                   oTile.poLBL.get())) {
                m_bHasRouting = true;
            } else {
                oTile.poNET.reset();
            }
        }

        // NOD (optional)
        const auto* pabyNOD = m_poFilesystem->GetSubfileData(osTileName + ".NOD");
        if (pabyNOD) {
            oTile.poNOD = std::make_unique<GarminIMGNODParser>();
            if (!oTile.poNOD->Parse(pabyNOD->data(), static_cast<uint32_t>(pabyNOD->size()))) {
                oTile.poNOD.reset();
            }
        }

        m_aoTiles.push_back(std::move(oTile));
    }

    if (m_aoTiles.empty()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG: No valid tiles found in: %s",
                 poOpenInfo->pszFilename);
        return false;
    }

    // TYP (optional — search for any .TYP subfile)
    for (const auto& [osName, oSubfile] : m_poFilesystem->GetSubfiles()) {
        if (oSubfile.osExtension == "TYP") {
            m_poTYPParser = std::make_unique<GarminIMGTYPParser>();
            if (!m_poTYPParser->Parse(oSubfile.abyData.data(),
                                      static_cast<uint32_t>(oSubfile.abyData.size()))) {
                m_poTYPParser.reset();
            }
            break;
        }
    }

    // External TYP file from open options
    if (!m_poTYPParser) {
        const char* pszTypFile = CSLFetchNameValue(poOpenInfo->papszOpenOptions, "TYP_FILE");
        if (pszTypFile) {
            m_poTYPParser = std::make_unique<GarminIMGTYPParser>();
            if (!m_poTYPParser->ParseFile(pszTypFile)) {
                m_poTYPParser.reset();
                CPLError(CE_Warning, CPLE_AppDefined,
                         "GarminIMG: Failed to parse external TYP file: %s",
                         pszTypFile);
            }
        }
    }

    // Dataset metadata
    SetMetadataItem("IMG_TILE_COUNT", CPLSPrintf("%d", GetTileCount()));
    SetMetadataItem("IMG_HAS_ROUTING", m_bHasRouting ? "YES" : "NO");
    SetMetadataItem("IMG_HAS_TYP", m_poTYPParser ? "YES" : "NO");
    if (!m_aoTiles.empty()) {
        SetMetadataItem("IMG_MAP_ID",
                        CPLSPrintf("%u", m_aoTiles[0].poTRE->GetMapID()));
    }

    CreateReadLayers();
    return true;
}

/************************************************************************/
/*                         CreateReadLayers()                           */
/************************************************************************/

void OGRGarminIMGDataSource::CreateReadLayers() {
    // Always create 3 base layers
    m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POI", wkbPoint, GarminIMGLayerType::POI, this));
    m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POLYLINE", wkbLineString, GarminIMGLayerType::POLYLINE, this));
    m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POLYGON", wkbPolygon, GarminIMGLayerType::POLYGON, this));

    // Optional routing layers
    if (m_bHasRouting) {
        m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
            "ROAD_NETWORK", wkbLineString, GarminIMGLayerType::ROAD_NETWORK, this));
        m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
            "ROUTING_NODE", wkbPoint, GarminIMGLayerType::ROUTING_NODE, this));
    }
}

/************************************************************************/
/*                           Create()                                   */
/************************************************************************/

OGRGarminIMGDataSource* OGRGarminIMGDataSource::Create(
    const char* pszFilename, char** /* papszOptions */) {
    OGRGarminIMGDataSource* poDS = new OGRGarminIMGDataSource();
    poDS->SetDescription(pszFilename);
    poDS->m_bUpdate = true;

    // Create empty layers for write mode
    poDS->m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POI", wkbPoint, GarminIMGLayerType::POI));
    poDS->m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POLYLINE", wkbLineString, GarminIMGLayerType::POLYLINE));
    poDS->m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "POLYGON", wkbPolygon, GarminIMGLayerType::POLYGON));
    poDS->m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "ROAD_NETWORK", wkbLineString, GarminIMGLayerType::ROAD_NETWORK));
    poDS->m_apoLayers.push_back(std::make_unique<OGRGarminIMGLayer>(
        "ROUTING_NODE", wkbPoint, GarminIMGLayerType::ROUTING_NODE));

    return poDS;
}

/************************************************************************/
/*                         GetLayerCount()                              */
/************************************************************************/

int OGRGarminIMGDataSource::GetLayerCount() OGRGARMINIMG_CONST {
    return static_cast<int>(m_apoLayers.size());
}

/************************************************************************/
/*                            GetLayer()                                */
/************************************************************************/

OGRLayer* OGRGarminIMGDataSource::GetLayer(int nLayer) OGRGARMINIMG_CONST {
    if (nLayer < 0 || nLayer >= GetLayerCount()) {
        return nullptr;
    }
    return m_apoLayers[nLayer].get();
}

/************************************************************************/
/*                        TestCapability()                              */
/************************************************************************/

int OGRGarminIMGDataSource::TestCapability(const char* pszCap) OGRGARMINIMG_CONST {
    if (EQUAL(pszCap, ODsCRandomLayerRead)) {
        return TRUE;
    }
    if (EQUAL(pszCap, ODsCCreateLayer)) {
        return m_bUpdate ? TRUE : FALSE;
    }
    return FALSE;
}

/************************************************************************/
/*                         ICreateLayer()                               */
/************************************************************************/

OGRLayer* OGRGarminIMGDataSource::ICreateLayer(
    const char* /* pszName */,
    const OGRGeomFieldDefn* poGeomFieldDefn,
    CSLConstList /* papszOptions */) {
    if (!m_bUpdate) {
        CPLError(CE_Failure, CPLE_NotSupported,
                 "GarminIMG: Cannot create layer in read-only mode");
        return nullptr;
    }

    OGRwkbGeometryType eType = poGeomFieldDefn
        ? poGeomFieldDefn->GetType()
        : wkbUnknown;

    // Route to fixed layer by geometry type
    switch (wkbFlatten(eType)) {
        case wkbPoint:
        case wkbMultiPoint:
            return m_apoLayers[0].get();  // POI
        case wkbLineString:
        case wkbMultiLineString:
            return m_apoLayers[1].get();  // POLYLINE
        case wkbPolygon:
        case wkbMultiPolygon:
            return m_apoLayers[2].get();  // POLYGON
        default:
            CPLError(CE_Failure, CPLE_NotSupported,
                     "GarminIMG: Unsupported geometry type: %s",
                     OGRGeometryTypeToName(eType));
            return nullptr;
    }
}
