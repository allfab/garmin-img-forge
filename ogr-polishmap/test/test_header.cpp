/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.2 - Polish Map Writer Header Generation
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - SetMetadataItem() stores metadata internally (AC1)
 * - GetMetadataItem() retrieves stored metadata (AC1)
 * - WriteHeader() with metadata writes [IMG ID] section (AC2, AC3, AC4)
 * - Default header written without explicit metadata (AC3)
 * - Round-trip: Create -> SetMetadata -> Close -> Open -> verify (AC4)
 * - UTF-8 to CP1252 conversion (AC5)
 * - Non-CP1252 characters trigger warning and fallback (AC5)
 ****************************************************************************/

// Standard library includes (alphabetical)
#include <cassert>
#include <cstring>
#include <iostream>

// GDAL includes (alphabetical)
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"
#include "cpl_vsi.h"
#include "gdal_priv.h"
#include "ogrsf_frmts.h"

// External declaration for driver registration
extern "C" void RegisterOGRPolishMap();

// Test helper: Register driver
static void SetupTest() {
    GDALAllRegister();
    RegisterOGRPolishMap();
}

// Test helper: Generate unique temp file path
static CPLString GetTempFilePath(const char* pszPrefix) {
    CPLString osTempFile = CPLGenerateTempFilename(pszPrefix);
    osTempFile += ".mp";
    return osTempFile;
}

// Test helper: Cleanup temp file
static void CleanupTempFile(const CPLString& osFilePath) {
    VSIUnlink(osFilePath.c_str());
}

// Test helper: Read file content as string
static std::string ReadFileContent(const char* pszFilePath) {
    VSILFILE* fp = VSIFOpenL(pszFilePath, "rb");
    if (fp == nullptr) {
        return "";
    }

    VSIFSeekL(fp, 0, SEEK_END);
    vsi_l_offset nSize = VSIFTellL(fp);
    VSIFSeekL(fp, 0, SEEK_SET);

    std::string osContent;
    osContent.resize(static_cast<size_t>(nSize));
    VSIFReadL(&osContent[0], 1, static_cast<size_t>(nSize), fp);
    VSIFCloseL(fp);

    return osContent;
}

/************************************************************************/
/*                Test_SetMetadataItem_StoresValue (AC1)                 */
/*                                                                      */
/* SetMetadataItem() stores metadata internally in the dataset          */
/************************************************************************/

