/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Layer implementation for Garmin IMG format
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

#include "ogrgarminimglayer.h"
#include "ogrgarminimgdatasource.h"
#include "garminimgfields.h"
#include "cpl_conv.h"
#include "cpl_error.h"

#include <cstring>

/************************************************************************/
/*                      OGRGarminIMGLayer() — Read mode                 */
/************************************************************************/

OGRGarminIMGLayer::OGRGarminIMGLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType,
                                     GarminIMGLayerType eLayerType,
                                     OGRGarminIMGDataSource* poDS)
    : m_eLayerType(eLayerType), m_poDS(poDS), m_bWriteMode(false) {
    InitializeLayerDefn();
    m_poFeatureDefn->SetName(pszLayerName);
    m_poFeatureDefn->SetGeomType(eGeomType);
    m_poFeatureDefn->Reference();

    m_poSRS = new OGRSpatialReference();
    m_poSRS->SetWellKnownGeogCS("WGS84");
    m_poSRS->SetAxisMappingStrategy(OAMS_TRADITIONAL_GIS_ORDER);
    m_poFeatureDefn->GetGeomFieldDefn(0)->SetSpatialRef(m_poSRS);
}

/************************************************************************/
/*                      OGRGarminIMGLayer() — Write mode                */
/************************************************************************/

OGRGarminIMGLayer::OGRGarminIMGLayer(const char* pszLayerName,
                                     OGRwkbGeometryType eGeomType,
                                     GarminIMGLayerType eLayerType)
    : m_eLayerType(eLayerType), m_poDS(nullptr), m_bWriteMode(true) {
    InitializeLayerDefn();
    m_poFeatureDefn->SetName(pszLayerName);
    m_poFeatureDefn->SetGeomType(eGeomType);
    m_poFeatureDefn->Reference();

    m_poSRS = new OGRSpatialReference();
    m_poSRS->SetWellKnownGeogCS("WGS84");
    m_poSRS->SetAxisMappingStrategy(OAMS_TRADITIONAL_GIS_ORDER);
    m_poFeatureDefn->GetGeomFieldDefn(0)->SetSpatialRef(m_poSRS);
}

/************************************************************************/
/*                     ~OGRGarminIMGLayer()                             */
/************************************************************************/

OGRGarminIMGLayer::~OGRGarminIMGLayer() {
    if (m_poFeatureDefn) {
        m_poFeatureDefn->Release();
    }
    if (m_poSRS) {
        m_poSRS->Release();
    }
}

/************************************************************************/
/*                     InitializeLayerDefn()                            */
/************************************************************************/

void OGRGarminIMGLayer::InitializeLayerDefn() {
    m_poFeatureDefn = new OGRFeatureDefn();

    // Determine layer flag
    unsigned int nLayerFlag = 0;
    switch (m_eLayerType) {
        case GarminIMGLayerType::POI:          nLayerFlag = LAYER_POI; break;
        case GarminIMGLayerType::POLYLINE:     nLayerFlag = LAYER_POLYLINE; break;
        case GarminIMGLayerType::POLYGON:      nLayerFlag = LAYER_POLYGON; break;
        case GarminIMGLayerType::ROAD_NETWORK: nLayerFlag = LAYER_ROAD; break;
        case GarminIMGLayerType::ROUTING_NODE: nLayerFlag = LAYER_NODE; break;
    }

    // Add applicable fields
    auto aoFields = GetFieldsForLayer(nLayerFlag);
    for (const auto* poFieldDef : aoFields) {
        OGRFieldDefn oField(poFieldDef->pszName, poFieldDef->eType);
        m_poFeatureDefn->AddFieldDefn(&oField);
    }
}

/************************************************************************/
/*                         ResetReading()                               */
/************************************************************************/

void OGRGarminIMGLayer::ResetReading() {
    m_nNextFID = 1;
    m_nCurrentTile = 0;
    m_nCurrentSubdiv = 0;
    m_nCurrentFeatureInSubdiv = 0;
    m_bEOF = false;
    m_nCachedTile = -1;
    m_nCachedSubdiv = -1;
    m_aoCachedPOIs.clear();
    m_aoCachedPolylines.clear();
    m_aoCachedPolygons.clear();
}

/************************************************************************/
/*                        GetNextFeature()                              */
/************************************************************************/

