/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG RGN (geometry) subfile
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

#ifndef GARMINIMGRGNPARSER_H_INCLUDED
#define GARMINIMGRGNPARSER_H_INCLUDED

#include "garminimgtreparser.h"
#include "garminimglblparser.h"
#include "garminimgbitreader.h"

#include <cstdint>
#include <string>
#include <vector>
#include <utility>

/************************************************************************/
/*                        RGN Feature Structures                        */
/************************************************************************/

struct RGNPoint {
    double dfLon = 0.0;
    double dfLat = 0.0;
};

struct RGNPOIFeature {
    uint16_t nType = 0;
    uint16_t nSubType = 0;
    std::string osLabel;
    double dfLon = 0.0;
    double dfLat = 0.0;
    int nEndLevel = 0;
    bool bExtended = false;  // Type >= 0x100
};

struct RGNPolyFeature {
    uint16_t nType = 0;
    uint16_t nSubType = 0;
    std::string osLabel;
    std::vector<RGNPoint> aoPoints;
    int nEndLevel = 0;
    bool bDirectionIndicator = false;
    bool bHasNetInfo = false;
    uint32_t nNetOffset = 0;   // NET1 offset if has_net_info
    bool bExtended = false;
};

/************************************************************************/
/*                     GarminIMGRGNParser                               */
/************************************************************************/

class GarminIMGRGNParser {
public:
    GarminIMGRGNParser();
    ~GarminIMGRGNParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize);

    // Decode features from a subdivision
    bool DecodePOIs(const TRESubdivision& oSubdiv,
                    const GarminIMGLBLParser* poLBL,
                    std::vector<RGNPOIFeature>& aoFeatures);

    bool DecodePolylines(const TRESubdivision& oSubdiv,
                         const GarminIMGLBLParser* poLBL,
                         std::vector<RGNPolyFeature>& aoFeatures);

    bool DecodePolygons(const TRESubdivision& oSubdiv,
                        const GarminIMGLBLParser* poLBL,
                        std::vector<RGNPolyFeature>& aoFeatures);

    // Extended types (>= 0x100)
    // nExtStart/nExtEnd are offsets within the ext_point/ext_line/ext_area
    // section of RGN, derived from TRE extTypeOffsets records.
    bool DecodeExtendedPOIs(const TRESubdivision& oSubdiv,
                            const GarminIMGLBLParser* poLBL,
                            uint32_t nExtStart, uint32_t nExtEnd,
                            std::vector<RGNPOIFeature>& aoFeatures);

    bool DecodeExtendedPolylines(const TRESubdivision& oSubdiv,
                                 const GarminIMGLBLParser* poLBL,
                                 uint32_t nExtStart, uint32_t nExtEnd,
                                 std::vector<RGNPolyFeature>& aoFeatures);

    bool DecodeExtendedPolygons(const TRESubdivision& oSubdiv,
                                const GarminIMGLBLParser* poLBL,
                                uint32_t nExtStart, uint32_t nExtEnd,
                                std::vector<RGNPolyFeature>& aoFeatures);

    uint32_t GetHeaderLength() const { return m_nHeaderLength; }

    // Extended type section offsets (from header)
    uint32_t GetExtPolylineOffset() const { return m_nExtPolylineOffset; }
    uint32_t GetExtPolygonOffset() const { return m_nExtPolygonOffset; }
    uint32_t GetExtPointOffset() const { return m_nExtPointOffset; }

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    uint32_t m_nHeaderLength = 125;  // Default RGN header

    // Standard data section
    uint32_t m_nDataOffset = 0;
    uint32_t m_nDataSize = 0;

    // Extended type sections
    uint32_t m_nExtPolygonOffset = 0;
    uint32_t m_nExtPolygonSize = 0;
    uint32_t m_nExtPolylineOffset = 0;
    uint32_t m_nExtPolylineSize = 0;
    uint32_t m_nExtPointOffset = 0;
    uint32_t m_nExtPointSize = 0;

    // Internal helpers
    bool DecodePolyBitstream(const uint8_t* pabyBits, uint32_t nBitstreamLen,
                             int32_t nStartLon, int32_t nStartLat,
                             int nShift,
                             std::vector<RGNPoint>& aoPoints);

    static uint16_t ReadLE16(const uint8_t* p);
    static uint32_t ReadLE24(const uint8_t* p);
    static int32_t ReadLE24Signed(const uint8_t* p);
    static uint32_t ReadLE32(const uint8_t* p);
    static int16_t ReadLE16Signed(const uint8_t* p);
};

#endif /* GARMINIMGRGNPARSER_H_INCLUDED */
