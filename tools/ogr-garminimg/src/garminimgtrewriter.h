/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG TRE (index spatial) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGTREWRITER_H_INCLUDED
#define GARMINIMGTREWRITER_H_INCLUDED

#include <cstdint>
#include <vector>

struct TREWriterLevel {
    int nLevel = 0;
    int nResolution = 0;
    bool bInherited = false;
};

struct TREWriterSubdiv {
    uint32_t nRGNOffset = 0;
    uint8_t nContentFlags = 0;
    int32_t nCenterLon = 0;
    int32_t nCenterLat = 0;
    uint16_t nWidth = 0;
    uint16_t nHeight = 0;
    uint16_t nFirstChild = 0;
    bool bLastSubdiv = false;
};

class GarminIMGTREWriter {
public:
    GarminIMGTREWriter();
    ~GarminIMGTREWriter();

    void SetBounds(double dfNorth, double dfEast, double dfSouth, double dfWest);
    void SetMapID(uint32_t nMapID) { m_nMapID = nMapID; }
    void SetMapProperties(bool bHasRouting, bool bTransparent, int nPriority);

    void AddLevel(int nResolution, bool bInherited);
    void AddSubdivision(const TREWriterSubdiv& oSubdiv);

    std::vector<uint8_t> Build(uint32_t nLastRGNPos);

private:
    int32_t m_nNorth = 0, m_nEast = 0, m_nSouth = 0, m_nWest = 0;
    uint32_t m_nMapID = 0;
    uint8_t m_nMapProps = 0;
    int m_nDrawPriority = 25;

    std::vector<TREWriterLevel> m_aoLevels;
    std::vector<TREWriterSubdiv> m_aoSubdivs;

    static void WriteLE16(std::vector<uint8_t>& buf, uint16_t val);
    static void WriteLE24(std::vector<uint8_t>& buf, uint32_t val);
    static void WriteLE24Signed(std::vector<uint8_t>& buf, int32_t val);
    static void WriteLE32(std::vector<uint8_t>& buf, uint32_t val);
};

#endif /* GARMINIMGTREWRITER_H_INCLUDED */