OGRFeature* OGRGarminIMGLayer::GetNextFeature() {
    if (m_bEOF || !m_poDS) {
        return nullptr;
    }

    while (true) {
        OGRFeature* poFeature = nullptr;

        switch (m_eLayerType) {
            case GarminIMGLayerType::POI:
                poFeature = GetNextPOIFeature();
                break;
            case GarminIMGLayerType::POLYLINE:
                poFeature = GetNextPolylineFeature();
                break;
            case GarminIMGLayerType::POLYGON:
                poFeature = GetNextPolygonFeature();
                break;
            case GarminIMGLayerType::ROAD_NETWORK:
                poFeature = GetNextRoadNetworkFeature();
                break;
            case GarminIMGLayerType::ROUTING_NODE:
                poFeature = GetNextRoutingNodeFeature();
                break;
        }

        if (poFeature == nullptr) {
            m_bEOF = true;
            return nullptr;
        }

        // Apply spatial filter
        if (m_poFilterGeom != nullptr &&
            !FilterGeometry(poFeature->GetGeometryRef())) {
            delete poFeature;
            continue;
        }

        // Apply attribute filter
        if (m_poAttrQuery != nullptr &&
            !m_poAttrQuery->Evaluate(poFeature)) {
            delete poFeature;
            continue;
        }

        poFeature->SetFID(m_nNextFID++);
        return poFeature;
    }
}

/************************************************************************/
/*                       GetFeatureCount()                              */
/************************************************************************/

GIntBig OGRGarminIMGLayer::GetFeatureCount(int bForce) {
    if (m_poFilterGeom != nullptr || m_poAttrQuery != nullptr) {
        if (!bForce) return -1;
        return OGRLayer::GetFeatureCount(bForce);
    }
    // TODO: implement fast count by iterating subdivisions
    return OGRLayer::GetFeatureCount(bForce);
}

/************************************************************************/
/*                           GetExtent()                                */
/************************************************************************/

OGRErr OGRGarminIMGLayer::IGetExtent(int iGeomField, OGREnvelope* psExtent,
                                     bool bForce) {
    if (iGeomField != 0) {
        return OGRERR_FAILURE;
    }

    if (!m_poDS || m_poDS->GetTiles().empty()) {
        if (bForce) {
            return OGRLayer::IGetExtent(iGeomField, psExtent, bForce);
        }
        return OGRERR_FAILURE;
    }

    // Union of all tile bounds
    bool bFirst = true;
    for (const auto& oTile : m_poDS->GetTiles()) {
        const TREBounds& oBounds = oTile.poTRE->GetBounds();
        if (bFirst) {
            psExtent->MinX = oBounds.dfWest;
            psExtent->MaxX = oBounds.dfEast;
            psExtent->MinY = oBounds.dfSouth;
            psExtent->MaxY = oBounds.dfNorth;
            bFirst = false;
        } else {
            psExtent->MinX = std::min(psExtent->MinX, oBounds.dfWest);
            psExtent->MaxX = std::max(psExtent->MaxX, oBounds.dfEast);
            psExtent->MinY = std::min(psExtent->MinY, oBounds.dfSouth);
            psExtent->MaxY = std::max(psExtent->MaxY, oBounds.dfNorth);
        }
    }

    return OGRERR_NONE;
}

/************************************************************************/
/*                        TestCapability()                              */
/************************************************************************/

int OGRGarminIMGLayer::TestCapability(const char* pszCap) OGRGARMINIMG_CONST {
    if (EQUAL(pszCap, OLCFastGetExtent)) {
        return TRUE;
    }
    if (EQUAL(pszCap, OLCStringsAsUTF8)) {
        return TRUE;
    }
    if (EQUAL(pszCap, OLCSequentialWrite)) {
        return m_bWriteMode ? TRUE : FALSE;
    }
    if (EQUAL(pszCap, OLCFastSpatialFilter)) {
        return TRUE;
    }
    return FALSE;
}

/************************************************************************/
/*                          CreateField()                               */
/************************************************************************/

OGRErr OGRGarminIMGLayer::CreateField(const OGRFieldDefn* /* poField */,
                                      int /* bApproxOK */) {
    // Accept-and-ignore pattern (like ogr-polishmap)
    return OGRERR_NONE;
}

/************************************************************************/
/*                        ICreateFeature()                              */
/************************************************************************/

