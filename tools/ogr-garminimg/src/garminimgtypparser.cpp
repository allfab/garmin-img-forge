/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG TYP (symbology) subfile
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

#include "garminimgtypparser.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_vsi.h"

#include <cstdio>
#include <cstring>

/************************************************************************/
/*                     GarminIMGTYPParser()                             */
/************************************************************************/

GarminIMGTYPParser::GarminIMGTYPParser() {
}

GarminIMGTYPParser::~GarminIMGTYPParser() {
}

uint16_t GarminIMGTYPParser::ReadLE16(const uint8_t* p) {
    return static_cast<uint16_t>(p[0]) | (static_cast<uint16_t>(p[1]) << 8);
}

uint32_t GarminIMGTYPParser::ReadLE32(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16) |
           (static_cast<uint32_t>(p[3]) << 24);
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGTYPParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x40) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TYP: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;

    // TYP header structure (varies by version)
    // Header length at 0x00 (LE16)
    uint16_t nHeaderLen = ReadLE16(pabyData);

    // Check for "GARMIN TYP" signature
    if (memcmp(pabyData + 0x02, "GARMIN", 6) != 0) {
        CPLDebug("OGR_GARMINIMG", "TYP: Missing GARMIN signature (non-fatal)");
    }

    // Section offsets depend on the TYP version
    // Common layout:
    // Point section: offset at 0x15 (LE32), size at 0x19 (LE32), record size at 0x1D (LE16)
    // Polyline section: offset at 0x1F (LE32), size at 0x23 (LE32), record size at 0x27 (LE16)
    // Polygon section: offset at 0x29 (LE32), size at 0x2D (LE32), record size at 0x31 (LE16)

    if (nHeaderLen >= 0x33) {
        uint32_t nPointOff  = ReadLE32(pabyData + 0x15);
        uint32_t nPointSize = ReadLE32(pabyData + 0x19);
        uint16_t nPointRec  = ReadLE16(pabyData + 0x1D);

        uint32_t nLineOff  = ReadLE32(pabyData + 0x1F);
        uint32_t nLineSize = ReadLE32(pabyData + 0x23);
        uint16_t nLineRec  = ReadLE16(pabyData + 0x27);

        uint32_t nPolyOff  = ReadLE32(pabyData + 0x29);
        uint32_t nPolySize = ReadLE32(pabyData + 0x2D);
        uint16_t nPolyRec  = ReadLE16(pabyData + 0x31);

        // Parse sections
        if (nPointSize > 0 && nPointRec > 0) {
            ParseSection(nPointOff, nPointSize, nPointRec, m_aoPointStyles, 0);
        }
        if (nLineSize > 0 && nLineRec > 0) {
            ParseSection(nLineOff, nLineSize, nLineRec, m_aoPolylineStyles, 1);
        }
        if (nPolySize > 0 && nPolyRec > 0) {
            ParseSection(nPolyOff, nPolySize, nPolyRec, m_aoPolygonStyles, 2);
        }
    }

    CPLDebug("OGR_GARMINIMG", "TYP: Parsed %zu point, %zu polyline, %zu polygon styles",
             m_aoPointStyles.size(), m_aoPolylineStyles.size(), m_aoPolygonStyles.size());

    return true;
}

/************************************************************************/
/*                          ParseFile()                                 */
/************************************************************************/

bool GarminIMGTYPParser::ParseFile(const char* pszFilename) {
    VSILFILE* fp = VSIFOpenL(pszFilename, "rb");
    if (!fp) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG TYP: Cannot open file: %s", pszFilename);
        return false;
    }

    VSIFSeekL(fp, 0, SEEK_END);
    vsi_l_offset nFileSize = VSIFTellL(fp);
    VSIFSeekL(fp, 0, SEEK_SET);

    if (nFileSize > 10 * 1024 * 1024) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TYP: File too large (%llu bytes)",
                 static_cast<unsigned long long>(nFileSize));
        VSIFCloseL(fp);
        return false;
    }

    m_abyOwnedData.resize(static_cast<size_t>(nFileSize));
    if (VSIFReadL(m_abyOwnedData.data(), 1, static_cast<size_t>(nFileSize), fp) !=
        static_cast<size_t>(nFileSize)) {
        CPLError(CE_Failure, CPLE_FileIO,
                 "GarminIMG TYP: Read error on file: %s", pszFilename);
        VSIFCloseL(fp);
        return false;
    }

    VSIFCloseL(fp);
    return Parse(m_abyOwnedData.data(), static_cast<uint32_t>(m_abyOwnedData.size()));
}

