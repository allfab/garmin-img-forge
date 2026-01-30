/******************************************************************************
 * Project:  OGR PolishMap Driver - POLYLINE Layer Tests
 * Purpose:  Test POLYLINE feature reading functionality
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

#include "gdal_priv.h"
#include "ogr_api.h"
#include "ogrsf_frmts.h"
#include "cpl_conv.h"
#include <iostream>
#include <cmath>

// Declare the driver registration function
extern "C" void RegisterOGRPolishMap();

#ifndef TEST_DATA_DIR
#define TEST_DATA_DIR "test/data"
#endif

static int g_nTests = 0;
static int g_nTestsPassed = 0;
static int g_nTestsFailed = 0;

#define TEST_START(name) \
    std::cout << "\n=== Test: " << name << " ===" << std::endl; \
    g_nTests++; \
    bool bTestPassed = true

#define CHECK(condition, message) \
    if (!(condition)) { \
        std::cerr << "[FAIL] " << message << std::endl; \
        bTestPassed = false; \
    } else { \
        std::cout << "[OK] " << message << std::endl; \
    }

#define CHECK_NEAR(val1, val2, epsilon, message) \
    if (std::fabs((val1) - (val2)) > (epsilon)) { \
        std::cerr << "[FAIL] " << message << " (expected: " << (val2) \
                  << ", got: " << (val1) << ")" << std::endl; \
        bTestPassed = false; \
    } else { \
        std::cout << "[OK] " << message << std::endl; \
    }

#define TEST_END() \
    if (bTestPassed) { \
        std::cout << "✓ Test PASSED" << std::endl; \
        g_nTestsPassed++; \
    } else { \
        std::cout << "✗ Test FAILED" << std::endl; \
        g_nTestsFailed++; \
    }

/************************************************************************/
/*               Test 5.1: Simple POLYLINE (minimal valid)              */
/************************************************************************/

void TestSimplePolyline() {
    TEST_START("Simple POLYLINE - 1 feature, 2 points (AC1, AC2, AC3, AC8)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-simple.mp");

    if (poDS != nullptr) {
        // Get POLYLINE layer (index 1 - POI is 0)
        OGRLayer* poLayer = poDS->GetLayer(1);
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            CHECK(std::string(poLayer->GetName()) == "POLYLINE", "Layer name is 'POLYLINE'");

            // Check layer definition (AC3)
            OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
            CHECK(poDefn != nullptr, "GetLayerDefn() returns non-null");
            CHECK(poDefn->GetGeomType() == wkbLineString, "Geometry type is wkbLineString");
            CHECK(poDefn->GetFieldCount() == 5, "5 fields defined");

            // Read first feature (AC1)
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                // AC8: Check FID
                CHECK(poFeature->GetFID() == 1, "FID is 1");

                // AC2: Check geometry
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");
                CHECK(poGeom->getGeometryType() == wkbLineString, "Geometry is LineString");

                OGRLineString* poLine = poGeom->toLineString();
                CHECK(poLine->getNumPoints() == 2, "LineString has 2 points");

                // Check coordinates (Data0 and Data1)
                CHECK_NEAR(poLine->getX(0), 2.3522, 0.0001, "Point 0 lon correct");
                CHECK_NEAR(poLine->getY(0), 48.8566, 0.0001, "Point 0 lat correct");
                CHECK_NEAR(poLine->getX(1), 2.3533, 0.0001, "Point 1 lon correct");
                CHECK_NEAR(poLine->getY(1), 48.8577, 0.0001, "Point 1 lat correct");

                // AC2/FR40: Check WGS84 spatial reference assigned
                const OGRSpatialReference* poSRS = poGeom->getSpatialReference();
                CHECK(poSRS != nullptr, "Geometry has spatial reference (FR40)");
                if (poSRS != nullptr) {
                    CHECK(poSRS->IsGeographic(), "SRS is geographic (WGS84)");
                }

                // AC1: Check fields
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x16", "Type field correct");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Mountain Trail", "Label field correct");

                OGRFeature::DestroyFeature(poFeature);
            }

            // Check no more features
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "No more features after first");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.2: Multiple POLYLINE Features                   */
/************************************************************************/

void TestMultiplePolylines() {
    TEST_START("Multiple POLYLINE - 5 features (AC1, AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            int nCount = 0;
            OGRFeature* poFeature = nullptr;

            // Count features
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nCount++;

                // Check second feature has Levels and EndLevel (AC4)
                if (nCount == 2) {
                    CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Highway A6", "Feature 2 label correct");
                    CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "EndLevel field is 3");
                    CHECK(std::string(poFeature->GetFieldAsString("Levels")) == "0-3", "Levels field is '0-3'");
                }

                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nCount == 5, "5 POLYLINE features found");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.3: POLYLINE with Many Points                    */
