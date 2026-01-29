/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Declaration of OGRPolishMapLayer class
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

#ifndef OGRPOLISHMAPLAYER_H_INCLUDED
#define OGRPOLISHMAPLAYER_H_INCLUDED

#include "ogrsf_frmts.h"
#include "ogr_spatialref.h"

/************************************************************************/
/*                         OGRPolishMapLayer                            */
/*                                                                      */
/* Story 1.3: Layer implementation with OGRFeatureDefn, geometry type,  */
/* field definitions (Type, Label, Data0, EndLevel, Levels), and WGS84  */
/* spatial reference system.                                            */
/************************************************************************/

class OGRPolishMapLayer final : public OGRLayer {
private:
    OGRFeatureDefn* m_poFeatureDefn;
    OGRSpatialReference* m_poSRS;
    GIntBig m_nNextFID;

public:
    // Story 1.3: Constructor now accepts geometry type for POI/POLYLINE/POLYGON
    OGRPolishMapLayer(const char* pszLayerName, OGRwkbGeometryType eGeomType);
    ~OGRPolishMapLayer() override;

    // Disable copy and assignment
    OGRPolishMapLayer(const OGRPolishMapLayer&) = delete;
    OGRPolishMapLayer& operator=(const OGRPolishMapLayer&) = delete;

    // OGRLayer pure virtual methods
    void ResetReading() override;
    OGRFeature* GetNextFeature() override;
    OGRFeatureDefn* GetLayerDefn() override;
    int TestCapability(const char* pszCap) override;
};

#endif /* OGRPOLISHMAPLAYER_H_INCLUDED */
