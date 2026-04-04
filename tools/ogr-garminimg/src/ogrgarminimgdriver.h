/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  Driver registration and identification for Garmin IMG format
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

#ifndef OGRGARMINIMGDRIVER_H_INCLUDED
#define OGRGARMINIMGDRIVER_H_INCLUDED

#include "gdal_priv.h"

/************************************************************************/
/*                        OGRGarminIMGDriver                            */
/************************************************************************/

class OGRGarminIMGDriver final : public GDALDriver {
public:
    OGRGarminIMGDriver();
    ~OGRGarminIMGDriver() override;

    static int Identify(GDALOpenInfo* poOpenInfo);
    static GDALDataset* Open(GDALOpenInfo* poOpenInfo);
    static GDALDataset* Create(const char* pszName, int nXSize, int nYSize,
                               int nBands, GDALDataType eType, char** papszOptions);
};

#if defined(__GNUC__) || defined(__clang__)
#  define OGR_GARMINIMG_EXPORT __attribute__((visibility("default")))
#elif defined(_MSC_VER)
#  define OGR_GARMINIMG_EXPORT __declspec(dllexport)
#else
#  define OGR_GARMINIMG_EXPORT
#endif

extern "C" {
    OGR_GARMINIMG_EXPORT void RegisterOGRGarminIMG();
    OGR_GARMINIMG_EXPORT void GDALRegisterMe();
    OGR_GARMINIMG_EXPORT void GDALRegister_GarminIMG();
}

#endif /* OGRGARMINIMGDRIVER_H_INCLUDED */
