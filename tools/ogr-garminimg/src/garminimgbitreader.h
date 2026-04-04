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

#ifndef GARMINIMGBITREADER_H_INCLUDED
#define GARMINIMGBITREADER_H_INCLUDED

#include <cstdint>

/************************************************************************/
/*                     GarminIMGBitReader                               */
/************************************************************************/

class GarminIMGBitReader {
public:
    GarminIMGBitReader(const uint8_t* pabyData, uint32_t nSizeBytes);
    ~GarminIMGBitReader();

    bool Get1();
    uint32_t Get(int nBits);
    int32_t SGet(int nBits);
    int32_t SGet2(int nBits);

    uint32_t GetPosition() const { return m_nBitPos; }
    bool HasMore() const { return m_nBitPos < m_nTotalBits; }
    uint32_t GetRemainingBits() const { return m_nTotalBits - m_nBitPos; }

    // Coordinate conversion utilities
    static int Base2Bits(int nBase);
    static int Bits2Base(int nBits);
    static double MapUnitsToDegrees(int32_t nMU);
    static int32_t DegreesToMapUnits(double dfDeg);

private:
    const uint8_t* m_pabyData;
    uint32_t m_nTotalBits;
    uint32_t m_nBitPos = 0;
};

#endif /* GARMINIMGBITREADER_H_INCLUDED */
