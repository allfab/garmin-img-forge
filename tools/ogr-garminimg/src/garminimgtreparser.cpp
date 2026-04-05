/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG TRE (index spatial) subfile
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

#include "garminimgtreparser.h"
#include "garminimgbitreader.h"
#include "cpl_error.h"

#include <algorithm>
#include <cstring>

// Little-endian read helpers
static inline uint16_t ReadLE16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0]) | (static_cast<uint16_t>(p[1]) << 8);
}

static inline uint32_t ReadLE24(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16);
}

static inline int32_t ReadLE24Signed(const uint8_t* p) {
    uint32_t val = ReadLE24(p);
    if (val & 0x800000) {
        return static_cast<int32_t>(val | 0xFF000000u);
    }
    return static_cast<int32_t>(val);
}

static inline uint32_t ReadLE32(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) |
           (static_cast<uint32_t>(p[3]) << 24);
}

/************************************************************************/
/*                     GarminIMGTREParser()                             */
/************************************************************************/

GarminIMGTREParser::GarminIMGTREParser() {
}

GarminIMGTREParser::~GarminIMGTREParser() {
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGTREParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x21) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TRE: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;

    // Common header
    m_nHeaderLength = ReadLE16(pabyData);

    // Check GARMIN signature
    if (memcmp(pabyData + 0x02, "GARMIN", 6) != 0) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "GarminIMG TRE: Missing GARMIN signature");
    }

    if (m_nHeaderLength < 0x74) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TRE: Header too short (%u bytes)", m_nHeaderLength);
        return false;
    }

    // Bounds: north, east, south, west as signed 24-bit LE (0x15-0x20)
    int32_t nNorth = ReadLE24Signed(pabyData + 0x15);
    int32_t nEast  = ReadLE24Signed(pabyData + 0x18);
    int32_t nSouth = ReadLE24Signed(pabyData + 0x1B);
    int32_t nWest  = ReadLE24Signed(pabyData + 0x1E);

    m_oBounds.dfNorth = GarminIMGBitReader::MapUnitsToDegrees(nNorth);
    m_oBounds.dfEast  = GarminIMGBitReader::MapUnitsToDegrees(nEast);
    m_oBounds.dfSouth = GarminIMGBitReader::MapUnitsToDegrees(nSouth);
    m_oBounds.dfWest  = GarminIMGBitReader::MapUnitsToDegrees(nWest);

    // Map levels section: offset at 0x21 (LE32), size at 0x25 (LE32)
    uint32_t nLevelsOffset = ReadLE32(pabyData + 0x21);
    uint32_t nLevelsSize   = ReadLE32(pabyData + 0x25);

    // Subdivisions section: offset at 0x29 (LE32), size at 0x2D (LE32)
    uint32_t nSubdivsOffset = ReadLE32(pabyData + 0x29);
    uint32_t nSubdivsSize   = ReadLE32(pabyData + 0x2D);

    // Map properties at 0x3E
    uint8_t nMapProps = pabyData[0x3E];
    m_bHasRouting = (nMapProps & 0x01) != 0;
    m_bTransparent = (nMapProps & 0x20) != 0;

    // Overviews
    uint32_t nPolylineOvOffset = ReadLE32(pabyData + 0x49);
    uint32_t nPolylineOvSize   = ReadLE32(pabyData + 0x4D);
    uint32_t nPolygonOvOffset  = ReadLE32(pabyData + 0x56);
    uint32_t nPolygonOvSize    = ReadLE32(pabyData + 0x5A);
    uint32_t nPointOvOffset    = ReadLE32(pabyData + 0x63);
    uint32_t nPointOvSize      = ReadLE32(pabyData + 0x67);

    // Map ID at 0x70
    if (m_nHeaderLength >= 0x74) {
        m_nMapID = ReadLE32(pabyData + 0x70);
    }

    // Extended type offsets at 0x7C (if header is large enough)
    uint32_t nExtTypeOffsetsOffset = 0;
    uint32_t nExtTypeOffsetsSize = 0;
    if (m_nHeaderLength >= 0x86 && nSize >= 0x86) {
        nExtTypeOffsetsOffset = ReadLE32(pabyData + 0x7C);
        nExtTypeOffsetsSize   = ReadLE32(pabyData + 0x80);
    }

    // Parse sections
    if (!ParseLevels(nLevelsOffset, nLevelsSize)) {
        return false;
    }

    if (!ParseSubdivisions(nSubdivsOffset, nSubdivsSize)) {
        return false;
    }

    // Parse overviews (non-fatal if they fail)
    ParseOverviews(nPolylineOvOffset, nPolylineOvSize, 2, m_aoPolylineOverviews);
    ParseOverviews(nPolygonOvOffset, nPolygonOvSize, 2, m_aoPolygonOverviews);
    ParseOverviews(nPointOvOffset, nPointOvSize, 3, m_aoPointOverviews);

    CalculateEndOffsets();

    // Parse extended type offsets (non-fatal)
    if (nExtTypeOffsetsSize > 0) {
        ParseExtTypeOffsets(nExtTypeOffsetsOffset, nExtTypeOffsetsSize);
    }

    CPLDebug("OGR_GARMINIMG", "TRE: bounds=(%.6f,%.6f)-(%.6f,%.6f), "
             "levels=%zu, subdivs=%zu, mapID=%u, routing=%d, transparent=%d, extTypeOffsets=%zu",
             m_oBounds.dfWest, m_oBounds.dfSouth,
             m_oBounds.dfEast, m_oBounds.dfNorth,
             m_aoLevels.size(), m_aoSubdivisions.size(),
             m_nMapID, m_bHasRouting, m_bTransparent,
             m_aoExtTypeOffsets.size());

    return true;
}

