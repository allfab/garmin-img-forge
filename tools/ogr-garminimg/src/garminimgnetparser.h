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

#ifndef GARMINIMGNETPARSER_H_INCLUDED
#define GARMINIMGNETPARSER_H_INCLUDED

#include <cstdint>
#include <string>
#include <vector>
#include <map>

/************************************************************************/
/*                        RoadDef                                       */
/************************************************************************/

struct RoadDef {
    std::vector<std::string> aosLabels;
    uint8_t nFlags = 0;
    int nRoadClass = 0;     // 0-7
    int nSpeed = 0;         // 0-7
    bool bOneWay = false;
    bool bToll = false;
    uint8_t nAccessFlags = 0;
    double dfLengthM = 0.0; // Road length in meters
    uint32_t nNOD2Offset = 0;
    bool bHasNOD2 = false;
    uint32_t nNET1Offset = 0;  // Offset in NET1 section
};

/************************************************************************/
/*                     GarminIMGNETParser                               */
/************************************************************************/

class GarminIMGNETParser {
public:
    GarminIMGNETParser();
    ~GarminIMGNETParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize,
               const class GarminIMGLBLParser* poLBL);

    const RoadDef* GetRoadDef(uint32_t nNET1Offset) const;
    const std::vector<RoadDef>& GetAllRoads() const { return m_aoRoads; }
    int GetRoadCount() const { return static_cast<int>(m_aoRoads.size()); }

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    uint16_t m_nHeaderLength = 0;

    // NET1 section (road definitions)
    uint32_t m_nNET1Offset = 0;
    uint32_t m_nNET1Size = 0;
    uint8_t m_nAddrShift = 0;

    // NET2 section
    uint32_t m_nNET2Offset = 0;
    uint32_t m_nNET2Size = 0;

    // NET3 section (sorted index)
    uint32_t m_nNET3Offset = 0;
    uint32_t m_nNET3Size = 0;

    std::vector<RoadDef> m_aoRoads;
    std::map<uint32_t, int> m_aoRoadIndex;  // NET1 offset -> index in m_aoRoads

    bool ParseNET1(const class GarminIMGLBLParser* poLBL);

    static uint16_t ReadLE16(const uint8_t* p);
    static uint32_t ReadLE24(const uint8_t* p);
    static uint32_t ReadLE32(const uint8_t* p);
};

#endif /* GARMINIMGNETPARSER_H_INCLUDED */
