/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.5 - POLYGON Feature Writing (CreateFeature for Polygons)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - 4.1: CreateFeature() with valid Polygon returns OGRERR_NONE (AC1)
 * - 4.2: File contains [POLYGON] section with all fields (AC4)
 * - 4.3: Coordinates on ONE line Data0 with "(lat,lon),(lat,lon)" format (AC2)
 * - 4.4: Coordinates with 6 decimals (AC2)
 * - 4.5: UTF-8 to CP1252 conversion ("Foret de Fontainebleau") (AC7)
 * - 4.6: Optional Label omitted when absent
 * - 4.7: Round-trip validation (AC5)
 * - 4.8: CreateFeature(no geometry) -> OGRERR_FAILURE + CPLError
 * - 4.9: CreateFeature(Polygon with 2 points) -> OGRERR_FAILURE (invalid)
 * - 4.10: Auto-close open ring: 3 points -> 4 points written + CPLDebug (AC3)
 * - 4.11: TestCapability(OLCSequentialWrite) = TRUE for POLYGON (AC8)
 * - 4.12: Mixed file: POI + POLYLINE + POLYGON in same file (AC6)
 * - 4.13: Wrong geometry type (Point on POLYGON layer) -> OGRERR_FAILURE
 * - 4.14: Empty Polygon (0 points in exterior ring) -> OGRERR_FAILURE
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

/************************************************************************/
/*     Test 4.1: CreateFeature_ValidPolygon_Returns_OGRERR_NONE (AC1)     */
/*                                                                      */
/* CreateFeature() with valid Polygon geometry returns OGRERR_NONE       */
/************************************************************************/

static bool Test_4_1_CreateFeature_ValidPolygon_Returns_OGRERR_NONE() {
    std::cout << "  Test 4.1: CreateFeature_ValidPolygon_Returns_OGRERR_NONE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_create");
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

    // Get POLYGON layer (index 2)
    OGRLayer* poPolygonLayer = poDS->GetLayer(2);
    if (poPolygonLayer == nullptr) {
        std::cout << "FAILED (GetLayer(2) returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify it's the POLYGON layer
    if (strcmp(poPolygonLayer->GetName(), "POLYGON") != 0) {
        std::cout << "FAILED (layer 2 is not POLYGON)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with Polygon geometry (4 points - closed ring)
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    if (poFeature == nullptr) {
        std::cout << "FAILED (CreateFeature returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set fields (AC1: Type="0x4C", Label="Forest", Data0=2)
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "Forest");
    poFeature->SetField("EndLevel", 2);

    // Create Polygon geometry with closed ring (X=lon, Y=lat)
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // lon, lat - Point A
    oRing.addPoint(2.3533, 48.8577);  // lon, lat - Point B
    oRing.addPoint(2.3544, 48.8566);  // lon, lat - Point C
    oRing.addPoint(2.3522, 48.8566);  // lon, lat - Point A (closing)
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    // Call CreateFeature
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);

    // Cleanup feature
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature() returned " << eErr << ", expected OGRERR_NONE)" << std::endl;
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
/*     Test 4.2: File_Contains_POLYGON_Section_With_All_Fields (AC4)      */
/*                                                                      */
/* File contains [POLYGON] section with Type, Label, Data0, EndLevel     */
/************************************************************************/

