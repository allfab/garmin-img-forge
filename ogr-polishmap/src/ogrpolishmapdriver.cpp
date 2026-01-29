/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Driver registration and identification for Polish Map format
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

#include "ogrpolishmapdriver.h"
#include "ogrpolishmapdatasource.h"
#include "polishmapparser.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"

/************************************************************************/
/*                       OGRPolishMapDriver()                           */
/************************************************************************/

OGRPolishMapDriver::OGRPolishMapDriver() {
    // Set driver name (used by gdalinfo --formats)
    SetDescription("PolishMap");

    // Set driver metadata
    SetMetadataItem(GDAL_DCAP_VECTOR, "YES");
    SetMetadataItem(GDAL_DMD_LONGNAME, "Polish Map Format");
    SetMetadataItem(GDAL_DMD_EXTENSION, "mp");
    SetMetadataItem(GDAL_DMD_HELPTOPIC, "docs/drivers/vector/polishmap.html");

    // Driver capabilities
    SetMetadataItem(GDAL_DCAP_VIRTUALIO, "YES");

    // Function pointers for driver operations
    pfnOpen = Open;
    pfnIdentify = Identify;
}

/************************************************************************/
/*                      ~OGRPolishMapDriver()                           */
/************************************************************************/

OGRPolishMapDriver::~OGRPolishMapDriver() {
    // Nothing to clean up
}

/************************************************************************/
/*                            Identify()                                */
/*                                                                      */
/* Identify whether this file is a Polish Map file by checking both    */
/* the file extension AND the presence of [IMG ID] header in content.  */
/*                                                                      */
/* Story 1.2: Enhanced to validate file content, not just extension.   */
/* Performance requirement: < 10ms using GDALOpenInfo::pabyHeader.     */
/************************************************************************/

int OGRPolishMapDriver::Identify(GDALOpenInfo* poOpenInfo) {
    // Check if the file handle is valid
    if (poOpenInfo->fpL == nullptr) {
        return FALSE;
    }

    const char* pszFilename = poOpenInfo->pszFilename;
    if (pszFilename == nullptr) {
        return FALSE;
    }

    // Check for .mp extension (case-insensitive)
    if (!EQUAL(CPLGetExtension(pszFilename), "mp")) {
        return FALSE;
    }

    // Story 1.2: Content validation - check for [IMG ID] header
    // Use GDALOpenInfo::pabyHeader for fast access (first 1024 bytes pre-read)
    if (poOpenInfo->pabyHeader == nullptr || poOpenInfo->nHeaderBytes < 8) {
        return FALSE;
    }

    // Search for "[IMG ID]" marker in header bytes (case-sensitive)
    // Polish Map format requires this header section
    // Note: pabyHeader is NOT guaranteed to be null-terminated, so we must
    // search within bounds using nHeaderBytes instead of strstr()
    const char* pszHeader = reinterpret_cast<const char*>(poOpenInfo->pabyHeader);
    const int nSearchLen = poOpenInfo->nHeaderBytes;
    const char* pszMarker = "[IMG ID]";
    const int nMarkerLen = 8;  // strlen("[IMG ID]")

    for (int i = 0; i <= nSearchLen - nMarkerLen; i++) {
        if (memcmp(pszHeader + i, pszMarker, nMarkerLen) == 0) {
            return TRUE;
        }
    }

    return FALSE;
}

/************************************************************************/
/*                              Open()                                  */
/*                                                                      */
/* Open a Polish Map file. Story 1.2: Parse and validate [IMG ID]       */
/* header, create dataset with metadata.                                */
/************************************************************************/

GDALDataset* OGRPolishMapDriver::Open(GDALOpenInfo* poOpenInfo) {
    // Verify this is likely a Polish Map file (checks extension AND [IMG ID])
    if (!Identify(poOpenInfo)) {
        return nullptr;
    }

    // Parse the header using PolishMapParser
    PolishMapParser oParser(poOpenInfo->pszFilename);
    if (!oParser.IsOpen()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Polish Map driver: Cannot open file '%s'",
                 poOpenInfo->pszFilename);
        return nullptr;
    }

    // Parse the [IMG ID] header section
    if (!oParser.ParseHeader()) {
        // Error already logged by ParseHeader()
        return nullptr;
    }

    // Create dataset
    OGRPolishMapDataSource* poDS = new OGRPolishMapDataSource();

    // Set the file path (FR26: pszName contains file path)
    poDS->SetDescription(poOpenInfo->pszFilename);

    // Store header metadata in dataset
    poDS->SetHeaderData(oParser.GetHeaderData());

    CPLDebug("OGR_POLISHMAP", "Opened Polish Map file: %s (Name: %s)",
             poOpenInfo->pszFilename,
             oParser.GetHeaderData().osName.c_str());

    return poDS;
}

/************************************************************************/
/*                        RegisterOGRPolishMap()                        */
/*                                                                      */
/* C-style registration function called by GDAL driver manager.         */
/************************************************************************/

extern "C" void RegisterOGRPolishMap() {
    if (GDALGetDriverByName("PolishMap") != nullptr) {
        return;  // Driver already registered
    }

    GDALDriver* poDriver = new OGRPolishMapDriver();
    GetGDALDriverManager()->RegisterDriver(poDriver);

    CPLDebug("OGR_POLISHMAP", "Driver registered successfully");
}