/************************************************************************/
/*                          ParseLevels()                               */
/************************************************************************/

bool GarminIMGTREParser::ParseLevels(uint32_t nOffset, uint32_t nSize) {
    if (nOffset + nSize > m_nSize) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TRE: Levels section out of bounds");
        return false;
    }

    int nCount = nSize / 4;  // 4 bytes per level
    m_aoLevels.reserve(nCount);

    for (int i = 0; i < nCount; i++) {
        const uint8_t* p = m_pabyData + nOffset + i * 4;

        TRELevel oLevel;
        uint8_t nLevelByte = p[0];
        oLevel.nLevel = nLevelByte & 0x0F;
        oLevel.bInherited = (nLevelByte & 0x80) != 0;
        oLevel.nResolution = p[1];
        oLevel.nSubdivCount = ReadLE16(p + 2);

        m_aoLevels.push_back(oLevel);
    }

    return true;
}

/************************************************************************/
/*                       ParseSubdivisions()                            */
/************************************************************************/

bool GarminIMGTREParser::ParseSubdivisions(uint32_t nOffset, uint32_t nSize) {
    if (nOffset + nSize > m_nSize) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TRE: Subdivisions section out of bounds");
        return false;
    }

    // Subdivision size: 14 bytes for leaf (no children), 16 bytes with children
    // Overview levels have children (first_child field), leaf levels don't
    uint32_t nPos = nOffset;
    int nLevelIdx = 0;
    int nSubdivsInLevel = 0;

    if (nSize < 18) return true;  // Need at least 14 bytes + 4-byte terminator

    while (nPos + 14 <= nOffset + nSize - 4) {
        if (nLevelIdx >= static_cast<int>(m_aoLevels.size())) break;

        bool bHasChildren = (nLevelIdx < static_cast<int>(m_aoLevels.size()) - 1);
        int nEntrySize = bHasChildren ? 16 : 14;

        if (nPos + nEntrySize > nOffset + nSize - 4) break;

        const uint8_t* p = m_pabyData + nPos;

        TRESubdivision oSubdiv;

        // RGN offset LE24 at 0x00 (body-relative!)
        oSubdiv.nRGNOffset = ReadLE24(p);

        // Content flags at 0x03
        oSubdiv.nContentFlags = p[0x03];

        // Center longitude: signed 24-bit LE at 0x04
        oSubdiv.nCenterLon = ReadLE24Signed(p + 0x04);

        // Center latitude: signed 24-bit LE at 0x07
        oSubdiv.nCenterLat = ReadLE24Signed(p + 0x07);

        // Width at 0x0A (bits 0-14 = width, bit 15 = last_subdiv_in_level)
        uint16_t nWidthRaw = ReadLE16(p + 0x0A);
        oSubdiv.nWidth = nWidthRaw & 0x7FFF;
        oSubdiv.bLastSubdiv = (nWidthRaw & 0x8000) != 0;

        // Height at 0x0C
        oSubdiv.nHeight = ReadLE16(p + 0x0C);

        // First child at 0x0E (only if has children)
        if (bHasChildren) {
            oSubdiv.nFirstChild = ReadLE16(p + 0x0E);
        }

        // Assign level info
        oSubdiv.nLevel = m_aoLevels[nLevelIdx].nLevel;
        oSubdiv.nResolution = m_aoLevels[nLevelIdx].nResolution;

        m_aoSubdivisions.push_back(oSubdiv);
        nPos += nEntrySize;

        nSubdivsInLevel++;
        // Advance to next level based on expected count from level header.
        // The bLastSubdiv flag in the data is informational but the level
        // header's subdivision count is authoritative.
        if (nSubdivsInLevel >= m_aoLevels[nLevelIdx].nSubdivCount) {
            nLevelIdx++;
            nSubdivsInLevel = 0;
        }
    }

    // Read terminator (4 bytes LE32 = lastRgnPos)
    // Faithful to mkgmap/imgforge: this is the end offset of the last subdivision's RGN data
    if (nPos + 4 <= nOffset + nSize) {
        uint32_t nTerminator = ReadLE32(m_pabyData + nPos);
        // Sanity: terminator must be >= last subdivision's offset to be valid
        if (!m_aoSubdivisions.empty() &&
            nTerminator >= m_aoSubdivisions.back().nRGNOffset &&
            nTerminator > 0) {
            m_nLastRGNPos = nTerminator;
        }
    }

    return true;
}

