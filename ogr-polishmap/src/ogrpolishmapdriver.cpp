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
/* Identify whether this file is a Polish Map file by checking the     */
/* file extension.                                                      */
/*                                                                      */
/* NOTE: Story 1.1 implements extension-only check as specified in     */
/* FR20. Full content validation ([IMG ID] header check) will be       */
/* implemented in Story 1.2 (Polish Map Header Parser).                */
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
    // Full content validation will be added in Story 1.2
    if (EQUAL(CPLGetExtension(pszFilename), "mp")) {
        return TRUE;
    }

    return FALSE;
}

/************************************************************************/
/*                              Open()                                  */
/*                                                                      */
/* Open a Polish Map file. For now, this is a stub that returns NULL   */
/* with an appropriate error message.                                   */
/************************************************************************/

GDALDataset* OGRPolishMapDriver::Open(GDALOpenInfo* poOpenInfo) {
    // Verify this is likely a Polish Map file
    if (!Identify(poOpenInfo)) {
        return nullptr;
    }

    // For now, return NULL with a descriptive error
    // Full implementation will come in subsequent stories
    CPLError(CE_Failure, CPLE_OpenFailed,
             "PolishMap driver: Open() not yet implemented. "
             "File parsing will be added in subsequent development stories.");

    return nullptr;
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
