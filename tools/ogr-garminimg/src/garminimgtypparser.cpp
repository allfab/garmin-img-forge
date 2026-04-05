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

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>

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

uint32_t GarminIMGTYPParser::ReadLE24(const uint8_t* p) {
    return static_cast<uint32_t>(p[0]) |
           (static_cast<uint32_t>(p[1]) << 8) |
           (static_cast<uint32_t>(p[2]) << 16);
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

/* TYP binary header layout (from mkgmap TYPHeader.java):
 *
 * 0x00-0x01: Header length (LE16)
 * 0x02-0x0B: "GARMIN TYP" signature
 * 0x0C-0x0D: Version/flags
 * 0x0E-0x14: Creation date (year LE16, month, day, hour, minute, second)
 * 0x15-0x16: Code page (LE16)
 *
 * Data sections (raw style data, variable-length records):
 * 0x17-0x1A: Polygon data offset (LE32)
 * 0x1B-0x1E: Polygon data size (LE32)
 * 0x1F-0x22: Polyline data offset (LE32)
 * 0x23-0x26: Polyline data size (LE32)
 * 0x27-0x2A: Point data offset (LE32)
 * 0x2B-0x2E: Point data size (LE32)
 *
 * 0x2F-0x30: Family ID (LE16)
 * 0x31-0x32: Product ID (LE16)
 *
 * Index sections (fixed-size records: packed_type + data_offset):
 * 0x33-0x36: Point index offset (LE32)
 * 0x37-0x38: Point index record size (LE16)
 * 0x39-0x3C: Point index size (LE32)
 *
 * 0x3D-0x40: Polyline index offset (LE32)
 * 0x41-0x42: Polyline index record size (LE16)
 * 0x43-0x46: Polyline index size (LE32)
 *
 * 0x47-0x4A: Polygon index offset (LE32)
 * 0x4B-0x4C: Polygon index record size (LE16)
 * 0x4D-0x50: Polygon index size (LE32)
 */

bool GarminIMGTYPParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x51) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TYP: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;

    uint16_t nHeaderLen = ReadLE16(pabyData);

    if (memcmp(pabyData + 0x02, "GARMIN", 6) != 0) {
        CPLDebug("OGR_GARMINIMG", "TYP: Missing GARMIN signature (non-fatal)");
    }

    if (nHeaderLen < 0x51) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG TYP: Header too short (%u bytes)", nHeaderLen);
        return false;
    }

    // Data sections (variable-length style data)
    uint32_t nPolyDataOff  = ReadLE32(pabyData + 0x17);
    uint32_t nPolyDataSize = ReadLE32(pabyData + 0x1B);
    uint32_t nLineDataOff  = ReadLE32(pabyData + 0x1F);
    uint32_t nLineDataSize = ReadLE32(pabyData + 0x23);
    uint32_t nPtDataOff    = ReadLE32(pabyData + 0x27);
    uint32_t nPtDataSize   = ReadLE32(pabyData + 0x2B);

    // Index sections (fixed-size index records)
    uint32_t nPtIdxOff   = ReadLE32(pabyData + 0x33);
    uint16_t nPtIdxRec   = ReadLE16(pabyData + 0x37);
    uint32_t nPtIdxSize  = ReadLE32(pabyData + 0x39);

    uint32_t nLineIdxOff  = ReadLE32(pabyData + 0x3D);
    uint16_t nLineIdxRec  = ReadLE16(pabyData + 0x41);
    uint32_t nLineIdxSize = ReadLE32(pabyData + 0x43);

    uint32_t nPolyIdxOff  = ReadLE32(pabyData + 0x47);
    uint16_t nPolyIdxRec  = ReadLE16(pabyData + 0x4B);
    uint32_t nPolyIdxSize = ReadLE32(pabyData + 0x4D);

    CPLDebug("OGR_GARMINIMG",
             "TYP: Point idx(off=0x%X, rec=%u, sz=%u), "
             "Polyline idx(off=0x%X, rec=%u, sz=%u), "
             "Polygon idx(off=0x%X, rec=%u, sz=%u)",
             nPtIdxOff, nPtIdxRec, nPtIdxSize,
             nLineIdxOff, nLineIdxRec, nLineIdxSize,
             nPolyIdxOff, nPolyIdxRec, nPolyIdxSize);

    // Parse each geometry type via index → data lookup
    if (nPtIdxSize > 0 && nPtIdxRec > 0) {
        ParseIndexSection(nPtIdxOff, nPtIdxSize, nPtIdxRec,
                          nPtDataOff, nPtDataSize,
                          m_aoPointStyles, 0);
    }
    if (nLineIdxSize > 0 && nLineIdxRec > 0) {
        ParseIndexSection(nLineIdxOff, nLineIdxSize, nLineIdxRec,
                          nLineDataOff, nLineDataSize,
                          m_aoPolylineStyles, 1);
    }
    if (nPolyIdxSize > 0 && nPolyIdxRec > 0) {
        ParseIndexSection(nPolyIdxOff, nPolyIdxSize, nPolyIdxRec,
                          nPolyDataOff, nPolyDataSize,
                          m_aoPolygonStyles, 2);
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
/*                      ParseIndexSection()                             */
/************************************************************************/

/* Index record format:
 *
 * Polyline/Polygon (recsize=4):
 *   LE16: packed_type = (type << 5) | (subtype & 0x1F)
 *   LE16: data_offset relative to their data section start
 *
 * Point (recsize=5):
 *   LE16: packed_type = (type << 5) | (subtype & 0x1F)
 *   LE24: data_offset relative to the data section start (3 bytes)
 *         For extended point types, this offset can reference data
 *         beyond the declared point data section (into the polyline
 *         or polygon data areas) — the TYP format stores all style
 *         data contiguously after the header.
 */

bool GarminIMGTYPParser::ParseIndexSection(
    uint32_t nIdxOffset, uint32_t nIdxSize, uint16_t nIdxRecordSize,
    uint32_t nDataOffset, uint32_t nDataSize,
    std::map<uint32_t, TypStyleDef>& aoStyles, int nGeomType) {

    if (nIdxOffset + nIdxSize > m_nSize || nIdxRecordSize == 0) {
        return false;
    }

    uint32_t nCount = nIdxSize / nIdxRecordSize;

    // Collect all entries with their data offsets, then compute sizes
    struct IndexEntry {
        uint16_t nType;
        uint16_t nSubType;
        uint32_t nDataOff;  // Relative to nDataOffset
    };
    std::vector<IndexEntry> aoEntries;
    aoEntries.reserve(nCount);

    for (uint32_t i = 0; i < nCount; i++) {
        uint32_t nRecOff = nIdxOffset + i * nIdxRecordSize;
        if (nRecOff + nIdxRecordSize > m_nSize) break;

        uint16_t nPacked = ReadLE16(m_pabyData + nRecOff);
        uint16_t nType = nPacked >> 5;
        uint16_t nSubType = nPacked & 0x1F;

        uint32_t nStyleOff = 0;
        if (nGeomType == 0 && nIdxRecordSize >= 5) {
            // Point: LE16 packed + LE24 offset
            nStyleOff = ReadLE24(m_pabyData + nRecOff + 2);
        } else if (nIdxRecordSize >= 4) {
            // Polyline/Polygon: LE16 packed + LE16 offset
            nStyleOff = ReadLE16(m_pabyData + nRecOff + 2);
        }

        aoEntries.push_back({nType, nSubType, nStyleOff});
    }

    // Parse style data for each entry
    for (size_t i = 0; i < aoEntries.size(); i++) {
        const auto& entry = aoEntries[i];

        // Compute absolute file offset for this entry's style data
        uint32_t nAbsOff = nDataOffset + entry.nDataOff;
        if (nAbsOff >= m_nSize) continue;

        // Compute available bytes until next entry or end of file data
        uint32_t nAvail;
        if (i + 1 < aoEntries.size()) {
            uint32_t nNextAbsOff = nDataOffset + aoEntries[i + 1].nDataOff;
            if (nNextAbsOff <= nAbsOff) {
                nAvail = 64;  // Fallback for non-monotonic offsets
            } else {
                nAvail = nNextAbsOff - nAbsOff;
            }
        } else {
            nAvail = m_nSize - nAbsOff;
        }
        if (nAbsOff + nAvail > m_nSize) {
            nAvail = m_nSize - nAbsOff;
        }

        TypStyleDef oStyle;
        ParseStyleData(m_pabyData + nAbsOff, nAvail, nGeomType, oStyle);

        uint32_t nKey = (static_cast<uint32_t>(entry.nType) << 16) | entry.nSubType;
        aoStyles[nKey] = std::move(oStyle);
    }

    return true;
}

/************************************************************************/
/*                        ParseStyleData()                              */
/************************************************************************/

/* Style data format (from mkgmap source — ColourInfo.java, Rgb.java):
 *
 * All colours are stored in BGR byte order (Blue, Green, Red).
 *
 * Polygon: [scheme_byte] [dayFG_BGR] [dayBG_BGR]? [nightFG_BGR]? [nightBG_BGR]?
 *   scheme bit 0x02 = day background transparent (omitted)
 *   scheme bit 0x04 = night background transparent (omitted)
 *
 * Polyline: [scheme|height] [flags] [dayFG_BGR] [dayBG_BGR]? [night...]?
 *   scheme in lower 3 bits of byte 0, bitmap height in upper 5
 *
 * Point: [flags] [width] [height] [numColors] [colorMode] [palette_BGR...]
 *   colorMode 0x10 = 3 bytes/color, 0x20 = 3 bytes + 4-bit alpha
 */

bool GarminIMGTYPParser::ParseStyleData(const uint8_t* pData, uint32_t nAvail,
                                         int nGeomType, TypStyleDef& oStyle) {
    if (nAvail < 1) return false;

    if (nGeomType == 2) {
        /* Polygon style data layout (from mkgmap TypPolygon.java):
         *
         * Byte 0: scheme flags
         *   bit 0 (0x01): S_NIGHT — has night colours
         *   bit 1 (0x02): S_DAY_TRANSPARENT — day background transparent
         *   bit 2 (0x04): S_NIGHT_TRANSPARENT — night background transparent
         *   bit 3 (0x08): S_HAS_BITMAP — followed by bitmap data
         *
         * Bytes 1+: non-transparent colours in BGR order (3 bytes each),
         *   written in order: dayFG, dayBG, nightFG, nightBG
         *   (transparent ones are omitted)
         */
        if (nAvail < 4) return false;

        uint8_t nScheme = pData[0];
        bool bDayBgTransparent = (nScheme & 0x02) != 0;

        // Day foreground (fill) — always present, BGR at bytes 1-3
        oStyle.osFillColor = ColorToHex(pData[3], pData[2], pData[1]);

        // Day background (border) — present only if not transparent
        if (!bDayBgTransparent && nAvail >= 7) {
            oStyle.osBorderColor = ColorToHex(pData[6], pData[5], pData[4]);
        }
    } else if (nGeomType == 1) {
        /* Polyline style data layout (from mkgmap TypLine.java):
         *
         * Byte 0: (scheme & 0x07) | (height << 3)
         *   scheme bits 0-2: same S_NIGHT / S_DAY_TRANSPARENT / S_NIGHT_TRANSPARENT
         *   bits 3-7: bitmap height (0 = solid line)
         * Byte 1: flags (F_LABEL, F_USE_ROTATION, F_EXTENDED)
         * Bytes 2+: non-transparent colours in BGR order (3 bytes each),
         *   written in order: dayFG (line), dayBG (border), nightFG, nightBG
         */
        if (nAvail < 5) return false;

        uint8_t nScheme = pData[0] & 0x07;
        bool bDayBgTransparent = (nScheme & 0x02) != 0;

        // Day foreground (line colour) — BGR at bytes 2-4
        oStyle.osLineColor = ColorToHex(pData[4], pData[3], pData[2]);

        // Day background (border colour) — present only if not transparent
        if (!bDayBgTransparent && nAvail >= 8) {
            oStyle.osBorderColor = ColorToHex(pData[7], pData[6], pData[5]);
        }
    } else if (nGeomType == 0) {
        /* Point style data layout (from mkgmap TypPoint.java):
         *
         * Byte 0: flags (F_BITMAP=0x01, F_NIGHT=0x02, F_LABEL=0x04, F_EXT=0x08)
         * Byte 1: icon width
         * Byte 2: icon height
         * Byte 3: number of colours in palette
         * Byte 4: colour mode (0x10 = indexed, 0x20 = with alpha)
         * Bytes 5+: colour palette in BGR order (3 bytes each for mode 0x10)
         */
        if (nAvail >= 8) {
            uint8_t nColorMode = pData[4];
            if (nColorMode == 0x20) {
                // Mode 0x20: BGR + 4-bit alpha per colour (3.5 bytes each)
                // Extract first colour BGR from bytes 5-7, skip 4-bit alpha
                oStyle.osFillColor = ColorToHex(pData[7], pData[6], pData[5]);
            } else {
                // Mode 0x10 or default: BGR 3 bytes each
                oStyle.osFillColor = ColorToHex(pData[7], pData[6], pData[5]);
            }
        }
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

/************************************************************************/
/*                        ParseTextFile()                               */
/************************************************************************/

/* Parses decompiled TYP text format (.txt) as produced by cgpsmapper
 * or GMapTool. Format is INI-like with sections:
 *
 *   [_polygon]          / [_line]         / [_point]
 *   Type=0xNN           / Type=0xNNNNN
 *   Xpm="W H C D"
 *   "C c #RRGGBB"       (color definitions)
 *   LineWidth=N          (lines only)
 *   BorderWidth=N        (lines only)
 *   String1=0xLL,Name    (localized name)
 *   [end]
 */

static std::string TrimWhitespace(const std::string& s) {
    size_t start = s.find_first_not_of(" \t\r\n");
    if (start == std::string::npos) return "";
    size_t end = s.find_last_not_of(" \t\r\n");
    return s.substr(start, end - start + 1);
}

bool GarminIMGTYPParser::ParseTextFile(const char* pszFilename) {
    std::ifstream ifs(pszFilename);
    if (!ifs.is_open()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "GarminIMG TYP: Cannot open text file: %s", pszFilename);
        return false;
    }

    enum SectionType { NONE, POLYGON, LINE, POINT };

    SectionType eSection = NONE;
    uint32_t nType = 0;
    uint16_t nSubType = 0;
    TypStyleDef oStyle;
    std::vector<std::string> aosColors;
    int nLineWidth = 0;
    int nBorderWidth = 0;
    bool bInSection = false;

    auto CommitSection = [&]() {
        if (!bInSection) return;

        // Extract colors from XPM color definitions
        // First non-"none" color = primary, second = secondary
        std::string osPrimaryColor;
        std::string osSecondaryColor;
        for (const auto& c : aosColors) {
            if (c != "none") {
                if (osPrimaryColor.empty())
                    osPrimaryColor = c;
                else if (osSecondaryColor.empty())
                    osSecondaryColor = c;
            }
        }

        uint32_t nKey = (static_cast<uint32_t>(nType) << 16) | nSubType;

        switch (eSection) {
        case POLYGON:
            oStyle.osFillColor = osPrimaryColor;
            if (!osSecondaryColor.empty())
                oStyle.osBorderColor = osSecondaryColor;
            m_aoPolygonStyles[nKey] = std::move(oStyle);
            break;
        case LINE:
            oStyle.osLineColor = osPrimaryColor;
            if (!osSecondaryColor.empty())
                oStyle.osBorderColor = osSecondaryColor;
            oStyle.nLineWidth = nLineWidth > 0 ? nLineWidth : 1;
            m_aoPolylineStyles[nKey] = std::move(oStyle);
            break;
        case POINT:
            oStyle.osFillColor = osPrimaryColor;
            m_aoPointStyles[nKey] = std::move(oStyle);
            break;
        default:
            break;
        }

        // Reset state
        oStyle = TypStyleDef();
        aosColors.clear();
        nLineWidth = 0;
        nBorderWidth = 0;
        bInSection = false;
    };

    std::string osLine;
    while (std::getline(ifs, osLine)) {
        std::string osTrimmed = TrimWhitespace(osLine);

        // Skip empty lines and comments
        if (osTrimmed.empty() || osTrimmed[0] == ';') continue;

        // Section headers
        if (osTrimmed == "[_polygon]") {
            CommitSection();
            eSection = POLYGON;
            bInSection = true;
            continue;
        }
        if (osTrimmed == "[_line]") {
            CommitSection();
            eSection = LINE;
            bInSection = true;
            continue;
        }
        if (osTrimmed == "[_point]") {
            CommitSection();
            eSection = POINT;
            bInSection = true;
            continue;
        }
        if (osTrimmed == "[end]" || osTrimmed == "[End]") {
            CommitSection();
            eSection = NONE;
            continue;
        }

        // Skip non-style sections
        if (osTrimmed[0] == '[') {
            bInSection = false;
            eSection = NONE;
            continue;
        }

        if (!bInSection) continue;

        // Parse Type=0xNNNNN
        if (osTrimmed.substr(0, 5) == "Type=") {
            std::string osVal = osTrimmed.substr(5);
            unsigned long nVal = strtoul(osVal.c_str(), nullptr, 0);

            if (eSection == POINT) {
                // Point: Type=0xTTTSS → type=TTT, subtype=SS
                nType = static_cast<uint16_t>((nVal >> 8) & 0xFFFF);
                nSubType = static_cast<uint16_t>(nVal & 0xFF);
            } else {
                // Polyline/Polygon: Type=0xNNN or Type=0x1NNSS
                if (nVal > 0xFF) {
                    // Extended type: 0x1NNSS → type=0x1NN, subtype=SS
                    nType = static_cast<uint16_t>((nVal >> 8) & 0xFFFF);
                    nSubType = static_cast<uint16_t>(nVal & 0xFF);
                } else {
                    // Standard type
                    nType = static_cast<uint16_t>(nVal);
                    nSubType = 0;
                }
            }
            continue;
        }

        // Parse LineWidth=N
        if (osTrimmed.substr(0, 10) == "LineWidth=") {
            nLineWidth = atoi(osTrimmed.c_str() + 10);
            continue;
        }

        // Parse BorderWidth=N
        if (osTrimmed.substr(0, 12) == "BorderWidth=") {
            nBorderWidth = atoi(osTrimmed.c_str() + 12);
            (void)nBorderWidth;  // stored in style if needed
            continue;
        }

        // Parse XPM color lines: "C c #RRGGBB" or "C c none"
        if (osTrimmed.size() > 2 && osTrimmed[0] == '"') {
            // Find " c #" or " c none" pattern
            size_t nCPos = osTrimmed.find(" c ");
            if (nCPos == std::string::npos) {
                nCPos = osTrimmed.find("\tc ");
            }
            if (nCPos != std::string::npos) {
                std::string osColorVal = TrimWhitespace(
                    osTrimmed.substr(nCPos + 3));
                // Remove trailing quote if present
                if (!osColorVal.empty() && osColorVal.back() == '"')
                    osColorVal.pop_back();
                osColorVal = TrimWhitespace(osColorVal);

                if (osColorVal == "none") {
                    aosColors.push_back("none");
                } else if (osColorVal.size() >= 7 && osColorVal[0] == '#') {
                    aosColors.push_back(osColorVal.substr(0, 7));
                }
            }
            continue;
        }

        // Parse String (localized name) — take first one as display name
        if (osTrimmed.substr(0, 7) == "String1" ||
            osTrimmed.substr(0, 7) == "String2") {
            size_t nComma = osTrimmed.find(',');
            if (nComma != std::string::npos && oStyle.osDisplayName.empty()) {
                oStyle.osDisplayName = TrimWhitespace(
                    osTrimmed.substr(nComma + 1));
            }
            continue;
        }
    }

    // Commit last section if file doesn't end with [end]
    CommitSection();

    CPLDebug("OGR_GARMINIMG",
             "TYP text: Parsed %zu point, %zu polyline, %zu polygon styles",
             m_aoPointStyles.size(), m_aoPolylineStyles.size(),
             m_aoPolygonStyles.size());

    return true;
}
