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

#ifndef GARMINIMGTREPARSER_H_INCLUDED
#define GARMINIMGTREPARSER_H_INCLUDED

#include <cstdint>
#include <string>
#include <vector>

/************************************************************************/
/*                        TRE Structures                                */
/************************************************************************/

struct TREBounds {
    double dfNorth = 0.0;
    double dfEast = 0.0;
    double dfSouth = 0.0;
    double dfWest = 0.0;
};

struct TRELevel {
    int nLevel = 0;
    int nResolution = 0;
    bool bInherited = false;
    uint16_t nSubdivCount = 0;
};

struct TRESubdivision {
    uint32_t nRGNOffset = 0;      // Body-relative offset into RGN
    uint8_t nContentFlags = 0;    // 0x10=points, 0x20=idx_points, 0x40=polylines, 0x80=polygons
    int32_t nCenterLon = 0;       // Map units (24-bit signed)
    int32_t nCenterLat = 0;       // Map units (24-bit signed)
    uint16_t nWidth = 0;
    uint16_t nHeight = 0;
    uint16_t nFirstChild = 0;
    bool bLastSubdiv = false;
    int nLevel = 0;
    int nResolution = 0;
    uint32_t nEndRGNOffset = 0;   // Calculated: next subdiv's RGN offset or terminator
};

struct TREOverview {
    uint8_t nType = 0;
    uint8_t nSubType = 0;   // Only for point overviews (3 bytes)
    uint8_t nMaxLevel = 0;
};

/************************************************************************/
/*                     GarminIMGTREParser                               */
/************************************************************************/

class GarminIMGTREParser {
public:
    GarminIMGTREParser();
    ~GarminIMGTREParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize);

    const TREBounds& GetBounds() const { return m_oBounds; }
    const std::vector<TRELevel>& GetLevels() const { return m_aoLevels; }
    const std::vector<TRESubdivision>& GetSubdivisions() const { return m_aoSubdivisions; }
    const std::vector<TREOverview>& GetPointOverviews() const { return m_aoPointOverviews; }
    const std::vector<TREOverview>& GetPolylineOverviews() const { return m_aoPolylineOverviews; }
    const std::vector<TREOverview>& GetPolygonOverviews() const { return m_aoPolygonOverviews; }

    bool HasRouting() const { return m_bHasRouting; }
    bool IsTransparent() const { return m_bTransparent; }
    uint32_t GetMapID() const { return m_nMapID; }
    uint32_t GetRGNHeaderLength() const { return m_nRGNHeaderLength; }

    // Get subdivisions at a specific level (finest = highest level number)
    std::vector<int> GetSubdivisionsAtLevel(int nLevel) const;
    int GetFinestLevel() const;

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    uint16_t m_nHeaderLength = 0;

    TREBounds m_oBounds;
    std::vector<TRELevel> m_aoLevels;
    std::vector<TRESubdivision> m_aoSubdivisions;
    std::vector<TREOverview> m_aoPointOverviews;
    std::vector<TREOverview> m_aoPolylineOverviews;
    std::vector<TREOverview> m_aoPolygonOverviews;

    bool m_bHasRouting = false;
    bool m_bTransparent = false;
    uint32_t m_nMapID = 0;
    uint32_t m_nRGNHeaderLength = 125;  // Default RGN header size

    bool ParseLevels(uint32_t nOffset, uint32_t nSize);
    bool ParseSubdivisions(uint32_t nOffset, uint32_t nSize);
    bool ParseOverviews(uint32_t nOffset, uint32_t nSize, int nItemSize,
                        std::vector<TREOverview>& aoOverviews);
    void CalculateEndOffsets();
};

#endif /* GARMINIMGTREPARSER_H_INCLUDED */
