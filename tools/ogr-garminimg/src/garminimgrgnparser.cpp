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

#include "garminimgrgnparser.h"
#include "cpl_error.h"

#include <cstring>

/************************************************************************/
/*                     GarminIMGRGNParser()                             */
/************************************************************************/

GarminIMGRGNParser::GarminIMGRGNParser() {
}

GarminIMGRGNParser::~GarminIMGRGNParser() {
}

// Static LE read helpers
uint16_t GarminIMGRGNParser::ReadLE16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0]) | (static_cast<uint16_t>(p[1]) << 8);
}

uint32_t GarminIMGRGNParser::ReadLE24(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16);
}

int32_t GarminIMGRGNParser::ReadLE24Signed(const uint8_t* p) {
    uint32_t val = ReadLE24(p);
    if (val & 0x800000) return static_cast<int32_t>(val | 0xFF000000u);
    return static_cast<int32_t>(val);
}

uint32_t GarminIMGRGNParser::ReadLE32(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) |
           (static_cast<uint32_t>(p[3]) << 24);
}

int16_t GarminIMGRGNParser::ReadLE16Signed(const uint8_t* p) {
    return static_cast<int16_t>(ReadLE16(p));
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGRGNParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x1D) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG RGN: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;

    // Common header
    m_nHeaderLength = ReadLE16(pabyData);

    // Validate header length against actual data size
    if (m_nHeaderLength > nSize) {
        m_nHeaderLength = static_cast<uint32_t>(nSize);
    }

    // Standard data section: offset at 0x15 (LE32), size at 0x19 (LE32)
    m_nDataOffset = ReadLE32(pabyData + 0x15);
    m_nDataSize   = ReadLE32(pabyData + 0x19);

    // Extended sections (if header is large enough AND data supports it)
    if (m_nHeaderLength >= 0x5D && nSize >= 0x5D) {
        // Extended polygon: offset at 0x1D
        m_nExtPolygonOffset = ReadLE32(pabyData + 0x1D);
        m_nExtPolygonSize   = ReadLE32(pabyData + 0x21);

        // Extended polyline: offset at 0x39
        if (m_nHeaderLength >= 0x41) {
            m_nExtPolylineOffset = ReadLE32(pabyData + 0x39);
            m_nExtPolylineSize   = ReadLE32(pabyData + 0x3D);
        }

        // Extended point: offset at 0x55
        if (m_nHeaderLength >= 0x5D) {
            m_nExtPointOffset = ReadLE32(pabyData + 0x55);
            m_nExtPointSize   = ReadLE32(pabyData + 0x59);
        }
    }

    CPLDebug("OGR_GARMINIMG", "RGN: header=%u, data at %u (%u bytes), "
             "ext_poly=%u, ext_line=%u, ext_point=%u",
             m_nHeaderLength, m_nDataOffset, m_nDataSize,
             m_nExtPolygonSize, m_nExtPolylineSize, m_nExtPointSize);

    return true;
}

/************************************************************************/
/*                      DecodePolyBitstream()                           */
/************************************************************************/

