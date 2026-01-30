/******************************************************************************
 * Project:  OGR PolishMap Driver - POLYGON Layer Tests
 * Purpose:  Test POLYGON feature reading functionality
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
#include <fstream>
#include <sstream>

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
/*               Test 5.1: Simple POLYGON (minimal valid)               */
/************************************************************************/

void TestSimplePolygon() {
    TEST_START("Simple POLYGON - 1 feature, 4 points triangle (AC1, AC2, AC3, AC8)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-simple.mp");

    if (poDS != nullptr) {
        // Get POLYGON layer (index 2 - POI is 0, POLYLINE is 1)
        OGRLayer* poLayer = poDS->GetLayer(2);
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            CHECK(std::string(poLayer->GetName()) == "POLYGON", "Layer name is 'POLYGON'");

            // Check layer definition (AC3)
            OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
            CHECK(poDefn != nullptr, "GetLayerDefn() returns non-null");
            CHECK(poDefn->GetGeomType() == wkbPolygon, "Geometry type is wkbPolygon (FR37)");
            CHECK(poDefn->GetFieldCount() == 5, "5 fields defined (FR38)");

            // Read first feature (AC1)
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                // AC8: Check FID
                CHECK(poFeature->GetFID() == 1, "FID is 1 (FR39)");

                // AC2: Check geometry
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");
                CHECK(poGeom->getGeometryType() == wkbPolygon, "Geometry is Polygon");

                OGRPolygon* poPolygon = poGeom->toPolygon();
                CHECK(poPolygon != nullptr, "Can cast to OGRPolygon");

                const OGRLinearRing* poRing = poPolygon->getExteriorRing();
                CHECK(poRing != nullptr, "Polygon has exterior ring");
                CHECK(poRing->getNumPoints() == 4, "Ring has 4 points (closed triangle)");

                // Check first and last coordinates are the same (closed ring)
                CHECK_NEAR(poRing->getX(0), poRing->getX(3), 0.0001, "Ring first X == last X (closed)");
                CHECK_NEAR(poRing->getY(0), poRing->getY(3), 0.0001, "Ring first Y == last Y (closed)");

                // Check coordinates (Data0: 48.8566,2.3522)
                CHECK_NEAR(poRing->getX(0), 2.3522, 0.0001, "Point 0 lon correct");
                CHECK_NEAR(poRing->getY(0), 48.8566, 0.0001, "Point 0 lat correct");
                CHECK_NEAR(poRing->getX(1), 2.3533, 0.0001, "Point 1 lon correct");
                CHECK_NEAR(poRing->getY(1), 48.8577, 0.0001, "Point 1 lat correct");

                // AC2/FR40: Check WGS84 spatial reference assigned
                const OGRSpatialReference* poSRS = poGeom->getSpatialReference();
                CHECK(poSRS != nullptr, "Geometry has spatial reference (FR40)");
                if (poSRS != nullptr) {
                    CHECK(poSRS->IsGeographic(), "SRS is geographic (WGS84)");
                }

                // AC1: Check fields
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x4C", "Type field correct");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Forest Area", "Label field correct");
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "EndLevel field correct");
                CHECK(std::string(poFeature->GetFieldAsString("Levels")) == "0-3", "Levels field correct");

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
/*               Test 5.2: Multiple POLYGON Features                    */
/************************************************************************/

void TestMultiplePolygons() {
    TEST_START("Multiple POLYGON - 5 features (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            int nCount = 0;
            OGRFeature* poFeature = nullptr;

            // Count features
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nCount++;

                // Verify geometry type
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, CPLSPrintf("Feature %d has geometry", nCount));
                CHECK(poGeom->getGeometryType() == wkbPolygon,
                      CPLSPrintf("Feature %d is Polygon", nCount));

                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nCount == 5, "5 POLYGON features found");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.3: POLYGON with Many Points                     */
/************************************************************************/