static bool Test_4_2_File_Contains_POLYGON_Section() {
    std::cout << "  Test 4.2: File_Contains_POLYGON_Section_With_All_Fields... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_section");
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

    // Create a feature
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "Test Forest");
    poFeature->SetField("EndLevel", 2);

    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);  // Closing point
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // Close to flush
    GDALClose(poDS);

    // Read file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POLYGON] section
    if (osContent.find("[POLYGON]") == std::string::npos) {
        std::cout << "FAILED (missing [POLYGON] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify [END] marker after POLYGON section
    size_t nEndCount = 0;
    size_t nPos = 0;
    while ((nPos = osContent.find("[END]", nPos)) != std::string::npos) {
        nEndCount++;
        nPos += 5;
    }
    if (nEndCount < 2) {
        std::cout << "FAILED (expected at least 2 [END] markers, found " << nEndCount << ")" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Type field
    if (osContent.find("Type=0x4C") == std::string::npos) {
        std::cout << "FAILED (missing Type=0x4C)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    if (osContent.find("Label=Test Forest") == std::string::npos) {
        std::cout << "FAILED (missing Label=Test Forest)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Data0 field
    if (osContent.find("Data0=") == std::string::npos) {
        std::cout << "FAILED (missing Data0= field)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify EndLevel field
    if (osContent.find("EndLevel=2") == std::string::npos) {
        std::cout << "FAILED (missing EndLevel=2)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.3: Data0_OneLine_Format (AC2)                              */
/*                                                                      */
/* Coordinates formatted on Data0 with "(lat,lon),(lat,lon)" format      */
/************************************************************************/

static bool Test_4_3_Data0_OneLine_Format() {
    std::cout << "  Test 4.3: Data0_OneLine_Format... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_data0");
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

    // AC2: Polygon with closed ring
    // [(48.8566,2.3522), (48.8577,2.3533), (48.8588,2.3522), (48.8566,2.3522)]
    // OGR: X=lon, Y=lat
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3522, 48.8588);
    oRing.addPoint(2.3522, 48.8566);  // Closing point
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // AC2: Data0=(48.856600,2.352200),(48.857700,2.353300),(48.858800,2.352200),(48.856600,2.352200)
    // ALL coordinates on ONE line, including closing point
    if (osContent.find("Data0=(48.856600,2.352200),(48.857700,2.353300),(48.858800,2.352200),(48.856600,2.352200)") == std::string::npos) {
        std::cout << "FAILED (expected Data0 with all coordinates on one line)" << std::endl;
        std::cout << "  File content: " << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.4: Coordinates_6_Decimals (AC2)                            */
/*                                                                      */
/* Coordinates formatted with 6 decimal precision                        */
/************************************************************************/

static bool Test_4_4_Coordinates_6_Decimals() {
    std::cout << "  Test 4.4: Coordinates_6_Decimals... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_coords");
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

    // Specific coordinates to test 6-decimal formatting
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // lon, lat
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Polish Map format: Data0=(lat,lon),(lat,lon) with 6 decimal precision
    if (osContent.find("48.856600") == std::string::npos ||
        osContent.find("2.352200") == std::string::npos) {
        std::cout << "FAILED (expected 6 decimal precision)" << std::endl;
        std::cout << "  File content: " << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.5: UTF8_To_CP1252_Label (AC7)                              */
/*                                                                      */
/* Label field converted from UTF-8 to CP1252                            */
/************************************************************************/

static bool Test_4_5_UTF8_To_CP1252_Label() {
    std::cout << "  Test 4.5: UTF8_To_CP1252_Label... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_utf8");
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
    // AC7: UTF-8 "Foret de Fontainebleau" = contains ê
    poFeature->SetField("Label", "For\xC3\xAAt de Fontainebleau");  // Foret in UTF-8

    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    // Read raw file content
    VSILFILE* fp = VSIFOpenL(osTempFile.c_str(), "rb");
    if (fp == nullptr) {
        std::cout << "FAILED (cannot read file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    char szBuffer[2048];
    memset(szBuffer, 0, sizeof(szBuffer));
    size_t nRead = VSIFReadL(szBuffer, 1, sizeof(szBuffer) - 1, fp);
    VSIFCloseL(fp);

    std::string osContent(szBuffer, nRead);

    // In CP1252: e = 0xEA
    // So "Foret" should be "For\xEAt"
    std::string osExpectedCP1252 = "Label=For\xEAt de Fontainebleau";

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
/*     Test 4.6: Optional_Label_Omitted                                   */
/*                                                                      */
/* Optional Label field omitted if not set                               */
/************************************************************************/

static bool Test_4_6_Optional_Label_Omitted() {
    std::cout << "  Test 4.6: Optional_Label_Omitted... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_nolabel");
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
    // Note: Label NOT set

    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POLYGON] section exists
    if (osContent.find("[POLYGON]") == std::string::npos) {
        std::cout << "FAILED (missing [POLYGON] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label= line is NOT present
    if (osContent.find("Label=") != std::string::npos) {
        std::cout << "FAILED (Label= should not be present when not set)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Type and Data0 are present
    if (osContent.find("Type=") == std::string::npos) {
        std::cout << "FAILED (missing Type= field)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    if (osContent.find("Data0=") == std::string::npos) {
        std::cout << "FAILED (missing Data0= field)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.7: RoundTrip (AC5)                                          */
/*                                                                      */
/* Round-trip: Create -> Write -> Close -> Open -> Read -> Verify        */
/************************************************************************/

static bool Test_4_7_RoundTrip() {
    std::cout << "  Test 4.7: RoundTrip... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_roundtrip");
    CleanupTempFile(osTempFile);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (PolishMap driver not found)" << std::endl;
        return false;
    }

    // Create and write
    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    if (poDS == nullptr) {
        std::cout << "FAILED (Create() returned nullptr)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);  // POLYGON layer (index 2)

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    poFeature->SetField("Label", "Test Forest");
    poFeature->SetField("EndLevel", 2);

    // Create Polygon with closed ring (X=lon, Y=lat)
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // lon, lat - Point A
    oRing.addPoint(2.3533, 48.8577);  // lon, lat - Point B
    oRing.addPoint(2.3544, 48.8566);  // lon, lat - Point C
    oRing.addPoint(2.3522, 48.8566);  // lon, lat - Point A (closing)
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Reopen and read
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

    poPolygonLayer->ResetReading();
    OGRFeature* poReadFeature = poPolygonLayer->GetNextFeature();

    if (poReadFeature == nullptr) {
        std::cout << "FAILED (no features found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Type field
    const char* pszType = poReadFeature->GetFieldAsString("Type");
    if (pszType == nullptr || strcmp(pszType, "0x4C") != 0) {
        std::cout << "FAILED (Type mismatch: " << (pszType ? pszType : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    const char* pszLabel = poReadFeature->GetFieldAsString("Label");
    if (pszLabel == nullptr || strcmp(pszLabel, "Test Forest") != 0) {
        std::cout << "FAILED (Label mismatch: " << (pszLabel ? pszLabel : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify EndLevel field
    int nEndLevel = poReadFeature->GetFieldAsInteger("EndLevel");
    if (nEndLevel != 2) {
        std::cout << "FAILED (EndLevel mismatch: " << nEndLevel << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify geometry
    OGRGeometry* poGeom = poReadFeature->GetGeometryRef();
    if (poGeom == nullptr || wkbFlatten(poGeom->getGeometryType()) != wkbPolygon) {
        std::cout << "FAILED (geometry is not Polygon)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRPolygon* poReadPolygon = poGeom->toPolygon();
    OGRLinearRing* poReadRing = poReadPolygon->getExteriorRing();
    if (poReadRing->getNumPoints() != 4) {
        std::cout << "FAILED (expected 4 points, got " << poReadRing->getNumPoints() << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check coordinates (tolerance for 6 decimal formatting)
    // Point 0 (and closing point 3)
    if (std::abs(poReadRing->getY(0) - 48.8566) > 1e-5 ||
        std::abs(poReadRing->getX(0) - 2.3522) > 1e-5) {
        std::cout << "FAILED (point 0 mismatch: " << poReadRing->getX(0) << "," << poReadRing->getY(0) << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Point 1
    if (std::abs(poReadRing->getY(1) - 48.8577) > 1e-5 ||
        std::abs(poReadRing->getX(1) - 2.3533) > 1e-5) {
        std::cout << "FAILED (point 1 mismatch)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Point 2
    if (std::abs(poReadRing->getY(2) - 48.8566) > 1e-5 ||
        std::abs(poReadRing->getX(2) - 2.3544) > 1e-5) {
        std::cout << "FAILED (point 2 mismatch)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Closing point (point 3) = first point (point 0)
    if (std::abs(poReadRing->getY(3) - 48.8566) > 1e-5 ||
        std::abs(poReadRing->getX(3) - 2.3522) > 1e-5) {
        std::cout << "FAILED (closing point mismatch)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRFeature::DestroyFeature(poReadFeature);
    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.8: NoGeometry_Returns_OGRERR_FAILURE                        */
/*                                                                      */
/* CreateFeature with no geometry returns OGRERR_FAILURE + CPLError      */
/************************************************************************/

static bool Test_4_8_NoGeometry_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.8: NoGeometry_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_nogeom");
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

    // Create feature WITHOUT geometry
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");
    // Note: NO geometry set!

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to missing geometry
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE, got " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify CPLError was logged
    if (CPLGetLastErrorType() != CE_Failure) {
        std::cout << "FAILED (expected CE_Failure error)" << std::endl;
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
/*     Test 4.9: TwoPoints_Returns_OGRERR_FAILURE                         */
/*                                                                      */
/* CreateFeature with Polygon containing only 2 points fails             */
/************************************************************************/

static bool Test_4_9_TwoPoints_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.9: TwoPoints_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_2points");
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

    // Create Polygon with only 2 points - invalid for POLYGON
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // Only 2 points
    oRing.addPoint(2.3533, 48.8577);
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to < 3 points
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for 2 points, got " << eErr << ")" << std::endl;
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
/*     Test 4.10: AutoClose_OpenRing (AC3)                                */
/*                                                                      */
/* Auto-close open ring: 3 points -> 4 points written + CPLDebug         */
/************************************************************************/

static bool Test_4_10_AutoClose_OpenRing() {
    std::cout << "  Test 4.10: AutoClose_OpenRing... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_autoclose");
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
    poFeature->SetField("Label", "Auto-closed");

    // AC3: Create Polygon with OPEN ring (3 points, first != last)
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // Point A
    oRing.addPoint(2.3533, 48.8577);  // Point B
    oRing.addPoint(2.3544, 48.8566);  // Point C
    // Note: NOT adding closing point A
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error for open ring)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Read file and verify ring was auto-closed
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // AC3: 3 points without closing -> 4 points written (auto-closed)
    // Data0 should end with (48.856600,2.352200) (the closing point)
    // Format: Data0=(48.856600,2.352200),(48.857700,2.353300),(48.856600,2.354400),(48.856600,2.352200)
    if (osContent.find("Data0=(48.856600,2.352200),(48.857700,2.353300),(48.856600,2.354400),(48.856600,2.352200)") == std::string::npos) {
        std::cout << "FAILED (ring not auto-closed)" << std::endl;
        std::cout << "  File content: " << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.11: TestCapability_OLCSequentialWrite (AC8)                 */
/*                                                                      */
/* TestCapability(OLCSequentialWrite) returns TRUE for POLYGON           */
/************************************************************************/

static bool Test_4_11_TestCapability_OLCSequentialWrite() {
    std::cout << "  Test 4.11: TestCapability_OLCSequentialWrite... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_capability");
    CleanupTempFile(osTempFile);

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

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);

    // Check OLCSequentialWrite in write mode - should be TRUE for POLYGON
    bool bSeqWrite = poPolygonLayer->TestCapability(OLCSequentialWrite);
    if (!bSeqWrite) {
        std::cout << "FAILED (OLCSequentialWrite should be TRUE for POLYGON in write mode)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check OLCRandomWrite - should be FALSE
    bool bRandWrite = poPolygonLayer->TestCapability(OLCRandomWrite);
    if (bRandWrite) {
        std::cout << "FAILED (OLCRandomWrite should be FALSE)" << std::endl;
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
/*     Test 4.12: MixedFile_POI_POLYLINE_POLYGON (AC6)                    */
/*                                                                      */
/* Mixed file: POI + POLYLINE + POLYGON in same file                     */
/************************************************************************/

static bool Test_4_12_MixedFile_POI_POLYLINE_POLYGON() {
    std::cout << "  Test 4.12: MixedFile_POI_POLYLINE_POLYGON... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_mixed");
    CleanupTempFile(osTempFile);

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

    // Get all three layers
    OGRLayer* poPOILayer = poDS->GetLayer(0);       // POI
    OGRLayer* poPolylineLayer = poDS->GetLayer(1);  // POLYLINE
    OGRLayer* poPolygonLayer = poDS->GetLayer(2);   // POLYGON

    // Create POI feature
    OGRFeature* poFeaturePOI = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeaturePOI->SetField("Type", "0x2C00");
    poFeaturePOI->SetField("Label", "Test POI");
    OGRPoint oPt(2.3522, 48.8566);
    poFeaturePOI->SetGeometry(&oPt);
    poPOILayer->CreateFeature(poFeaturePOI);
    OGRFeature::DestroyFeature(poFeaturePOI);

    // Create POLYLINE feature
    OGRFeature* poFeatureLine = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeatureLine->SetField("Type", "0x16");
    poFeatureLine->SetField("Label", "Test Trail");
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeatureLine->SetGeometry(&oLine);
    poPolylineLayer->CreateFeature(poFeatureLine);
    OGRFeature::DestroyFeature(poFeatureLine);

    // Create POLYGON feature
    OGRFeature* poFeaturePoly = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeaturePoly->SetField("Type", "0x4C");
    poFeaturePoly->SetField("Label", "Test Forest");
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3533, 48.8577);
    oRing.addPoint(2.3544, 48.8566);
    oRing.addPoint(2.3522, 48.8566);
    oPolygon.addRing(&oRing);
    poFeaturePoly->SetGeometry(&oPolygon);
    poPolygonLayer->CreateFeature(poFeaturePoly);
    OGRFeature::DestroyFeature(poFeaturePoly);

    GDALClose(poDS);

    // Reopen and verify all three layer types
    poDS = static_cast<GDALDataset*>(GDALOpenEx(osTempFile.c_str(), GDAL_OF_VECTOR, nullptr, nullptr, nullptr));
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check POI layer
    poPOILayer = poDS->GetLayerByName("POI");
    if (poPOILayer == nullptr) {
        std::cout << "FAILED (POI layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    poPOILayer->ResetReading();
    OGRFeature* poReadPOI = poPOILayer->GetNextFeature();
    if (poReadPOI == nullptr) {
        std::cout << "FAILED (no POI feature found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    OGRFeature::DestroyFeature(poReadPOI);

    // Check POLYLINE layer
    poPolylineLayer = poDS->GetLayerByName("POLYLINE");
    if (poPolylineLayer == nullptr) {
        std::cout << "FAILED (POLYLINE layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    poPolylineLayer->ResetReading();
    OGRFeature* poReadLine = poPolylineLayer->GetNextFeature();
    if (poReadLine == nullptr) {
        std::cout << "FAILED (no POLYLINE feature found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    OGRFeature::DestroyFeature(poReadLine);

    // Check POLYGON layer
    poPolygonLayer = poDS->GetLayerByName("POLYGON");
    if (poPolygonLayer == nullptr) {
        std::cout << "FAILED (POLYGON layer not found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    poPolygonLayer->ResetReading();
    OGRFeature* poReadPoly = poPolygonLayer->GetNextFeature();
    if (poReadPoly == nullptr) {
        std::cout << "FAILED (no POLYGON feature found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }
    OGRFeature::DestroyFeature(poReadPoly);

    GDALClose(poDS);
    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*     Test 4.13: WrongGeometryType_Returns_OGRERR_FAILURE               */
/*                                                                      */
/* CreateFeature with Point geometry on POLYGON layer fails              */
/************************************************************************/

static bool Test_4_13_WrongGeometryType_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.13: WrongGeometryType_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_wronggeom");
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

    OGRLayer* poPolygonLayer = poDS->GetLayer(2);  // POLYGON layer

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolygonLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x4C");

    // Set POINT geometry instead of Polygon - wrong type for POLYGON layer
    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to wrong geometry type
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for Point geometry, got " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify CPLError was logged
    if (CPLGetLastErrorType() != CE_Failure) {
        std::cout << "FAILED (expected CE_Failure error)" << std::endl;
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
/*     Test 4.14: EmptyPolygon_Returns_OGRERR_FAILURE                     */
/*                                                                      */
/* CreateFeature with empty Polygon (0 points in exterior ring) fails    */
/************************************************************************/

static bool Test_4_14_EmptyPolygon_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.14: EmptyPolygon_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_empty");
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

    // Create empty Polygon (no points in ring)
    OGRPolygon oPolygon;
    OGRLinearRing oRing;  // Empty - no points added
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to < 3 points
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for empty Polygon, got " << eErr << ")" << std::endl;
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
/*     Test 4.15: OnePoint_Returns_OGRERR_FAILURE (Code Review M1)        */
/*                                                                      */
/* CreateFeature with Polygon containing only 1 point fails               */
/************************************************************************/

static bool Test_4_15_OnePoint_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.15: OnePoint_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_1point");
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

    // Create Polygon with only 1 point - invalid for POLYGON
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);  // Only 1 point
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to < 3 points
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for 1 point, got " << eErr << ")" << std::endl;
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
/*     Test 4.16: DegeneratePolygon_Returns_OGRERR_FAILURE (Code Review H2)*/
/*                                                                      */
/* CreateFeature with degenerate Polygon (all points at same location) fails */
/************************************************************************/

static bool Test_4_16_DegeneratePolygon_Returns_OGRERR_FAILURE() {
    std::cout << "  Test 4.16: DegeneratePolygon_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polygon_degenerate");
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

    // Create degenerate Polygon - 4 points all at same location
    OGRPolygon oPolygon;
    OGRLinearRing oRing;
    oRing.addPoint(2.3522, 48.8566);
    oRing.addPoint(2.3522, 48.8566);  // Same point
    oRing.addPoint(2.3522, 48.8566);  // Same point
    oRing.addPoint(2.3522, 48.8566);  // Same point (closing)
    oPolygon.addRing(&oRing);
    poFeature->SetGeometry(&oPolygon);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to degenerate polygon
    OGRErr eErr = poPolygonLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for degenerate polygon, got " << eErr << ")" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify CPLError was logged
    if (CPLGetLastErrorType() != CE_Failure) {
        std::cout << "FAILED (expected CE_Failure error)" << std::endl;
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
    std::cout << "=== Story 2.5: POLYGON Feature Writing (CreateFeature for Polygons) ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // Test 4.1: CreateFeature with valid Polygon (AC1)
    if (Test_4_1_CreateFeature_ValidPolygon_Returns_OGRERR_NONE()) nPassed++; else nFailed++;

    // Test 4.2: File contains [POLYGON] section (AC4)
    if (Test_4_2_File_Contains_POLYGON_Section()) nPassed++; else nFailed++;

    // Test 4.3: Data0 format with all coordinates on one line (AC2)
    if (Test_4_3_Data0_OneLine_Format()) nPassed++; else nFailed++;

    // Test 4.4: Coordinates with 6 decimals (AC2)
    if (Test_4_4_Coordinates_6_Decimals()) nPassed++; else nFailed++;

    // Test 4.5: UTF-8 to CP1252 conversion (AC7)
    if (Test_4_5_UTF8_To_CP1252_Label()) nPassed++; else nFailed++;

    // Test 4.6: Optional Label omitted
    if (Test_4_6_Optional_Label_Omitted()) nPassed++; else nFailed++;

    // Test 4.7: Round-trip validation (AC5)
    if (Test_4_7_RoundTrip()) nPassed++; else nFailed++;

    // Test 4.8: No geometry case
    if (Test_4_8_NoGeometry_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Test 4.9: Two points invalid case (< 3 points)
    if (Test_4_9_TwoPoints_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Test 4.10: Auto-close open ring (AC3)
    if (Test_4_10_AutoClose_OpenRing()) nPassed++; else nFailed++;

    // Test 4.11: TestCapability(OLCSequentialWrite) = TRUE (AC8)
    if (Test_4_11_TestCapability_OLCSequentialWrite()) nPassed++; else nFailed++;

    // Test 4.12: Mixed file POI + POLYLINE + POLYGON (AC6)
    if (Test_4_12_MixedFile_POI_POLYLINE_POLYGON()) nPassed++; else nFailed++;

    // Test 4.13: Wrong geometry type (Point instead of Polygon)
    if (Test_4_13_WrongGeometryType_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Test 4.14: Empty Polygon (0 points in exterior ring)
    if (Test_4_14_EmptyPolygon_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Test 4.15: One point polygon (Code Review M1)
    if (Test_4_15_OnePoint_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Test 4.16: Degenerate polygon - all points at same location (Code Review H2)
    if (Test_4_16_DegeneratePolygon_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