bool GarminIMGRGNParser::DecodePolyBitstream(
    const uint8_t* pabyBits, uint32_t nBitstreamLen,
    int32_t nStartLon, int32_t nStartLat,
    int nShift,
    std::vector<RGNPoint>& aoPoints) {

    if (nBitstreamLen == 0) return true;
    if (nBitstreamLen < 2) return true;  // Need at least 10 bits for header (4+4+1+1)

    GarminIMGBitReader oBits(pabyBits, nBitstreamLen);

    // Read bitstream header sequentially (variable length 10-12 bits)
    // Faithful to mkgmap LinePreparer / imgforge line_preparer.rs
    int nXBase = static_cast<int>(oBits.Get(4));
    int nYBase = static_cast<int>(oBits.Get(4));
    bool bXSameSign = oBits.Get1();
    bool bXSign = false;
    if (bXSameSign) {
        bXSign = oBits.Get1();
    }
    bool bYSameSign = oBits.Get1();
    bool bYSign = false;
    if (bYSameSign) {
        bYSign = oBits.Get1();
    }

    int nXBits = GarminIMGBitReader::Base2Bits(nXBase);
    int nYBits = GarminIMGBitReader::Base2Bits(nYBase);

    // If same_sign, we don't need the sign bit
    if (!bXSameSign) nXBits++;
    if (!bYSameSign) nYBits++;

    // Current position in map units
    int32_t nCurLon = nStartLon;
    int32_t nCurLat = nStartLat;

    // First point already added by caller
    while (oBits.HasMore() && oBits.GetRemainingBits() >= static_cast<uint32_t>(nXBits + nYBits)) {
        int32_t nDeltaX, nDeltaY;

        if (bXSameSign) {
            nDeltaX = static_cast<int32_t>(oBits.Get(nXBits));
            if (bXSign) nDeltaX = -nDeltaX;
        } else {
            nDeltaX = oBits.SGet2(nXBits);
        }

        if (bYSameSign) {
            nDeltaY = static_cast<int32_t>(oBits.Get(nYBits));
            if (bYSign) nDeltaY = -nDeltaY;
        } else {
            nDeltaY = oBits.SGet2(nYBits);
        }

        nCurLon += nDeltaX << nShift;
        nCurLat += nDeltaY << nShift;

        RGNPoint oPoint;
        oPoint.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nCurLon);
        oPoint.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nCurLat);
        aoPoints.push_back(oPoint);
    }

    return true;
}

/************************************************************************/
/*                          DecodePOIs()                                */
/************************************************************************/

bool GarminIMGRGNParser::DecodePOIs(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    std::vector<RGNPOIFeature>& aoFeatures) {

    if (!(oSubdiv.nContentFlags & 0x30)) return true;  // No points

    uint32_t nStart = m_nHeaderLength + oSubdiv.nRGNOffset;
    uint32_t nEnd = (oSubdiv.nEndRGNOffset != 0xFFFFFFFF)
                    ? m_nHeaderLength + oSubdiv.nEndRGNOffset
                    : m_nSize;

    if (nStart >= m_nSize || nEnd > m_nSize) return false;

    // Skip internal pointers (2 bytes each)
    // Faithful to imgforge rgn.rs: pointers are only written if the target
    // section exists AND at least one earlier section also exists.
    // Order: points → indexed_points → polylines → polygons
    uint32_t nPos = nStart;
    bool bHasStdPoints = (oSubdiv.nContentFlags & 0x10) != 0;
    bool bHasIdxPoints = (oSubdiv.nContentFlags & 0x20) != 0;
    bool bHasPolylines = (oSubdiv.nContentFlags & 0x40) != 0;
    bool bHasPolygons  = (oSubdiv.nContentFlags & 0x80) != 0;

    int nPointers = 0;
    if (bHasIdxPoints && bHasStdPoints) nPointers++;
    if (bHasPolylines && (bHasStdPoints || bHasIdxPoints)) nPointers++;
    if (bHasPolygons && (bHasStdPoints || bHasIdxPoints || bHasPolylines)) nPointers++;

    // The order in the block is: points, indexed_points, polylines, polygons
    // Pointers point to: indexed_points, polylines, polygons
    uint32_t nPointsEnd = nEnd;
    if (nPointers > 0 && nPos + nPointers * 2 <= nEnd) {
        // Pointers are relative to subdivision start (nStart)
        uint16_t nPtrVal = ReadLE16(m_pabyData + nPos);
        nPointsEnd = nStart + nPtrVal;
        if (nPointsEnd > nEnd) nPointsEnd = nEnd;  // Bounds check
        nPos += nPointers * 2;
    }

    int nShift = 24 - oSubdiv.nResolution;

    // Decode standard POIs
    while (nPos + 8 <= nPointsEnd) {
        uint8_t nType = m_pabyData[nPos];

        // Label offset LE24 (bits 0-21 = offset, bit 22 = is_poi, bit 23 = has_subtype)
        uint32_t nLabelRaw = ReadLE24(m_pabyData + nPos + 1);
        uint32_t nLabelOffset = nLabelRaw & 0x3FFFFF;
        bool bHasSubtype = (nLabelRaw & 0x800000) != 0;

        // Delta coordinates LE16s
        int16_t nDeltaLon = ReadLE16Signed(m_pabyData + nPos + 4);
        int16_t nDeltaLat = ReadLE16Signed(m_pabyData + nPos + 6);

        RGNPOIFeature oPOI;
        oPOI.nType = nType;
        oPOI.nEndLevel = oSubdiv.nLevel;

        // Absolute coordinates
        int32_t nAbsLon = oSubdiv.nCenterLon + (static_cast<int32_t>(nDeltaLon) << nShift);
        int32_t nAbsLat = oSubdiv.nCenterLat + (static_cast<int32_t>(nDeltaLat) << nShift);
        oPOI.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nAbsLon);
        oPOI.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nAbsLat);

        nPos += 8;

        // Subtype
        if (bHasSubtype && nPos < nPointsEnd) {
            oPOI.nSubType = m_pabyData[nPos];
            nPos++;
        }

        // Label
        if (poLBL && nLabelOffset > 0) {
            oPOI.osLabel = poLBL->GetLabel(nLabelOffset);
        }

        aoFeatures.push_back(std::move(oPOI));
    }

    return true;
}