/************************************************************************/
/*                        ParseOverviews()                              */
/************************************************************************/

bool GarminIMGTREParser::ParseOverviews(uint32_t nOffset, uint32_t nSize,
                                        int nItemSize,
                                        std::vector<TREOverview>& aoOverviews) {
    if (nSize == 0) return true;
    if (nOffset + nSize > m_nSize) return false;

    int nCount = nSize / nItemSize;
    aoOverviews.reserve(nCount);

    for (int i = 0; i < nCount; i++) {
        const uint8_t* p = m_pabyData + nOffset + i * nItemSize;

        TREOverview oOv;
        oOv.nType = p[0];
        oOv.nMaxLevel = p[1];
        if (nItemSize >= 3) {
            oOv.nSubType = p[2];
        }

        aoOverviews.push_back(oOv);
    }

    return true;
}

/************************************************************************/
/*                      CalculateEndOffsets()                           */
/************************************************************************/

void GarminIMGTREParser::CalculateEndOffsets() {
    // Calculate end RGN offsets: each subdivision's data ends where the next starts
    for (size_t i = 0; i < m_aoSubdivisions.size(); i++) {
        if (i + 1 < m_aoSubdivisions.size()) {
            m_aoSubdivisions[i].nEndRGNOffset =
                m_aoSubdivisions[i + 1].nRGNOffset;
        } else {
            // Last subdivision: use the TRE terminator (lastRgnPos)
            m_aoSubdivisions[i].nEndRGNOffset = m_nLastRGNPos;
        }
    }
}

/************************************************************************/
/*                    GetSubdivisionsAtLevel()                          */
/************************************************************************/

std::vector<int> GarminIMGTREParser::GetSubdivisionsAtLevel(int nLevel) const {
    std::vector<int> anIndices;
    for (size_t i = 0; i < m_aoSubdivisions.size(); i++) {
        if (m_aoSubdivisions[i].nLevel == nLevel) {
            anIndices.push_back(static_cast<int>(i));
        }
    }
    return anIndices;
}

/************************************************************************/
/*                        GetFinestLevel()                              */
/************************************************************************/

int GarminIMGTREParser::GetFinestLevel() const {
    if (m_aoLevels.empty()) return 0;

    // The finest level is the one with the highest resolution (most bits),
    // which is typically level 0. We skip inherited levels as they don't
    // contain their own data.
    int nFinest = -1;
    int nBestResolution = 0;
    for (const auto& oLevel : m_aoLevels) {
        if (!oLevel.bInherited && oLevel.nResolution > nBestResolution) {
            nBestResolution = oLevel.nResolution;
            nFinest = oLevel.nLevel;
        }
    }
    return (nFinest >= 0) ? nFinest : 0;
}

/************************************************************************/
/*                      ParseExtTypeOffsets()                           */
/************************************************************************/

bool GarminIMGTREParser::ParseExtTypeOffsets(uint32_t nOffset, uint32_t nSize) {
    if (nOffset + nSize > m_nSize || nSize < 13) return false;

    // Each record is 13 bytes: areas_off(4) + lines_off(4) + points_off(4) + kinds(1)
    // One record per subdivision (including topdiv at index 0) + 1 terminator.
    // kinds == 0 does NOT mean terminator — subdivisions without ext data also
    // have kinds == 0. Parse all records based on count.
    int nCount = nSize / 13;
    m_aoExtTypeOffsets.reserve(nCount);

    for (int i = 0; i < nCount; i++) {
        const uint8_t* p = m_pabyData + nOffset + i * 13;

        TREExtTypeOffset oExt;
        oExt.nExtAreasOffset  = ReadLE32(p);
        oExt.nExtLinesOffset  = ReadLE32(p + 4);
        oExt.nExtPointsOffset = ReadLE32(p + 8);
        oExt.nKinds           = p[12];

        m_aoExtTypeOffsets.push_back(oExt);
    }

    CPLDebug("OGR_GARMINIMG", "TRE: parsed %zu extTypeOffset records",
             m_aoExtTypeOffsets.size());

    return true;
}