OGRErr OGRGarminIMGLayer::ICreateFeature(OGRFeature* /* poFeature */) {
    if (!m_bWriteMode) {
        CPLError(CE_Failure, CPLE_NotSupported,
                 "GarminIMG: Cannot create features in read-only mode");
        return OGRERR_FAILURE;
    }
    // TODO: Implement in Phase 4 (Tâche 18)
    return OGRERR_NONE;
}

/************************************************************************/
/*                    GetNext*Feature() — stubs                         */
/*    Will be fully implemented in Phase 3 (Tâche 12)                  */
/************************************************************************/

OGRFeature* OGRGarminIMGLayer::GetNextPOIFeature() {
    if (!m_poDS) return nullptr;

    const auto& aoTiles = m_poDS->GetTiles();

    while (m_nCurrentTile < static_cast<int>(aoTiles.size())) {
        const auto& oTile = aoTiles[m_nCurrentTile];
        const auto& aoSubdivs = oTile.poTRE->GetSubdivisions();
        const auto& aoExtOffsets = oTile.poTRE->GetExtTypeOffsets();
        int nFinestLevel = oTile.poTRE->GetFinestLevel();

        while (m_nCurrentSubdiv < static_cast<int>(aoSubdivs.size())) {
            const auto& oSubdiv = aoSubdivs[m_nCurrentSubdiv];

            // Standard POIs: only at finest level to avoid duplicates
            bool bHasStdPoints = (oSubdiv.nContentFlags & 0x30) != 0
                                 && oSubdiv.nLevel == nFinestLevel;
            // Extended POIs: use extTypeOffsets (index = subdiv + 1 for topdiv)
            int nExtIdx = m_nCurrentSubdiv + 1;
            bool bHasExtPoints = false;
            uint32_t nExtPtStart = 0, nExtPtEnd = 0;
            if (nExtIdx < static_cast<int>(aoExtOffsets.size()) &&
                nExtIdx + 1 < static_cast<int>(aoExtOffsets.size())) {
                nExtPtStart = aoExtOffsets[nExtIdx].nExtPointsOffset;
                nExtPtEnd   = aoExtOffsets[nExtIdx + 1].nExtPointsOffset;
                if (nExtPtEnd > nExtPtStart) bHasExtPoints = true;
            }

            if (!bHasStdPoints && !bHasExtPoints) {
                m_nCurrentSubdiv++;
                m_nCurrentFeatureInSubdiv = 0;
                continue;
            }

            // Cache: decode only when subdivision changes
            if (m_nCachedTile != m_nCurrentTile || m_nCachedSubdiv != m_nCurrentSubdiv) {
                m_aoCachedPOIs.clear();
                if (bHasStdPoints) {
                    oTile.poRGN->DecodePOIs(oSubdiv, oTile.poLBL.get(), m_aoCachedPOIs);
                }
                if (bHasExtPoints) {
                    oTile.poRGN->DecodeExtendedPOIs(oSubdiv, oTile.poLBL.get(),
                                                     nExtPtStart, nExtPtEnd, m_aoCachedPOIs);
                }
                m_nCachedTile = m_nCurrentTile;
                m_nCachedSubdiv = m_nCurrentSubdiv;
            }

            if (m_nCurrentFeatureInSubdiv < static_cast<int>(m_aoCachedPOIs.size())) {
                const auto& oPOI = m_aoCachedPOIs[m_nCurrentFeatureInSubdiv];
                m_nCurrentFeatureInSubdiv++;

                OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);
                OGRPoint oPoint(oPOI.dfLon, oPOI.dfLat);
                oPoint.assignSpatialReference(m_poSRS);
                poFeature->SetGeometryDirectly(oPoint.clone());

                // Set fields
                poFeature->SetField("Type", CPLSPrintf("0x%04X", oPOI.nType));
                poFeature->SetField("SubType", CPLSPrintf("0x%02X", oPOI.nSubType));
                poFeature->SetField("Label", oPOI.osLabel.c_str());
                poFeature->SetField("EndLevel", oPOI.nEndLevel);

                // TYP enrichment
                if (m_poDS->GetTYPParser()) {
                    const auto* poStyle = m_poDS->GetTYPParser()->GetTypInfo(
                        oPOI.nType, oPOI.nSubType);
                    if (poStyle) {
                        if (!poStyle->osFillColor.empty())
                            poFeature->SetField("TYP_FillColor",
                                                poStyle->osFillColor.c_str());
                        if (!poStyle->osLineColor.empty())
                            poFeature->SetField("TYP_LineColor",
                                                poStyle->osLineColor.c_str());
                        if (!poStyle->osDisplayName.empty())
                            poFeature->SetField("TYP_DisplayName",
                                                poStyle->osDisplayName.c_str());
                        if (!poStyle->abyIconData.empty()) {
                            poFeature->SetField(
                                poFeature->GetFieldIndex("TYP_IconData"),
                                static_cast<int>(poStyle->abyIconData.size()),
                                poStyle->abyIconData.data());
                        }
                    }
                }

                return poFeature;
            }

            m_nCurrentSubdiv++;
            m_nCurrentFeatureInSubdiv = 0;
        }

        m_nCurrentTile++;
        m_nCurrentSubdiv = 0;
        m_nCurrentFeatureInSubdiv = 0;
    }

    return nullptr;
}