/************************************************************************/
/*                       DecodePolylines()                              */
/************************************************************************/

bool GarminIMGRGNParser::DecodePolylines(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    std::vector<RGNPolyFeature>& aoFeatures) {

    if (!(oSubdiv.nContentFlags & 0x40)) return true;

    uint32_t nStart = m_nHeaderLength + oSubdiv.nRGNOffset;
    uint32_t nEnd = (oSubdiv.nEndRGNOffset != 0xFFFFFFFF)
                    ? m_nHeaderLength + oSubdiv.nEndRGNOffset
                    : m_nSize;

    if (nStart >= m_nSize || nEnd > m_nSize) return false;

    // Find polyline section via internal pointers
    // Faithful to imgforge rgn.rs write_subdivision():
    //   ptr to ind_points: if has_ind_points && has_points
    //   ptr to polylines:  if has_polylines && (has_points || has_ind_points)
    //   ptr to polygons:   if has_polygons && (has_points || has_ind_points || has_polylines)
    uint32_t nPos = nStart;
    bool bHasStdPoints = (oSubdiv.nContentFlags & 0x10) != 0;
    bool bHasIdxPoints = (oSubdiv.nContentFlags & 0x20) != 0;
    bool bHasPolygons  = (oSubdiv.nContentFlags & 0x80) != 0;
    bool bHasPreceding = (bHasStdPoints || bHasIdxPoints);

    // Count all pointers in this subdivision block
    int nTotalPointers = 0;
    if (bHasIdxPoints && bHasStdPoints) nTotalPointers++;
    if (bHasPreceding) nTotalPointers++;  // polyline pointer
    // Polygon pointer: polylines exist (we're in DecodePolylines), so preceding is true
    if (bHasPolygons) nTotalPointers++;

    uint32_t nPolylineStart = nStart;
    uint32_t nPolylineEnd = nEnd;

    if (bHasPreceding) {
        // Polyline pointer exists — it's right after the ind_points pointer (if any)
        int nPolylinePtrIdx = (bHasIdxPoints && bHasStdPoints) ? 1 : 0;
        if (nPos + nTotalPointers * 2 <= nEnd) {
            nPolylineStart = nStart + ReadLE16(m_pabyData + nPos + nPolylinePtrIdx * 2);
        } else {
            return false;
        }
        // Polyline end = polygon pointer value, or block end
        if (bHasPolygons) {
            int nPolygonPtrIdx = nPolylinePtrIdx + 1;
            if (nPos + (nPolygonPtrIdx + 1) * 2 <= nEnd) {
                nPolylineEnd = nStart + ReadLE16(m_pabyData + nPos + nPolygonPtrIdx * 2);
            }
        }
    } else {
        // No points/indexed: polylines are the first section, no polyline pointer
        // But if polygons exist, there's 1 pointer (to polygons, since polylines precede)
        if (bHasPolygons) {
            if (nPos + 2 <= nEnd) {
                nPolylineEnd = nStart + ReadLE16(m_pabyData + nPos);
                nPolylineStart = nStart + 2;  // after the polygon pointer
            } else {
                return false;
            }
        } else {
            // Only polylines in this subdivision, no pointers at all
            nPolylineStart = nStart;
            nPolylineEnd = nEnd;
        }
    }

    // Bounds validation on pointer-derived offsets
    if (nPolylineStart > nEnd) nPolylineStart = nEnd;
    if (nPolylineEnd > nEnd) nPolylineEnd = nEnd;
    if (nPolylineStart > nPolylineEnd) return false;

    int nShift = 24 - oSubdiv.nResolution;
    nPos = nPolylineStart;

    while (nPos + 8 <= nPolylineEnd) {
        uint8_t nTypeByte = m_pabyData[nPos];
        bool bDirectionIndicator = (nTypeByte & 0x40) != 0;
        bool b2ByteLen = (nTypeByte & 0x80) != 0;
        uint8_t nType = nTypeByte & 0x3F;

        // Label/NET offset LE24
        uint32_t nLabelRaw = ReadLE24(m_pabyData + nPos + 1);
        bool bHasNetInfo = (nLabelRaw & 0x800000) != 0;
        uint32_t nLabelOffset = nLabelRaw & 0x3FFFFF;

        // First delta LE16s
        int16_t nDeltaLon = ReadLE16Signed(m_pabyData + nPos + 4);
        int16_t nDeltaLat = ReadLE16Signed(m_pabyData + nPos + 6);
        nPos += 8;

        // Bitstream length
        uint32_t nBitstreamLen;
        if (b2ByteLen) {
            if (nPos + 2 > nPolylineEnd) break;
            nBitstreamLen = ReadLE16(m_pabyData + nPos) + 1;
            nPos += 2;
        } else {
            if (nPos + 1 > nPolylineEnd) break;
            nBitstreamLen = m_pabyData[nPos] + 1;  // stored = actual - 1
            nPos++;
        }

        if (nPos + nBitstreamLen > nPolylineEnd) break;

        // Build feature
        RGNPolyFeature oPoly;
        oPoly.nType = nType;
        oPoly.bDirectionIndicator = bDirectionIndicator;
        oPoly.bHasNetInfo = bHasNetInfo;
        oPoly.nNetOffset = bHasNetInfo ? nLabelOffset : 0;
        oPoly.nEndLevel = oSubdiv.nLevel;

        if (poLBL && !bHasNetInfo && nLabelOffset > 0) {
            oPoly.osLabel = poLBL->GetLabel(nLabelOffset);
        }

        // First point
        int32_t nAbsLon = oSubdiv.nCenterLon + (static_cast<int32_t>(nDeltaLon) << nShift);
        int32_t nAbsLat = oSubdiv.nCenterLat + (static_cast<int32_t>(nDeltaLat) << nShift);

        RGNPoint oFirstPt;
        oFirstPt.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nAbsLon);
        oFirstPt.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nAbsLat);
        oPoly.aoPoints.push_back(oFirstPt);

        // Decode bitstream for remaining points
        DecodePolyBitstream(m_pabyData + nPos, nBitstreamLen,
                            nAbsLon, nAbsLat, nShift, oPoly.aoPoints);

        nPos += nBitstreamLen;
        aoFeatures.push_back(std::move(oPoly));
    }

    return true;
}

