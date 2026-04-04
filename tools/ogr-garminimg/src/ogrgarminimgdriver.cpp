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

#include "ogrgarminimgdriver.h"
#include "ogrgarminimgdatasource.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"
#include <memory>
#include <cstring>

/************************************************************************/
/*                       OGRGarminIMGDriver()                           */
/************************************************************************/

OGRGarminIMGDriver::OGRGarminIMGDriver() {
    SetDescription("GarminIMG");

    SetMetadataItem(GDAL_DCAP_VECTOR, "YES");
    SetMetadataItem(GDAL_DMD_LONGNAME, "Garmin IMG Format");
    SetMetadataItem(GDAL_DMD_EXTENSION, "img");

    SetMetadataItem(GDAL_DCAP_VIRTUALIO, "YES");
    SetMetadataItem(GDAL_DCAP_CREATE, "YES");

    SetMetadataItem(GDAL_DMD_CREATIONFIELDDATATYPES, "String Integer Real");
    SetMetadataItem(GDAL_DMD_SUPPORTED_SQL_DIALECTS, "OGRSQL");

    SetMetadataItem(GDAL_DMD_CREATIONOPTIONLIST,
        "<CreationOptionList>"
        "  <Option name='MAP_NAME' type='string' description="
        "'Map name stored in the IMG header'/>"
        "  <Option name='MAP_ID' type='int' description="
        "'Map ID (32-bit unsigned integer)'/>"
        "  <Option name='CODEPAGE' type='string-select' default='1252' description="
        "'Character encoding codepage'>"
        "    <Value>1252</Value>"
        "    <Value>65001</Value>"
        "  </Option>"
        "  <Option name='LABEL_FORMAT' type='string-select' default='6' description="
        "'Label encoding format'>"
        "    <Value>6</Value>"
        "    <Value>9</Value>"
        "    <Value>10</Value>"
        "  </Option>"
        "  <Option name='LEVELS' type='string' description="
        "'Comma-separated resolution levels (e.g., 24,22,20,18)'/>"
        "  <Option name='TRANSPARENT' type='boolean' default='NO' description="
        "'Transparent map overlay'/>"
        "  <Option name='DRAW_PRIORITY' type='int' default='25' description="
        "'Draw priority (0-31)'/>"
        "  <Option name='TYP_FILE' type='string' description="
        "'Path to a TYP file to embed in the IMG'/>"
        "</CreationOptionList>");

    SetMetadataItem(GDAL_DMD_OPENOPTIONLIST,
        "<OpenOptionList>"
        "  <Option name='TYP_FILE' type='string' description="
        "'Path to an external TYP file to join if none is embedded in the IMG'/>"
        "</OpenOptionList>");

    pfnOpen = Open;
    pfnIdentify = Identify;
    pfnCreate = Create;
}

/************************************************************************/
/*                      ~OGRGarminIMGDriver()                           */
/************************************************************************/

OGRGarminIMGDriver::~OGRGarminIMGDriver() {
}

/************************************************************************/
/*                            Identify()                                */
/*                                                                      */
/* Check for magic "DSKIMG\0" at offset 0x10 + .img extension.         */
/************************************************************************/

int OGRGarminIMGDriver::Identify(GDALOpenInfo* poOpenInfo) {
    if (poOpenInfo->fpL == nullptr) {
        return FALSE;
    }

    const char* pszFilename = poOpenInfo->pszFilename;
    if (pszFilename == nullptr) {
        return FALSE;
    }

    // Check .img extension (case-insensitive)
    if (!EQUAL(CPLGetExtension(pszFilename), "img")) {
        return FALSE;
    }

    // Check for "DSKIMG\0" magic at offset 0x10 (7 bytes)
    if (poOpenInfo->pabyHeader == nullptr || poOpenInfo->nHeaderBytes < 0x17) {
        return FALSE;
    }

    if (memcmp(poOpenInfo->pabyHeader + 0x10, "DSKIMG\0", 7) != 0) {
        return FALSE;
    }

    return TRUE;
}

/************************************************************************/
/*                              Open()                                  */
/************************************************************************/

GDALDataset* OGRGarminIMGDriver::Open(GDALOpenInfo* poOpenInfo) {
    if (!Identify(poOpenInfo)) {
        return nullptr;
    }

    OGRGarminIMGDataSource* poDS = new OGRGarminIMGDataSource();

    if (!poDS->Open(poOpenInfo)) {
        delete poDS;
        return nullptr;
    }

    CPLDebug("OGR_GARMINIMG", "Opened Garmin IMG file: %s",
             poOpenInfo->pszFilename);

    return poDS;
}

/************************************************************************/
/*                             Create()                                  */
/************************************************************************/

GDALDataset* OGRGarminIMGDriver::Create(const char* pszName, int /* nXSize */,
                                        int /* nYSize */, int /* nBands */,
                                        GDALDataType /* eType */,
                                        char** papszOptions) {
    return OGRGarminIMGDataSource::Create(pszName, papszOptions);
}

/************************************************************************/
/*                        RegisterOGRGarminIMG()                        */
/************************************************************************/

extern "C" void RegisterOGRGarminIMG() {
    if (GDALGetDriverByName("GarminIMG") != nullptr) {
        return;
    }

    GDALDriver* poDriver = new OGRGarminIMGDriver();
    GetGDALDriverManager()->RegisterDriver(poDriver);

    CPLDebug("OGR_GARMINIMG", "Driver registered successfully");
}

/************************************************************************/
/*                          GDALRegisterMe()                            */
/************************************************************************/

extern "C" OGR_GARMINIMG_EXPORT void GDALRegisterMe() {
    RegisterOGRGarminIMG();
}

/************************************************************************/
/*                       GDALRegister_GarminIMG()                       */
/************************************************************************/

extern "C" OGR_GARMINIMG_EXPORT void GDALRegister_GarminIMG() {
    RegisterOGRGarminIMG();
}
