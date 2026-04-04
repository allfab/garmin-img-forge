/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG RGN (geometry) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGRGNWRITER_H_INCLUDED
#define GARMINIMGRGNWRITER_H_INCLUDED

#include "garminimgbitwriter.h"
#include "garminimgrgnparser.h"

#include <cstdint>
#include <vector>

class GarminIMGRGNWriter {
public:
    GarminIMGRGNWriter();
    ~GarminIMGRGNWriter();

    void WritePOI(uint8_t nType, uint32_t nLabelOffset,
                  int16_t nDeltaLon, int16_t nDeltaLat,
                  uint8_t nSubType = 0, bool bHasSubType = false);

    void WritePolyline(uint8_t nType, uint32_t nLabelOffset,
                       int16_t nFirstDeltaLon, int16_t nFirstDeltaLat,
                       const std::vector<uint8_t>& abyBitstream,
                       bool bDirectionIndicator = false,
                       bool bHasNetInfo = false);

    void WritePolygon(uint8_t nType, uint32_t nLabelOffset,
                      int16_t nFirstDeltaLon, int16_t nFirstDeltaLat,
                      const std::vector<uint8_t>& abyBitstream);

    uint32_t GetCurrentOffset() const {
        return static_cast<uint32_t>(m_abyBody.size());
    }

    std::vector<uint8_t> Build();

private:
    std::vector<uint8_t> m_abyBody;

    static void WriteLE16(std::vector<uint8_t>& buf, uint16_t val);
    static void WriteLE24(std::vector<uint8_t>& buf, uint32_t val);
    static void WriteLE32(std::vector<uint8_t>& buf, uint32_t val);
};

#endif /* GARMINIMGRGNWRITER_H_INCLUDED */
