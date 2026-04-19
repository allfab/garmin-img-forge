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
#include <memory>

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

    // Story 2.1 Task 4.1: Add GDAL_DCAP_CREATE capability
    SetMetadataItem(GDAL_DCAP_CREATE, "YES");

    // Story 2.6: Additional metadata for full GDAL integration
    // Supported field data types for writing (String only for Polish Map)
    SetMetadataItem(GDAL_DMD_CREATIONFIELDDATATYPES, "String");

    // SQL dialects supported (OGRSQL is the default OGR SQL)
    SetMetadataItem(GDAL_DMD_SUPPORTED_SQL_DIALECTS, "OGRSQL");

    // Story 2.2.8 + 4.4 + Tech-spec #2 Task 2/4: Dataset creation options
    SetMetadataItem(GDAL_DMD_CREATIONOPTIONLIST,
        "<CreationOptionList>"
        "  <Option name='HEADER_TEMPLATE' type='string' description="
        "'Path to a Polish Map file (.mp) whose header ([IMG ID] section) will be copied to the output file. "
        "If specified, this takes precedence over metadata set via SetMetadataItem(). "
        "The template file must contain a valid [IMG ID] section.'/>"
        "  <Option name='FIELD_MAPPING' type='string' description="
        "'Path to a YAML configuration file that defines field mappings from source to Polish Map fields. "
        "Enables automatic field name translation (e.g., NAME to Label, ROAD_TYPE to Type) during ogr2ogr conversion.'/>"
        "  <Option name='MULTI_GEOM_FIELDS' type='boolean' default='NO' description="
        "'Enable multi-geometry fields on POLYLINE and POLYGON layers (Data1=, Data2=, ... DataK= "
        "in the Polish Map output). POI stays mono-geom (MP spec §4.4.3.1).'/>"
        "  <Option name='MAX_DATA_LEVEL' type='int' default='4' min='1' max='9' description="
        "'Maximum Data index K when MULTI_GEOM_FIELDS=YES. Adds K OGRGeomFieldDefn "
        "(geom_level_1..geom_level_K) beside the primary geometry.'/>"
        "</CreationOptionList>");

    // Tech-spec #2 Task 4: Open options (strict opt-in, no auto-detection).
    SetMetadataItem(GDAL_DMD_OPENOPTIONLIST,
        "<OpenOptionList>"
        "  <Option name='MULTI_GEOM_FIELDS' type='boolean' default='NO' description="
        "'Expose Data1=..DataK= lines parsed from the file as additional OGR geometry "
        "fields on POLYLINE/POLYGON layers. Without this option the file is opened as "
        "strictly single-geom features (any DataN>0 lines are parsed but not exposed).'/>"
        "  <Option name='MAX_DATA_LEVEL' type='int' default='4' min='1' max='9' description="
        "'Maximum Data index K to expose when MULTI_GEOM_FIELDS=YES at open.'/>"
        "</OpenOptionList>");

    // Function pointers for driver operations
    pfnOpen = Open;
    pfnIdentify = Identify;
    // Story 2.1 Task 1.1: Add pfnCreate function pointer
    pfnCreate = Create;
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

    // Story 1.4: Create parser with unique_ptr for ownership transfer
    std::unique_ptr<PolishMapParser> poParser = std::make_unique<PolishMapParser>(poOpenInfo->pszFilename);
    if (!poParser->IsOpen()) {
        CPLError(CE_Failure, CPLE_OpenFailed,
                 "Polish Map driver: Cannot open file '%s'",
                 poOpenInfo->pszFilename);
        return nullptr;
    }

    // Parse the [IMG ID] header section
    if (!poParser->ParseHeader()) {
        // Error already logged by ParseHeader()
        return nullptr;
    }

    // Create dataset
    OGRPolishMapDataSource* poDS = new OGRPolishMapDataSource();

    // Set the file path (FR26: pszName contains file path)
    poDS->SetDescription(poOpenInfo->pszFilename);

    // Store header metadata in dataset
    poDS->SetHeaderData(poParser->GetHeaderData());

    // Tech-spec #2 Task 4: explicit opt-in to multi-geom exposure on read path
    // via open option MULTI_GEOM_FIELDS=YES + MAX_DATA_LEVEL=K. Without the
    // option, DataN>0 lines are parsed internally but not surfaced via OGR
    // geom fields (behaviour is predictable: stricly single-geom features).
    const char* pszMultiGeom =
        CSLFetchNameValue(poOpenInfo->papszOpenOptions, "MULTI_GEOM_FIELDS");
    if (pszMultiGeom != nullptr && CPLTestBool(pszMultiGeom)) {
        const char* pszMaxLevel =
            CSLFetchNameValue(poOpenInfo->papszOpenOptions, "MAX_DATA_LEVEL");
        int nMaxLevel = (pszMaxLevel != nullptr) ? atoi(pszMaxLevel) : 4;
        if (nMaxLevel < 1 || nMaxLevel > 9) {
            CPLError(CE_Failure, CPLE_AppDefined,
                     "MAX_DATA_LEVEL must be in [1, 9], got %d", nMaxLevel);
            delete poDS;
            return nullptr;
        }
        poDS->SetMultiGeomFields(true, nMaxLevel);
    }

    // Story 1.4: Transfer parser ownership to dataset
    poDS->SetParser(std::move(poParser));

    CPLDebug("OGR_POLISHMAP", "Opened Polish Map file: %s (Name: %s)",
             poOpenInfo->pszFilename,
             poDS->GetHeaderData().osName.c_str());

    return poDS;
}

/************************************************************************/
/*                             Create()                                  */
/*                                                                      */
/* Story 2.1: Create a new Polish Map file for writing.                 */
/* Parameters nXSize, nYSize, nBands, eType are ignored for vector.     */
/************************************************************************/

GDALDataset* OGRPolishMapDriver::Create(const char* pszName, int /* nXSize */,
                                        int /* nYSize */, int /* nBands */,
                                        GDALDataType /* eType */,
                                        char** papszOptions) {
    // Task 1.2: Delegate to OGRPolishMapDataSource::Create()
    // Story 4.4: Pass papszOptions for FIELD_MAPPING support
    return OGRPolishMapDataSource::Create(pszName, papszOptions);
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

/************************************************************************/
/*                          GDALRegisterMe()                            */
/*                                                                      */
/* Standard entry point for external GDAL plugins. GDAL calls this      */
/* function when loading plugins from GDAL_DRIVER_PATH.                 */
/* Must be exported with default visibility for dynamic loading.        */
/************************************************************************/

extern "C" OGR_POLISHMAP_EXPORT void GDALRegisterMe() {
    RegisterOGRPolishMap();
}

/************************************************************************/
/*                       GDALRegister_PolishMap()                        */
/*                                                                      */
/* GDAL 3.9+ plugin entry point. GDAL looks for this symbol when        */
/* loading plugins from GDAL_DRIVER_PATH.                               */
/* Format: GDALRegister_<FormatName> with proper casing.                */
/************************************************************************/

extern "C" OGR_POLISHMAP_EXPORT void GDALRegister_PolishMap() {
    RegisterOGRPolishMap();
}