/************************************************************************/
/*                        DecodePolygons()                              */
/************************************************************************/

bool GarminIMGRGNParser::DecodePolygons(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    std::vector<RGNPolyFeature>& aoFeatures) {

    if (!(oSubdiv.nContentFlags & 0x80)) return true;

    uint32_t nStart = m_nHeaderLength + oSubdiv.nRGNOffset;
    uint32_t nEnd = (oSubdiv.nEndRGNOffset != 0xFFFFFFFF)
                    ? m_nHeaderLength + oSubdiv.nEndRGNOffset
                    : m_nSize;

    if (nStart >= m_nSize || nEnd > m_nSize) return false;

    // Find polygon section via internal pointers
    // Faithful to imgforge rgn.rs: polygon pointer exists only if preceding sections exist
    uint32_t nPos = nStart;
    bool bHasStdPoints = (oSubdiv.nContentFlags & 0x10) != 0;
    bool bHasIdxPoints = (oSubdiv.nContentFlags & 0x20) != 0;
    bool bHasPolylines = (oSubdiv.nContentFlags & 0x40) != 0;
    bool bHasPreceding = (bHasStdPoints || bHasIdxPoints || bHasPolylines);

    uint32_t nPolygonStart = nStart;

    if (bHasPreceding) {
        // Count pointers before the polygon pointer
        int nPointerIdx = 0;
        if (bHasIdxPoints && bHasStdPoints) nPointerIdx++;
        if (bHasPolylines && (bHasStdPoints || bHasIdxPoints)) nPointerIdx++;
        // Polygon pointer is next
        int nTotalPointers = nPointerIdx + 1;

        if (nPos + nTotalPointers * 2 <= nEnd) {
            nPolygonStart = nStart + ReadLE16(m_pabyData + nPos + nPointerIdx * 2);
        } else {
            return false;
        }
    }

    // Bounds validation on pointer-derived offset
    if (nPolygonStart > nEnd) nPolygonStart = nEnd;

    int nShift = 24 - oSubdiv.nResolution;
    nPos = nPolygonStart;

    while (nPos + 8 <= nEnd) {
        uint8_t nTypeByte = m_pabyData[nPos];
        bool b2ByteLen = (nTypeByte & 0x80) != 0;
        uint8_t nType = nTypeByte & 0x7F;

        uint32_t nLabelRaw = ReadLE24(m_pabyData + nPos + 1);
        uint32_t nLabelOffset = nLabelRaw & 0x3FFFFF;

        int16_t nDeltaLon = ReadLE16Signed(m_pabyData + nPos + 4);
        int16_t nDeltaLat = ReadLE16Signed(m_pabyData + nPos + 6);
        nPos += 8;

        uint32_t nBitstreamLen;
        if (b2ByteLen) {
            if (nPos + 2 > nEnd) break;
            nBitstreamLen = ReadLE16(m_pabyData + nPos) + 1;
            nPos += 2;
        } else {
            if (nPos + 1 > nEnd) break;
            nBitstreamLen = m_pabyData[nPos] + 1;
            nPos++;
        }

        if (nPos + nBitstreamLen > nEnd) break;

        RGNPolyFeature oPoly;
        oPoly.nType = nType;
        oPoly.nEndLevel = oSubdiv.nLevel;

        if (poLBL && nLabelOffset > 0) {
            oPoly.osLabel = poLBL->GetLabel(nLabelOffset);
        }

        int32_t nAbsLon = oSubdiv.nCenterLon + (static_cast<int32_t>(nDeltaLon) << nShift);
        int32_t nAbsLat = oSubdiv.nCenterLat + (static_cast<int32_t>(nDeltaLat) << nShift);

        RGNPoint oFirstPt;
        oFirstPt.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nAbsLon);
        oFirstPt.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nAbsLat);
        oPoly.aoPoints.push_back(oFirstPt);

        DecodePolyBitstream(m_pabyData + nPos, nBitstreamLen,
                            nAbsLon, nAbsLat, nShift, oPoly.aoPoints);

        nPos += nBitstreamLen;
        aoFeatures.push_back(std::move(oPoly));
    }

    return true;
}

