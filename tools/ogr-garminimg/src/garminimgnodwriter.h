/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG NOD (routing nodes) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGNODWRITER_H_INCLUDED
#define GARMINIMGNODWRITER_H_INCLUDED

#include "garminimgnodparser.h"

#include <cstdint>
#include <vector>

class GarminIMGNODWriter {
public:
    GarminIMGNODWriter();
    ~GarminIMGNODWriter();

    void AddNode(double dfLat, double dfLon,
                 const std::vector<RoutingArc>& aoArcs);

    std::vector<uint8_t> Build();

private:
    std::vector<uint8_t> m_abyNOD1;

    static void WriteLE16(std::vector<uint8_t>& buf, uint16_t val);
    static void WriteLE24(std::vector<uint8_t>& buf, uint32_t val);
    static void WriteLE24Signed(std::vector<uint8_t>& buf, int32_t val);
    static void WriteLE32(std::vector<uint8_t>& buf, uint32_t val);
};

#endif /* GARMINIMGNODWRITER_H_INCLUDED */
