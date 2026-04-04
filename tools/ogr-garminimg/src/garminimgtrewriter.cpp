/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG TRE (index spatial) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgtrewriter.h"
#include "garminimgbitreader.h"

#include <cstring>

GarminIMGTREWriter::GarminIMGTREWriter() {
}

GarminIMGTREWriter::~GarminIMGTREWriter() {
}

void GarminIMGTREWriter::WriteLE16(std::vector<uint8_t>& buf, uint16_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
}

void GarminIMGTREWriter::WriteLE24(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
}

void GarminIMGTREWriter::WriteLE24Signed(std::vector<uint8_t>& buf, int32_t val) {
    WriteLE24(buf, static_cast<uint32_t>(val) & 0xFFFFFF);
}

void GarminIMGTREWriter::WriteLE32(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
    buf.push_back((val >> 24) & 0xFF);
}

void GarminIMGTREWriter::SetBounds(double dfNorth, double dfEast,
                                    double dfSouth, double dfWest) {
    m_nNorth = GarminIMGBitReader::DegreesToMapUnits(dfNorth);
    m_nEast  = GarminIMGBitReader::DegreesToMapUnits(dfEast);
    m_nSouth = GarminIMGBitReader::DegreesToMapUnits(dfSouth);
    m_nWest  = GarminIMGBitReader::DegreesToMapUnits(dfWest);
}

void GarminIMGTREWriter::SetMapProperties(bool bHasRouting, bool bTransparent,
                                           int nPriority) {
    m_nMapProps = 0;
    if (bHasRouting)  m_nMapProps |= 0x01;
    if (bTransparent) m_nMapProps |= 0x20;
    m_nDrawPriority = nPriority;
}

void GarminIMGTREWriter::AddLevel(int nResolution, bool bInherited) {
    TREWriterLevel oLevel;
    oLevel.nLevel = static_cast<int>(m_aoLevels.size());
    oLevel.nResolution = nResolution;
    oLevel.bInherited = bInherited;
    m_aoLevels.push_back(oLevel);
}

void GarminIMGTREWriter::AddSubdivision(const TREWriterSubdiv& oSubdiv) {
    m_aoSubdivs.push_back(oSubdiv);
}

std::vector<uint8_t> GarminIMGTREWriter::Build(uint32_t nLastRGNPos) {
    std::vector<uint8_t> abyResult;

    // Common header (21 bytes) + TRE header (to 0x74 minimum = 188 bytes)
    const uint16_t nHeaderLen = 188;
    WriteLE16(abyResult, nHeaderLen);      // 0x00
    abyResult.insert(abyResult.end(), {'G','A','R','M','I','N',' ','T','R','E','\0'}); // 0x02
    abyResult.push_back(0x01);             // 0x0D: version
    abyResult.push_back(0x00);             // 0x0E: lock

    // Pad to 0x15
    while (abyResult.size() < 0x15) abyResult.push_back(0x00);

    // Bounds at 0x15-0x20
    WriteLE24Signed(abyResult, m_nNorth);  // 0x15
    WriteLE24Signed(abyResult, m_nEast);   // 0x18
    WriteLE24Signed(abyResult, m_nSouth);  // 0x1B
    WriteLE24Signed(abyResult, m_nWest);   // 0x1E

    // Section pointers - will be filled after building sections
    // Levels at 0x21
    uint32_t nLevelsOffset = nHeaderLen;
    uint32_t nLevelsSize = static_cast<uint32_t>(m_aoLevels.size() * 4);
    WriteLE32(abyResult, nLevelsOffset);   // 0x21
    WriteLE32(abyResult, nLevelsSize);     // 0x25

    // Subdivisions at 0x29
    uint32_t nSubdivsOffset = nLevelsOffset + nLevelsSize;
    // Each subdiv is 14 or 16 bytes + 4-byte terminator
    uint32_t nSubdivsSize = 0;
    for (size_t i = 0; i < m_aoSubdivs.size(); i++) {
        // Leaf subdivisions = 14 bytes, non-leaf = 16 bytes
        nSubdivsSize += 14;  // Simplified: all leaf for now
    }
    nSubdivsSize += 4;  // Terminator
    WriteLE32(abyResult, nSubdivsOffset);  // 0x29
    WriteLE32(abyResult, nSubdivsSize);    // 0x2D

    // Copyright at 0x31 (empty)
    uint32_t nCopyrightOffset = nSubdivsOffset + nSubdivsSize;
    WriteLE32(abyResult, nCopyrightOffset); // 0x31
    WriteLE32(abyResult, 0);               // 0x35 size=0

    // Pad to 0x3E
    while (abyResult.size() < 0x3E) abyResult.push_back(0x00);

    // Map properties at 0x3E
    abyResult.push_back(m_nMapProps);

    // Pad to 0x49
    while (abyResult.size() < 0x49) abyResult.push_back(0x00);

    // Polyline overviews at 0x49 (empty)
    WriteLE32(abyResult, nCopyrightOffset); // 0x49
    WriteLE32(abyResult, 0);               // 0x4D

    // Pad to 0x56
    while (abyResult.size() < 0x56) abyResult.push_back(0x00);

    // Polygon overviews at 0x56 (empty)
    WriteLE32(abyResult, nCopyrightOffset); // 0x56
    WriteLE32(abyResult, 0);               // 0x5A

    // Pad to 0x63
    while (abyResult.size() < 0x63) abyResult.push_back(0x00);

    // Point overviews at 0x63 (empty)
    WriteLE32(abyResult, nCopyrightOffset); // 0x63
    WriteLE32(abyResult, 0);               // 0x67

    // Pad to 0x70
    while (abyResult.size() < 0x70) abyResult.push_back(0x00);

    // Map ID at 0x70
    WriteLE32(abyResult, m_nMapID);

    // Pad to header length
    while (abyResult.size() < nHeaderLen) abyResult.push_back(0x00);

    // Build levels section
    for (const auto& oLevel : m_aoLevels) {
        uint8_t nLevelByte = static_cast<uint8_t>(oLevel.nLevel);
        if (oLevel.bInherited) nLevelByte |= 0x80;
        abyResult.push_back(nLevelByte);
        abyResult.push_back(static_cast<uint8_t>(oLevel.nResolution));

        // Count subdivisions at this level
        uint16_t nCount = 0;
        for (const auto& s : m_aoSubdivs) {
            // Simplified: all at level 0 for now
            (void)s;
            nCount++;
        }
        if (&oLevel == &m_aoLevels.back()) {
            nCount = static_cast<uint16_t>(m_aoSubdivs.size());
        } else {
            nCount = 0;
        }
        WriteLE16(abyResult, nCount);
    }

    // Build subdivisions section
    for (size_t i = 0; i < m_aoSubdivs.size(); i++) {
        const auto& s = m_aoSubdivs[i];

        WriteLE24(abyResult, s.nRGNOffset);
        abyResult.push_back(s.nContentFlags);
        WriteLE24Signed(abyResult, s.nCenterLon);
        WriteLE24Signed(abyResult, s.nCenterLat);

        uint16_t nWidthRaw = s.nWidth;
        if (s.bLastSubdiv || i == m_aoSubdivs.size() - 1) {
            nWidthRaw |= 0x8000;
        }
        WriteLE16(abyResult, nWidthRaw);
        WriteLE16(abyResult, s.nHeight);
    }

    // CRITICAL: Write terminator (4 bytes lastRgnPos)
    WriteLE32(abyResult, nLastRGNPos);

    return abyResult;
}
