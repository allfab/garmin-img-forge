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

#ifndef GARMINIMGTYPPARSER_H_INCLUDED
#define GARMINIMGTYPPARSER_H_INCLUDED

#include <cstdint>
#include <map>
#include <string>
#include <vector>

/************************************************************************/
/*                       TypStyleDef                                    */
/************************************************************************/

struct TypStyleDef {
    std::string osFillColor;    // "#RRGGBB" or empty
    std::string osBorderColor;  // "#RRGGBB" or empty
    std::string osLineColor;    // "#RRGGBB" or empty
    int nLineWidth = 0;
    std::vector<uint8_t> abyIconData;    // Raw PNG/BMP icon data
    std::vector<uint8_t> abyPatternData; // Raw pattern data
    std::string osDisplayName;  // Localized type name
};

/************************************************************************/
/*                     GarminIMGTYPParser                               */
/************************************************************************/

class GarminIMGTYPParser {
public:
    GarminIMGTYPParser();
    ~GarminIMGTYPParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize);
    bool ParseFile(const char* pszFilename);
    bool ParseTextFile(const char* pszFilename);

    // Get style info for a type+subtype combination
    // Key = (type << 16) | subtype
    const TypStyleDef* GetTypInfo(uint16_t nType, uint16_t nSubType) const;

    bool HasStyles() const { return !m_aoPointStyles.empty() ||
                                    !m_aoPolylineStyles.empty() ||
                                    !m_aoPolygonStyles.empty(); }

    const std::map<uint32_t, TypStyleDef>& GetPointStyles() const { return m_aoPointStyles; }
    const std::map<uint32_t, TypStyleDef>& GetPolylineStyles() const { return m_aoPolylineStyles; }
    const std::map<uint32_t, TypStyleDef>& GetPolygonStyles() const { return m_aoPolygonStyles; }

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    std::vector<uint8_t> m_abyOwnedData;  // When loaded from file

    // Styles indexed by (type << 16) | subtype
    std::map<uint32_t, TypStyleDef> m_aoPointStyles;
    std::map<uint32_t, TypStyleDef> m_aoPolylineStyles;
    std::map<uint32_t, TypStyleDef> m_aoPolygonStyles;

    // Binary parsing helpers
    bool ParseIndexSection(uint32_t nIdxOffset, uint32_t nIdxSize,
                           uint16_t nIdxRecordSize,
                           uint32_t nDataOffset, uint32_t nDataSize,
                           std::map<uint32_t, TypStyleDef>& aoStyles,
                           int nGeomType);
    bool ParseStyleData(const uint8_t* pData, uint32_t nAvail,
                        int nGeomType, TypStyleDef& oStyle);
    std::string ColorToHex(uint8_t r, uint8_t g, uint8_t b) const;

    static uint16_t ReadLE16(const uint8_t* p);
    static uint32_t ReadLE32(const uint8_t* p);
    static uint32_t ReadLE24(const uint8_t* p);
};

#endif /* GARMINIMGTYPPARSER_H_INCLUDED */