OGRFeature* OGRGarminIMGLayer::GetNextPolylineFeature() {
    if (!m_poDS) return nullptr;

    const auto& aoTiles = m_poDS->GetTiles();

    while (m_nCurrentTile < static_cast<int>(aoTiles.size())) {
        const auto& oTile = aoTiles[m_nCurrentTile];
        const auto& aoSubdivs = oTile.poTRE->GetSubdivisions();
        int nFinestLevel = oTile.poTRE->GetFinestLevel();

        while (m_nCurrentSubdiv < static_cast<int>(aoSubdivs.size())) {
            const auto& oSubdiv = aoSubdivs[m_nCurrentSubdiv];

            if (oSubdiv.nLevel != nFinestLevel ||
                !(oSubdiv.nContentFlags & 0x40)) {
                m_nCurrentSubdiv++;
                m_nCurrentFeatureInSubdiv = 0;
                continue;
            }

            // Cache: decode only when subdivision changes
            if (m_nCachedTile != m_nCurrentTile || m_nCachedSubdiv != m_nCurrentSubdiv) {
                m_aoCachedPolylines.clear();
                oTile.poRGN->DecodePolylines(oSubdiv, oTile.poLBL.get(), m_aoCachedPolylines);
                // TODO: extended polylines when DecodeExtendedPolylines is implemented
                m_nCachedTile = m_nCurrentTile;
                m_nCachedSubdiv = m_nCurrentSubdiv;
            }

            if (m_nCurrentFeatureInSubdiv < static_cast<int>(m_aoCachedPolylines.size())) {
                const auto& oPoly = m_aoCachedPolylines[m_nCurrentFeatureInSubdiv];
                m_nCurrentFeatureInSubdiv++;

                OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

                OGRLineString oLine;
                for (const auto& oPt : oPoly.aoPoints) {
                    oLine.addPoint(oPt.dfLon, oPt.dfLat);
                }
                oLine.assignSpatialReference(m_poSRS);
                poFeature->SetGeometryDirectly(oLine.clone());

                poFeature->SetField("Type", CPLSPrintf("0x%04X", oPoly.nType));
                poFeature->SetField("SubType", CPLSPrintf("0x%02X", oPoly.nSubType));
                poFeature->SetField("Label", oPoly.osLabel.c_str());
                poFeature->SetField("EndLevel", oPoly.nEndLevel);
                poFeature->SetField("DirIndicator",
                                    oPoly.bDirectionIndicator ? 1 : 0);

                // TYP enrichment
                if (m_poDS->GetTYPParser()) {
                    const auto* poStyle = m_poDS->GetTYPParser()->GetTypInfo(
                        oPoly.nType, oPoly.nSubType);
                    if (poStyle) {
                        if (!poStyle->osLineColor.empty())
                            poFeature->SetField("TYP_LineColor",
                                                poStyle->osLineColor.c_str());
                        if (poStyle->nLineWidth > 0)
                            poFeature->SetField("TYP_LineWidth",
                                                poStyle->nLineWidth);
                        if (!poStyle->osDisplayName.empty())
                            poFeature->SetField("TYP_DisplayName",
                                                poStyle->osDisplayName.c_str());
                    }
                }

                return poFeature;
            }

            m_nCurrentSubdiv++;
            m_nCurrentFeatureInSubdiv = 0;
        }

        m_nCurrentTile++;
        m_nCurrentSubdiv = 0;
        m_nCurrentFeatureInSubdiv = 0;
    }

    return nullptr;
}

