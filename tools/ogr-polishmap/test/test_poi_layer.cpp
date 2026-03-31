/******************************************************************************
 * Project:  OGR PolishMap Driver - POI Layer Tests
 * Purpose:  Test POI feature reading functionality
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
/*                     Test 7.1: Single POI Feature                     */
/************************************************************************/

void TestSinglePOIFeature() {
    TEST_START("Single POI Feature (AC1, AC2, AC3, AC8)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-simple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-simple.mp");

    if (poDS != nullptr) {
        // Get POI layer (index 0)
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            CHECK(std::string(poLayer->GetName()) == "POI", "Layer name is 'POI'");

            // Check layer definition (AC3)
            OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
            CHECK(poDefn != nullptr, "GetLayerDefn() returns non-null");

            if (poDefn != nullptr) {
                CHECK(poDefn->GetGeomType() == wkbPoint, "Geometry type is wkbPoint");
                CHECK(poDefn->GetFieldCount() == 16, "Field count is 16");

                // Check key field definitions by name
                int nTypeIdx = poDefn->GetFieldIndex("Type");
                CHECK(nTypeIdx >= 0, "Field 'Type' exists");
                if (nTypeIdx >= 0)
                    CHECK(poDefn->GetFieldDefn(nTypeIdx)->GetType() == OFTString, "Type field is OFTString");

                int nLabelIdx = poDefn->GetFieldIndex("Label");
                CHECK(nLabelIdx >= 0, "Field 'Label' exists");
                if (nLabelIdx >= 0)
                    CHECK(poDefn->GetFieldDefn(nLabelIdx)->GetType() == OFTString, "Label field is OFTString");

                int nData0Idx = poDefn->GetFieldIndex("Data0");
                CHECK(nData0Idx >= 0, "Field 'Data0' exists");
                if (nData0Idx >= 0)
                    CHECK(poDefn->GetFieldDefn(nData0Idx)->GetType() == OFTInteger, "Data0 field is OFTInteger");
            }

            // Read first feature (AC1)
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "GetNextFeature() returns feature");

            if (poFeature != nullptr) {
                // Check FID (AC8)
                CHECK(poFeature->GetFID() == 1, "First FID is 1");

                // Check geometry type (AC1)
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr, "Feature has geometry");

                if (poGeom != nullptr) {
                    CHECK(poGeom->getGeometryType() == wkbPoint, "Geometry is wkbPoint");

                    // Check geometry coordinates (AC2)
                    OGRPoint* poPoint = poGeom->toPoint();
                    CHECK_NEAR(poPoint->getX(), 2.3522, 0.0001, "Longitude is 2.3522");
                    CHECK_NEAR(poPoint->getY(), 48.8566, 0.0001, "Latitude is 48.8566");

                    // Check spatial reference (AC2)
                    const OGRSpatialReference* poSRS = poGeom->getSpatialReference();
                    CHECK(poSRS != nullptr, "Geometry has spatial reference");
                    if (poSRS != nullptr) {
                        CHECK(poSRS->IsGeographic(), "SRS is geographic");
                    }
                }

                // Check attributes (AC1)
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x2C00", "Type is '0x2C00'");
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Restaurant Le Paris", "Label is 'Restaurant Le Paris'");
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "EndLevel is 3");

                OGRFeature::DestroyFeature(poFeature);
            }

            // Check no more features (AC5)
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "nullptr after last feature");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*                  Test 7.2: Multiple POI Features                     */
/************************************************************************/

void TestMultiplePOIFeatures() {
    TEST_START("Multiple POI Features (AC5, AC8)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Iterate over all features
            int nCount = 0;
            OGRFeature* poFeature;

            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nCount++;

                // Check FID is sequential (AC8)
                CHECK(poFeature->GetFID() == nCount, "FID is sequential");

                // Check geometry exists
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr && poGeom->getGeometryType() == wkbPoint, "Geometry is wkbPoint");

                // Check required fields
                CHECK(poFeature->IsFieldSetAndNotNull(poFeature->GetFieldIndex("Type")), "Type field is set");
                CHECK(poFeature->IsFieldSetAndNotNull(poFeature->GetFieldIndex("Label")), "Label field is set");

                OGRFeature::DestroyFeature(poFeature);
            }

            // Verify we read exactly 10 features (AC5)
            CHECK(nCount == 10, "Read 10 POI features");

            // Verify nullptr after last feature (AC5)
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "nullptr after reading all features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*                   Test 7.4: ResetReading()                           */
/************************************************************************/

void TestResetReading() {
    TEST_START("ResetReading() (AC6, FR28)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // First iteration - read all features
            int nFirstCount = 0;
            OGRFeature* poFeature;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nFirstCount++;
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(nFirstCount == 10, "First iteration: 10 features");

            // Verify we're at end
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "At end after first iteration");

            // Reset reading (AC6, FR28)
            poLayer->ResetReading();
            std::cout << "[OK] Called ResetReading()" << std::endl;

            // Second iteration - should read same features again
            int nSecondCount = 0;
            GIntBig nFirstFID = 0;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nSecondCount++;
                if (nSecondCount == 1) {
                    nFirstFID = poFeature->GetFID();
                }
                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nSecondCount == 10, "Second iteration: 10 features");
            CHECK(nFirstFID == 1, "First FID after reset is 1");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*                Test 7.5: Correct Coordinates                         */
