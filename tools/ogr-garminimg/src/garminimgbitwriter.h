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

#ifndef GARMINIMGBITWRITER_H_INCLUDED
#define GARMINIMGBITWRITER_H_INCLUDED

#include <cstdint>
#include <vector>

/************************************************************************/
/*                     GarminIMGBitWriter                               */
/************************************************************************/

class GarminIMGBitWriter {
public:
    GarminIMGBitWriter();
    ~GarminIMGBitWriter();

    void Put1(bool b);
    void PutN(uint32_t nVal, int nBits);
    void SPutN(int32_t nVal, int nBits);

    const std::vector<uint8_t>& GetBuffer() const { return m_abyBuffer; }
    uint32_t GetBitPosition() const { return m_nBitPos; }

    static int BitsNeeded(int32_t nVal);

private:
    std::vector<uint8_t> m_abyBuffer;
    uint32_t m_nBitPos = 0;
};

#endif /* GARMINIMGBITWRITER_H_INCLUDED */
