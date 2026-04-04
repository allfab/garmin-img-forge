/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG NET (road network) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGNETWRITER_H_INCLUDED
#define GARMINIMGNETWRITER_H_INCLUDED

#include <cstdint>
#include <string>
#include <vector>

class GarminIMGNETWriter {
public:
    GarminIMGNETWriter();
    ~GarminIMGNETWriter();

    uint32_t AddRoad(const std::vector<uint32_t>& anLabelOffsets,
                     uint8_t nFlags, int nRoadClass, int nSpeed,
                     bool bOneWay, bool bToll, uint8_t nAccessFlags,
                     double dfLengthM);

    std::vector<uint8_t> Build();

private:
    std::vector<uint8_t> m_abyNET1;
    std::vector<uint32_t> m_anNET3Index;

    static void WriteLE16(std::vector<uint8_t>& buf, uint16_t val);
    static void WriteLE24(std::vector<uint8_t>& buf, uint32_t val);
    static void WriteLE32(std::vector<uint8_t>& buf, uint32_t val);
};

#endif /* GARMINIMGNETWRITER_H_INCLUDED */
