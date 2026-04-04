/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG LBL (labels) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#include "garminimglblwriter.h"

#include <cstring>

GarminIMGLBLWriter::GarminIMGLBLWriter() {
    // Reserve offset 0 for empty label
    m_abyLabelData.push_back(0x00);
}

GarminIMGLBLWriter::~GarminIMGLBLWriter() {
}

void GarminIMGLBLWriter::WriteLE16(std::vector<uint8_t>& buf, uint16_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
}

void GarminIMGLBLWriter::WriteLE32(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
    buf.push_back((val >> 24) & 0xFF);
}

void GarminIMGLBLWriter::WriteLE24(std::vector<uint8_t>& buf, uint32_t val) {
    buf.push_back(val & 0xFF);
    buf.push_back((val >> 8) & 0xFF);
    buf.push_back((val >> 16) & 0xFF);
}

uint32_t GarminIMGLBLWriter::AddLabel(const std::string& osLabel) {
    if (osLabel.empty()) return 0;

    auto it = m_aoLabelIndex.find(osLabel);
    if (it != m_aoLabelIndex.end()) {
        return it->second;
    }

    uint32_t nOffset = static_cast<uint32_t>(m_abyLabelData.size());
    m_aoLabelIndex[osLabel] = nOffset;

    switch (m_nEncodingFormat) {
        case 6:  EncodeFormat6(osLabel); break;
        case 9:  EncodeFormat9(osLabel); break;
        case 10: EncodeFormat10(osLabel); break;
        default: EncodeFormat10(osLabel); break;
    }

    return nOffset;
}

void GarminIMGLBLWriter::EncodeFormat6(const std::string& osLabel) {
    // Pack 4 6-bit chars per 3 bytes
    std::vector<uint8_t> aChars;
    for (char c : osLabel) {
        uint8_t val;
        if (c == ' ') val = 0x00;
        else if (c >= 'A' && c <= 'Z') val = static_cast<uint8_t>(c - 'A' + 1);
        else if (c >= 'a' && c <= 'z') val = static_cast<uint8_t>(c - 'a' + 1);
        else if (c >= '0' && c <= '9') val = static_cast<uint8_t>(c - '0' + 0x20);
        else val = 0x00;  // Default to space for unsupported chars
        aChars.push_back(val);
    }
    aChars.push_back(0x3F);  // Terminator

    // Pad to multiple of 4
    while (aChars.size() % 4 != 0) {
        aChars.push_back(0x3F);
    }

    // Pack groups of 4 into 3 bytes
    for (size_t i = 0; i < aChars.size(); i += 4) {
        uint32_t packed = aChars[i] |
                          (aChars[i + 1] << 6) |
                          (aChars[i + 2] << 12) |
                          (aChars[i + 3] << 18);
        m_abyLabelData.push_back(packed & 0xFF);
        m_abyLabelData.push_back((packed >> 8) & 0xFF);
        m_abyLabelData.push_back((packed >> 16) & 0xFF);
    }
}

void GarminIMGLBLWriter::EncodeFormat9(const std::string& osLabel) {
    for (char c : osLabel) {
        m_abyLabelData.push_back(static_cast<uint8_t>(c));
    }
    m_abyLabelData.push_back(0x00);
}

void GarminIMGLBLWriter::EncodeFormat10(const std::string& osLabel) {
    for (char c : osLabel) {
        m_abyLabelData.push_back(static_cast<uint8_t>(c));
    }
    m_abyLabelData.push_back(0x00);
}

std::vector<uint8_t> GarminIMGLBLWriter::Build() {
    std::vector<uint8_t> abyResult;

    // Common header (21 bytes)
    const uint16_t nHeaderLen = 196;
    WriteLE16(abyResult, nHeaderLen);       // 0x00: header length
    abyResult.insert(abyResult.end(), {'G','A','R','M','I','N',' ','L','B','L','\0'});  // 0x02: signature
    abyResult.push_back(0x01);             // 0x0D: version
    abyResult.push_back(0x00);             // 0x0E: lock

    // Pad to 0x15 (date fields etc.)
    while (abyResult.size() < 0x15) {
        abyResult.push_back(0x00);
    }

    // Label data section offset and size at 0x15
    WriteLE32(abyResult, nHeaderLen);      // 0x15: label data offset
    WriteLE32(abyResult, static_cast<uint32_t>(m_abyLabelData.size()));  // 0x19: label data size

    // Pad to 0x1E
    while (abyResult.size() < 0x1E) {
        abyResult.push_back(0x00);
    }

    // Encoding format at 0x1E
    abyResult.push_back(m_nEncodingFormat);

    // Pad to 0xAA
    while (abyResult.size() < 0xAA) {
        abyResult.push_back(0x00);
    }

    // Codepage at 0xAA
    WriteLE16(abyResult, m_nCodepage);

    // Pad to header length
    while (abyResult.size() < nHeaderLen) {
        abyResult.push_back(0x00);
    }

    // Append label data
    abyResult.insert(abyResult.end(), m_abyLabelData.begin(), m_abyLabelData.end());

    return abyResult;
}
