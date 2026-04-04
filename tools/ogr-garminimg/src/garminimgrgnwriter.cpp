/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG RGN (geometry) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimgrgnwriter.h"

GarminIMGRGNWriter::GarminIMGRGNWriter() {
}

GarminIMGRGNWriter::~GarminIMGRGNWriter() {
}

void GarminIMGRGNWriter::WriteLE16(std::vector<uint8_t>& buf, uint16_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
}

void GarminIMGRGNWriter::WriteLE24(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
}

void GarminIMGRGNWriter::WriteLE32(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
    buf.push_back((val >> 24) & 0xFF);
}

void GarminIMGRGNWriter::WritePOI(uint8_t nType, uint32_t nLabelOffset,
                                   int16_t nDeltaLon, int16_t nDeltaLat,
                                   uint8_t nSubType, bool bHasSubType) {
    m_abyBody.push_back(nType);

    // Label offset LE24 with flags
    uint32_t nLabelRaw = nLabelOffset & 0x3FFFFF;
    nLabelRaw |= 0x400000;  // bit 22 = is_poi
    if (bHasSubType) nLabelRaw |= 0x800000;  // bit 23 = has_subtype
    WriteLE24(m_abyBody, nLabelRaw);

    // Delta coordinates LE16s
    WriteLE16(m_abyBody, static_cast<uint16_t>(nDeltaLon));
    WriteLE16(m_abyBody, static_cast<uint16_t>(nDeltaLat));

    if (bHasSubType) {
        m_abyBody.push_back(nSubType);
    }
}

void GarminIMGRGNWriter::WritePolyline(uint8_t nType, uint32_t nLabelOffset,
                                        int16_t nFirstDeltaLon,
                                        int16_t nFirstDeltaLat,
                                        const std::vector<uint8_t>& abyBitstream,
                                        bool bDirectionIndicator,
                                        bool bHasNetInfo) {
    // Type byte: bit 6 = direction, bit 7 = 2-byte length
    uint8_t nTypeByte = nType & 0x3F;
    if (bDirectionIndicator) nTypeByte |= 0x40;

    // CRITICAL: bitstream length = actual_bytes - 1
    uint32_t nBitstreamLen = static_cast<uint32_t>(abyBitstream.size());
    uint32_t nStoredLen = (nBitstreamLen > 0) ? nBitstreamLen - 1 : 0;

    // CRITICAL: 2-byte length when stored >= 256
    bool b2ByteLen = (nStoredLen >= 256);
    if (b2ByteLen) nTypeByte |= 0x80;

    m_abyBody.push_back(nTypeByte);

    // Label/NET offset LE24
    uint32_t nLabelRaw = nLabelOffset & 0x3FFFFF;
    if (bHasNetInfo) nLabelRaw |= 0x800000;
    WriteLE24(m_abyBody, nLabelRaw);

    // First point delta LE16s
    WriteLE16(m_abyBody, static_cast<uint16_t>(nFirstDeltaLon));
    WriteLE16(m_abyBody, static_cast<uint16_t>(nFirstDeltaLat));

    // Bitstream length
    if (b2ByteLen) {
        WriteLE16(m_abyBody, static_cast<uint16_t>(nStoredLen));
    } else {
        m_abyBody.push_back(static_cast<uint8_t>(nStoredLen));
    }

    // Bitstream data
    m_abyBody.insert(m_abyBody.end(), abyBitstream.begin(), abyBitstream.end());
}

void GarminIMGRGNWriter::WritePolygon(uint8_t nType, uint32_t nLabelOffset,
                                       int16_t nFirstDeltaLon,
                                       int16_t nFirstDeltaLat,
                                       const std::vector<uint8_t>& abyBitstream) {
    uint32_t nBitstreamLen = static_cast<uint32_t>(abyBitstream.size());
    uint32_t nStoredLen = (nBitstreamLen > 0) ? nBitstreamLen - 1 : 0;

    uint8_t nTypeByte = nType & 0x7F;
    if (nStoredLen >= 256) nTypeByte |= 0x80;

    m_abyBody.push_back(nTypeByte);

    uint32_t nLabelRaw = nLabelOffset & 0x3FFFFF;
    WriteLE24(m_abyBody, nLabelRaw);

    WriteLE16(m_abyBody, static_cast<uint16_t>(nFirstDeltaLon));
    WriteLE16(m_abyBody, static_cast<uint16_t>(nFirstDeltaLat));

    if (nStoredLen >= 256) {
        WriteLE16(m_abyBody, static_cast<uint16_t>(nStoredLen));
    } else {
        m_abyBody.push_back(static_cast<uint8_t>(nStoredLen));
    }

    m_abyBody.insert(m_abyBody.end(), abyBitstream.begin(), abyBitstream.end());
}

std::vector<uint8_t> GarminIMGRGNWriter::Build() {
    std::vector<uint8_t> abyResult;
    const uint16_t nHeaderLen = 125;

    // Common header
    WriteLE16(abyResult, nHeaderLen);
    abyResult.insert(abyResult.end(), {'G','A','R','M','I','N',' ','R','G','N','\0'});
    abyResult.push_back(0x01);  // version
    abyResult.push_back(0x00);  // lock

    // Pad to 0x15
    while (abyResult.size() < 0x15) abyResult.push_back(0x00);

    // Standard data section offset and size
    WriteLE32(abyResult, nHeaderLen);
    WriteLE32(abyResult, static_cast<uint32_t>(m_abyBody.size()));

    // Extended sections (empty)
    while (abyResult.size() < nHeaderLen) abyResult.push_back(0x00);

    // Body data (offsets are body-relative!)
    abyResult.insert(abyResult.end(), m_abyBody.begin(), m_abyBody.end());

    return abyResult;
}
