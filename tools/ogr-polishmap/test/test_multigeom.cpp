/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 4.2 - Multi-Geometry Decomposition
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - 1.1: Test_MultiPoint_5pts_Creates_5_POI_Sections (AC3)
 * - 1.2: Test_MultiLineString_2lines_Creates_2_POLYLINE_Sections (AC2)
 * - 1.3: Test_MultiPolygon_3poly_Creates_3_POLYGON_Sections (AC1)
 * - 1.4: Test_MultiPolygon_Attributes_Duplicated (AC1)
 * - 1.5: Test_MultiPolygon_Empty_Part_Handled (AC1)
 * - 1.6: Test_RoundTrip_Multi_Decomposition (AC5)
 * - 1.7: Test_ogr2ogr_MultiPolygon_Integration (AC4)
 * - 1.8: Test_SingleGeometry_Still_Works (regression)
 ****************************************************************************/

// Standard library includes (alphabetical)
#include <cassert>
#include <cstring>
#include <iostream>
#include <cmath>

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

// Test helper: Count occurrences of a substring
static int CountOccurrences(const std::string& osContent, const std::string& osSubstr) {
    int nCount = 0;
    size_t nPos = 0;
    while ((nPos = osContent.find(osSubstr, nPos)) != std::string::npos) {
        nCount++;
        nPos += osSubstr.length();
    }
    return nCount;
}

/************************************************************************/
/*     Test 1.1: MultiPoint_5pts_Creates_5_POI_Sections (AC3)            */
/*                                                                      */
/* Given a feature with MultiPoint geometry containing 5 points          */
/* When CreateFeature() is called on the POI layer                       */
/* Then 5 separate [POI] sections are written to the .mp file            */
/************************************************************************/

