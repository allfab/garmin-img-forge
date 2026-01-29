/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 1.3 - Dataset Implementation with 3 Empty Layers
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - GetLayerCount() returns 3
 * - GetLayer() returns correct layers by index
 * - GetLayer() returns nullptr for invalid index
 * - Layer names are "POI", "POLYLINE", "POLYGON"
 * - Layer geometry types are wkbPoint, wkbLineString, wkbPolygon
 * - OGRFeatureDefn field definitions
 * - TestCapability() for dataset and layers
 ****************************************************************************/

#include <iostream>
#include <cassert>
#include <cstring>
#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "cpl_conv.h"
#include "cpl_string.h"
#include "cpl_vsi.h"

// External declaration for driver registration
extern "C" void RegisterOGRPolishMap();

// Test helper: Register driver and create temp file
static void SetupTest() {
    GDALAllRegister();
    RegisterOGRPolishMap();  // Register our custom PolishMap driver
}

// Test helper: Create a minimal valid .mp file for testing
static CPLString CreateTempMPFile() {
    CPLString osTempFile = CPLGenerateTempFilename("test_layers");
    osTempFile += ".mp";

    VSILFILE* fp = VSIFOpenL(osTempFile.c_str(), "wb");
    if (fp != nullptr) {
        const char* pszContent =
            "[IMG ID]\n"
            "Name=Test Layers\n"
            "ID=12345678\n"
            "CodePage=1252\n"
            "[END-IMG ID]\n";
        VSIFWriteL(pszContent, 1, strlen(pszContent), fp);
        VSIFCloseL(fp);
    }
    return osTempFile;
}

// Test helper: Cleanup temp file
static void CleanupTempFile(const CPLString& osFilePath) {
    VSIUnlink(osFilePath.c_str());
}

/************************************************************************/
/*                     Test_GetLayerCount_Returns_3                      */
/*                                                                      */
/* Task 9.1: Test GetLayerCount() returns 3                             */
/************************************************************************/