/************************************************************************/

void TestPolylineManyPoints() {
    TEST_START("POLYLINE with 10+ points (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-many-points.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-many-points.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");

                OGRLineString* poLine = poGeom->toLineString();
                CHECK(poLine->getNumPoints() == 12, "LineString has 12 points (Data0-Data11)");

                // Validate first and last coordinates
                CHECK_NEAR(poLine->getX(0), 2.3500, 0.0001, "First point lon correct");
                CHECK_NEAR(poLine->getY(0), 48.8500, 0.0001, "First point lat correct");
                CHECK_NEAR(poLine->getX(11), 2.3610, 0.0001, "Last point lon correct");
                CHECK_NEAR(poLine->getY(11), 48.8610, 0.0001, "Last point lat correct");

                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.4: GetNextFeature() returns nullptr             */
/************************************************************************/

void TestPolylineEOF() {
    TEST_START("GetNextFeature() returns nullptr after last (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-simple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            // Read first feature
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "First feature exists");
            if (poFeature) OGRFeature::DestroyFeature(poFeature);

            // Try reading second feature (should be nullptr)
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "Second GetNextFeature() returns nullptr");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.5: ResetReading() allows re-iteration           */
/************************************************************************/

void TestPolylineResetReading() {
    TEST_START("ResetReading() permits re-iteration (AC7)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            // First pass: count features
            int nFirstPass = 0;
            OGRFeature* poFeature = nullptr;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nFirstPass++;
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(nFirstPass == 5, "First pass: 5 features");

            // Reset and read again
            poLayer->ResetReading();
            int nSecondPass = 0;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nSecondPass++;
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(nSecondPass == 5, "Second pass after reset: 5 features");

            // Verify first feature after reset
            poLayer->ResetReading();
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "First feature after reset exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Trail 1",
                      "First feature label correct after reset");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.6: Coordinate Correctness                       */
/************************************************************************/

void TestPolylineCoordinates() {
    TEST_START("POLYLINE coordinates correct (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-simple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRLineString* poLine = poGeom->toLineString();

                // Verify all coordinates match test data exactly
                CHECK_NEAR(poLine->getX(0), 2.3522, 0.0001, "Data0 lon = 2.3522");
                CHECK_NEAR(poLine->getY(0), 48.8566, 0.0001, "Data0 lat = 48.8566");
                CHECK_NEAR(poLine->getX(1), 2.3533, 0.0001, "Data1 lon = 2.3533");
                CHECK_NEAR(poLine->getY(1), 48.8577, 0.0001, "Data1 lat = 48.8577");

                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.7: Type, Label, Levels Fields                   */
/************************************************************************/

void TestPolylineFields() {
    TEST_START("POLYLINE Type, Label, Levels fields (AC1, AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            // Test Feature 1: Trail 1
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 1 exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x16", "Feature 1 Type");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Trail 1", "Feature 1 Label");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Test Feature 2: Highway A6 with Levels
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 2 exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x0D", "Feature 2 Type");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Highway A6", "Feature 2 Label");
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "Feature 2 EndLevel");
                CHECK(std::string(poFeature->GetFieldAsString("Levels")) == "0-3", "Feature 2 Levels");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.8: Layer Filtering (POLYLINE only)              */
/************************************************************************/

void TestPolylineLayerFiltering() {
    TEST_START("POLYLINE layer ignores POI/POLYGON sections (AC5)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-mixed-sections.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-mixed-sections.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            int nPolylineCount = 0;
            OGRFeature* poFeature = nullptr;

            // Expected POLYLINE labels (not POI: Restaurant, Hotel; not POLYGON: Park Zone)
            const char* apszExpectedLabels[] = {"Trail Alpha", "Route Beta", "River Gamma"};

            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                // Verify all features are POLYLINE type (geometry is LineString)
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");
                CHECK(poGeom->getGeometryType() == wkbLineString, "Geometry is LineString (not Point/Polygon)");

                // Verify label matches expected POLYLINE (not POI/POLYGON)
                if (nPolylineCount < 3) {
                    std::string osLabel = poFeature->GetFieldAsString("Label");
                    CHECK(osLabel == apszExpectedLabels[nPolylineCount],
                          CPLSPrintf("Feature %d label is '%s' (POLYLINE, not POI/POLYGON)",
                                     nPolylineCount + 1, apszExpectedLabels[nPolylineCount]));
                }

                nPolylineCount++;
                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nPolylineCount == 3, "3 POLYLINE features (POI/POLYGON filtered out)");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.9: FID Sequential (1, 2, 3...)                  */
/************************************************************************/

void TestPolylineFIDSequential() {
    TEST_START("FID sequential starting at 1 (AC8)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            for (int i = 1; i <= 5; i++) {
                OGRFeature* poFeature = poLayer->GetNextFeature();
                CHECK(poFeature != nullptr, CPLSPrintf("Feature %d exists", i));
                if (poFeature) {
                    CHECK(poFeature->GetFID() == i, CPLSPrintf("Feature FID is %d", i));
                    OGRFeature::DestroyFeature(poFeature);
                }
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.10: Invalid POLYLINE (1 point)                  */
/************************************************************************/

// Global variable to capture error messages for AC6 test
static CPLString g_osLastErrorMsg;
static int g_nLastErrorType = CE_None;

static void CPLTestErrorHandler(CPLErr eErrClass, CPLErrorNum /* nError */, const char* pszMsg) {
    g_nLastErrorType = eErrClass;
    g_osLastErrorMsg = pszMsg;
}

void TestPolylineOnePointInvalid() {
    TEST_START("POLYLINE with 1 point skipped with warning (AC6)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "error-recovery/polyline-one-point.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-one-point.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            // AC6: Install error handler to capture CE_Warning
            g_osLastErrorMsg.clear();
            g_nLastErrorType = CE_None;
            CPLPushErrorHandler(CPLTestErrorHandler);

            // First feature should be skipped (1 point), second should be valid
            OGRFeature* poFeature = poLayer->GetNextFeature();

            // AC6: Verify CE_Warning was logged
            CPLPopErrorHandler();
            CHECK(g_nLastErrorType == CE_Warning, "CPLError(CE_Warning) was logged (AC6)");
            CHECK(g_osLastErrorMsg.find("less than 2 points") != std::string::npos,
                  "Warning message mentions 'less than 2 points'");

            CHECK(poFeature != nullptr, "Valid feature found (invalid skipped)");

            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Valid Trail Two Points",
                      "Valid feature is the 2-point trail");
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRLineString* poLine = poGeom->toLineString();
                CHECK(poLine->getNumPoints() == 2, "Valid feature has 2 points");
                OGRFeature::DestroyFeature(poFeature);
            }

            // No more features after valid one
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "No more features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.11: Coordinates Without Parentheses             */
/************************************************************************/

void TestPolylineNoParentheses() {
    TEST_START("POLYLINE coordinates without parentheses (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polyline-no-parens.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polyline-no-parens.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE layer
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRLineString* poLine = poGeom->toLineString();
                CHECK(poLine->getNumPoints() == 3, "LineString has 3 points");

                // Verify coordinates parsed correctly without parentheses
                CHECK_NEAR(poLine->getX(0), 2.3522, 0.0001, "Point 0 lon correct");
                CHECK_NEAR(poLine->getY(0), 48.8566, 0.0001, "Point 0 lat correct");
                CHECK_NEAR(poLine->getX(2), 2.3544, 0.0001, "Point 2 lon correct");
                CHECK_NEAR(poLine->getY(2), 48.8588, 0.0001, "Point 2 lat correct");

                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*                             main()                                   */
/************************************************************************/

int main(int /* argc */, char* /* argv */[]) {
    std::cout << "\n========================================" << std::endl;
    std::cout << "OGR PolishMap Driver - POLYLINE Layer Tests" << std::endl;
    std::cout << "Story 1.5: POLYLINE Layer Implementation" << std::endl;
    std::cout << "========================================\n" << std::endl;

    // Register all GDAL drivers
    GDALAllRegister();

    // Explicitly register PolishMap driver
    RegisterOGRPolishMap();

    // Run all tests
    TestSimplePolyline();              // 5.1
    TestMultiplePolylines();           // 5.2
    TestPolylineManyPoints();          // 5.3
    TestPolylineEOF();                 // 5.4
    TestPolylineResetReading();        // 5.5
    TestPolylineCoordinates();         // 5.6
    TestPolylineFields();              // 5.7
    TestPolylineLayerFiltering();      // 5.8
    TestPolylineFIDSequential();       // 5.9
    TestPolylineOnePointInvalid();     // 5.10
    TestPolylineNoParentheses();       // 5.11

    // Print summary
    std::cout << "\n========================================" << std::endl;
    std::cout << "Test Summary:" << std::endl;
    std::cout << "  Total:  " << g_nTests << std::endl;
    std::cout << "  Passed: " << g_nTestsPassed << " ✓" << std::endl;
    std::cout << "  Failed: " << g_nTestsFailed << " ✗" << std::endl;
    std::cout << "========================================\n" << std::endl;

    return (g_nTestsFailed > 0) ? 1 : 0;
}
