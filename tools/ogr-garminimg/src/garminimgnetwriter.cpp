/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG NET (road network) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgnetwriter.h"

#include <algorithm>
#include <cmath>

GarminIMGNETWriter::GarminIMGNETWriter() {
}

GarminIMGNETWriter::~GarminIMGNETWriter() {
}

void GarminIMGNETWriter::WriteLE16(std::vector<uint8_t>& buf, uint16_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
}

void GarminIMGNETWriter::WriteLE24(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
}

void GarminIMGNETWriter::WriteLE32(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
    buf.push_back((val >> 24) & 0xFF);
}

uint32_t GarminIMGNETWriter::AddRoad(
    const std::vector<uint32_t>& anLabelOffsets,
    uint8_t nFlags, int nRoadClass, int nSpeed,
    bool bOneWay, bool bToll, uint8_t nAccessFlags,
    double dfLengthM) {

    uint32_t nOffset = static_cast<uint32_t>(m_abyNET1.size());

    // Label offsets (last has bit 23 clear)
    for (size_t i = 0; i < anLabelOffsets.size(); i++) {
        uint32_t nRaw = anLabelOffsets[i] & 0x3FFFFF;
        if (i < anLabelOffsets.size() - 1) {
            nRaw |= 0x400000;  // More labels follow
        }
        WriteLE24(m_abyNET1, nRaw);
    }
    if (anLabelOffsets.empty()) {
        WriteLE24(m_abyNET1, 0);  // Empty label
    }

    // Flags byte
    uint8_t nFlagsByte = nFlags | 0x04;  // bit 2 always set
    if (bOneWay) nFlagsByte |= 0x02;
    m_abyNET1.push_back(nFlagsByte);

    // Road length LE24 (meters / 4.8)
    uint32_t nLenRaw = static_cast<uint32_t>(std::round(dfLengthM / 4.8));
    WriteLE24(m_abyNET1, nLenRaw);

    // Road class + speed byte
    uint8_t nClassSpeed = static_cast<uint8_t>(nRoadClass & 0x07) |
                          (static_cast<uint8_t>(nSpeed & 0x07) << 3);
    if (bToll) nClassSpeed |= 0x80;
    m_abyNET1.push_back(nClassSpeed);

    // Level count (simplified: 1 polyline at level 0)
    m_abyNET1.push_back(0x11);  // (1 << 4) | 0x01

    // Polyline number + subdivision (placeholder)
    m_abyNET1.push_back(0x00);
    m_abyNET1.push_back(0x00);
    m_abyNET1.push_back(0x00);

    m_anNET3Index.push_back(nOffset);
    return nOffset;
}

std::vector<uint8_t> GarminIMGNETWriter::Build() {
    std::vector<uint8_t> abyResult;
    const uint16_t nHeaderLen = 55;

    // Common header
    WriteLE16(abyResult, nHeaderLen);
    abyResult.insert(abyResult.end(), {'G','A','R','M','I','N',' ','N','E','T','\0'});
    abyResult.push_back(0x01);  // version
    abyResult.push_back(0x00);  // lock

    while (abyResult.size() < 0x15) abyResult.push_back(0x00);

    // NET1 section
    WriteLE32(abyResult, nHeaderLen);                                    // 0x15
    WriteLE32(abyResult, static_cast<uint32_t>(m_abyNET1.size()));       // 0x19
    abyResult.push_back(0x00);  // addr_shift at 0x1D

    // NET2 section (empty)
    uint32_t nNET2Off = nHeaderLen + static_cast<uint32_t>(m_abyNET1.size());
    WriteLE32(abyResult, nNET2Off);  // 0x1E
    WriteLE32(abyResult, 0);         // 0x22 size

    // Pad to 0x27
    while (abyResult.size() < 0x27) abyResult.push_back(0x00);

    // NET3 section
    uint32_t nNET3Size = static_cast<uint32_t>(m_anNET3Index.size() * 3);
    WriteLE32(abyResult, nNET2Off);   // 0x27
    WriteLE32(abyResult, nNET3Size);  // 0x2B

    // Record size 3 at 0x2F
    while (abyResult.size() < 0x2F) abyResult.push_back(0x00);
    abyResult.push_back(0x03);

    // Pad to header length
    while (abyResult.size() < nHeaderLen) abyResult.push_back(0x00);

    // NET1 data
    abyResult.insert(abyResult.end(), m_abyNET1.begin(), m_abyNET1.end());

    // NET3 data (sorted index → NET1 offsets)
    auto anSorted = m_anNET3Index;
    std::sort(anSorted.begin(), anSorted.end());
    for (uint32_t nOff : anSorted) {
        WriteLE24(abyResult, nOff);
    }

    return abyResult;
}
