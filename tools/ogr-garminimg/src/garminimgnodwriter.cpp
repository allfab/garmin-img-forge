/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG NOD (routing nodes) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgnodwriter.h"
#include "garminimgbitreader.h"

GarminIMGNODWriter::GarminIMGNODWriter() {
}

GarminIMGNODWriter::~GarminIMGNODWriter() {
}

void GarminIMGNODWriter::WriteLE16(std::vector<uint8_t>& buf, uint16_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
}

void GarminIMGNODWriter::WriteLE24(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
}

void GarminIMGNODWriter::WriteLE24Signed(std::vector<uint8_t>& buf, int32_t val) {
    WriteLE24(buf, static_cast<uint32_t>(val) & 0xFFFFFF);
}

void GarminIMGNODWriter::WriteLE32(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
    buf.push_back((val >> 24) & 0xFF);
}

void GarminIMGNODWriter::AddNode(double dfLat, double dfLon,
                                  const std::vector<RoutingArc>& aoArcs) {
    int32_t nLon = GarminIMGBitReader::DegreesToMapUnits(dfLon);
    int32_t nLat = GarminIMGBitReader::DegreesToMapUnits(dfLat);

    WriteLE24Signed(m_abyNOD1, nLon);
    WriteLE24Signed(m_abyNOD1, nLat);

    WriteLE16(m_abyNOD1, static_cast<uint16_t>(aoArcs.size()));

    // Table A: 5 bytes per arc
    for (const auto& oArc : aoArcs) {
        // NET1 offset (LE24)
        uint32_t nRaw = oArc.nNET1Offset & 0x3FFFFF;
        WriteLE24(m_abyNOD1, nRaw);

        // tabAInfo byte
        uint8_t nTabA = 0;
        if (oArc.bToll) nTabA |= 0x80;
        nTabA |= (static_cast<uint8_t>(oArc.nRoadClass & 0x07) << 4);
        if (oArc.bOneWay) nTabA |= 0x08;
        nTabA |= static_cast<uint8_t>(oArc.nSpeed & 0x07);
        m_abyNOD1.push_back(nTabA);

        // Access flags byte
        m_abyNOD1.push_back(oArc.nAccessFlags);
    }
}

std::vector<uint8_t> GarminIMGNODWriter::Build() {
    std::vector<uint8_t> abyResult;
    const uint16_t nHeaderLen = 127;

    // Common header
    WriteLE16(abyResult, nHeaderLen);
    abyResult.insert(abyResult.end(), {'G','A','R','M','I','N',' ','N','O','D','\0'});
    abyResult.push_back(0x01);  // version
    abyResult.push_back(0x00);  // lock

    while (abyResult.size() < 0x15) abyResult.push_back(0x00);

    // NOD1 section
    WriteLE32(abyResult, nHeaderLen);
    WriteLE32(abyResult, static_cast<uint32_t>(m_abyNOD1.size()));

    // NOD2 (empty)
    while (abyResult.size() < 0x1F) abyResult.push_back(0x00);
    uint32_t nNOD2Off = nHeaderLen + static_cast<uint32_t>(m_abyNOD1.size());
    WriteLE32(abyResult, nNOD2Off);
    WriteLE32(abyResult, 0);

    // NOD3 (empty)
    while (abyResult.size() < 0x29) abyResult.push_back(0x00);
    WriteLE32(abyResult, nNOD2Off);
    WriteLE32(abyResult, 0);

    // Pad to header length
    while (abyResult.size() < nHeaderLen) abyResult.push_back(0x00);

    // NOD1 data
    abyResult.insert(abyResult.end(), m_abyNOD1.begin(), m_abyNOD1.end());

    return abyResult;
}
