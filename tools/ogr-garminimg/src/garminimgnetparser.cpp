/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG NET (road network) subfile
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

#include "garminimgnetparser.h"
#include "garminimglblparser.h"
#include "cpl_error.h"

#include <cstring>

/************************************************************************/
/*                     GarminIMGNETParser()                             */
/************************************************************************/

GarminIMGNETParser::GarminIMGNETParser() {
}

GarminIMGNETParser::~GarminIMGNETParser() {
}

uint16_t GarminIMGNETParser::ReadLE16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0]) | (static_cast<uint16_t>(p[1]) << 8);
}

uint32_t GarminIMGNETParser::ReadLE24(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16);
}

uint32_t GarminIMGNETParser::ReadLE32(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) |
           (static_cast<uint32_t>(p[3]) << 24);
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGNETParser::Parse(const uint8_t* pabyData, uint32_t nSize,
                               const GarminIMGLBLParser* poLBL) {
    if (!pabyData || nSize < 0x30) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG NET: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;
    m_nHeaderLength = ReadLE16(pabyData);

    // NET1 section: offset at 0x15 (LE32), size at 0x19 (LE32)
    m_nNET1Offset = ReadLE32(pabyData + 0x15);
    m_nNET1Size   = ReadLE32(pabyData + 0x19);
    m_nAddrShift  = pabyData[0x1D];

    // NET2 section
    m_nNET2Offset = ReadLE32(pabyData + 0x1E);
    m_nNET2Size   = ReadLE32(pabyData + 0x22);

    // NET3 section
    m_nNET3Offset = ReadLE32(pabyData + 0x27);
    m_nNET3Size   = ReadLE32(pabyData + 0x2B);

    CPLDebug("OGR_GARMINIMG", "NET: header=%u, NET1 at %u (%u bytes), "
             "NET2 at %u, NET3 at %u",
             m_nHeaderLength, m_nNET1Offset, m_nNET1Size,
             m_nNET2Offset, m_nNET3Offset);

    return ParseNET1(poLBL);
}

/************************************************************************/
/*                          ParseNET1()                                 */
/************************************************************************/

bool GarminIMGNETParser::ParseNET1(const GarminIMGLBLParser* poLBL) {
    if (m_nNET1Offset + m_nNET1Size > m_nSize) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG NET: NET1 section out of bounds");
        return false;
    }

    uint32_t nPos = m_nNET1Offset;
    uint32_t nEnd = m_nNET1Offset + m_nNET1Size;

    while (nPos + 3 < nEnd) {
        RoadDef oRoad;
        oRoad.nNET1Offset = nPos - m_nNET1Offset;

        // Read label offsets (LE24 each, last has bit 23 clear = 0x800000 not set)
        // Actually: bit 23 set means "more labels follow"
        while (nPos + 3 <= nEnd) {
            uint32_t nLabelRaw = ReadLE24(m_pabyData + nPos);
            nPos += 3;

            uint32_t nLabelOffset = nLabelRaw & 0x3FFFFF;
            bool bMoreLabels = (nLabelRaw & 0x400000) != 0;

            if (poLBL && nLabelOffset > 0) {
                oRoad.aosLabels.push_back(poLBL->GetLabel(nLabelOffset));
            }

            if (!bMoreLabels) break;
        }

        if (nPos + 1 > nEnd) break;

        // Flags byte
        oRoad.nFlags = m_pabyData[nPos];
        oRoad.bOneWay = (oRoad.nFlags & 0x02) != 0;
        bool bHasNOD2 = (oRoad.nFlags & 0x40) != 0;
        nPos++;

        // Road length LE24 (in meters / 4.8)
        if (nPos + 3 > nEnd) break;
        uint32_t nLenRaw = ReadLE24(m_pabyData + nPos);
        oRoad.dfLengthM = static_cast<double>(nLenRaw) * 4.8;
        nPos += 3;

        // Road class and speed are encoded in the next byte
        if (nPos + 1 > nEnd) break;
        uint8_t nClassSpeed = m_pabyData[nPos];
        oRoad.nRoadClass = nClassSpeed & 0x07;
        oRoad.nSpeed = (nClassSpeed >> 3) & 0x07;
        oRoad.bToll = (nClassSpeed & 0x80) != 0;
        nPos++;

        // Level count byte
        if (nPos + 1 > nEnd) break;
        uint8_t nLevelCount = m_pabyData[nPos];
        nPos++;

        // Skip polyline/subdivision references (variable size)
        int nPolyRefs = (nLevelCount >> 4) & 0x0F;
        // Each ref is at least 3 bytes (polyline number + subdivision)
        nPos += nPolyRefs * 3;

        // NOD2 offset (optional)
        if (bHasNOD2 && nPos + 1 <= nEnd) {
            uint8_t nNOD2SizeInd = m_pabyData[nPos];
            nPos++;
            int nOffsetSize = (nNOD2SizeInd & 0x03) + 1;
            if (nPos + nOffsetSize <= nEnd) {
                oRoad.nNOD2Offset = 0;
                for (int i = 0; i < nOffsetSize; i++) {
                    oRoad.nNOD2Offset |= static_cast<uint32_t>(m_pabyData[nPos + i]) << (i * 8);
                }
                oRoad.bHasNOD2 = true;
                nPos += nOffsetSize;
            }
        }

        m_aoRoadIndex[oRoad.nNET1Offset] = static_cast<int>(m_aoRoads.size());
        m_aoRoads.push_back(std::move(oRoad));
    }

    CPLDebug("OGR_GARMINIMG", "NET: Parsed %zu roads", m_aoRoads.size());
    return true;
}

/************************************************************************/
/*                         GetRoadDef()                                 */
/************************************************************************/

const RoadDef* GarminIMGNETParser::GetRoadDef(uint32_t nNET1Offset) const {
    auto it = m_aoRoadIndex.find(nNET1Offset);
    if (it != m_aoRoadIndex.end() && it->second < static_cast<int>(m_aoRoads.size())) {
        return &m_aoRoads[it->second];
    }
    return nullptr;
}