OGRFeature* OGRGarminIMGLayer::GetNextPolygonFeature() {
    if (!m_poDS) return nullptr;

    const auto& aoTiles = m_poDS->GetTiles();

    while (m_nCurrentTile < static_cast<int>(aoTiles.size())) {
        const auto& oTile = aoTiles[m_nCurrentTile];
        const auto& aoSubdivs = oTile.poTRE->GetSubdivisions();
        int nFinestLevel = oTile.poTRE->GetFinestLevel();

        while (m_nCurrentSubdiv < static_cast<int>(aoSubdivs.size())) {
            const auto& oSubdiv = aoSubdivs[m_nCurrentSubdiv];

            if (oSubdiv.nLevel != nFinestLevel ||
                !(oSubdiv.nContentFlags & 0x80)) {
                m_nCurrentSubdiv++;
                m_nCurrentFeatureInSubdiv = 0;
                continue;
            }

            // Cache: decode only when subdivision changes
            if (m_nCachedTile != m_nCurrentTile || m_nCachedSubdiv != m_nCurrentSubdiv) {
                m_aoCachedPolygons.clear();
                oTile.poRGN->DecodePolygons(oSubdiv, oTile.poLBL.get(), m_aoCachedPolygons);
                // TODO: extended polygons when DecodeExtendedPolygons is implemented
                m_nCachedTile = m_nCurrentTile;
                m_nCachedSubdiv = m_nCurrentSubdiv;
            }

            if (m_nCurrentFeatureInSubdiv < static_cast<int>(m_aoCachedPolygons.size())) {
                const auto& oPoly = m_aoCachedPolygons[m_nCurrentFeatureInSubdiv];
                m_nCurrentFeatureInSubdiv++;

                OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

                OGRPolygon oPolygon;
                OGRLinearRing oRing;
                for (const auto& oPt : oPoly.aoPoints) {
                    oRing.addPoint(oPt.dfLon, oPt.dfLat);
                }
                // Close the ring
                if (!oPoly.aoPoints.empty()) {
                    oRing.addPoint(oPoly.aoPoints[0].dfLon,
                                   oPoly.aoPoints[0].dfLat);
                }
                oPolygon.addRing(&oRing);
                oPolygon.assignSpatialReference(m_poSRS);
                poFeature->SetGeometryDirectly(oPolygon.clone());

                poFeature->SetField("Type", CPLSPrintf("0x%04X", oPoly.nType));
                poFeature->SetField("SubType", CPLSPrintf("0x%02X", oPoly.nSubType));
                poFeature->SetField("Label", oPoly.osLabel.c_str());
                poFeature->SetField("EndLevel", oPoly.nEndLevel);

                // TYP enrichment
                if (m_poDS->GetTYPParser()) {
                    const auto* poStyle = m_poDS->GetTYPParser()->GetTypInfo(
                        oPoly.nType, oPoly.nSubType);
                    if (poStyle) {
                        if (!poStyle->osFillColor.empty())
                            poFeature->SetField("TYP_FillColor",
                                                poStyle->osFillColor.c_str());
                        if (!poStyle->osBorderColor.empty())
                            poFeature->SetField("TYP_BorderColor",
                                                poStyle->osBorderColor.c_str());
                        if (!poStyle->osDisplayName.empty())
                            poFeature->SetField("TYP_DisplayName",
                                                poStyle->osDisplayName.c_str());
                        if (!poStyle->abyPatternData.empty()) {
                            poFeature->SetField(
                                poFeature->GetFieldIndex("TYP_PatternData"),
                                static_cast<int>(poStyle->abyPatternData.size()),
                                poStyle->abyPatternData.data());
                        }
                    }
                }

                return poFeature;
            }

            m_nCurrentSubdiv++;
            m_nCurrentFeatureInSubdiv = 0;
        }

        m_nCurrentTile++;
        m_nCurrentSubdiv = 0;
        m_nCurrentFeatureInSubdiv = 0;
    }

    return nullptr;
}