/************************************************************************/
/*                        ParseSection()                                */
/************************************************************************/

bool GarminIMGTYPParser::ParseSection(uint32_t nOffset, uint32_t nSize,
                                      uint16_t nRecordSize,
                                      std::map<uint32_t, TypStyleDef>& aoStyles,
                                      int nGeomType) {
    if (nOffset + nSize > m_nSize || nRecordSize == 0) {
        return false;
    }

    int nCount = nSize / nRecordSize;

    // TYP index record layout:
    //   Polyline/Polygon: type_code (LE16) + style_data_offset (LE32) [+ more]
    //   Point: type_code (LE16) + subtype (byte) + style_data_offset (LE32) [+ more]
    //
    // The style_data_offset points into the TYP data area where the actual
    // color/icon/pattern info is stored. The record may be larger than the
    // minimum (nRecordSize includes padding/extra fields).

    for (int i = 0; i < nCount; i++) {
        uint32_t nRecOff = nOffset + static_cast<uint32_t>(i) * nRecordSize;
        if (nRecOff + nRecordSize > m_nSize) break;

        uint16_t nType = ReadLE16(m_pabyData + nRecOff);
        uint16_t nSubType = 0;

        uint32_t nStyleOff = 0;

        if (nGeomType == 0) {
            // Point: type (2B) + subtype (1B) + offset (LE32)
            if (nRecordSize < 7) continue;
            nSubType = m_pabyData[nRecOff + 2];
            nStyleOff = ReadLE32(m_pabyData + nRecOff + 3);
        } else {
            // Polyline/Polygon: type (2B) + offset (LE32)
            if (nRecordSize < 6) continue;
            nStyleOff = ReadLE32(m_pabyData + nRecOff + 2);
        }

        TypStyleDef oStyle;

        // Read color info from style data (with strict bounds checking)
        if (nStyleOff > 0 && nStyleOff < m_nSize) {
            uint32_t nAvail = m_nSize - nStyleOff;
            const uint8_t* pStyle = m_pabyData + nStyleOff;

            if (nGeomType == 2 && nAvail >= 3) {
                // Polygon: fill color
                oStyle.osFillColor = ColorToHex(pStyle[0], pStyle[1], pStyle[2]);
                if (nAvail >= 6) {
                    oStyle.osBorderColor = ColorToHex(pStyle[3], pStyle[4], pStyle[5]);
                }
            } else if (nGeomType == 1 && nAvail >= 3) {
                // Polyline: line color
                oStyle.osLineColor = ColorToHex(pStyle[0], pStyle[1], pStyle[2]);
                if (nAvail >= 4) {
                    oStyle.nLineWidth = pStyle[3];
                }
            } else if (nGeomType == 0 && nAvail >= 3) {
                // Point: icon color
                oStyle.osFillColor = ColorToHex(pStyle[0], pStyle[1], pStyle[2]);
            }
        }

        uint32_t nKey = (static_cast<uint32_t>(nType) << 16) | nSubType;
        aoStyles[nKey] = std::move(oStyle);
    }

    return true;
}

/************************************************************************/
/*                          ColorToHex()                                */
/************************************************************************/

std::string GarminIMGTYPParser::ColorToHex(uint8_t r, uint8_t g, uint8_t b) const {
    char szBuf[8];
    snprintf(szBuf, sizeof(szBuf), "#%02X%02X%02X", r, g, b);
    return szBuf;
}

/************************************************************************/
/*                          GetTypInfo()                                */
/************************************************************************/

const TypStyleDef* GarminIMGTYPParser::GetTypInfo(uint16_t nType,
                                                   uint16_t nSubType) const {
    uint32_t nKey = (static_cast<uint32_t>(nType) << 16) | nSubType;

    // Search all style maps
    auto it = m_aoPointStyles.find(nKey);
    if (it != m_aoPointStyles.end()) return &it->second;

    it = m_aoPolylineStyles.find(nKey);
    if (it != m_aoPolylineStyles.end()) return &it->second;

    it = m_aoPolygonStyles.find(nKey);
    if (it != m_aoPolygonStyles.end()) return &it->second;

    return nullptr;
}