void TestPolygonManyPoints() {
    TEST_START("POLYGON with 20 points (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-many-points.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-many-points.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");

                OGRPolygon* poPolygon = poGeom->toPolygon();
                const OGRLinearRing* poRing = poPolygon->getExteriorRing();
                CHECK(poRing->getNumPoints() == 20, "Ring has 20 points (all from Data0)");

                // Validate first and last coordinates
                CHECK_NEAR(poRing->getX(0), 2.3400, 0.0001, "First point lon correct");
                CHECK_NEAR(poRing->getY(0), 48.8500, 0.0001, "First point lat correct");

                // Check ring is closed
                CHECK_NEAR(poRing->getX(0), poRing->getX(19), 0.0001, "Ring X closed");
                CHECK_NEAR(poRing->getY(0), poRing->getY(19), 0.0001, "Ring Y closed");

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

void TestPolygonEOF() {
    TEST_START("GetNextFeature() returns nullptr after last (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-simple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

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

void TestPolygonResetReading() {
    TEST_START("ResetReading() permits re-iteration (AC7)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

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
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Forest 1",
                      "First feature label correct after reset");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.6: Coordinate Correctness (ring closed)         */
/************************************************************************/

void TestPolygonCoordinates() {
    TEST_START("POLYGON coordinates and closed ring (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-simple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRPolygon* poPolygon = poGeom->toPolygon();
                const OGRLinearRing* poRing = poPolygon->getExteriorRing();

                // Verify all coordinates match test data exactly
                // Note: All coords come from Data0 line in correct MP format
                CHECK_NEAR(poRing->getX(0), 2.3522, 0.0001, "Point 0 lon = 2.3522");
                CHECK_NEAR(poRing->getY(0), 48.8566, 0.0001, "Point 0 lat = 48.8566");
                CHECK_NEAR(poRing->getX(1), 2.3533, 0.0001, "Point 1 lon = 2.3533");
                CHECK_NEAR(poRing->getY(1), 48.8577, 0.0001, "Point 1 lat = 48.8577");
                CHECK_NEAR(poRing->getX(2), 2.3522, 0.0001, "Point 2 lon = 2.3522");
                CHECK_NEAR(poRing->getY(2), 48.8588, 0.0001, "Point 2 lat = 48.8588");
                CHECK_NEAR(poRing->getX(3), 2.3522, 0.0001, "Point 3 lon = 2.3522 (ring closed)");
                CHECK_NEAR(poRing->getY(3), 48.8566, 0.0001, "Point 3 lat = 48.8566 (ring closed)");

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

void TestPolygonFields() {
    TEST_START("POLYGON Type, Label, Levels fields (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            // Test Feature 1: Forest 1
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 1 exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x4C", "Feature 1 Type");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Forest 1", "Feature 1 Label");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Test Feature 2: Water Body with EndLevel
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 2 exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x17", "Feature 2 Type");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Water Body", "Feature 2 Label");
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 5, "Feature 2 EndLevel");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Skip to Feature 4: Building Zone with Levels
            poFeature = poLayer->GetNextFeature();  // Skip Feature 3
            if (poFeature) OGRFeature::DestroyFeature(poFeature);
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 4 exists");
            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Building Zone", "Feature 4 Label");
                CHECK(std::string(poFeature->GetFieldAsString("Levels")) == "0-2", "Feature 4 Levels");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.8: Layer Filtering (POLYGON only)               */
/************************************************************************/

void TestPolygonLayerFiltering() {
    TEST_START("POLYGON layer ignores POI/POLYLINE sections (AC5)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-mixed-layers.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-mixed-layers.mp");

    if (poDS != nullptr) {
        // Check POI layer count
        OGRLayer* poPOILayer = poDS->GetLayer(0);
        CHECK(poPOILayer != nullptr, "Get POI layer");
        if (poPOILayer) {
            int nPOICount = 0;
            OGRFeature* poFeature = nullptr;
            while ((poFeature = poPOILayer->GetNextFeature()) != nullptr) {
                nPOICount++;
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(nPOICount == 3, "POI layer has 3 features");
        }

        // Check POLYLINE layer count
        OGRLayer* poPolylineLayer = poDS->GetLayer(1);
        CHECK(poPolylineLayer != nullptr, "Get POLYLINE layer");
        if (poPolylineLayer) {
            int nPolylineCount = 0;
            OGRFeature* poFeature = nullptr;
            while ((poFeature = poPolylineLayer->GetNextFeature()) != nullptr) {
                nPolylineCount++;
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(nPolylineCount == 2, "POLYLINE layer has 2 features");
        }

        // Check POLYGON layer count
        OGRLayer* poPolygonLayer = poDS->GetLayer(2);
        CHECK(poPolygonLayer != nullptr, "Get POLYGON layer");

        if (poPolygonLayer != nullptr) {
            int nPolygonCount = 0;
            OGRFeature* poFeature = nullptr;

            // Expected POLYGON labels (not POI: Town Hall, Restaurant, Bridge; not POLYLINE: Main Street, River)
            const char* apszExpectedLabels[] = {"City Park", "Lake"};

            while ((poFeature = poPolygonLayer->GetNextFeature()) != nullptr) {
                // Verify all features are POLYGON type
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");
                CHECK(poGeom->getGeometryType() == wkbPolygon, "Geometry is Polygon (not Point/LineString)");

                // Verify label matches expected POLYGON (not POI/POLYLINE)
                if (nPolygonCount < 2) {
                    std::string osLabel = poFeature->GetFieldAsString("Label");
                    CHECK(osLabel == apszExpectedLabels[nPolygonCount],
                          CPLSPrintf("Feature %d label is '%s' (POLYGON, not POI/POLYLINE)",
                                     nPolygonCount + 1, apszExpectedLabels[nPolygonCount]));
                }

                nPolygonCount++;
                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nPolygonCount == 2, "2 POLYGON features (POI/POLYLINE filtered out)");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.9: FID Sequential (1, 2, 3...)                  */
/************************************************************************/

void TestPolygonFIDSequential() {
    TEST_START("FID sequential starting at 1 (AC8, FR39)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

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
/*               Test 5.10: POLYGON open ring auto-closure              */
/************************************************************************/

void TestPolygonOpenRingAutoClosure() {
    TEST_START("POLYGON open ring auto-closed with CPLDebug (AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "error-recovery/polygon-open-ring.mp", nullptr);

    // Create temporary log file to capture CPLDebug output
    CPLString osLogFile = CPLFormFilename(CPLGetConfigOption("TMPDIR", "/tmp"),
                                          "test_polygon_autoclosure.log", nullptr);

    // Enable CPLDebug output for OGR_POLISHMAP category and redirect to log file
    CPLSetConfigOption("CPL_DEBUG", "OGR_POLISHMAP");
    CPLSetConfigOption("CPL_LOG", osLogFile.c_str());

    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-open-ring.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature created despite open ring");

            if (poFeature != nullptr) {
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRPolygon* poPolygon = poGeom->toPolygon();
                const OGRLinearRing* poRing = poPolygon->getExteriorRing();

                // Original data has 3 points, but should be auto-closed to 4
                int nPoints = poRing->getNumPoints();
                CHECK(nPoints == 4, CPLSPrintf("Ring has 4 points after auto-close (got %d)", nPoints));

                // Verify ring is now closed (first point == last point)
                if (nPoints >= 2) {
                    CHECK_NEAR(poRing->getX(0), poRing->getX(nPoints - 1), 0.0001,
                              "Ring X is closed after auto-close");
                    CHECK_NEAR(poRing->getY(0), poRing->getY(nPoints - 1), 0.0001,
                              "Ring Y is closed after auto-close");
                }

                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    // Reset config options to flush log
    CPLSetConfigOption("CPL_DEBUG", nullptr);
    CPLSetConfigOption("CPL_LOG", nullptr);

    // AC4 verification: Read log file and verify CPLDebug message was emitted
    bool bFoundAutoCloseMsg = false;
    std::ifstream logFile(osLogFile.c_str());
    if (logFile.is_open()) {
        std::stringstream buffer;
        buffer << logFile.rdbuf();
        std::string logContent = buffer.str();
        bFoundAutoCloseMsg = (logContent.find("Auto-closing POLYGON ring") != std::string::npos);
        logFile.close();
    }
    CHECK(bFoundAutoCloseMsg, "CPLDebug 'Auto-closing POLYGON ring' message found in log");

    // Clean up log file
    VSIUnlink(osLogFile.c_str());

    TEST_END();
}

/************************************************************************/
/*               Test 5.11: POLYGON with < 3 points (invalid)           */
/************************************************************************/

// Global variable to capture error messages
static CPLString g_osLastErrorMsg;
static int g_nLastErrorType = CE_None;

static void CPLTestErrorHandler(CPLErr eErrClass, CPLErrorNum /* nError */, const char* pszMsg) {
    g_nLastErrorType = eErrClass;
    g_osLastErrorMsg = pszMsg;
}

void TestPolygonTwoPointsInvalid() {
    TEST_START("POLYGON with 2 points skipped with warning");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "error-recovery/polygon-two-points.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-two-points.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            // Install error handler to capture CE_Warning
            g_osLastErrorMsg.clear();
            g_nLastErrorType = CE_None;
            CPLPushErrorHandler(CPLTestErrorHandler);

            // First feature should be skipped (2 points), second should be valid
            OGRFeature* poFeature = poLayer->GetNextFeature();

            // Verify CE_Warning was logged
            CPLPopErrorHandler();
            CHECK(g_nLastErrorType == CE_Warning, "CPLError(CE_Warning) was logged");
            CHECK(g_osLastErrorMsg.find("less than 3 points") != std::string::npos,
                  "Warning message mentions 'less than 3 points'");

            CHECK(poFeature != nullptr, "Valid feature found (invalid skipped)");

            if (poFeature) {
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Valid Polygon After Invalid",
                      "Valid feature is the correct one");
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                OGRPolygon* poPolygon = poGeom->toPolygon();
                const OGRLinearRing* poRing = poPolygon->getExteriorRing();
                CHECK(poRing->getNumPoints() == 4, "Valid feature has 4 points");
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
/*               Test 5.12: API Integration Test (AC6)                  */
/*                                                                      */
/* Note: This test verifies the underlying GDAL API behavior that       */
/* ogrinfo -al relies on. A true CLI test would require executing       */
/* ogrinfo as a subprocess and parsing its output. The API test         */
/* validates the same functionality at the driver level.                */
/************************************************************************/

void TestOgrinfoIntegration() {
    TEST_START("API integration: 3 layers with correct geometry types (AC6)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-mixed-layers.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-mixed-layers.mp");

    if (poDS != nullptr) {
        // Check we have 3 layers
        CHECK(poDS->GetLayerCount() == 3, "Dataset has 3 layers (FR43)");

        // Get all layers
        OGRLayer* poPOILayer = poDS->GetLayer(0);
        OGRLayer* poPolylineLayer = poDS->GetLayer(1);
        OGRLayer* poPolygonLayer = poDS->GetLayer(2);

        CHECK(poPOILayer != nullptr && std::string(poPOILayer->GetName()) == "POI",
              "Layer 0 is POI");
        CHECK(poPolylineLayer != nullptr && std::string(poPolylineLayer->GetName()) == "POLYLINE",
              "Layer 1 is POLYLINE");
        CHECK(poPolygonLayer != nullptr && std::string(poPolygonLayer->GetName()) == "POLYGON",
              "Layer 2 is POLYGON");

        // Verify geometry types
        if (poPOILayer) {
            CHECK(poPOILayer->GetLayerDefn()->GetGeomType() == wkbPoint, "POI geometry is Point");
        }
        if (poPolylineLayer) {
            CHECK(poPolylineLayer->GetLayerDefn()->GetGeomType() == wkbLineString, "POLYLINE geometry is LineString");
        }
        if (poPolygonLayer) {
            CHECK(poPolygonLayer->GetLayerDefn()->GetGeomType() == wkbPolygon, "POLYGON geometry is Polygon");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.13: POLYGON CP1252 Encoding (AC1)               */
/************************************************************************/

void TestPolygonCP1252Encoding() {
    TEST_START("POLYGON CP1252 encoding conversion to UTF-8 (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-cp1252.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-cp1252.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            // Feature 1: "Forêt Réservée" (CP1252: For\xEAt R\xE9serv\xE9e)
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 1 exists");
            if (poFeature) {
                std::string osLabel = poFeature->GetFieldAsString("Label");
                // UTF-8: ê = 0xC3 0xAA, é = 0xC3 0xA9
                CHECK(osLabel == "Forêt Réservée",
                      CPLSPrintf("Feature 1 Label CP1252->UTF-8: '%s'", osLabel.c_str()));
                OGRFeature::DestroyFeature(poFeature);
            }

            // Feature 2: "Zürich Grünfläche" (CP1252: Z\xFCrich Gr\xFCnfl\xE4che)
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature 2 exists");
            if (poFeature) {
                std::string osLabel = poFeature->GetFieldAsString("Label");
                // UTF-8: ü = 0xC3 0xBC, ä = 0xC3 0xA4
                CHECK(osLabel == "Zürich Grünfläche",
                      CPLSPrintf("Feature 2 Label CP1252->UTF-8: '%s'", osLabel.c_str()));
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*               Test 5.14: POLYGON without Label (AC1)                 */
/************************************************************************/

void TestPolygonNoLabel() {
    TEST_START("POLYGON without Label field (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/polygon-no-label.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open polygon-no-label.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON layer
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Feature created without Label");

            if (poFeature != nullptr) {
                // Verify geometry is valid
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");
                CHECK(poGeom->getGeometryType() == wkbPolygon, "Geometry is Polygon");

                // Verify Label field is empty string (not null crash)
                std::string osLabel = poFeature->GetFieldAsString("Label");
                CHECK(osLabel.empty(), "Label field is empty string for POLYGON without Label");

                // Verify Type field is populated
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x4C", "Type field correct");

                // Verify EndLevel field is populated
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "EndLevel field correct");

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
    std::cout << "OGR PolishMap Driver - POLYGON Layer Tests" << std::endl;
    std::cout << "Story 1.6: POLYGON Layer Implementation" << std::endl;
    std::cout << "========================================\n" << std::endl;

    // Register all GDAL drivers
    GDALAllRegister();

    // Explicitly register PolishMap driver
    RegisterOGRPolishMap();

    // Run all tests
    TestSimplePolygon();              // 5.1
    TestMultiplePolygons();           // 5.2
    TestPolygonManyPoints();          // 5.3
    TestPolygonEOF();                 // 5.4
    TestPolygonResetReading();        // 5.5
    TestPolygonCoordinates();         // 5.6
    TestPolygonFields();              // 5.7
    TestPolygonLayerFiltering();      // 5.8
    TestPolygonFIDSequential();       // 5.9
    TestPolygonOpenRingAutoClosure(); // 5.10
    TestPolygonTwoPointsInvalid();    // 5.11
    TestOgrinfoIntegration();         // 5.12
    TestPolygonCP1252Encoding();      // 5.13
    TestPolygonNoLabel();             // 5.14

    // Print summary
    std::cout << "\n========================================" << std::endl;
    std::cout << "Test Summary:" << std::endl;
    std::cout << "  Total:  " << g_nTests << std::endl;
    std::cout << "  Passed: " << g_nTestsPassed << " ✓" << std::endl;
    std::cout << "  Failed: " << g_nTestsFailed << " ✗" << std::endl;
    std::cout << "========================================\n" << std::endl;

    return (g_nTestsFailed > 0) ? 1 : 0;
}