OGRFeature* OGRGarminIMGLayer::GetNextRoadNetworkFeature() {
    if (!m_poDS) return nullptr;

    const auto& aoTiles = m_poDS->GetTiles();

    while (m_nCurrentTile < static_cast<int>(aoTiles.size())) {
        const auto& oTile = aoTiles[m_nCurrentTile];

        if (!oTile.poNET) {
            m_nCurrentTile++;
            m_nCurrentFeatureInSubdiv = 0;
            continue;
        }

        const auto& aoRoads = oTile.poNET->GetAllRoads();

        if (m_nCurrentFeatureInSubdiv < static_cast<int>(aoRoads.size())) {
            const auto& oRoad = aoRoads[m_nCurrentFeatureInSubdiv];
            m_nCurrentFeatureInSubdiv++;

            OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

            // Label from first label
            std::string osLabel;
            if (!oRoad.aosLabels.empty()) {
                osLabel = oRoad.aosLabels[0];
            }
            poFeature->SetField("Label", osLabel.c_str());

            // Road attributes
            poFeature->SetField("RoadClass", oRoad.nRoadClass);
            poFeature->SetField("Speed", oRoad.nSpeed);
            poFeature->SetField("OneWay", oRoad.bOneWay ? 1 : 0);
            poFeature->SetField("Toll", oRoad.bToll ? 1 : 0);
            poFeature->SetField("AccessFlags",
                                CPLSPrintf("0x%02X", oRoad.nAccessFlags));
            poFeature->SetField("RoadLength", oRoad.dfLengthM);

            // Decompose access flags
            poFeature->SetField("DeniedEmergency",
                                (oRoad.nAccessFlags & 0x01) ? 1 : 0);
            poFeature->SetField("DeniedDelivery",
                                (oRoad.nAccessFlags & 0x02) ? 1 : 0);
            poFeature->SetField("DeniedCar",
                                (oRoad.nAccessFlags & 0x04) ? 1 : 0);
            poFeature->SetField("DeniedBus",
                                (oRoad.nAccessFlags & 0x08) ? 1 : 0);
            poFeature->SetField("DeniedTaxi",
                                (oRoad.nAccessFlags & 0x10) ? 1 : 0);
            poFeature->SetField("DeniedPedestrian",
                                (oRoad.nAccessFlags & 0x20) ? 1 : 0);
            poFeature->SetField("DeniedBicycle",
                                (oRoad.nAccessFlags & 0x40) ? 1 : 0);
            poFeature->SetField("DeniedTruck",
                                (oRoad.nAccessFlags & 0x80) ? 1 : 0);

            // TODO: geometry from RGN cross-reference (Phase 3)
            // For now, create empty geometry
            OGRLineString oLine;
            oLine.assignSpatialReference(m_poSRS);
            poFeature->SetGeometryDirectly(oLine.clone());

            return poFeature;
        }

        m_nCurrentTile++;
        m_nCurrentFeatureInSubdiv = 0;
    }

    return nullptr;
}

OGRFeature* OGRGarminIMGLayer::GetNextRoutingNodeFeature() {
    if (!m_poDS) return nullptr;

    const auto& aoTiles = m_poDS->GetTiles();

    while (m_nCurrentTile < static_cast<int>(aoTiles.size())) {
        const auto& oTile = aoTiles[m_nCurrentTile];

        if (!oTile.poNOD) {
            m_nCurrentTile++;
            m_nCurrentFeatureInSubdiv = 0;
            continue;
        }

        const auto& aoNodes = oTile.poNOD->GetNodes();

        if (m_nCurrentFeatureInSubdiv < static_cast<int>(aoNodes.size())) {
            const auto& oNode = aoNodes[m_nCurrentFeatureInSubdiv];
            m_nCurrentFeatureInSubdiv++;

            OGRFeature* poFeature = new OGRFeature(m_poFeatureDefn);

            OGRPoint oPoint(oNode.dfLon, oNode.dfLat);
            oPoint.assignSpatialReference(m_poSRS);
            poFeature->SetGeometryDirectly(oPoint.clone());

            poFeature->SetField("NodeType", oNode.osNodeType.c_str());
            poFeature->SetField("ArcCount",
                                static_cast<int>(oNode.aoArcs.size()));

            // Connected roads as comma-separated NET1 offsets
            std::string osConnected;
            for (size_t i = 0; i < oNode.aoArcs.size(); i++) {
                if (i > 0) osConnected += ",";
                osConnected += CPLSPrintf("%u", oNode.aoArcs[i].nNET1Offset);
            }
            poFeature->SetField("ConnectedRoads", osConnected.c_str());

            return poFeature;
        }

        m_nCurrentTile++;
        m_nCurrentFeatureInSubdiv = 0;
    }

    return nullptr;
}
