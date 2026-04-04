/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Parser for Garmin IMG LBL (labels) subfile
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 ****************************************************************************/

#ifndef GARMINIMGLBLPARSER_H_INCLUDED
#define GARMINIMGLBLPARSER_H_INCLUDED

#include <cstdint>
#include <string>
#include <vector>

/************************************************************************/
/*                     GarminIMGLBLParser                               */
/************************************************************************/

class GarminIMGLBLParser {
public:
    GarminIMGLBLParser();
    ~GarminIMGLBLParser();

    bool Parse(const uint8_t* pabyData, uint32_t nSize);

    std::string GetLabel(uint32_t nOffset) const;

    uint8_t GetEncodingFormat() const { return m_nEncodingFormat; }
    uint16_t GetCodepage() const { return m_nCodepage; }

private:
    const uint8_t* m_pabyData = nullptr;
    uint32_t m_nSize = 0;
    uint16_t m_nHeaderLength = 0;

    // Label data section
    uint32_t m_nLabelDataOffset = 0;
    uint32_t m_nLabelDataSize = 0;
    uint8_t m_nEncodingFormat = 6;  // Default: 6-bit packed ASCII
    uint16_t m_nCodepage = 1252;

    std::string DecodeFormat6(uint32_t nOffset) const;
    std::string DecodeFormat9(uint32_t nOffset) const;
    std::string DecodeFormat10(uint32_t nOffset) const;
};

#endif /* GARMINIMGLBLPARSER_H_INCLUDED */
