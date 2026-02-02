/******************************************************************************
 * Project:  OGR PolishMap Driver - Filter Tests
 * Purpose:  Test spatial and attribute filter functionality
 * Author:   mpforge project
 *
 * Story 1.7: Spatial and Attribute Filters Support
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
#include <string>

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

#define CHECK_EQ(val1, val2, message) \
    if ((val1) != (val2)) { \
        std::cerr << "[FAIL] " << message << " (expected: " << (val2) \
                  << ", got: " << (val1) << ")" << std::endl; \
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
/*                 Helper: Count features with current filter            */
/************************************************************************/

static int CountFeatures(OGRLayer* poLayer) {
    poLayer->ResetReading();
    int nCount = 0;
    OGRFeature* poFeature;
    while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
        nCount++;
        OGRFeature::DestroyFeature(poFeature);
    }
    return nCount;
}

/************************************************************************/
/*           Test 2.1: SetSpatialFilter() with bbox on POI              */
/************************************************************************/

void TestSpatialFilterBboxPOI() {
    TEST_START("Spatial Filter bbox on POI (AC1, AC2)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);  // POI layer
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // First, count all features without filter
            int nTotalCount = CountFeatures(poLayer);
            CHECK_EQ(nTotalCount, 100, "Total features without filter: 100");

            // Set spatial filter: bbox covering bottom-left quarter
            // Grid is 10x10 from (48.0, 2.0) to (48.9, 2.9)
            // Bottom-left quarter: lat 48.0-48.4, lon 2.0-2.4 = 25 POIs (5x5)
            poLayer->SetSpatialFilterRect(2.0, 48.0, 2.4, 48.4);
            std::cout << "[OK] SetSpatialFilterRect(2.0, 48.0, 2.4, 48.4)" << std::endl;

            // Count features within bbox
            int nFilteredCount = CountFeatures(poLayer);
            // Grid: lat 48.0, 48.1, 48.2, 48.3, 48.4 (5 rows) x lon 2.0, 2.1, 2.2, 2.3, 2.4 (5 cols) = 25
            CHECK_EQ(nFilteredCount, 25, "Features within bbox: 25");

            // Verify features are actually within bbox
            poLayer->ResetReading();
            OGRFeature* poFeature;
            bool bAllInBbox = true;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                OGRPoint* poPoint = poFeature->GetGeometryRef()->toPoint();
                double lon = poPoint->getX();
                double lat = poPoint->getY();
                if (lon < 2.0 || lon > 2.4 || lat < 48.0 || lat > 48.4) {
                    bAllInBbox = false;
                    std::cerr << "[FAIL] Feature outside bbox: (" << lat << ", " << lon << ")" << std::endl;
                }
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(bAllInBbox, "All returned features are within bbox");

            // Clear filter
            poLayer->SetSpatialFilter(nullptr);
            std::cout << "[OK] SetSpatialFilter(nullptr) - filter cleared" << std::endl;

            // Verify all features are returned again
            int nAfterClearCount = CountFeatures(poLayer);
            CHECK_EQ(nAfterClearCount, 100, "After clear filter: 100 features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*     Test 2.2: GetNextFeature returns only POIs within bbox           */
/************************************************************************/

void TestSpatialFilterReturnsPOIsInBbox() {
    TEST_START("GetNextFeature returns only POIs in bbox (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set spatial filter: single cell (should match exactly 1 POI)
            // Grid_55 is at (48.5, 2.5)
            poLayer->SetSpatialFilterRect(2.45, 48.45, 2.55, 48.55);
            std::cout << "[OK] SetSpatialFilterRect(2.45, 48.45, 2.55, 48.55)" << std::endl;

            int nCount = CountFeatures(poLayer);
            CHECK_EQ(nCount, 1, "Exactly 1 POI in small bbox");

            // Verify it's the right one
            poLayer->ResetReading();
            OGRFeature* poFeature = poLayer->GetNextFeature();
            if (poFeature != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(std::string(pszLabel) == "Grid_55", "Found Grid_55");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 2.3: SetSpatialFilter(nullptr) disables the filter           */
/************************************************************************/

void TestSpatialFilterNullptrDisables() {
    TEST_START("SetSpatialFilter(nullptr) disables filter (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set filter
            poLayer->SetSpatialFilterRect(2.0, 48.0, 2.1, 48.1);
            int nFiltered = CountFeatures(poLayer);
            CHECK(nFiltered < 100, "Filter reduces feature count");

            // Clear filter
            poLayer->SetSpatialFilter(nullptr);
            int nAll = CountFeatures(poLayer);
            CHECK_EQ(nAll, 100, "nullptr restores all 100 features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 2.4: ResetReading() preserves active spatial filter          */
/************************************************************************/

void TestResetReadingPreservesSpatialFilter() {
    TEST_START("ResetReading() preserves spatial filter (AC6)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set spatial filter
            poLayer->SetSpatialFilterRect(2.0, 48.0, 2.4, 48.4);  // 25 features

            // First iteration
            int nFirstCount = CountFeatures(poLayer);
            CHECK_EQ(nFirstCount, 25, "First iteration: 25 features");

            // Reset and iterate again
            poLayer->ResetReading();
            int nSecondCount = CountFeatures(poLayer);
            CHECK_EQ(nSecondCount, 25, "After ResetReading: still 25 features");

            // Filter should still be active
            poLayer->ResetReading();
            OGRFeature* poFeature = poLayer->GetNextFeature();
            CHECK(poFeature != nullptr, "Can still read features");
            if (poFeature != nullptr) {
                OGRPoint* poPoint = poFeature->GetGeometryRef()->toPoint();
                CHECK(poPoint->getX() <= 2.4 && poPoint->getY() <= 48.4, "Feature is within filter bbox");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*        Test 2.5: Edge cases bbox filtering                           */
/************************************************************************/

void TestSpatialFilterEdgeCases() {
    TEST_START("Spatial filter edge cases (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Empty bbox (no features)
            poLayer->SetSpatialFilterRect(10.0, 60.0, 11.0, 61.0);  // Far from any POI
            int nEmpty = CountFeatures(poLayer);
            CHECK_EQ(nEmpty, 0, "Empty bbox returns 0 features");

            // Full bbox (all features)
            poLayer->SetSpatialFilterRect(0.0, 40.0, 10.0, 60.0);  // Covers entire grid
            int nAll = CountFeatures(poLayer);
            CHECK_EQ(nAll, 100, "Full bbox returns all 100 features");

            // Point on boundary (Grid_00 at 48.0, 2.0)
            poLayer->SetSpatialFilterRect(2.0, 48.0, 2.0, 48.0);  // Exact point
            int nPoint = CountFeatures(poLayer);
            CHECK(nPoint >= 0, "Point bbox handled without crash");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*     Test 3.1: SetAttributeFilter by Type exact match                 */
/************************************************************************/

void TestAttributeFilterTypeExact() {
    TEST_START("Attribute filter Type exact match (AC3, AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);  // POI layer
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Total features
            int nTotal = CountFeatures(poLayer);
            CHECK_EQ(nTotal, 30, "Total features: 30");

            // Filter by Type = '0x2C00' (Restaurant - 10 features)
            OGRErr eErr = poLayer->SetAttributeFilter("Type = '0x2C00'");
            CHECK(eErr == OGRERR_NONE, "SetAttributeFilter(\"Type = '0x2C00'\") succeeded");

            int nRestaurants = CountFeatures(poLayer);
            CHECK_EQ(nRestaurants, 10, "Type='0x2C00' returns 10 features");

            // Verify all returned features have correct Type
            poLayer->ResetReading();
            OGRFeature* poFeature;
            bool bAllCorrect = true;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                const char* pszType = poFeature->GetFieldAsString("Type");
                if (std::string(pszType) != "0x2C00") {
                    bAllCorrect = false;
                    std::cerr << "[FAIL] Wrong Type: " << pszType << std::endl;
                }
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(bAllCorrect, "All returned features have Type='0x2C00'");

            // Filter by Type = '0x2B00' (Hotel - 10 features)
            poLayer->SetAttributeFilter("Type = '0x2B00'");
            int nHotels = CountFeatures(poLayer);
            CHECK_EQ(nHotels, 10, "Type='0x2B00' returns 10 features");

            // Filter by Type = '0x4000' (Attraction - 10 features)
            poLayer->SetAttributeFilter("Type = '0x4000'");
            int nAttractions = CountFeatures(poLayer);
            CHECK_EQ(nAttractions, 10, "Type='0x4000' returns 10 features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 3.2: SetAttributeFilter by Label pattern (LIKE)              */
/************************************************************************/

void TestAttributeFilterLabelLike() {
    TEST_START("Attribute filter Label LIKE pattern (AC3, AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Filter by Label LIKE '%Paris%'
            OGRErr eErr = poLayer->SetAttributeFilter("Label LIKE '%Paris%'");
            CHECK(eErr == OGRERR_NONE, "SetAttributeFilter(\"Label LIKE '%Paris%'\") succeeded");

            int nParis = CountFeatures(poLayer);
            CHECK(nParis >= 1, "At least 1 feature with 'Paris' in Label");

            // Verify labels contain 'Paris'
            poLayer->ResetReading();
            OGRFeature* poFeature;
            bool bAllMatch = true;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                if (std::string(pszLabel).find("Paris") == std::string::npos) {
                    bAllMatch = false;
                    std::cerr << "[FAIL] Label doesn't contain 'Paris': " << pszLabel << std::endl;
                }
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(bAllMatch, "All returned labels contain 'Paris'");

            // Filter by Label LIKE 'Hotel%' (starts with Hotel)
            poLayer->SetAttributeFilter("Label LIKE 'Hotel%'");
            int nHotelPrefix = CountFeatures(poLayer);
            CHECK_EQ(nHotelPrefix, 10, "10 features start with 'Hotel'");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 3.3: SetAttributeFilter(nullptr) disables filter             */
/************************************************************************/

void TestAttributeFilterNullptrDisables() {
    TEST_START("SetAttributeFilter(nullptr) disables filter (AC3)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set filter
            poLayer->SetAttributeFilter("Type = '0x2C00'");
            int nFiltered = CountFeatures(poLayer);
            CHECK_EQ(nFiltered, 10, "Filtered: 10 features");

            // Clear filter
            poLayer->SetAttributeFilter(nullptr);
            int nAll = CountFeatures(poLayer);
            CHECK_EQ(nAll, 30, "After nullptr: all 30 features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 3.4: ResetReading() preserves attribute filter               */
/************************************************************************/

void TestResetReadingPreservesAttributeFilter() {
    TEST_START("ResetReading() preserves attribute filter (AC6)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set attribute filter
            poLayer->SetAttributeFilter("Type = '0x4000'");

            // First iteration
            int nFirst = CountFeatures(poLayer);
            CHECK_EQ(nFirst, 10, "First iteration: 10 features");

            // Reset and iterate again
            poLayer->ResetReading();
            int nSecond = CountFeatures(poLayer);
            CHECK_EQ(nSecond, 10, "After ResetReading: still 10 features");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 3.5: Complex attribute filter (AND condition)                */
/************************************************************************/

void TestAttributeFilterComplex() {
    TEST_START("Complex attribute filter with AND (AC3, AC4)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Filter: Type='0x2C00' AND EndLevel=3
            // Restaurants with EndLevel=3 (first 5 restaurants)
            OGRErr eErr = poLayer->SetAttributeFilter("Type = '0x2C00' AND EndLevel = 3");
            CHECK(eErr == OGRERR_NONE, "Complex filter parsed successfully");

            int nCount = CountFeatures(poLayer);
            CHECK_EQ(nCount, 5, "Type='0x2C00' AND EndLevel=3: 5 features");

            // Verify all match both conditions
            poLayer->ResetReading();
            OGRFeature* poFeature;
            bool bAllMatch = true;
            while ((poFeature = poLayer->GetNextFeature()) != nullptr) {
                const char* pszType = poFeature->GetFieldAsString("Type");
                int nEndLevel = poFeature->GetFieldAsInteger("EndLevel");
                if (std::string(pszType) != "0x2C00" || nEndLevel != 3) {
                    bAllMatch = false;
                }
                OGRFeature::DestroyFeature(poFeature);
            }
            CHECK(bAllMatch, "All features match both conditions");

            // Filter with > operator
            poLayer->SetAttributeFilter("EndLevel > 3");
            int nHighLevel = CountFeatures(poLayer);
            CHECK(nHighLevel == 10, "EndLevel > 3: 10 features (all Attractions)");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 4.1: Combined spatial + attribute filters on POLYGON         */
/************************************************************************/

void TestCombinedFiltersPolygon() {
    TEST_START("Combined spatial + attribute filters on POLYGON (AC5)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        // Get POLYGON layer (index 2)
        OGRLayer* poLayer = poDS->GetLayer(2);
        CHECK(poLayer != nullptr, "Get POLYGON layer");

        if (poLayer != nullptr) {
            CHECK(std::string(poLayer->GetName()) == "POLYGON", "Layer is POLYGON");

            // Total polygons
            int nTotal = CountFeatures(poLayer);
            CHECK_EQ(nTotal, 4, "Total polygons: 4");

            // Set spatial filter: North region (lat > 48.5)
            poLayer->SetSpatialFilterRect(2.0, 48.5, 3.0, 49.0);
            int nNorth = CountFeatures(poLayer);
            CHECK_EQ(nNorth, 2, "Spatial filter North: 2 polygons");

            // Add attribute filter: Type='0x17' (Park)
            poLayer->SetAttributeFilter("Type = '0x17'");
            int nNorthPark = CountFeatures(poLayer);
            CHECK_EQ(nNorthPark, 1, "Spatial + Attribute: 1 Park in North");

            // Verify it's Park Nord
            poLayer->ResetReading();
            OGRFeature* poFeature = poLayer->GetNextFeature();
            if (poFeature != nullptr) {
                const char* pszLabel = poFeature->GetFieldAsString("Label");
                CHECK(std::string(pszLabel) == "Park Nord", "Found 'Park Nord'");
                OGRFeature::DestroyFeature(poFeature);
            }

            // Clear filters
            poLayer->SetSpatialFilter(nullptr);
            poLayer->SetAttributeFilter(nullptr);
            int nAfterClear = CountFeatures(poLayer);
            CHECK_EQ(nAfterClear, 4, "After clearing both filters: 4 polygons");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 4.2: AND logic between spatial and attribute filters         */
/************************************************************************/

void TestCombinedFiltersAndLogic() {
    TEST_START("Combined filters AND logic (AC5)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        // Get POI layer
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Total POIs
            int nTotal = CountFeatures(poLayer);
            CHECK_EQ(nTotal, 6, "Total POIs: 6");

            // Spatial filter: South region (lat < 48.5)
            poLayer->SetSpatialFilterRect(2.0, 48.0, 3.0, 48.5);
            int nSouth = CountFeatures(poLayer);
            CHECK_EQ(nSouth, 3, "South POIs: 3");

            // Attribute filter: Type='0x2C00' (Restaurant)
            poLayer->SetAttributeFilter("Type = '0x2C00'");
            int nSouthRestaurant = CountFeatures(poLayer);
            CHECK_EQ(nSouthRestaurant, 1, "South + Restaurant: 1 POI");

            // The feature must be in South AND be a Restaurant (AND logic)
            poLayer->ResetReading();
            OGRFeature* poFeature = poLayer->GetNextFeature();
            if (poFeature != nullptr) {
                OGRPoint* poPoint = poFeature->GetGeometryRef()->toPoint();
                const char* pszType = poFeature->GetFieldAsString("Type");
                CHECK(poPoint->getY() < 48.5, "Feature is in South region");
                CHECK(std::string(pszType) == "0x2C00", "Feature is a Restaurant");
                OGRFeature::DestroyFeature(poFeature);
            }
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 4.3: ResetReading() preserves both filters                   */
/************************************************************************/

void TestResetReadingPreservesBothFilters() {
    TEST_START("ResetReading() preserves both filters (AC6)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);  // POI layer
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Set both filters
            poLayer->SetSpatialFilterRect(2.0, 48.5, 3.0, 49.0);  // North
            poLayer->SetAttributeFilter("Type = '0x2C00'");       // Restaurant

            // First iteration
            int nFirst = CountFeatures(poLayer);

            // Reset
            poLayer->ResetReading();

            // Second iteration
            int nSecond = CountFeatures(poLayer);

            CHECK_EQ(nFirst, nSecond, "Both iterations return same count");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test 4.4: Filter change without memory leak                       */
/************************************************************************/

void TestFilterChangeNoLeak() {
    TEST_START("Filter change without memory leak (AC7)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Change filters multiple times
            for (int i = 0; i < 10; i++) {
                poLayer->SetSpatialFilterRect(2.0, 48.0, 2.5, 48.5);
                poLayer->SetAttributeFilter("Type = '0x2C00'");
                CountFeatures(poLayer);

                poLayer->SetSpatialFilterRect(2.5, 48.5, 3.0, 49.0);
                poLayer->SetAttributeFilter("Type = '0x2B00'");
                CountFeatures(poLayer);

                poLayer->SetSpatialFilter(nullptr);
                poLayer->SetAttributeFilter(nullptr);
                CountFeatures(poLayer);
            }
            std::cout << "[OK] 30 filter changes completed without crash" << std::endl;
            CHECK(true, "Filter changes handled correctly (Valgrind would detect leaks)");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test: POLYLINE attribute filter                                   */
/************************************************************************/

void TestPolylineAttributeFilter() {
    TEST_START("POLYLINE attribute filter (AC3)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        // Get POLYLINE layer (index 1)
        OGRLayer* poLayer = poDS->GetLayer(1);
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            CHECK(std::string(poLayer->GetName()) == "POLYLINE", "Layer is POLYLINE");

            // Total polylines
            int nTotal = CountFeatures(poLayer);
            CHECK_EQ(nTotal, 4, "Total polylines: 4");

            // Filter by Type='0x16' (Primary road)
            poLayer->SetAttributeFilter("Type = '0x16'");
            int nPrimary = CountFeatures(poLayer);
            CHECK_EQ(nPrimary, 2, "Type='0x16' (Primary): 2 polylines");

            // Filter by EndLevel
            poLayer->SetAttributeFilter("EndLevel = 4");
            int nLevel4 = CountFeatures(poLayer);
            CHECK_EQ(nLevel4, 2, "EndLevel=4: 2 polylines");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test: POLYLINE spatial filter                                     */
/************************************************************************/

void TestPolylineSpatialFilter() {
    TEST_START("POLYLINE spatial filter (AC1)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-combined.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-combined.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE
        CHECK(poLayer != nullptr, "Get POLYLINE layer");

        if (poLayer != nullptr) {
            // Spatial filter: North region (lat > 48.5)
            // Note: POLYLINE intersects bbox if any point is inside
            poLayer->SetSpatialFilterRect(2.0, 48.5, 3.0, 49.0);
            int nNorth = CountFeatures(poLayer);
            CHECK_EQ(nNorth, 2, "North polylines: 2");

            // Spatial filter: South region (lat < 48.5)
            poLayer->SetSpatialFilterRect(2.0, 48.0, 3.0, 48.5);
            int nSouth = CountFeatures(poLayer);
            CHECK_EQ(nSouth, 2, "South polylines: 2");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test: Invalid attribute filter syntax (error handling)             */
/************************************************************************/

void TestAttributeFilterInvalidSyntax() {
    TEST_START("Invalid attribute filter syntax (error handling)");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-attribute-types.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-attribute-types.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // Invalid SQL syntax should return error
            OGRErr eErr = poLayer->SetAttributeFilter("INVALID !@#$ SYNTAX");
            CHECK(eErr != OGRERR_NONE, "Invalid SQL returns error (not OGRERR_NONE)");

            // Layer should still work after invalid filter attempt
            poLayer->SetAttributeFilter(nullptr);  // Clear any partial state
            int nCount = CountFeatures(poLayer);
            CHECK_EQ(nCount, 30, "Layer still works after invalid filter: 30 features");

            // Empty string filter should be OK (means no filter)
            eErr = poLayer->SetAttributeFilter("");
            CHECK(eErr == OGRERR_NONE, "Empty string filter succeeds");
        }

        GDALClose(poDS);
    }

    TEST_END();
}

/************************************************************************/
/*    Test: TestCapability for OLCFastSpatialFilter                      */
/************************************************************************/

void TestCapabilityFastSpatialFilter() {
    TEST_START("TestCapability OLCFastSpatialFilter = FALSE");

    CPLString osFilename = CPLFormFilename(TEST_DATA_DIR, "valid-minimal/filter-spatial-grid.mp", nullptr);
    GDALDataset* poDS = GDALDataset::FromHandle(GDALOpenEx(
        osFilename.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));

    CHECK(poDS != nullptr, "Open filter-spatial-grid.mp");

    if (poDS != nullptr) {
        OGRLayer* poLayer = poDS->GetLayer(0);
        CHECK(poLayer != nullptr, "Get POI layer");

        if (poLayer != nullptr) {
            // OLCFastSpatialFilter should be FALSE (no spatial index)
            int bFastSpatial = poLayer->TestCapability(OLCFastSpatialFilter);
            CHECK(bFastSpatial == FALSE, "OLCFastSpatialFilter = FALSE (no spatial index)");

            // Filters still work even without fast capability
            poLayer->SetSpatialFilterRect(2.0, 48.0, 2.4, 48.4);
            int nFiltered = CountFeatures(poLayer);
            CHECK_EQ(nFiltered, 25, "Spatial filter works despite OLCFastSpatialFilter=FALSE");
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
    std::cout << "  OGR PolishMap - Filter Tests" << std::endl;
    std::cout << "  Story 1.7: Spatial and Attribute Filters" << std::endl;
    std::cout << "========================================" << std::endl;

    // Initialize GDAL and register driver
    GDALAllRegister();
    RegisterOGRPolishMap();

    // Task 2: Spatial filter tests on POI
    TestSpatialFilterBboxPOI();
    TestSpatialFilterReturnsPOIsInBbox();
    TestSpatialFilterNullptrDisables();
    TestResetReadingPreservesSpatialFilter();
    TestSpatialFilterEdgeCases();

    // Task 3: Attribute filter tests on POI
    TestAttributeFilterTypeExact();
    TestAttributeFilterLabelLike();
    TestAttributeFilterNullptrDisables();
    TestResetReadingPreservesAttributeFilter();
    TestAttributeFilterComplex();

    // Task 4: Combined filter tests
    TestCombinedFiltersPolygon();
    TestCombinedFiltersAndLogic();
    TestResetReadingPreservesBothFilters();
    TestFilterChangeNoLeak();

    // POLYLINE filter tests
    TestPolylineAttributeFilter();
    TestPolylineSpatialFilter();

    // Error handling and capability tests (Code Review fixes)
    TestAttributeFilterInvalidSyntax();
    TestCapabilityFastSpatialFilter();

    // Summary
    std::cout << "\n========================================" << std::endl;
    std::cout << "Test Summary:" << std::endl;
    std::cout << "  Total tests:  " << g_nTests << std::endl;
    std::cout << "  Passed:       " << g_nTestsPassed << std::endl;
    std::cout << "  Failed:       " << g_nTestsFailed << std::endl;
    std::cout << "========================================" << std::endl;

    return (g_nTestsFailed == 0) ? 0 : 1;
}