/************************************************************************/

void TestCorrectCoordinates() {
    TEST_START("Correct Coordinates (AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-multiple.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-multiple.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Read first feature: Data0=(48.8566,2.3522)
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get first feature");

            if (poFeature != nullptr) {
                OGRPoint* poPoint = poFeature->GetGeometryRef()->toPoint();
                CHECK_NEAR(poPoint->getX(), 2.3522, 0.0001, "First feature: X (lon) = 2.3522");
                CHECK_NEAR(poPoint->getY(), 48.8566, 0.0001, "First feature: Y (lat) = 48.8566");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Read second feature: Data0=(48.8577,2.3533)
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get second feature");

            if (poFeature != nullptr) {
                OGRPoint* poPoint = poFeature->GetGeometryRef()->toPoint();
                CHECK_NEAR(poPoint->getX(), 2.3533, 0.0001, "Second feature: X (lon) = 2.3533");
                CHECK_NEAR(poPoint->getY(), 48.8577, 0.0001, "Second feature: Y (lat) = 48.8577");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*            Test 7.7: CP1252 to UTF-8 Encoding                        */
/************************************************************************/

void TestEncodingConversion() {
    TEST_START("CP1252 to UTF-8 Encoding (AC7)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-with-encoding.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-with-encoding.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Test "Café Français" with accented characters
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get first feature");

            if (poFeature != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(std::string(pszLabel) == "Café Français", "CP1252 é to UTF-8");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Test "Hôtel Español"
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get second feature");

            if (poFeature != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(std::string(pszLabel) == "Hôtel Español", "CP1252 ô, ñ to UTF-8");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Test "Über Shop"
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get third feature");

            if (poFeature != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(std::string(pszLabel) == "Über Shop", "CP1252 Ü to UTF-8");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*          Test 7.9: Ignore POLYLINE/POLYGON Sections                  */
/************************************************************************/

void TestIgnoreMixedSections() {
    TEST_START("Ignore POLYLINE/POLYGON Sections");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-mixed-sections.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-mixed-sections.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);  // POI layer
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Count POI features (should be 3, ignoring POLYLINE and POLYGON)
            int nPOICount = 0;
            OGRFeature* poFeature;

            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                nPOICount++;

                // All features should be points
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr && poGeom->getGeometryType() == wkbPoint,
                      "POI layer returns only wkbPoint");

                OGRFeature::DestroyFeature(poFeature);
            }

            CHECK(nPOICount == 3, "Read only 3 POI features (ignore POLYLINE/POLYGON)");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*              Test 7.10: POI Without Label                            */
/************************************************************************/

void TestPOIWithoutLabel() {
    TEST_START("POI Without Label (M3)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/poi-no-label.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open poi-no-label.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // First POI: no Label field
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get first feature (no label)");

            if (poFeature != nullptr) {
                // Check Type field is set
                CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x2C00", "Type is '0x2C00'");

                // Check Label field is empty string (not null, just empty)
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(pszLabel != nullptr && std::string(pszLabel).empty(), "Label is empty string");

                // Check EndLevel is set
                CHECK(poFeature->GetFieldAsInteger("EndLevel") == 3, "EndLevel is 3");

                // Check geometry exists and is valid
                OGRGeometry* poGeom = poFeature->GetGeometryRef();
                CHECK(poGeom != nullptr && poGeom->getGeometryType() == wkbPoint, "Geometry is wkbPoint");

                if (poGeom != nullptr) {
                    OGRPoint* poPoint = poGeom->toPoint();
                    CHECK_NEAR(poPoint->getX(), 2.3522, 0.0001, "Longitude is 2.3522");
                    CHECK_NEAR(poPoint->getY(), 48.8566, 0.0001, "Latitude is 48.8566");
                }

                OGRFeature::DestroyFeature(poFeature);
            }

            // Second POI: has Label
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Get second feature (with label)");

            if (poFeature != nullptr) {
                CHECK(std::string(poFeature->GetFieldAsString("Label")) == "With Label", "Label is 'With Label'");
                OGRFeature::DestroyFeature(poFeature);
            }

            // No more features
            poFeature = poLayer->GetNextFeature();
            CHECK(poFeature == nullptr, "nullptr after last feature");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*                              Main                                    */
/************************************************************************/

int main() {
    std::cout << "========================================" << std::endl;
    std::cout << "  OGR PolishMap - POI Layer Tests" << std::endl;
    std::cout << "  Story 1.4: Feature Reading" << std::endl;
    std::cout << "========================================" << std::endl;

    // Initialize GDAL and register driver
    GDALAllRegister();
    RegisterOGRPolishMap();

    // Run tests
    TestSinglePOIFeature();
    TestMultiplePOIFeatures();
    TestResetReading();
    TestCorrectCoordinates();
    TestEncodingConversion();
    TestIgnoreMixedSections();
    TestPOIWithoutLabel();

    // Summary
    std::cout << "\n========================================" << std::endl;
    std::cout << "Test Summary:" << std::endl;
    std::cout << "  Total tests:  " << g_nTests << std::endl;
    std::cout << "  Passed:       " << g_nTestsPassed << std::endl;
    std::cout << "  Failed:       " << g_nTestsFailed << std::endl;
    std::cout << "========================================" << std::endl;

    return (g_nTestsFailed == 0) ? 0 : 1;
}
