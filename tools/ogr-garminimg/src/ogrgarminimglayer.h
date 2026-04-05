/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Layer for Garmin IMG format
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

#ifndef OGRGARMINIMGLAYER_H_INCLUDED
#define OGRGARMINIMGLAYER_H_INCLUDED

#include "ogrsf_frmts.h"
#include "ogrgarminimg_compat.h"
#include "garminimgrgnparser.h"

#include <string>
#include <vector>

class OGRGarminIMGDataSource;

/************************************************************************/
/*                        LayerType                                     */
/************************************************************************/

enum class GarminIMGLayerType {
    POI,
    POLYLINE,
    POLYGON,
    ROAD_NETWORK,
    ROUTING_NODE
};

/************************************************************************/
/*                       OGRGarminIMGLayer                              */
/************************************************************************/

class OGRGarminIMGLayer final : public OGRLayer {
public:
    // Read mode
    OGRGarminIMGLayer(const char* pszLayerName,
                      OGRwkbGeometryType eGeomType,
                      GarminIMGLayerType eLayerType,
                      OGRGarminIMGDataSource* poDS);

    // Write mode
    OGRGarminIMGLayer(const char* pszLayerName,
                      OGRwkbGeometryType eGeomType,
                      GarminIMGLayerType eLayerType);

    ~OGRGarminIMGLayer() override;

    void ResetReading() override;
    OGRFeature* GetNextFeature() override;

#if GDAL_VERSION_NUM >= 3120000
    const OGRFeatureDefn* GetLayerDefn() const override { return m_poFeatureDefn; }
#endif
    OGRFeatureDefn* GetLayerDefn() override { return m_poFeatureDefn; }

    int TestCapability(const char* pszCap) OGRGARMINIMG_CONST override;

    GIntBig GetFeatureCount(int bForce) override;

protected:
    OGRErr IGetExtent(int iGeomField, OGREnvelope* psExtent, bool bForce) override;

public:

    OGRErr CreateField(const OGRFieldDefn* poField, int bApproxOK) override;
    OGRErr ICreateFeature(OGRFeature* poFeature) override;

    GarminIMGLayerType GetLayerType() const { return m_eLayerType; }

private:
    OGRFeatureDefn* m_poFeatureDefn = nullptr;
    OGRSpatialReference* m_poSRS = nullptr;
    GIntBig m_nNextFID = 1;
    GarminIMGLayerType m_eLayerType;
    OGRGarminIMGDataSource* m_poDS = nullptr;
    bool m_bWriteMode = false;
    bool m_bEOF = false;

    // Iteration state
    int m_nCurrentTile = 0;
    int m_nCurrentSubdiv = 0;
    int m_nCurrentFeatureInSubdiv = 0;

    // Cached decoded features for current subdivision (avoids O(N²) re-decode)
    std::vector<RGNPOIFeature> m_aoCachedPOIs;
    std::vector<RGNPolyFeature> m_aoCachedPolylines;
    std::vector<RGNPolyFeature> m_aoCachedPolygons;
    int m_nCachedTile = -1;
    int m_nCachedSubdiv = -1;

    void InitializeLayerDefn();
    OGRFeature* GetNextPOIFeature();
    OGRFeature* GetNextPolylineFeature();
    OGRFeature* GetNextPolygonFeature();
    OGRFeature* GetNextRoadNetworkFeature();
    OGRFeature* GetNextRoutingNodeFeature();
};

#endif /* OGRGARMINIMGLAYER_H_INCLUDED */
