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

#include "garminimglblparser.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"

#include <cstring>

/************************************************************************/
/*                     GarminIMGLBLParser()                             */
/************************************************************************/

GarminIMGLBLParser::GarminIMGLBLParser() {
}

/************************************************************************/
/*                    ~GarminIMGLBLParser()                             */
/************************************************************************/

GarminIMGLBLParser::~GarminIMGLBLParser() {
}

/************************************************************************/
/*                             Parse()                                  */
/************************************************************************/

bool GarminIMGLBLParser::Parse(const uint8_t* pabyData, uint32_t nSize) {
    if (!pabyData || nSize < 0x21) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG LBL: Data too short (%u bytes)", nSize);
        return false;
    }

    m_pabyData = pabyData;
    m_nSize = nSize;

    // Common header: header_length at offset 0x00 (LE16)
    m_nHeaderLength = static_cast<uint16_t>(pabyData[0x00]) |
                      (static_cast<uint16_t>(pabyData[0x01]) << 8);

    // Check "GARMIN" signature at offset 0x02
    if (memcmp(pabyData + 0x02, "GARMIN", 6) != 0) {
        CPLError(CE_Warning, CPLE_AppDefined,
                 "GarminIMG LBL: Missing GARMIN signature");
    }

    // Label data section: offset at 0x15 (LE32), size at 0x19 (LE32)
    if (m_nHeaderLength < 0x1F) {
        CPLError(CE_Failure, CPLE_AppDefined,
                 "GarminIMG LBL: Header too short for label section");
        return false;
    }

    m_nLabelDataOffset = static_cast<uint32_t>(pabyData[0x15]) |
                         (static_cast<uint32_t>(pabyData[0x16]) << 8) |
                         (static_cast<uint32_t>(pabyData[0x17]) << 16) |
                         (static_cast<uint32_t>(pabyData[0x18]) << 24);

    m_nLabelDataSize = static_cast<uint32_t>(pabyData[0x19]) |
                       (static_cast<uint32_t>(pabyData[0x1A]) << 8) |
                       (static_cast<uint32_t>(pabyData[0x1B]) << 16) |
                       (static_cast<uint32_t>(pabyData[0x1C]) << 24);

    // Encoding format at 0x1E
    m_nEncodingFormat = pabyData[0x1E];

    // Codepage at 0xAA (LE16) — only if header is large enough
    if (m_nHeaderLength >= 0xAC) {
        m_nCodepage = static_cast<uint16_t>(pabyData[0xAA]) |
                      (static_cast<uint16_t>(pabyData[0xAB]) << 8);
    }

    CPLDebug("OGR_GARMINIMG", "LBL: header=%u, labels at %u (%u bytes), "
             "format=%u, codepage=%u",
             m_nHeaderLength, m_nLabelDataOffset, m_nLabelDataSize,
             m_nEncodingFormat, m_nCodepage);

    return true;
}

/************************************************************************/
/*                           GetLabel()                                 */
/************************************************************************/

std::string GarminIMGLBLParser::GetLabel(uint32_t nOffset) const {
    if (!m_pabyData || nOffset >= m_nLabelDataSize) {
        return "";
    }

    switch (m_nEncodingFormat) {
        case 6:
            return DecodeFormat6(nOffset);
        case 9:
            return DecodeFormat9(nOffset);
        case 10:
            return DecodeFormat10(nOffset);
        default:
            CPLDebug("OGR_GARMINIMG", "LBL: Unknown encoding format %u",
                     m_nEncodingFormat);
            return DecodeFormat10(nOffset);  // Fallback to raw bytes
    }
}

/************************************************************************/
/*                        DecodeFormat6()                               */
/*                                                                      */
/* 6-bit packed ASCII: 3 chars per 3 bytes (via 18 bits)               */
/* 0x00=space, 0x01-0x1A=A-Z, 0x1B=reserved, 0x1C=shift symbols,      */
/* 0x1D-0x1F=reserved, 0x20-0x29=0-9, 0x2A-0x3E=symbols, 0x3F=end     */
/************************************************************************/

std::string GarminIMGLBLParser::DecodeFormat6(uint32_t nOffset) const {
    std::string osResult;
    uint32_t nPos = m_nLabelDataOffset + nOffset;

    // Symbol table for shift mode (0x1C sets shift, 0x1C again unsets)
    static const char* s_pszSymbols = " @!\"#$%&'()*+,-./";
    bool bShiftMode = false;

    while (nPos + 2 < m_nSize) {
        // Read 3 bytes = 24 bits, extract 4 6-bit values (only 3 chars though)
        uint32_t nPacked = static_cast<uint32_t>(m_pabyData[nPos]) |
                           (static_cast<uint32_t>(m_pabyData[nPos + 1]) << 8) |
                           (static_cast<uint32_t>(m_pabyData[nPos + 2]) << 16);

        for (int i = 0; i < 4; i++) {
            uint8_t nChar = (nPacked >> (i * 6)) & 0x3F;

            if (nChar == 0x3F) {
                // Terminator
                return osResult;
            }

            if (nChar == 0x1C) {
                bShiftMode = !bShiftMode;
                continue;
            }

            if (bShiftMode) {
                if (nChar < 17) {
                    osResult += s_pszSymbols[nChar];
                }
            } else {
                if (nChar == 0x00) {
                    osResult += ' ';
                } else if (nChar >= 0x01 && nChar <= 0x1A) {
                    osResult += static_cast<char>('A' + nChar - 1);
                } else if (nChar >= 0x20 && nChar <= 0x29) {
                    osResult += static_cast<char>('0' + nChar - 0x20);
                }
            }
        }

        nPos += 3;
    }

    return osResult;
}

/************************************************************************/
/*                        DecodeFormat9()                               */
/*                                                                      */
/* Single-byte codepage (CP1252 by default), null-terminated.          */
/************************************************************************/

std::string GarminIMGLBLParser::DecodeFormat9(uint32_t nOffset) const {
    uint32_t nPos = m_nLabelDataOffset + nOffset;
    std::string osRaw;

    while (nPos < m_nSize && m_pabyData[nPos] != 0x00) {
        osRaw += static_cast<char>(m_pabyData[nPos]);
        nPos++;
    }

    if (osRaw.empty()) {
        return "";
    }

    // Convert from source codepage to UTF-8
    char szCodepage[16];
    snprintf(szCodepage, sizeof(szCodepage), "CP%u", m_nCodepage);
    char* pszUTF8 = CPLRecode(osRaw.c_str(), szCodepage, CPL_ENC_UTF8);
    if (pszUTF8) {
        std::string osResult(pszUTF8);
        CPLFree(pszUTF8);
        return osResult;
    }

    return osRaw;
}

/************************************************************************/
/*                        DecodeFormat10()                              */
/*                                                                      */
/* UTF-8, null-terminated. Direct read.                                */
/************************************************************************/

std::string GarminIMGLBLParser::DecodeFormat10(uint32_t nOffset) const {
    uint32_t nPos = m_nLabelDataOffset + nOffset;
    std::string osResult;

    while (nPos < m_nSize && m_pabyData[nPos] != 0x00) {
        osResult += static_cast<char>(m_pabyData[nPos]);
        nPos++;
    }

    return osResult;
}