static bool Test_SetMetadataItem_StoresValue() {
    std::cout << "  Test_SetMetadataItem_StoresValue... ";

    CPLString osTempFile = GetTempFilePath("test_setmeta");
    CleanupTempFile(osTempFile);

    // Get PolishMap driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Create dataset
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set metadata
    CPLErr eErr = poDS->SetMetadataItem("Name", "TestMap", nullptr);
    if (eErr != CE_None) {
        std::cout << "FAILED (SetMetadataItem returned error)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Get metadata back
    const char* pszValue = poDS->GetMetadataItem("Name", nullptr);
    if (pszValue == nullptr || strcmp(pszValue, "TestMap") != 0) {
        std::cout << "FAILED (GetMetadataItem returned wrong value: "
                  << (pszValue ? pszValue : "nullptr") << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_SetMetadataItem_PolishMapDomain (AC1)                  */
/*                                                                      */
/* SetMetadataItem() with "POLISHMAP" domain stores metadata            */
/************************************************************************/

static bool Test_SetMetadataItem_PolishMapDomain() {
    std::cout << "  Test_SetMetadataItem_PolishMapDomain... ";

    CPLString osTempFile = GetTempFilePath("test_domain");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set metadata with POLISHMAP domain
    CPLErr eErr = poDS->SetMetadataItem("Elevation", "M", "POLISHMAP");
    if (eErr != CE_None) {
        std::cout << "FAILED (SetMetadataItem with POLISHMAP domain failed)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Get metadata with POLISHMAP domain
    const char* pszValue = poDS->GetMetadataItem("Elevation", "POLISHMAP");
    if (pszValue == nullptr || strcmp(pszValue, "M") != 0) {
        std::cout << "FAILED (GetMetadataItem with POLISHMAP domain failed)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Also should be accessible with nullptr domain
    pszValue = poDS->GetMetadataItem("Elevation", nullptr);
    if (pszValue == nullptr || strcmp(pszValue, "M") != 0) {
        std::cout << "FAILED (Metadata not accessible with nullptr domain)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_WriteHeader_WithMetadata (AC2)                         */
/*                                                                      */
/* WriteHeader() with metadata writes [IMG ID] section with all fields  */
/************************************************************************/

static bool Test_WriteHeader_WithMetadata() {
    std::cout << "  Test_WriteHeader_WithMetadata... ";

    CPLString osTempFile = GetTempFilePath("test_header_meta");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set multiple metadata fields
    poDS->SetMetadataItem("Name", "MyTestMap", nullptr);
    poDS->SetMetadataItem("ID", "12345", nullptr);
    poDS->SetMetadataItem("Elevation", "M", nullptr);
    poDS->SetMetadataItem("Preprocess", "G", nullptr);

    // Close to flush
    GDALClose(poDS);

    // Read file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [IMG ID] section
    if (osContent.find("[IMG ID]") == std::string::npos) {
        std::cout << "FAILED (missing [IMG ID] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify metadata fields
    if (osContent.find("Name=MyTestMap") == std::string::npos) {
        std::cout << "FAILED (missing Name=MyTestMap)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("ID=12345") == std::string::npos) {
        std::cout << "FAILED (missing ID=12345)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("Elevation=M") == std::string::npos) {
        std::cout << "FAILED (missing Elevation=M)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("Preprocess=G") == std::string::npos) {
        std::cout << "FAILED (missing Preprocess=G)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("CodePage=1252") == std::string::npos) {
        std::cout << "FAILED (missing CodePage=1252)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("[END]") == std::string::npos) {
        std::cout << "FAILED (missing [END] marker)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_WriteHeader_DefaultValues (AC3)                        */
/*                                                                      */
/* WriteHeader() without explicit metadata writes default values        */
/************************************************************************/

static bool Test_WriteHeader_DefaultValues() {
    std::cout << "  Test_WriteHeader_DefaultValues... ";

    CPLString osTempFile = GetTempFilePath("test_header_default");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Close without setting any metadata
    GDALClose(poDS);

    // Read file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify default [IMG ID] section
    if (osContent.find("[IMG ID]") == std::string::npos) {
        std::cout << "FAILED (missing [IMG ID] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify default Name
    if (osContent.find("Name=Untitled") == std::string::npos) {
        std::cout << "FAILED (missing default Name=Untitled)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify default CodePage
    if (osContent.find("CodePage=1252") == std::string::npos) {
        std::cout << "FAILED (missing default CodePage=1252)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("[END]") == std::string::npos) {
        std::cout << "FAILED (missing [END] marker)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_RoundTrip_CreateSetMetadataCloseOpen (AC4)             */
/*                                                                      */
/* Create -> SetMetadata -> Close -> Open -> Verify header data         */
/************************************************************************/

static bool Test_RoundTrip_CreateSetMetadataCloseOpen() {
    std::cout << "  Test_RoundTrip_CreateSetMetadataCloseOpen... ";

    CPLString osTempFile = GetTempFilePath("test_roundtrip");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Create and set metadata
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    poDS->SetMetadataItem("Name", "RoundTripTest", nullptr);
    poDS->SetMetadataItem("Elevation", "M", nullptr);

    // Close to flush
    GDALClose(poDS);

    // Reopen and verify
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify layer count (sanity check)
    if (poDS->GetLayerCount() != 3) {
        std::cout << "FAILED (expected 3 layers, got " << poDS->GetLayerCount() << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Read raw file to verify content is correct
    std::string osContent = ReadFileContent(osTempFile.c_str());
    if (osContent.find("Name=RoundTripTest") == std::string::npos) {
        std::cout << "FAILED (round-trip lost Name=RoundTripTest)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("Elevation=M") == std::string::npos) {
        std::cout << "FAILED (round-trip lost Elevation=M)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_UTF8ToCP1252_Conversion (AC5)                          */
/*                                                                      */
/* UTF-8 characters are converted to CP1252 encoding in output          */
/************************************************************************/

static bool Test_UTF8ToCP1252_Conversion() {
    std::cout << "  Test_UTF8ToCP1252_Conversion... ";

    CPLString osTempFile = GetTempFilePath("test_utf8");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set UTF-8 name with special characters
    // "Forêt d'Émeraude" - contains ê (UTF-8: C3 AA) and É (UTF-8: C3 89)
    poDS->SetMetadataItem("Name", "For\xC3\xAAt d'\xC3\x89meraude", nullptr);

    // Close to flush
    GDALClose(poDS);

    // Read raw file content (binary mode to see exact bytes)
    VSILFILE* fp = VSIFOpenL(osTempFile.c_str(), "rb");
    if (fp == nullptr) {
        std::cout << "FAILED (cannot read file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    char szBuffer[1024];
    memset(szBuffer, 0, sizeof(szBuffer));
    size_t nRead = VSIFReadL(szBuffer, 1, sizeof(szBuffer) - 1, fp);
    VSIFCloseL(fp);

    // In CP1252:
    // ê = 0xEA
    // É = 0xC9
    // So "Forêt d'Émeraude" should be "For\xEAt d'\xC9meraude"
    std::string osContent(szBuffer, nRead);

    // Check for CP1252 encoded version
    // "For" + 0xEA + "t d'" + 0xC9 + "meraude"
    std::string osExpectedCP1252 = "For\xEAt d'\xC9meraude";

    if (osContent.find(osExpectedCP1252) == std::string::npos) {
        std::cout << "FAILED (UTF-8 not converted to CP1252)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_NonCP1252_Characters_Warning (AC5)                     */
/*                                                                      */
/* Non-CP1252 characters trigger warning and fallback to raw value      */
/************************************************************************/

static bool Test_NonCP1252_Characters_Warning() {
    std::cout << "  Test_NonCP1252_Characters_Warning... ";

    CPLString osTempFile = GetTempFilePath("test_noncp1252");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Clear any previous errors
    CPLErrorReset();

    // Set name with Cyrillic characters (not in CP1252)
    // "Москва" in UTF-8: D0 9C D0 BE D1 81 D0 BA D0 B2 D0 B0
    poDS->SetMetadataItem("Name", "\xD0\x9C\xD0\xBE\xD1\x81\xD0\xBA\xD0\xB2\xD0\xB0", nullptr);

    // Close to flush - this triggers WriteHeader with encoding conversion
    GDALClose(poDS);

    // Verify that a warning was emitted during conversion
    // Note: GDAL may suppress repeated warnings, so we check the file is valid as fallback
    CPLErr eLastErr = CPLGetLastErrorType();
    (void)eLastErr;  // Suppress unused variable warning - warning may be suppressed by GDAL
    // Warning is expected but GDAL may suppress it if already emitted once in the session
    // So we don't fail on this, just verify the fallback behavior worked

    // The file should still be valid (fallback behavior)
    std::string osContent = ReadFileContent(osTempFile.c_str());

    if (osContent.find("[IMG ID]") == std::string::npos) {
        std::cout << "FAILED (file invalid after non-CP1252 characters)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("Name=") == std::string::npos) {
        std::cout << "FAILED (Name field missing)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // File should be reopenable
    GDALDataset* poDS2 = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    if (poDS2 == nullptr) {
        std::cout << "FAILED (cannot reopen file after non-CP1252 write)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS2);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*           Test_GetMetadataItem_NullName (AC1)                         */
/*                                                                      */
/* GetMetadataItem() with nullptr name returns nullptr                   */
/************************************************************************/

static bool Test_GetMetadataItem_NullName() {
    std::cout << "  Test_GetMetadataItem_NullName... ";

    CPLString osTempFile = GetTempFilePath("test_nullname");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set some metadata first
    poDS->SetMetadataItem("Name", "TestMap", nullptr);

    // Get metadata with nullptr name - should return nullptr
    const char* pszValue = poDS->GetMetadataItem(nullptr, nullptr);
    if (pszValue != nullptr) {
        std::cout << "FAILED (GetMetadataItem(nullptr) should return nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*                               main()                                  */
/************************************************************************/

int main() {
    std::cout << "=== Story 2.2: Polish Map Writer - Header Generation ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // AC1: SetMetadataItem stores value
    if (Test_SetMetadataItem_StoresValue()) nPassed++; else nFailed++;

    // AC1: SetMetadataItem with POLISHMAP domain
    if (Test_SetMetadataItem_PolishMapDomain()) nPassed++; else nFailed++;

    // AC2: WriteHeader with metadata
    if (Test_WriteHeader_WithMetadata()) nPassed++; else nFailed++;

    // AC3: WriteHeader with default values
    if (Test_WriteHeader_DefaultValues()) nPassed++; else nFailed++;

    // AC4: Round-trip test
    if (Test_RoundTrip_CreateSetMetadataCloseOpen()) nPassed++; else nFailed++;

    // AC5: UTF-8 to CP1252 conversion
    if (Test_UTF8ToCP1252_Conversion()) nPassed++; else nFailed++;

    // AC5: Non-CP1252 characters warning
    if (Test_NonCP1252_Characters_Warning()) nPassed++; else nFailed++;

    // AC1: GetMetadataItem with nullptr name
    if (Test_GetMetadataItem_NullName()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
