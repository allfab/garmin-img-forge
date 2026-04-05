/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  LSB-first bitstream reader for Garmin IMG coordinate decoding
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

#include "garminimgbitreader.h"

#include <cmath>

/************************************************************************/
/*                     GarminIMGBitReader()                             */
/************************************************************************/

GarminIMGBitReader::GarminIMGBitReader(const uint8_t* pabyData,
                                       uint32_t nSizeBytes)
    : m_pabyData(pabyData),
      m_nTotalBits(nSizeBytes * 8) {
}

/************************************************************************/
/*                    ~GarminIMGBitReader()                             */
/************************************************************************/

GarminIMGBitReader::~GarminIMGBitReader() {
}

/************************************************************************/
/*                             Get1()                                   */
/************************************************************************/

bool GarminIMGBitReader::Get1() {
    if (m_nBitPos >= m_nTotalBits) {
        return false;
    }

    uint32_t nByteIdx = m_nBitPos / 8;
    uint32_t nBitIdx = m_nBitPos % 8;  // LSB-first
    m_nBitPos++;

    return (m_pabyData[nByteIdx] >> nBitIdx) & 1;
}

/************************************************************************/
/*                              Get()                                   */
/************************************************************************/

uint32_t GarminIMGBitReader::Get(int nBits) {
    uint32_t nVal = 0;
    for (int i = 0; i < nBits; i++) {
        if (Get1()) {
            nVal |= (1u << i);
        }
    }
    return nVal;
}

/************************************************************************/
/*                             SGet()                                   */
/************************************************************************/

int32_t GarminIMGBitReader::SGet(int nBits) {
    if (nBits <= 0) return 0;

    uint32_t nVal = Get(nBits);

    // MSB is sign bit
    uint32_t nSignBit = 1u << (nBits - 1);
    if (nVal & nSignBit) {
        // Negative: sign-extend
        return static_cast<int32_t>(nVal) - static_cast<int32_t>(1u << nBits);
    }

    return static_cast<int32_t>(nVal);
}

/************************************************************************/
/*                             SGet2()                                  */
/*                                                                      */
/* Read with overflow support: if value == 1 << (n-1), accumulate      */
/* and read again with n bits.                                          */
/************************************************************************/

int32_t GarminIMGBitReader::SGet2(int nBits) {
    if (nBits <= 0) return 0;
    if (nBits == 1) return SGet(1);  // nMask would be 0, overflow loop can't progress

    // Faithful to mkgmap BitReader.sget2() / imgforge bit_reader.rs sget2():
    // Read unsigned chunks. If chunk == top (overflow marker), accumulate
    // mask into base and read next chunk. Sign-extend only the final chunk.
    uint32_t nTop = 1u << (nBits - 1);
    uint32_t nMask = nTop - 1;
    uint32_t nBase = 0;

    uint32_t nRes = Get(nBits);
    while (nRes == nTop) {
        nBase += nMask;
        nRes = Get(nBits);
    }

    if ((nRes & nTop) == 0) {
        // Positive: final value + accumulated base
        return static_cast<int32_t>(nRes + nBase);
    } else {
        // Negative: sign-extend final chunk, subtract accumulated base
        int32_t nSigned = static_cast<int32_t>(nRes | ~nMask);
        return nSigned - static_cast<int32_t>(nBase);
    }
}

/************************************************************************/
/*                          Base2Bits()                                 */
/************************************************************************/

int GarminIMGBitReader::Base2Bits(int nBase) {
    if (nBase < 10) {
        return 2 + nBase;
    }
    return 2 + 2 * nBase - 9;
}

/************************************************************************/
/*                          Bits2Base()                                 */
/************************************************************************/

int GarminIMGBitReader::Bits2Base(int nBits) {
    if (nBits < 12) {
        return nBits - 2;
    }
    return (nBits - 2 + 9) / 2;
}

/************************************************************************/
/*                      MapUnitsToDegrees()                             */
/************************************************************************/

double GarminIMGBitReader::MapUnitsToDegrees(int32_t nMU) {
    return static_cast<double>(nMU) * 360.0 / (1 << 24);
}

/************************************************************************/
/*                      DegreesToMapUnits()                             */
/************************************************************************/

int32_t GarminIMGBitReader::DegreesToMapUnits(double dfDeg) {
    return static_cast<int32_t>(std::round(dfDeg * (1 << 24) / 360.0));
}
