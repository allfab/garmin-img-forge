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

// Forward declarations
class PolishMapParser;
class PolishMapWriter;

/************************************************************************/
/*                         OGRPolishMapLayer                            */
/*                                                                      */
/* Story 1.3: Layer implementation with OGRFeatureDefn, geometry type,  */
/* field definitions (Type, Label, Data0, EndLevel, Levels), and WGS84  */
/* spatial reference system.                                            */
/************************************************************************/

class OGRPolishMapLayer final : public OGRLayer {
private:
    // Ring closure tolerance: 1e-9 degrees (~0.1mm at Earth surface)
    // Used for comparing first/last point to determine if POLYGON ring needs auto-closure
    static constexpr double RING_CLOSURE_TOLERANCE = 1e-9;

    OGRFeatureDefn* m_poFeatureDefn;
    OGRSpatialReference* m_poSRS;
    GIntBig m_nNextFID;

    // Story 1.4: Parser and layer type for feature reading
    PolishMapParser* m_poParser;       // Non-owning pointer to parser
    CPLString m_osLayerType;           // "POI", "POLYLINE", or "POLYGON"
    bool m_bEOF;                       // End of file reached
    bool m_bReaderInitialized;        // Parser position initialized

    // Story 2.3: Write mode support
    bool m_bWriteMode;                 // true if layer is in write mode
    PolishMapWriter* m_poWriter;       // Non-owning pointer to writer (write mode only)

public:
    // Story 1.3: Constructor now accepts geometry type for POI/POLYLINE/POLYGON
    OGRPolishMapLayer(const char* pszLayerName, OGRwkbGeometryType eGeomType);

    // Story 1.4: Constructor with parser for feature reading
    OGRPolishMapLayer(const char* pszLayerName, OGRwkbGeometryType eGeomType,
                      PolishMapParser* poParser);

    ~OGRPolishMapLayer() override;

    // Disable copy and assignment
    OGRPolishMapLayer(const OGRPolishMapLayer&) = delete;
    OGRPolishMapLayer& operator=(const OGRPolishMapLayer&) = delete;

    // OGRLayer pure virtual methods
    void ResetReading() override;
    OGRFeature* GetNextFeature() override;
    OGRFeatureDefn* GetLayerDefn() override;
    int TestCapability(const char* pszCap) override;

    // Story 2.3: Write mode support
    /** @brief Set the writer for this layer (write mode).
     *  @param poWriter Non-owning pointer to PolishMapWriter instance.
     *  @note Enables write mode and OLCSequentialWrite capability.
     */
    void SetWriter(PolishMapWriter* poWriter);

protected:
    // Story 2.3: GDAL convention - override ICreateFeature, not CreateFeature
    OGRErr ICreateFeature(OGRFeature* poFeature) override;

private:
    // Initialize feature definition, SRS, and field definitions
    void InitializeLayerDefn(const char* pszLayerName, OGRwkbGeometryType eGeomType);

    // Story 1.4-1.6: Type-specific feature readers
    OGRFeature* GetNextPOIFeature();
    OGRFeature* GetNextPolylineFeature();
    OGRFeature* GetNextPolygonFeature();
};

#endif /* OGRPOLISHMAPLAYER_H_INCLUDED */
