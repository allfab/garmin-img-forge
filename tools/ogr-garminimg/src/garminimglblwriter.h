/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Writer for Garmin IMG LBL (labels) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 * SPDX-License-Identifier: MIT
 ****************************************************************************/

#ifndef GARMINIMGLBLWRITER_H_INCLUDED
#define GARMINIMGLBLWRITER_H_INCLUDED

#include <cstdint>
#include <string>
#include <unordered_map>
#include <vector>

class GarminIMGLBLWriter {
public:
    GarminIMGLBLWriter();
    ~GarminIMGLBLWriter();

    void SetEncodingFormat(uint8_t nFormat) { m_nEncodingFormat = nFormat; }
    void SetCodepage(uint16_t nCodepage) { m_nCodepage = nCodepage; }

    uint32_t AddLabel(const std::string& osLabel);
    std::vector<uint8_t> Build();

private:
    uint8_t m_nEncodingFormat = 6;
    uint16_t m_nCodepage = 1252;

    std::vector<uint8_t> m_abyLabelData;
    std::unordered_map<std::string, uint32_t> m_aoLabelIndex;

    void EncodeFormat6(const std::string& osLabel);
    void EncodeFormat9(const std::string& osLabel);
    void EncodeFormat10(const std::string& osLabel);

    static void WriteLE16(std::vector<uint8_t>& buf, uint16_t val);
    static void WriteLE32(std::vector<uint8_t>& buf, uint32_t val);
    static void WriteLE24(std::vector<uint8_t>& buf, uint32_t val);
};

#endif /* GARMINIMGLBLWRITER_H_INCLUDED */
