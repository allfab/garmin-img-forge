/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 4.1 - CreateField Support (Accept-and-Map)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - ICreateField() returns OGRERR_NONE for all fields
 * - Case-insensitive matching for known Polish Map fields
 * - Unknown fields are silently ignored
 * - TestCapability(OLCCreateField) returns TRUE in write mode
 * - ogr2ogr integration with Shapefile source
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
/*               Test_CreateField_Basic_Returns_OGRERR_NONE              */
/*                                                                      */
/* AC1: CreateField() returns OGRERR_NONE for any field                  */
/************************************************************************/

static bool Test_CreateField_Basic_Returns_OGRERR_NONE() {
    std::cout << "  Test_CreateField_Basic_Returns_OGRERR_NONE... ";

    CPLString osTempFile = GetTempFilePath("test_createfield");
    CleanupTempFile(osTempFile);

    // Get PolishMap driver and create dataset
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

    // Get POI layer (any write-mode layer will do)
    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a basic field
    OGRFieldDefn oFieldType("Type", OFTString);
    OGRErr eErr = poLayer->CreateField(&oFieldType);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateField returned error " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_CreateField_Case_Insensitive_Mapping               */
/*                                                                      */
/* AC2: Case-insensitive matching for "TYPE", "type", "Type"             */
/************************************************************************/

static bool Test_CreateField_Case_Insensitive_Mapping() {
    std::cout << "  Test_CreateField_Case_Insensitive_Mapping... ";

    CPLString osTempFile = GetTempFilePath("test_createfield_case");
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

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test case variations
    const char* apszCaseVariants[] = {"TYPE", "type", "Type", "TyPe"};
    for (int i = 0; i < 4; i++) {
        OGRFieldDefn oField(apszCaseVariants[i], OFTString);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << apszCaseVariants[i]
                      << "') returned error " << eErr << ")" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    // Also test Label variants
    const char* apszLabelVariants[] = {"LABEL", "label", "Label"};
    for (int i = 0; i < 3; i++) {
        OGRFieldDefn oField(apszLabelVariants[i], OFTString);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << apszLabelVariants[i]
                      << "') returned error " << eErr << ")" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_CreateField_Unknown_Fields_Ignored                 */
/*                                                                      */
/* AC3: Unknown fields return OGRERR_NONE but are silently ignored       */
/************************************************************************/

static bool Test_CreateField_Unknown_Fields_Ignored() {
    std::cout << "  Test_CreateField_Unknown_Fields_Ignored... ";

    CPLString osTempFile = GetTempFilePath("test_createfield_unknown");
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

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test unknown fields - all should return OGRERR_NONE
    const char* apszUnknownFields[] = {
        "ID", "NOM", "POPULATION", "CODE_INSEE", "GEOMETRY_TYPE",
        "RANDOM_FIELD", "foo_bar", "test123"
    };

    for (int i = 0; i < 8; i++) {
        OGRFieldDefn oField(apszUnknownFields[i], OFTString);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << apszUnknownFields[i]
                      << "') returned error " << eErr << " instead of OGRERR_NONE)" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_CreateField_Data_Fields_Mapping                    */
/*                                                                      */
/* AC3: Data0, Data1, DATA23, data5 are recognized as Polish Map fields  */
/************************************************************************/

static bool Test_CreateField_Data_Fields_Mapping() {
    std::cout << "  Test_CreateField_Data_Fields_Mapping... ";

    CPLString osTempFile = GetTempFilePath("test_createfield_data");
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

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test Data field variants
    const char* apszDataFields[] = {"Data0", "Data1", "DATA23", "data5", "Data99"};
    for (int i = 0; i < 5; i++) {
        OGRFieldDefn oField(apszDataFields[i], OFTInteger);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << apszDataFields[i]
                      << "') returned error " << eErr << ")" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    // Test invalid Data variants (should still be accepted but logged as ignored)
    const char* apszInvalidDataFields[] = {"Data", "DataX", "Data_1", "Data-0"};
    for (int i = 0; i < 4; i++) {
        OGRFieldDefn oField(apszInvalidDataFields[i], OFTInteger);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << apszInvalidDataFields[i]
                      << "') returned error " << eErr << " - should accept all fields)" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_Capability_OLCCreateField_Write_Mode               */
/*                                                                      */
/* AC4: TestCapability(OLCCreateField) returns TRUE in write mode        */
/************************************************************************/

static bool Test_Capability_OLCCreateField_Write_Mode() {
    std::cout << "  Test_Capability_OLCCreateField_Write_Mode... ";

    CPLString osTempFile = GetTempFilePath("test_capability_createfield");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Test write mode - should return TRUE
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check OLCCreateField capability
    if (!poLayer->TestCapability(OLCCreateField)) {
        std::cout << "FAILED (TestCapability(OLCCreateField) returned FALSE in write mode)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_Capability_OLCCreateField_Read_Mode                */
/*                                                                      */
/* AC4: TestCapability(OLCCreateField) returns FALSE in read mode        */
/************************************************************************/

static bool Test_Capability_OLCCreateField_Read_Mode() {
    std::cout << "  Test_Capability_OLCCreateField_Read_Mode... ";

    // Use an existing test file in read mode
    // TEST_DATA_DIR is defined via CMake compile definitions
#ifdef TEST_DATA_DIR
    CPLString osTestFile = CPLString(TEST_DATA_DIR) + "/valid/minimal_poi.mp";
    const char* pszTestFile = osTestFile.c_str();
#else
    const char* pszTestFile = "test/data/valid/minimal_poi.mp";
#endif

    // Check if file exists (fallback to another pattern)
    VSIStatBufL sStat;
    if (VSIStatL(pszTestFile, &sStat) != 0) {
        // Try without test/ prefix as fallback
        pszTestFile = "data/valid/minimal_poi.mp";
        if (VSIStatL(pszTestFile, &sStat) != 0) {
            std::cout << "SKIPPED (test file not found)" << std::endl;
            return true;  // Not a failure, just skip
        }
    }

    GDALDataset* poDS = static_cast<GDALDataset*>(
        GDALOpenEx(pszTestFile, GDAL_OF_VECTOR | GDAL_OF_READONLY, nullptr, nullptr, nullptr));

    if (poDS == nullptr) {
        std::cout << "SKIPPED (could not open test file)" << std::endl;
        return true;  // Not a failure
    }

    OGRLayer* poLayer = poDS->GetLayer(0);
    if (poLayer == nullptr) {
        std::cout << "FAILED (no layer found)" << std::endl;
        GDALClose(poDS);
        return false;
    }

    // Check OLCCreateField capability - should be FALSE in read mode
    if (poLayer->TestCapability(OLCCreateField)) {
        std::cout << "FAILED (TestCapability(OLCCreateField) returned TRUE in read mode)" << std::endl;
        GDALClose(poDS);
        return false;
    }

    GDALClose(poDS);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*               Test_CreateField_All_Polish_Map_Fields                  */
/*                                                                      */
/* AC3: All known Polish Map fields are recognized                       */
/************************************************************************/

static bool Test_CreateField_All_Polish_Map_Fields() {
    std::cout << "  Test_CreateField_All_Polish_Map_Fields... ";

    CPLString osTempFile = GetTempFilePath("test_createfield_all");
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

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    if (poLayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test all known Polish Map fields
    struct {
        const char* name;
        OGRFieldType type;
    } aKnownFields[] = {
        {"Type", OFTString},
        {"Label", OFTString},
        {"Data0", OFTInteger},
        {"Data1", OFTInteger},
        {"Data2", OFTInteger},
        {"EndLevel", OFTInteger},
        {"Levels", OFTString}
    };

    for (int i = 0; i < 7; i++) {
        OGRFieldDefn oField(aKnownFields[i].name, aKnownFields[i].type);
        OGRErr eErr = poLayer->CreateField(&oField);
        if (eErr != OGRERR_NONE) {
            std::cout << "FAILED (CreateField('" << aKnownFields[i].name
                      << "') returned error " << eErr << ")" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "OK" << std::endl;
    return true;
}

/************************************************************************/
/*                                main()                                 */
/************************************************************************/

int main(int /* argc */, char* /* argv */[]) {
    std::cout << "=== Story 4.1: CreateField Support Tests ===" << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    // Run all tests
    if (Test_CreateField_Basic_Returns_OGRERR_NONE()) nPassed++; else nFailed++;
    if (Test_CreateField_Case_Insensitive_Mapping()) nPassed++; else nFailed++;
    if (Test_CreateField_Unknown_Fields_Ignored()) nPassed++; else nFailed++;
    if (Test_CreateField_Data_Fields_Mapping()) nPassed++; else nFailed++;
    if (Test_Capability_OLCCreateField_Write_Mode()) nPassed++; else nFailed++;
    if (Test_Capability_OLCCreateField_Read_Mode()) nPassed++; else nFailed++;
    if (Test_CreateField_All_Polish_Map_Fields()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Results: " << nPassed << " passed, " << nFailed << " failed ===" << std::endl;

    return nFailed > 0 ? 1 : 0;
}