static bool Test_GetLayerCount_Returns_3() {
    std::cout << "  Test_GetLayerCount_Returns_3... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    int nLayerCount = poDS->GetLayerCount();
    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (nLayerCount != 3) {
        std::cout << "FAILED (expected 3, got " << nLayerCount << ")" << std::endl;
        return false;
    }

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*                  Test_GetLayer_Returns_Correct_Layers                 */
/*                                                                      */
/* Task 9.2: Test GetLayer() returns correct layers by index            */
/************************************************************************/

static bool Test_GetLayer_Returns_Correct_Layers() {
    std::cout << "  Test_GetLayer_Returns_Correct_Layers... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;
    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            bSuccess = false;
            break;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*               Test_GetLayer_Invalid_Index_Returns_Nullptr             */
/*                                                                      */
/* Task 9.3: Test GetLayer() returns nullptr for invalid index          */
/************************************************************************/

static bool Test_GetLayer_Invalid_Index_Returns_Nullptr() {
    std::cout << "  Test_GetLayer_Invalid_Index_Returns_Nullptr... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;

    // Test negative index
    if (poDS->GetLayer(-1) != nullptr) {
        std::cout << "FAILED (GetLayer(-1) should return nullptr)" << std::endl;
        bSuccess = false;
    }

    // Test index out of bounds (>= 3)
    if (bSuccess && poDS->GetLayer(3) != nullptr) {
        std::cout << "FAILED (GetLayer(3) should return nullptr)" << std::endl;
        bSuccess = false;
    }

    // Test large negative index
    if (bSuccess && poDS->GetLayer(-100) != nullptr) {
        std::cout << "FAILED (GetLayer(-100) should return nullptr)" << std::endl;
        bSuccess = false;
    }

    // Test large positive index
    if (bSuccess && poDS->GetLayer(100) != nullptr) {
        std::cout << "FAILED (GetLayer(100) should return nullptr)" << std::endl;
        bSuccess = false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                    Test_Layer_Names_Are_Correct                       */
/*                                                                      */
/* Task 9.4: Test layer names are "POI", "POLYLINE", "POLYGON"          */
/************************************************************************/

static bool Test_Layer_Names_Are_Correct() {
    std::cout << "  Test_Layer_Names_Are_Correct... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;
    const char* apszExpectedNames[] = {"POI", "POLYLINE", "POLYGON"};

    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            bSuccess = false;
            break;
        }

        const char* pszName = poLayer->GetName();
        if (strcmp(pszName, apszExpectedNames[i]) != 0) {
            std::cout << "FAILED (layer " << i << " name is '" << pszName
                      << "', expected '" << apszExpectedNames[i] << "')" << std::endl;
            bSuccess = false;
            break;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                 Test_Layer_Geometry_Types_Are_Correct                 */
/*                                                                      */
/* Task 9.5: Test layer geometry types                                  */
/************************************************************************/

static bool Test_Layer_Geometry_Types_Are_Correct() {
    std::cout << "  Test_Layer_Geometry_Types_Are_Correct... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;
    OGRwkbGeometryType aeExpectedTypes[] = {wkbPoint, wkbLineString, wkbPolygon};
    const char* apszTypeNames[] = {"wkbPoint", "wkbLineString", "wkbPolygon"};

    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            bSuccess = false;
            break;
        }

        OGRwkbGeometryType eGeomType = poLayer->GetGeomType();
        if (eGeomType != aeExpectedTypes[i]) {
            std::cout << "FAILED (layer " << i << " geometry type is " << eGeomType
                      << ", expected " << apszTypeNames[i] << ")" << std::endl;
            bSuccess = false;
            break;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                  Test_FeatureDefn_Field_Definitions                   */
/*                                                                      */
/* Task 9.6: Test OGRFeatureDefn field definitions                      */
/************************************************************************/

static bool Test_FeatureDefn_Field_Definitions() {
    std::cout << "  Test_FeatureDefn_Field_Definitions... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;
    OGRLayer* poLayer = poDS->GetLayer(0);  // Test POI layer

    if (poLayer == nullptr) {
        std::cout << "FAILED (GetLayer(0) returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
    if (poDefn == nullptr) {
        std::cout << "FAILED (GetLayerDefn() returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Expected fields: Type, Label, Data0, EndLevel, Levels
    const char* apszExpectedFields[] = {"Type", "Label", "Data0", "EndLevel", "Levels"};
    OGRFieldType aeExpectedTypes[] = {OFTString, OFTString, OFTInteger, OFTInteger, OFTString};
    int nExpectedCount = 5;

    if (poDefn->GetFieldCount() != nExpectedCount) {
        std::cout << "FAILED (expected " << nExpectedCount << " fields, got "
                  << poDefn->GetFieldCount() << ")" << std::endl;
        bSuccess = false;
    }

    if (bSuccess) {
        for (int i = 0; i < nExpectedCount; i++) {
            int nFieldIndex = poDefn->GetFieldIndex(apszExpectedFields[i]);
            if (nFieldIndex < 0) {
                std::cout << "FAILED (field '" << apszExpectedFields[i]
                          << "' not found)" << std::endl;
                bSuccess = false;
                break;
            }

            OGRFieldDefn* poFieldDefn = poDefn->GetFieldDefn(nFieldIndex);
            if (poFieldDefn->GetType() != aeExpectedTypes[i]) {
                std::cout << "FAILED (field '" << apszExpectedFields[i]
                          << "' has wrong type)" << std::endl;
                bSuccess = false;
                break;
            }
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                      Test_TestCapability_Dataset                      */
/*                                                                      */
/* Task 9.7a: Test TestCapability() for dataset                         */
/************************************************************************/

static bool Test_TestCapability_Dataset() {
    std::cout << "  Test_TestCapability_Dataset... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;

    // ODsCRandomLayerRead should be TRUE
    if (!poDS->TestCapability(ODsCRandomLayerRead)) {
        std::cout << "FAILED (ODsCRandomLayerRead should be TRUE)" << std::endl;
        bSuccess = false;
    }

    // ODsCCreateLayer should be FALSE (not implemented yet)
    if (bSuccess && poDS->TestCapability(ODsCCreateLayer)) {
        std::cout << "FAILED (ODsCCreateLayer should be FALSE)" << std::endl;
        bSuccess = false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                       Test_TestCapability_Layer                       */
/*                                                                      */
/* Task 9.7b: Test TestCapability() for layer                           */
/************************************************************************/

static bool Test_TestCapability_Layer() {
    std::cout << "  Test_TestCapability_Layer... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLayer* poLayer = poDS->GetLayer(0);
    if (poLayer == nullptr) {
        std::cout << "FAILED (GetLayer(0) returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;

    // OLCRandomRead should be FALSE (not implemented)
    if (poLayer->TestCapability(OLCRandomRead)) {
        std::cout << "FAILED (OLCRandomRead should be FALSE)" << std::endl;
        bSuccess = false;
    }

    // OLCSequentialWrite should be FALSE (not implemented yet)
    if (bSuccess && poLayer->TestCapability(OLCSequentialWrite)) {
        std::cout << "FAILED (OLCSequentialWrite should be FALSE)" << std::endl;
        bSuccess = false;
    }

    // OLCFastFeatureCount should be FALSE (no optimization)
    if (bSuccess && poLayer->TestCapability(OLCFastFeatureCount)) {
        std::cout << "FAILED (OLCFastFeatureCount should be FALSE)" << std::endl;
        bSuccess = false;
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                    Test_Layer_Spatial_Reference                       */
/*                                                                      */
/* Additional: Test WGS84 spatial reference is assigned                 */
/************************************************************************/

static bool Test_Layer_Spatial_Reference() {
    std::cout << "  Test_Layer_Spatial_Reference... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;

    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            bSuccess = false;
            break;
        }

        OGRSpatialReference* poSRS = poLayer->GetSpatialRef();
        if (poSRS == nullptr) {
            std::cout << "FAILED (layer " << i << " has no spatial reference)" << std::endl;
            bSuccess = false;
            break;
        }

        // Check if it's WGS84
        if (!poSRS->IsGeographic()) {
            std::cout << "FAILED (layer " << i << " SRS is not geographic)" << std::endl;
            bSuccess = false;
            break;
        }

        // Check authority code (should be EPSG:4326 for WGS84)
        const char* pszAuthorityCode = poSRS->GetAuthorityCode("GEOGCS");
        if (pszAuthorityCode == nullptr || strcmp(pszAuthorityCode, "4326") != 0) {
            // WGS84 might not always have authority code set, so also check name
            const char* pszGeogCS = poSRS->GetAttrValue("GEOGCS");
            if (pszGeogCS == nullptr || strstr(pszGeogCS, "WGS") == nullptr) {
                std::cout << "FAILED (layer " << i << " SRS is not WGS84)" << std::endl;
                bSuccess = false;
                break;
            }
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                    Test_Layer_Feature_Count_Is_Zero                   */
/*                                                                      */
/* L2 Fix: Test that empty layers return feature count of 0              */
/************************************************************************/

static bool Test_Layer_Feature_Count_Is_Zero() {
    std::cout << "  Test_Layer_Feature_Count_Is_Zero... ";

    CPLString osTempFile = CreateTempMPFile();
    GDALDataset* poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);

    if (poDS == nullptr) {
        std::cout << "FAILED (cannot open file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    bool bSuccess = true;

    for (int i = 0; i < 3; i++) {
        OGRLayer* poLayer = poDS->GetLayer(i);
        if (poLayer == nullptr) {
            std::cout << "FAILED (GetLayer(" << i << ") returned nullptr)" << std::endl;
            bSuccess = false;
            break;
        }

        GIntBig nCount = poLayer->GetFeatureCount();
        if (nCount != 0) {
            std::cout << "FAILED (layer " << i << " has " << nCount
                      << " features, expected 0)" << std::endl;
            bSuccess = false;
            break;
        }
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    if (bSuccess) {
        std::cout << "PASSED" << std::endl;
    }
    return bSuccess;
}

/************************************************************************/
/*                               main()                                  */
/************************************************************************/

int main() {
    std::cout << "=== Story 1.3: Dataset Implementation with 3 Empty Layers ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // Task 9.1
    if (Test_GetLayerCount_Returns_3()) nPassed++; else nFailed++;

    // Task 9.2
    if (Test_GetLayer_Returns_Correct_Layers()) nPassed++; else nFailed++;

    // Task 9.3
    if (Test_GetLayer_Invalid_Index_Returns_Nullptr()) nPassed++; else nFailed++;

    // Task 9.4
    if (Test_Layer_Names_Are_Correct()) nPassed++; else nFailed++;

    // Task 9.5
    if (Test_Layer_Geometry_Types_Are_Correct()) nPassed++; else nFailed++;

    // Task 9.6
    if (Test_FeatureDefn_Field_Definitions()) nPassed++; else nFailed++;

    // Task 9.7a
    if (Test_TestCapability_Dataset()) nPassed++; else nFailed++;

    // Task 9.7b
    if (Test_TestCapability_Layer()) nPassed++; else nFailed++;

    // Additional: SRS test
    if (Test_Layer_Spatial_Reference()) nPassed++; else nFailed++;

    // L2 Fix: Feature count test
    if (Test_Layer_Feature_Count_Is_Zero()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