/************************************************************************/
/*                     DecodeExtendedPOIs()                             */
/************************************************************************/

bool GarminIMGRGNParser::DecodeExtendedPOIs(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    uint32_t nExtStart, uint32_t nExtEnd,
    std::vector<RGNPOIFeature>& aoFeatures) {

    if (m_nExtPointSize == 0 || nExtStart >= nExtEnd) return true;

    // Extended points are in the ext_point section of the RGN file
    uint32_t nAbsStart = m_nExtPointOffset + nExtStart;
    uint32_t nAbsEnd   = m_nExtPointOffset + nExtEnd;
    if (nAbsStart >= m_nSize || nAbsEnd > m_nSize) return false;

    int nShift = 24 - oSubdiv.nResolution;
    uint32_t nPos = nAbsStart;

    // Extended point format (imgforge point.rs write_ext):
    //   type_high(1B) + type_low_with_flags(1B) + dx(i16 LE) + dy(i16 LE) + [label(3B)]
    while (nPos + 6 <= nAbsEnd) {
        uint8_t nTypeHigh = m_pabyData[nPos];
        uint8_t nTypeLow  = m_pabyData[nPos + 1];
        bool bHasLabel = (nTypeLow & 0x20) != 0;
        uint16_t nType = (static_cast<uint16_t>(nTypeHigh) << 8) | (nTypeLow & 0x1F);

        int16_t nDeltaLon = ReadLE16Signed(m_pabyData + nPos + 2);
        int16_t nDeltaLat = ReadLE16Signed(m_pabyData + nPos + 4);
        nPos += 6;

        RGNPOIFeature oPOI;
        oPOI.nType = nType;
        oPOI.bExtended = true;
        oPOI.nEndLevel = oSubdiv.nLevel;

        int32_t nAbsLon = oSubdiv.nCenterLon + (static_cast<int32_t>(nDeltaLon) << nShift);
        int32_t nAbsLat = oSubdiv.nCenterLat + (static_cast<int32_t>(nDeltaLat) << nShift);
        oPOI.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nAbsLon);
        oPOI.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nAbsLat);

        if (bHasLabel && nPos + 3 <= nAbsEnd) {
            uint32_t nLabelOffset = ReadLE24(m_pabyData + nPos);
            nPos += 3;
            if (poLBL && nLabelOffset > 0) {
                oPOI.osLabel = poLBL->GetLabel(nLabelOffset);
            }
        }

        aoFeatures.push_back(std::move(oPOI));
    }

    return true;
}

/************************************************************************/
/*                   DecodeExtendedPolylines()                          */
/************************************************************************/

bool GarminIMGRGNParser::DecodeExtendedPolylines(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    uint32_t nExtStart, uint32_t nExtEnd,
    std::vector<RGNPolyFeature>& aoFeatures) {
    (void)oSubdiv; (void)poLBL; (void)nExtStart; (void)nExtEnd; (void)aoFeatures;
    if (m_nExtPolylineSize == 0) return true;
    // TODO: Implement extended polyline parsing
    return true;
}

/************************************************************************/
/*                   DecodeExtendedPolygons()                           */
/************************************************************************/

bool GarminIMGRGNParser::DecodeExtendedPolygons(
    const TRESubdivision& oSubdiv,
    const GarminIMGLBLParser* poLBL,
    uint32_t nExtStart, uint32_t nExtEnd,
    std::vector<RGNPolyFeature>& aoFeatures) {
    (void)oSubdiv; (void)poLBL; (void)nExtStart; (void)nExtEnd; (void)aoFeatures;
    if (m_nExtPolygonSize == 0) return true;
    // TODO: Implement extended polygon parsing
    return true;
}
