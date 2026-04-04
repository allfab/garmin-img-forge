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

#ifndef GARMINIMGNODPARSER_H_INCLUDED
#define GARMINIMGNODPARSER_H_INCLUDED

#include <cstdint>
#include <string>
#include <vector>

/************************************************************************/
/*                       RoutingArc                                     */
/************************************************************************/

struct RoutingArc {
    uint32_t nNET1Offset = 0;
    uint8_t nAccessFlags = 0;
    int nRoadClass = 0;
    int nSpeed = 0;
    bool bOneWay = false;
    bool bToll = false;
};

/************************************************************************/
/*                       RoutingNode                                    */
/************************************************************************/

struct RoutingNode {
    double dfLon = 0.0;
    double dfLat = 0.0;
    std::string osNodeType;   // "junction" or "endpoint"
    std::vector<RoutingArc> aoArcs;
};

/************************************************************************/
/*                     GarminIMGNODParser                               */
/************************************************************************/

class GarminIMGNODParser {
public:
    GarminIMGNODParser();
    ~GarminIMGNODParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize);

    const std::vector<RoutingNode>& GetNodes() const { return m_aoNodes; }
    int GetNodeCount() const { return static_cast<int>(m_aoNodes.size()); }

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    uint16_t m_nHeaderLength = 0;

    // NOD1 section
    uint32_t m_nNOD1Offset = 0;
    uint32_t m_nNOD1Size = 0;

    // NOD2 section
    uint32_t m_nNOD2Offset = 0;
    uint32_t m_nNOD2Size = 0;

    // NOD3 section
    uint32_t m_nNOD3Offset = 0;
    uint32_t m_nNOD3Size = 0;

    std::vector<RoutingNode> m_aoNodes;

    bool ParseNOD1();

    static uint16_t ReadLE16(const uint8_t* p);
    static uint32_t ReadLE24(const uint8_t* p);
    static int32_t ReadLE24Signed(const uint8_t* p);
    static uint32_t ReadLE32(const uint8_t* p);
};

#endif /* GARMINIMGNODPARSER_H_INCLUDED */
