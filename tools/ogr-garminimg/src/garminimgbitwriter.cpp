/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  LSB-first bitstream writer for Garmin IMG coordinate encoding
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

#include "garminimgbitwriter.h"

#include <cstdlib>

/************************************************************************/
/*                     GarminIMGBitWriter()                             */
/************************************************************************/

GarminIMGBitWriter::GarminIMGBitWriter() {
}

GarminIMGBitWriter::~GarminIMGBitWriter() {
}

/************************************************************************/
/*                             Put1()                                   */
/************************************************************************/

void GarminIMGBitWriter::Put1(bool b) {
    uint32_t nByteIdx = m_nBitPos / 8;
    uint32_t nBitIdx = m_nBitPos % 8;

    // Extend buffer if needed
    if (nByteIdx >= m_abyBuffer.size()) {
        m_abyBuffer.push_back(0);
    }

    if (b) {
        m_abyBuffer[nByteIdx] |= (1u << nBitIdx);
    }

    m_nBitPos++;
}

/************************************************************************/
/*                             PutN()                                   */
/************************************************************************/

void GarminIMGBitWriter::PutN(uint32_t nVal, int nBits) {
    for (int i = 0; i < nBits; i++) {
        Put1((nVal >> i) & 1);
    }
}

/************************************************************************/
/*                            SPutN()                                   */
/************************************************************************/

void GarminIMGBitWriter::SPutN(int32_t nVal, int nBits) {
    if (nBits <= 0) return;

    // Encode as unsigned with MSB = sign bit
    uint32_t nUVal;
    if (nVal < 0) {
        nUVal = static_cast<uint32_t>(nVal + (1 << nBits));
    } else {
        nUVal = static_cast<uint32_t>(nVal);
    }

    PutN(nUVal, nBits);
}

/************************************************************************/
/*                          BitsNeeded()                                */
/************************************************************************/

int GarminIMGBitWriter::BitsNeeded(int32_t nVal) {
    if (nVal == 0) return 1;
    uint32_t nAbs = static_cast<uint32_t>(std::abs(nVal));
    int nBits = 0;
    while (nAbs > 0) {
        nBits++;
        nAbs >>= 1;
    }
    return nBits;
}