static bool Test_MultiPoint_5pts_Creates_5_POI_Sections() {
    std::cout << "  Test 1.1: MultiPoint_5pts_Creates_5_POI_Sections... ";

    CPLString osTempFile = GetTempFilePath("test_multipoint");
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

    // Get POI layer (index 0)
    OGRLayer* poPOILayer = poDS->GetLayer(0);
    if (poPOILayer == nullptr || strcmp(poPOILayer->GetName(), "POI") != 0) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with MultiPoint geometry containing 5 points
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "MultiPOI");

    // Create MultiPoint with 5 points
    OGRMultiPoint oMultiPoint;
    OGRPoint oPt1(2.3522, 48.8566);  // Paris
    OGRPoint oPt2(2.3533, 48.8577);
    OGRPoint oPt3(2.3544, 48.8588);
    OGRPoint oPt4(2.3555, 48.8599);
    OGRPoint oPt5(2.3566, 48.8610);
    oMultiPoint.addGeometry(&oPt1);
    oMultiPoint.addGeometry(&oPt2);
    oMultiPoint.addGeometry(&oPt3);
    oMultiPoint.addGeometry(&oPt4);
    oMultiPoint.addGeometry(&oPt5);
    poFeature->SetGeometry(&oMultiPoint);

    OGRErr eErr = poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and count [POI] sections
    std::string osContent = ReadFileContent(osTempFile.c_str());

    int nPOICount = CountOccurrences(osContent, "[POI]");
    if (nPOICount != 5) {
        std::cout << "FAILED (expected 5 [POI] sections, found " << nPOICount << ")" << std::endl;
        std::cout << "  File content:\n" << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify all 5 sections have the same Label
    int nLabelCount = CountOccurrences(osContent, "Label=MultiPOI");
    if (nLabelCount != 5) {
        std::cout << "FAILED (expected 5 Label=MultiPOI, found " << nLabelCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify all 5 sections have the same Type
    int nTypeCount = CountOccurrences(osContent, "Type=0x2C00");
    if (nTypeCount != 5) {
        std::cout << "FAILED (expected 5 Type=0x2C00, found " << nTypeCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.2: MultiLineString_2lines_Creates_2_POLYLINE_Sections (AC2)*/
/*                                                                      */
/* Given a feature with MultiLineString geometry containing 2 linestrings*/
/* When CreateFeature() is called on the POLYLINE layer                  */
/* Then 2 separate [POLYLINE] sections are written                       */
/************************************************************************/

static bool Test_MultiLineString_2lines_Creates_2_POLYLINE_Sections() {
    std::cout << "  Test 1.2: MultiLineString_2lines_Creates_2_POLYLINE_Sections... ";

    CPLString osTempFile = GetTempFilePath("test_multilinestring");
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

    // Get POLYLINE layer (index 1)
    OGRLayer* poPolylineLayer = poDS->GetLayer(1);
    if (poPolylineLayer == nullptr || strcmp(poPolylineLayer->GetName(), "POLYLINE") != 0) {
        std::cout << "FAILED (POLYLINE layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with MultiLineString geometry containing 2 linestrings
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Label", "MultiRoad");

    // Create MultiLineString with 2 LineStrings
    OGRMultiLineString oMultiLine;

    OGRLineString oLine1;
    oLine1.addPoint(2.3522, 48.8566);
    oLine1.addPoint(2.3533, 48.8577);
    oLine1.addPoint(2.3544, 48.8588);

    OGRLineString oLine2;
    oLine2.addPoint(2.4000, 48.9000);
    oLine2.addPoint(2.4100, 48.9100);

    oMultiLine.addGeometry(&oLine1);
    oMultiLine.addGeometry(&oLine2);
    poFeature->SetGeometry(&oMultiLine);

    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and count [POLYLINE] sections
    std::string osContent = ReadFileContent(osTempFile.c_str());

    int nPolylineCount = CountOccurrences(osContent, "[POLYLINE]");
    if (nPolylineCount != 2) {
        std::cout << "FAILED (expected 2 [POLYLINE] sections, found " << nPolylineCount << ")" << std::endl;
        std::cout << "  File content:\n" << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify all 2 sections have the same Label
    int nLabelCount = CountOccurrences(osContent, "Label=MultiRoad");
    if (nLabelCount != 2) {
        std::cout << "FAILED (expected 2 Label=MultiRoad, found " << nLabelCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.3: MultiPolygon_3poly_Creates_3_POLYGON_Sections (AC1)     */
/*                                                                      */
/* Given a feature with MultiPolygon geometry containing 3 polygons      */
/* When CreateFeature() is called on the POLYGON layer                   */
/* Then 3 separate [POLYGON] sections are written to the .mp file        */
/************************************************************************/

static bool Test_MultiPolygon_3poly_Creates_3_POLYGON_Sections() {
    std::cout << "  Test 1.3: MultiPolygon_3poly_Creates_3_POLYGON_Sections... ";

    CPLString osTempFile = GetTempFilePath("test_multipolygon");
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

    // Get POLYGON layer (index 2)
    OGRLayer* poPolygonLayer = poDS->GetLayer(2);
    if (poPolygonLayer == nullptr || strcmp(poPolygonLayer->GetName(), "POLYGON") != 0) {
        std::cout << "FAILED (POLYGON layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with MultiPolygon geometry containing 3 polygons
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "MultiForest");
    poFeature->SetField("EndLevel", 3);

    // Create MultiPolygon with 3 Polygons
    OGRMultiPolygon oMultiPoly;

    // Polygon 1 - Triangle
    OGRPolygon oPoly1;
    OGRLinearRing oRing1;
    oRing1.addPoint(2.3522, 48.8566);
    oRing1.addPoint(2.3533, 48.8577);
    oRing1.addPoint(2.3544, 48.8566);
    oRing1.addPoint(2.3522, 48.8566);  // Closing
    oPoly1.addRing(&oRing1);

    // Polygon 2 - Square
    OGRPolygon oPoly2;
    OGRLinearRing oRing2;
    oRing2.addPoint(2.4000, 48.9000);
    oRing2.addPoint(2.4100, 48.9000);
    oRing2.addPoint(2.4100, 48.9100);
    oRing2.addPoint(2.4000, 48.9100);
    oRing2.addPoint(2.4000, 48.9000);  // Closing
    oPoly2.addRing(&oRing2);

    // Polygon 3 - Another triangle
    OGRPolygon oPoly3;
    OGRLinearRing oRing3;
    oRing3.addPoint(2.5000, 49.0000);
    oRing3.addPoint(2.5100, 49.0100);
    oRing3.addPoint(2.5200, 49.0000);
    oRing3.addPoint(2.5000, 49.0000);  // Closing
    oPoly3.addRing(&oRing3);

    oMultiPoly.addGeometry(&oPoly1);
    oMultiPoly.addGeometry(&oPoly2);
    oMultiPoly.addGeometry(&oPoly3);
    poFeature->SetGeometry(&oMultiPoly);

    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and count [POLYGON] sections
    std::string osContent = ReadFileContent(osTempFile.c_str());

    int nPolygonCount = CountOccurrences(osContent, "[POLYGON]");
    if (nPolygonCount != 3) {
        std::cout << "FAILED (expected 3 [POLYGON] sections, found " << nPolygonCount << ")" << std::endl;
        std::cout << "  File content:\n" << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.4: MultiPolygon_Attributes_Duplicated (AC1)                */
/*                                                                      */
/* Verify all sections from decomposed MultiPolygon share same attributes*/
/************************************************************************/

static bool Test_MultiPolygon_Attributes_Duplicated() {
    std::cout << "  Test 1.4: MultiPolygon_Attributes_Duplicated... ";

    CPLString osTempFile = GetTempFilePath("test_multipolygon_attrs");
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

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "SharedLabel");
    poFeature->SetField("EndLevel", 5);

    // Create MultiPolygon with 3 Polygons
    OGRMultiPolygon oMultiPoly;

    for (int i = 0; i < 3; i++) {
        OGRPolygon oPoly;
        OGRLinearRing oRing;
        double dfOffset = i * 0.1;
        oRing.addPoint(2.3522 + dfOffset, 48.8566);
        oRing.addPoint(2.3533 + dfOffset, 48.8577);
        oRing.addPoint(2.3544 + dfOffset, 48.8566);
        oRing.addPoint(2.3522 + dfOffset, 48.8566);
        oPoly.addRing(&oRing);
        oMultiPoly.addGeometry(&oPoly);
    }
    poFeature->SetGeometry(&oMultiPoly);

    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and verify attributes are duplicated
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // All 3 sections should have Type=0x4C
    int nTypeCount = CountOccurrences(osContent, "Type=0x4C");
    if (nTypeCount != 3) {
        std::cout << "FAILED (expected 3 Type=0x4C, found " << nTypeCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // All 3 sections should have Label=SharedLabel
    int nLabelCount = CountOccurrences(osContent, "Label=SharedLabel");
    if (nLabelCount != 3) {
        std::cout << "FAILED (expected 3 Label=SharedLabel, found " << nLabelCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // All 3 sections should have EndLevel=5
    int nEndLevelCount = CountOccurrences(osContent, "EndLevel=5");
    if (nEndLevelCount != 3) {
        std::cout << "FAILED (expected 3 EndLevel=5, found " << nEndLevelCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.5: MultiPolygon_Empty_Part_Handled (AC1)                   */
/*                                                                      */
/* Verify empty parts in MultiPolygon are skipped gracefully             */
/************************************************************************/

static bool Test_MultiPolygon_Empty_Part_Handled() {
    std::cout << "  Test 1.5: MultiPolygon_Empty_Part_Handled... ";

    CPLString osTempFile = GetTempFilePath("test_multipolygon_empty");
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

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "PartialForest");

    // Create MultiPolygon: 1 valid polygon + 1 empty polygon + 1 valid polygon
    OGRMultiPolygon oMultiPoly;

    // Valid Polygon 1
    OGRPolygon oPoly1;
    OGRLinearRing oRing1;
    oRing1.addPoint(2.3522, 48.8566);
    oRing1.addPoint(2.3533, 48.8577);
    oRing1.addPoint(2.3544, 48.8566);
    oRing1.addPoint(2.3522, 48.8566);
    oPoly1.addRing(&oRing1);
    oMultiPoly.addGeometry(&oPoly1);

    // Empty Polygon (no exterior ring points - but has a ring object)
    OGRPolygon oPolyEmpty;
    OGRLinearRing oRingEmpty;
    // Don't add any points - empty ring
    oPolyEmpty.addRing(&oRingEmpty);
    oMultiPoly.addGeometry(&oPolyEmpty);

    // Valid Polygon 2
    OGRPolygon oPoly2;
    OGRLinearRing oRing2;
    oRing2.addPoint(2.4000, 48.9000);
    oRing2.addPoint(2.4100, 48.9100);
    oRing2.addPoint(2.4200, 48.9000);
    oRing2.addPoint(2.4000, 48.9000);
    oPoly2.addRing(&oRing2);
    oMultiPoly.addGeometry(&oPoly2);

    poFeature->SetGeometry(&oMultiPoly);

    CPLErrorReset();
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // Should succeed - empty parts are skipped with warning, not failure
    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and count [POLYGON] sections - should be 2 (empty skipped)
    std::string osContent = ReadFileContent(osTempFile.c_str());

    int nPolygonCount = CountOccurrences(osContent, "[POLYGON]");
    if (nPolygonCount != 2) {
        std::cout << "FAILED (expected 2 [POLYGON] sections, found " << nPolygonCount << ")" << std::endl;
        std::cout << "  File content:\n" << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.6: RoundTrip_Multi_Decomposition (AC5)                     */
/*                                                                      */
/* Given a MultiPolygon with 3 parts is decomposed                       */
/* When the .mp file is read back with Open()                            */
/* Then 3 separate POLYGON features are returned                         */
/************************************************************************/

static bool Test_RoundTrip_Multi_Decomposition() {
    std::cout << "  Test 1.6: RoundTrip_Multi_Decomposition... ";

    CPLString osTempFile = GetTempFilePath("test_multipolygon_roundtrip");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Write phase
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "RoundTripForest");

    // Create MultiPolygon with 3 Polygons
    OGRMultiPolygon oMultiPoly;

    for (int i = 0; i < 3; i++) {
        OGRPolygon oPoly;
        OGRLinearRing oRing;
        double dfOffset = i * 0.1;
        oRing.addPoint(2.3522 + dfOffset, 48.8566);
        oRing.addPoint(2.3533 + dfOffset, 48.8577);
        oRing.addPoint(2.3544 + dfOffset, 48.8566);
        oRing.addPoint(2.3522 + dfOffset, 48.8566);
        oPoly.addRing(&oRing);
        oMultiPoly.addGeometry(&oPoly);
    }
    poFeature->SetGeometry(&oMultiPoly);

    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read phase
    poDS = static_cast<GDALDataset*>(GDALOpenEx(osTempFile.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    poPolygonLayer = poDS->GetLayerByName("POLYGON");
    if (poPolygonLayer == nullptr) {
        std::cout << "FAILED (POLYGON layer not found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // AC5: Should have 3 separate features (not 1 MultiPolygon)
    GIntBig nFeatureCount = poPolygonLayer->GetFeatureCount();
    if (nFeatureCount != 3) {
        std::cout << "FAILED (expected 3 features, found " << nFeatureCount << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify each feature has the same attributes
    poPolygonLayer->ResetReading();
    for (int i = 0; i < 3; i++) {
        OGRFeature* poReadFeature = poPolygonLayer->GetNextFeature();
        if (poReadFeature == nullptr) {
            std::cout << "FAILED (could not read feature " << i << ")" << std::endl;
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }

        const char* pszType = poReadFeature->GetFieldAsString("Type");
        if (pszType == nullptr || strcmp(pszType, "0x4C") != 0) {
            std::cout << "FAILED (feature " << i << " has wrong Type: " << (pszType ? pszType : "null") << ")" << std::endl;
            OGRFeature::DestroyFeature(poReadFeature);
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }

        const char* pszLabel = poReadFeature->GetFieldAsString("Label");
        if (pszLabel == nullptr || strcmp(pszLabel, "RoundTripForest") != 0) {
            std::cout << "FAILED (feature " << i << " has wrong Label: " << (pszLabel ? pszLabel : "null") << ")" << std::endl;
            OGRFeature::DestroyFeature(poReadFeature);
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }

        // Verify geometry is simple Polygon (not MultiPolygon)
        OGRGeometry* poGeom = poReadFeature->GetGeometryRef();
        if (poGeom == nullptr || wkbFlatten(poGeom->getGeometryType()) != wkbPolygon) {
            std::cout << "FAILED (feature " << i << " geometry is not Polygon)" << std::endl;
            OGRFeature::DestroyFeature(poReadFeature);
            GDALClose(poDS);
            CleanupTempFile(osTempFile);
            return false;
        }

        // M2 Fix: AC5 - Verify FIDs are sequential (not preserving original FID)
        GIntBig nFID = poReadFeature->GetFID();
        static GIntBig nBaseFID = -1;
        if (i == 0) {
            nBaseFID = nFID;  // Store base FID from first feature
        } else {
            GIntBig nExpectedFID = nBaseFID + i;
            if (nFID != nExpectedFID) {
                std::cout << "FAILED (feature " << i << " has FID " << nFID
                          << ", expected sequential FID " << nExpectedFID << ")" << std::endl;
                OGRFeature::DestroyFeature(poReadFeature);
                GDALClose(poDS);
                CleanupTempFile(osTempFile);
                return false;
            }
        }

        OGRFeature::DestroyFeature(poReadFeature);
    }

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.7: SingleGeometry_Still_Works (regression)                 */
/*                                                                      */
/* Verify simple geometries still work after multi-geometry changes      */
/************************************************************************/

static bool Test_SingleGeometry_Still_Works() {
    std::cout << "  Test 1.7: SingleGeometry_Still_Works (regression)... ";

    CPLString osTempFile = GetTempFilePath("test_single_geom_regression");
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

    // Test simple Point
    OGRLayer* poPOILayer = poDS->GetLayer(0);
    OGRFeature* poFeature1 = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature1->SetField("Type", "0x2C00");
    poFeature1->SetField("Label", "SinglePOI");
    OGRPoint oPt(2.3522, 48.8566);
    poFeature1->SetGeometry(&oPt);
    OGRErr eErr1 = poPOILayer->CreateFeature(poFeature1);
    OGRFeature::DestroyFeature(poFeature1);

    if (eErr1 != OGRERR_NONE) {
        std::cout << "FAILED (single Point CreateFeature failed)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test simple LineString
    OGRLayer* poPolylineLayer = poDS->GetLayer(1);
    OGRFeature* poFeature2 = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature2->SetField("Type", "0x16");
    poFeature2->SetField("Label", "SingleRoad");
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeature2->SetGeometry(&oLine);
    OGRErr eErr2 = poPolylineLayer->CreateFeature(poFeature2);
    OGRFeature::DestroyFeature(poFeature2);

    if (eErr2 != OGRERR_NONE) {
        std::cout << "FAILED (single LineString CreateFeature failed)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Test simple Polygon
    OGRLayer* poPolygonLayer = poDS->GetLayer(2);
    OGRFeature* poFeature3 = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature3->SetField("Type", "0x4C");
    poFeature3->SetField("Label", "SingleForest");
    OGRPolygon oPoly;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);
    oPoly.addRing(&oRing);
    poFeature3->SetGeometry(&oPoly);
    OGRErr eErr3 = poPolygonLayer->CreateFeature(poFeature3);
    OGRFeature::DestroyFeature(poFeature3);

    if (eErr3 != OGRERR_NONE) {
        std::cout << "FAILED (single Polygon CreateFeature failed)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Verify file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    if (CountOccurrences(osContent, "[POI]") != 1) {
        std::cout << "FAILED (expected 1 [POI] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (CountOccurrences(osContent, "[POLYLINE]") != 1) {
        std::cout << "FAILED (expected 1 [POLYLINE] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (CountOccurrences(osContent, "[POLYGON]") != 1) {
        std::cout << "FAILED (expected 1 [POLYGON] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 1.8: GeometryCollection_Not_Supported                        */
/*                                                                      */
/* Verify GeometryCollection (mixed types) is properly rejected          */
/************************************************************************/

static bool Test_GeometryCollection_Not_Supported() {
    std::cout << "  Test 1.8: GeometryCollection_Not_Supported... ";

    CPLString osTempFile = GetTempFilePath("test_geomcollection");
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

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");

    // Create GeometryCollection with mixed types - this should fail
    OGRGeometryCollection oGeomColl;
    OGRPoint oPt(2.3522, 48.8566);
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    oGeomColl.addGeometry(&oPt);
    oGeomColl.addGeometry(&oLine);
    poFeature->SetGeometry(&oGeomColl);

    CPLErrorReset();
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // GeometryCollection (mixed types) should be rejected
    if (eErr == OGRERR_NONE) {
        std::cout << "FAILED (expected error for GeometryCollection, got success)" << std::endl;
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
    std::cout << "=== Story 4.2: Multi-Geometry Decomposition ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // Test 1.1: MultiPoint decomposition (AC3)
    if (Test_MultiPoint_5pts_Creates_5_POI_Sections()) nPassed++; else nFailed++;

    // Test 1.2: MultiLineString decomposition (AC2)
    if (Test_MultiLineString_2lines_Creates_2_POLYLINE_Sections()) nPassed++; else nFailed++;

    // Test 1.3: MultiPolygon decomposition (AC1)
    if (Test_MultiPolygon_3poly_Creates_3_POLYGON_Sections()) nPassed++; else nFailed++;

    // Test 1.4: Attributes duplicated on all parts (AC1)
    if (Test_MultiPolygon_Attributes_Duplicated()) nPassed++; else nFailed++;

    // Test 1.5: Empty parts handled gracefully (AC1)
    if (Test_MultiPolygon_Empty_Part_Handled()) nPassed++; else nFailed++;

    // Test 1.6: Round-trip verification (AC5)
    if (Test_RoundTrip_Multi_Decomposition()) nPassed++; else nFailed++;

    // Test 1.7: Single geometry regression test
    if (Test_SingleGeometry_Still_Works()) nPassed++; else nFailed++;

    // Test 1.8: GeometryCollection rejection
    if (Test_GeometryCollection_Not_Supported()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
