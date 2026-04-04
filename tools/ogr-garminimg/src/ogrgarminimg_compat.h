/******************************************************************************
 * Project:  OGR GarminIMG Driver
 * Purpose:  GDAL version compatibility macros
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

#ifndef OGRGARMINIMG_COMPAT_H_INCLUDED
#define OGRGARMINIMG_COMPAT_H_INCLUDED

#include "gdal_version.h"

// GDAL 3.12 made several virtual methods const:
// - GDALDataset::GetLayerCount(), GetLayer(), TestCapability()
// - OGRLayer::GetLayerDefn(), TestCapability()
#if GDAL_VERSION_NUM >= 3120000
#define OGRGARMINIMG_CONST const
#else
#define OGRGARMINIMG_CONST
#endif

// MSVC uses __declspec(dllexport) instead of __attribute__((visibility))
#ifdef _MSC_VER
#define OGR_GARMINIMG_EXPORT __declspec(dllexport)
#else
#define OGR_GARMINIMG_EXPORT __attribute__((visibility("default")))
#endif

#endif /* OGRGARMINIMG_COMPAT_H_INCLUDED */
