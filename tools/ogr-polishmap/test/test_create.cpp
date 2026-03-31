/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.1 - Create() Method & Empty Dataset Creation
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - Create() with valid path creates new file
 * - Create() on existing file overwrites it
 * - Create() with invalid path returns NULL + CPLError
 * - GetLayerCount() returns 3 for created dataset
 * - Created dataset can be reopened after close
 * - Driver metadata GDAL_DCAP_CREATE is set
 ****************************************************************************/

#include <iostream>
#include <cassert>
#include <cstring>
#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "cpl_conv.h"
#include "cpl_string.h"
#include "cpl_vsi.h"
#include "cpl_error.h"

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

/************************************************************************/
/*               Test_Create_Valid_Path_Creates_Dataset                  */
/*                                                                      */
/* AC1: Create() with valid path creates new file with 3 layers         */
/************************************************************************/

static bool Test_Create_Valid_Path_Creates_Dataset() {
    std::cout << "  Test_Create_Valid_Path_Creates_Dataset... ";

    CPLString osTempFile = GetTempFilePath("test_create");

    // Ensure file doesn't exist before test
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

    // Check layer count is 3
    int nLayerCount = poDS->GetLayerCount();
    if (nLayerCount != 3) {
        std::cout << "FAILED (expected 3 layers, got " << nLayerCount << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify layer names
    const char* apszExpectedNames[] = {"POI", "POLYLINE", "POLYGON"};
    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr || strcmp(poLayer->GetName(), apszExpectedNames[i]) != 0) {
            std::cout << "FAILED (layer " << i << " name mismatch)" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_Create_Overwrites_Existing_File                    */
/*                                                                      */
/* AC2: Create() on existing file overwrites it                         */
/************************************************************************/

static bool Test_Create_Overwrites_Existing_File() {
    std::cout << "  Test_Create_Overwrites_Existing_File... ";

    CPLString osTempFile = GetTempFilePath("test_overwrite");

    // Create initial file with some content
    VSILFILE* fp = VSIFOpenL(osTempFile.c_str(), "wb");
    if (fp != nullptr) {
        const char* pszContent = "This is old content that should be overwritten";
        VSIFWriteL(pszContent, 1, strlen(pszContent), fp);
        VSIFCloseL(fp);
    }

    // Verify file exists
    VSIStatBufL sStat;
    if (VSIStatL(osTempFile.c_str(), &sStat) != 0) {
        std::cout << "FAILED (could not create initial file)" << std::endl;
        return false;
    }

    vsi_l_offset nOldSize = sStat.st_size;

    // Get PolishMap driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create dataset (should overwrite)
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);

    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Close to flush content
    GDALClose(poDS);

    // Verify file exists and has different content
    if (VSIStatL(osTempFile.c_str(), &sStat) != 0) {
        std::cout << "FAILED (file does not exist after Create)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Read file content to verify it's a valid Polish Map file
    fp = VSIFOpenL(osTempFile.c_str(), "rb");
    if (fp == nullptr) {
        std::cout << "FAILED (cannot read created file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    char szBuffer[256];
    memset(szBuffer, 0, sizeof(szBuffer));
    VSIFReadL(szBuffer, 1, sizeof(szBuffer) - 1, fp);
    VSIFCloseL(fp);

    // Verify [IMG ID] header exists (Polish Map format)
    if (strstr(szBuffer, "[IMG ID]") == nullptr) {
        std::cout << "FAILED (file does not contain [IMG ID] header)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_Create_Invalid_Path_Returns_Null                   */
/*                                                                      */
/* AC3: Create() with invalid path returns NULL + CPLError              */
/************************************************************************/

static bool Test_Create_Invalid_Path_Returns_Null() {
    std::cout << "  Test_Create_Invalid_Path_Returns_Null... ";

    // Use an invalid path (non-existent directory)
    const char* pszInvalidPath = "/nonexistent/directory/that/does/not/exist/output.mp";

    // Clear previous errors
    CPLErrorReset();

    // Get PolishMap driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Create dataset (should fail)
    GDALDataset* poDS = poDriver->Create(pszInvalidPath, 0, 0, 0, GDT_Unknown, nullptr);

    if (poDS != nullptr) {
        std::cout << "FAILED (Create() should return nullptr for invalid path)" << std::endl;
        GDALClose(poDS);
        return false;
    }

    // Verify CPLError was logged
    if (CPLGetLastErrorNo() != CPLE_OpenFailed) {
        std::cout << "FAILED (expected CPLE_OpenFailed error, got "
                  << CPLGetLastErrorNo() << ")" << std::endl;
        return false;
    }

    std::cout << "PASSED" << std::endl;
    return true;
}

// Note: Test_Create_Null_Path cannot be tested via GDALDriver::Create()
// because GDAL's internal code calls VSIStatExL(nullptr) which crashes
// before reaching our Create() method. The NULL check in our code
// serves as defense-in-depth for direct calls to OGRPolishMapDataSource::Create().

/************************************************************************/
/*              Test_GetLayerCount_Returns_3_Empty_Layers                */
/*                                                                      */
/* AC4: GetLayerCount() returns 3, all layers empty                     */
/************************************************************************/

static bool Test_GetLayerCount_Returns_3_Empty_Layers() {
    std::cout << "  Test_GetLayerCount_Returns_3_Empty_Layers... ";

    CPLString osTempFile = GetTempFilePath("test_layers");
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

    // Check layer count
    if (poDS->GetLayerCount() != 3) {
        std::cout << "FAILED (expected 3 layers)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check all layers are empty
    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }

        GIntBig nCount = poLayer->GetFeatureCount();
        if (nCount != 0) {
            std::cout << "FAILED (layer " << i << " has " << nCount
                      << " features, expected 0)" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_Created_Dataset_Can_Be_Reopened                    */
/*                                                                      */
/* AC5: Created dataset can be reopened with Open() after close         */
/************************************************************************/

static bool Test_Created_Dataset_Can_Be_Reopened() {
    std::cout << "  Test_Created_Dataset_Can_Be_Reopened... ";

    CPLString osTempFile = GetTempFilePath("test_reopen");
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

    // Close the dataset (this should write the minimal file)
    GDALClose(poDS);

    // Verify file exists
    VSIStatBufL sStat;
    if (VSIStatL(osTempFile.c_str(), &sStat) != 0) {
        std::cout << "FAILED (file does not exist after close)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Try to reopen with Open()
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen created file with Open())" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify it has 3 layers
    if (poDS->GetLayerCount() != 3) {
        std::cout << "FAILED (reopened file has " << poDS->GetLayerCount()
                  << " layers, expected 3)" << std::endl;
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
/*               Test_Driver_Metadata_GDAL_DCAP_CREATE                    */
/*                                                                      */
/* Task 4.1: Driver has GDAL_DCAP_CREATE = "YES"                        */
/************************************************************************/

static bool Test_Driver_Metadata_GDAL_DCAP_CREATE() {
    std::cout << "  Test_Driver_Metadata_GDAL_DCAP_CREATE... ";

    // Get PolishMap driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Check GDAL_DCAP_CREATE metadata
    const char* pszCreate = poDriver->GetMetadataItem(GDAL_DCAP_CREATE);
    if (pszCreate == nullptr || strcmp(pszCreate, "YES") != 0) {
        std::cout << "FAILED (GDAL_DCAP_CREATE is not 'YES')" << std::endl;
        return false;
    }

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*            Test_TestCapability_ODsCCreateLayer                        */
/*                                                                      */
/* Story 2.6: ODsCCreateLayer TRUE in write mode (for ogr2ogr),         */
/* FALSE in read mode.                                                   */
/************************************************************************/

static bool Test_TestCapability_ODsCCreateLayer_Always_False() {
    std::cout << "  Test_TestCapability_ODsCCreateLayer... ";

    CPLString osTempFile = GetTempFilePath("test_capability");
    CleanupTempFile(osTempFile);

    // Get PolishMap driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Create dataset (write mode)
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);

    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Story 2.6: Check ODsCCreateLayer capability in write mode - should be TRUE
    // This enables ogr2ogr to work with the driver via ICreateLayer()
    bool bCreateLayerWrite = poDS->TestCapability(ODsCCreateLayer);

    GDALClose(poDS);

    if (!bCreateLayerWrite) {
        std::cout << "FAILED (ODsCCreateLayer should be TRUE in write mode for ogr2ogr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Also test in read mode - reopen the file
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file for read mode test)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bCreateLayerRead = poDS->TestCapability(ODsCCreateLayer);

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bCreateLayerRead) {
        std::cout << "FAILED (ODsCCreateLayer should be FALSE in read mode)" << std::endl;
        return false;
    }

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*                               main()                                  */
/************************************************************************/

int main() {
    std::cout << "=== Story 2.1: Create() Method & Empty Dataset Creation ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // AC1: Create with valid path
    if (Test_Create_Valid_Path_Creates_Dataset()) nPassed++; else nFailed++;

    // AC2: Overwrite existing file
    if (Test_Create_Overwrites_Existing_File()) nPassed++; else nFailed++;

    // AC3: Invalid path returns NULL
    if (Test_Create_Invalid_Path_Returns_Null()) nPassed++; else nFailed++;

    // Note: NULL path test skipped - GDAL crashes before reaching our code

    // AC4: 3 empty layers
    if (Test_GetLayerCount_Returns_3_Empty_Layers()) nPassed++; else nFailed++;

    // AC5: Reopen after close
    if (Test_Created_Dataset_Can_Be_Reopened()) nPassed++; else nFailed++;

    // Task 4.1: GDAL_DCAP_CREATE metadata
    if (Test_Driver_Metadata_GDAL_DCAP_CREATE()) nPassed++; else nFailed++;

    // ODsCCreateLayer should always be FALSE (fixed layers)
    if (Test_TestCapability_ODsCCreateLayer_Always_False()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
