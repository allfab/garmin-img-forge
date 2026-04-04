/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG NOD (routing nodes) subfile
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

#include "garminimgnodparser.h"
#include "garminimgbitreader.h"
#include "cpl_error.h"

#include <cstring>

/************************************************************************/
/*                     GarminIMGNODParser()                             */
/************************************************************************/

GarminIMGNODParser::GarminIMGNODParser() {
}

GarminIMGNODParser::~GarminIMGNODParser() {
}

uint16_t GarminIMGNODParser::ReadLE16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0]) | (static_cast<uint16_t>(p[1]) << 8);
}

uint32_t GarminIMGNODParser::ReadLE24(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16);
}

int32_t GarminIMGNODParser::ReadLE24Signed(const uint8_t* p) {
    uint32_t val = ReadLE24(p);
    if (val & 0x800000) return static_cast<int32_t>(val | 0xFF000000u);
    return static_cast<int32_t>(val);
}

uint32_t GarminIMGNODParser::ReadLE32(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) |
           (static_cast<uint32_t>(p[3]) << 24);
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGNODParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x20) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG NOD: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;
    m_nHeaderLength = ReadLE16(pabyData);

    // NOD1: offset at 0x15 (LE32), size at 0x19 (LE32)
    m_nNOD1Offset = ReadLE32(pabyData + 0x15);
    m_nNOD1Size   = ReadLE32(pabyData + 0x19);

    // NOD2
    if (m_nHeaderLength >= 0x27) {
        m_nNOD2Offset = ReadLE32(pabyData + 0x1F);
        m_nNOD2Size   = ReadLE32(pabyData + 0x23);
    }

    // NOD3
    if (m_nHeaderLength >= 0x30) {
        m_nNOD3Offset = ReadLE32(pabyData + 0x29);
        m_nNOD3Size   = ReadLE32(pabyData + 0x2D);
    }

    CPLDebug("OGR_GARMINIMG", "NOD: header=%u, NOD1 at %u (%u bytes), "
             "NOD2 at %u (%u bytes), NOD3 at %u (%u bytes)",
             m_nHeaderLength, m_nNOD1Offset, m_nNOD1Size,
             m_nNOD2Offset, m_nNOD2Size, m_nNOD3Offset, m_nNOD3Size);

    return ParseNOD1();
}

/************************************************************************/
/*                          ParseNOD1()                                 */
/************************************************************************/

bool GarminIMGNODParser::ParseNOD1() {
    if (m_nNOD1Offset + m_nNOD1Size > m_nSize) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG NOD: NOD1 section out of bounds");
        return false;
    }

    uint32_t nPos = m_nNOD1Offset;
    uint32_t nEnd = m_nNOD1Offset + m_nNOD1Size;

    while (nPos + 12 < nEnd) {
        RoutingNode oNode;

        // Bounding box: 4 × LE24 signed (north, east, south, west)
        // Simplified: read longitude and latitude for node position
        // The actual format stores coords as 24-bit signed map units
        int32_t nLon = ReadLE24Signed(m_pabyData + nPos);
        int32_t nLat = ReadLE24Signed(m_pabyData + nPos + 3);
        nPos += 6;

        oNode.dfLon = GarminIMGBitReader::MapUnitsToDegrees(nLon);
        oNode.dfLat = GarminIMGBitReader::MapUnitsToDegrees(nLat);

        // Number of arcs
        if (nPos + 2 > nEnd) break;
        uint16_t nArcCount = ReadLE16(m_pabyData + nPos);
        nPos += 2;

        // Determine node type
        oNode.osNodeType = (nArcCount <= 2) ? "endpoint" : "junction";

        // Table A: 5 bytes per arc
        for (uint16_t i = 0; i < nArcCount; i++) {
            if (nPos + 5 > nEnd) break;

            RoutingArc oArc;
            // NET1 offset (LE24, bits 0-21)
            uint32_t nRaw = ReadLE24(m_pabyData + nPos);
            oArc.nNET1Offset = nRaw & 0x3FFFFF;

            // tabAInfo byte
            uint8_t nTabA = m_pabyData[nPos + 3];
            oArc.bToll = (nTabA & 0x80) != 0;
            oArc.nRoadClass = (nTabA >> 4) & 0x07;
            oArc.bOneWay = (nTabA & 0x08) != 0;
            oArc.nSpeed = nTabA & 0x07;

            // Access flags byte
            oArc.nAccessFlags = m_pabyData[nPos + 4];

            oNode.aoArcs.push_back(oArc);
            nPos += 5;
        }

        m_aoNodes.push_back(std::move(oNode));
    }

    CPLDebug("OGR_GARMINIMG", "NOD: Parsed %zu routing nodes", m_aoNodes.size());
    return true;
}
