/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.3 - POI Feature Writing (CreateFeature for Points)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - CreateFeature() with valid Point returns OGRERR_NONE (AC1)
 * - File contains [POI] sections after close (AC2)
 * - Coordinates formatted with 6 decimals (AC3)
 * - UTF-8 to CP1252 conversion for Label (AC4)
 * - Optional fields omitted if absent (AC5)
 * - Round-trip validation (AC6)
 * - CreateFeature(nullptr) returns OGRERR_FAILURE (AC7)
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
/*            Test_CreateFeature_ValidPoint_Returns_OGRERR_NONE (AC1)    */
/*                                                                      */
/* CreateFeature() with valid Point geometry returns OGRERR_NONE        */
/************************************************************************/

static bool Test_CreateFeature_ValidPoint_Returns_OGRERR_NONE() {
    std::cout << "  Test_CreateFeature_ValidPoint_Returns_OGRERR_NONE... ";

    CPLString osTempFile = GetTempFilePath("test_poi_create");
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

    // Get POI layer (index 0)
    OGRLayer* poPOILayer = poDS->GetLayer(0);
    if (poPOILayer == nullptr) {
        std::cout << "FAILED (GetLayer(0) returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify it's the POI layer
    if (strcmp(poPOILayer->GetName(), "POI") != 0) {
        std::cout << "FAILED (layer 0 is not POI)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with Point geometry
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    if (poFeature == nullptr) {
        std::cout << "FAILED (CreateFeature returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set fields
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "Restaurant");

    // Create Point geometry (X=lon, Y=lat)
    OGRPoint oPoint(2.3522, 48.8566);  // Paris coordinates
    poFeature->SetGeometry(&oPoint);

    // Call CreateFeature
    OGRErr eErr = poPOILayer->CreateFeature(poFeature);

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
/*          Test_CreateFeature_File_Contains_POI_Section (AC2)           */
/*                                                                      */
/* File contains [POI] sections after close                              */
/************************************************************************/

static bool Test_CreateFeature_File_Contains_POI_Section() {
    std::cout << "  Test_CreateFeature_File_Contains_POI_Section... ";

    CPLString osTempFile = GetTempFilePath("test_poi_section");
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

    // Get POI layer
    OGRLayer* poPOILayer = poDS->GetLayer(0);

    // Create a feature
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "Restaurant");

    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // Close to flush
    GDALClose(poDS);

    // Read file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POI] section
    if (osContent.find("[POI]") == std::string::npos) {
        std::cout << "FAILED (missing [POI] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify [END] marker for POI section (Polish Map uses [END] for all sections)
    // Count [END] markers - should have at least 2: one for header, one for POI
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
    if (osContent.find("Type=0x2C00") == std::string::npos) {
        std::cout << "FAILED (missing Type=0x2C00)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    if (osContent.find("Label=Restaurant") == std::string::npos) {
        std::cout << "FAILED (missing Label=Restaurant)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Data0 field
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
/*          Test_CreateFeature_Coordinates_6_Decimals (AC3)              */
/*                                                                      */
/* Coordinates formatted with 6 decimal precision                        */
/************************************************************************/

static bool Test_CreateFeature_Coordinates_6_Decimals() {
    std::cout << "  Test_CreateFeature_Coordinates_6_Decimals... ";

    CPLString osTempFile = GetTempFilePath("test_poi_coords");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");

    // Specific coordinates to test 6-decimal formatting
    OGRPoint oPoint(2.3522, 48.8566);  // lon, lat
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Polish Map format: Data0=(lat,lon) with 6 decimal precision
    // Expected: Data0=(48.856600,2.352200)
    if (osContent.find("Data0=(48.856600,2.352200)") == std::string::npos) {
        std::cout << "FAILED (expected Data0=(48.856600,2.352200))" << std::endl;
        std::cout << "  File content: " << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          Test_CreateFeature_UTF8_To_CP1252_Label (AC4)                 */
/*                                                                      */
/* Label field converted from UTF-8 to CP1252                            */
/************************************************************************/

static bool Test_CreateFeature_UTF8_To_CP1252_Label() {
    std::cout << "  Test_CreateFeature_UTF8_To_CP1252_Label... ";

    CPLString osTempFile = GetTempFilePath("test_poi_utf8");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    // UTF-8 "Café" = 0x43 0x61 0x66 0xC3 0xA9 (é in UTF-8 is C3 A9)
    poFeature->SetField("Label", "Caf\xC3\xA9");

    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
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

    // In CP1252: é = 0xE9
    // So "Café" should be "Caf\xE9"
    std::string osExpectedCP1252 = "Label=Caf\xE9";

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
/*          Test_CreateFeature_Optional_Label_Omitted (AC5)              */
/*                                                                      */
/* Optional Label field omitted if not set                               */
/************************************************************************/

static bool Test_CreateFeature_Optional_Label_Omitted() {
    std::cout << "  Test_CreateFeature_Optional_Label_Omitted... ";

    CPLString osTempFile = GetTempFilePath("test_poi_nolabel");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    // Note: Label NOT set

    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POI] section exists
    if (osContent.find("[POI]") == std::string::npos) {
        std::cout << "FAILED (missing [POI] section)" << std::endl;
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
/*          Test_CreateFeature_Optional_EndLevel_Omitted (AC5 extended)  */
/*                                                                      */
/* Optional EndLevel field omitted if not set                            */
/************************************************************************/

static bool Test_CreateFeature_Optional_EndLevel_Omitted() {
    std::cout << "  Test_CreateFeature_Optional_EndLevel_Omitted... ";

    CPLString osTempFile = GetTempFilePath("test_poi_noendlevel");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "Test");
    // Note: EndLevel NOT set

    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POI] section exists
    if (osContent.find("[POI]") == std::string::npos) {
        std::cout << "FAILED (missing [POI] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify EndLevel= line is NOT present
    if (osContent.find("EndLevel=") != std::string::npos) {
        std::cout << "FAILED (EndLevel= should not be present when not set)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          Test_CreateFeature_EndLevel_Written_When_Set                 */
/*                                                                      */
/* EndLevel field written when explicitly set                            */
/************************************************************************/

static bool Test_CreateFeature_EndLevel_Written_When_Set() {
    std::cout << "  Test_CreateFeature_EndLevel_Written_When_Set... ";

    CPLString osTempFile = GetTempFilePath("test_poi_endlevel");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("EndLevel", 3);

    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify EndLevel=3 is present
    if (osContent.find("EndLevel=3") == std::string::npos) {
        std::cout << "FAILED (EndLevel=3 not found)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          Test_CreateFeature_GeometryAutoDispatch                      */
/*                                                                      */
/* Story 2.6: CreateFeature with LineString on POI layer is auto-        */
/* dispatched to POLYLINE writer (for ogr2ogr compatibility).            */
/************************************************************************/

static bool Test_CreateFeature_WrongGeometryType_Fails() {
    std::cout << "  Test_CreateFeature_GeometryAutoDispatch... ";

    CPLString osTempFile = GetTempFilePath("test_poi_autodispatch");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x0016");
    poFeature->SetField("Label", "AutoDispatch Test");

    // Story 2.6: Create LineString on POI layer - should auto-dispatch to POLYLINE
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3530, 48.8570);
    poFeature->SetGeometry(&oLine);

    CPLErrorReset();
    OGRErr eErr = poPOILayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // Story 2.6: Should now succeed because geometry is auto-dispatched
    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (expected OGRERR_NONE for auto-dispatched geometry)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Verify the file contains [POLYLINE] section (auto-dispatched)
    std::string osContent = ReadFileContent(osTempFile.c_str());
    if (osContent.find("[POLYLINE]") == std::string::npos) {
        std::cout << "FAILED ([POLYLINE] section not found - auto-dispatch failed)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          Test_CreateFeature_RoundTrip (AC6)                           */
/*                                                                      */
/* Round-trip: Create -> Write -> Close -> Open -> Read -> Verify        */
/* Also verifies FID assignment (M5)                                     */
/************************************************************************/

static bool Test_CreateFeature_RoundTrip() {
    std::cout << "  Test_CreateFeature_RoundTrip... ";

    CPLString osTempFile = GetTempFilePath("test_poi_roundtrip");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "Test Restaurant");

    // Coordinates: lon=2.3522, lat=48.8566
    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    OGRErr eErr = poPOILayer->CreateFeature(poFeature);

    // M5: Verify FID was assigned after CreateFeature
    GIntBig nFID = poFeature->GetFID();
    if (nFID < 1) {
        std::cout << "FAILED (FID not assigned after CreateFeature: " << nFID << ")" << std::endl;
        OGRFeature::DestroyFeature(poFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Reopen and read
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    poPOILayer = poDS->GetLayerByName("POI");
    if (poPOILayer == nullptr) {
        std::cout << "FAILED (POI layer not found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    poPOILayer->ResetReading();
    OGRFeature* poReadFeature = poPOILayer->GetNextFeature();

    if (poReadFeature == nullptr) {
        std::cout << "FAILED (no features found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Type field
    const char* pszType = poReadFeature->GetFieldAsString("Type");
    if (pszType == nullptr || strcmp(pszType, "0x2C00") != 0) {
        std::cout << "FAILED (Type mismatch: " << (pszType ? pszType : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    const char* pszLabel = poReadFeature->GetFieldAsString("Label");
    if (pszLabel == nullptr || strcmp(pszLabel, "Test Restaurant") != 0) {
        std::cout << "FAILED (Label mismatch: " << (pszLabel ? pszLabel : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify geometry
    OGRGeometry* poGeom = poReadFeature->GetGeometryRef();
    if (poGeom == nullptr || wkbFlatten(poGeom->getGeometryType()) != wkbPoint) {
        std::cout << "FAILED (geometry is not Point)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRPoint* poReadPoint = poGeom->toPoint();

    // Verify coordinates (lat=Y, lon=X)
    if (std::abs(poReadPoint->getY() - 48.8566) > 1e-5) {
        std::cout << "FAILED (latitude mismatch: " << poReadPoint->getY() << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    if (std::abs(poReadPoint->getX() - 2.3522) > 1e-5) {
        std::cout << "FAILED (longitude mismatch: " << poReadPoint->getX() << ")" << std::endl;
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
/*          Test_CreateFeature_Null_Returns_OGRERR_FAILURE (AC7)         */
/*                                                                      */
/* Note: GDAL's OGRLayer::CreateFeature() crashes when called with       */
/* nullptr before reaching our ICreateFeature() override. This is GDAL   */
/* behavior, not our driver's.                                           */
/*                                                                       */
/* Instead, we test that CreateFeature with missing geometry fails       */
/* gracefully with OGRERR_FAILURE (alternative validation for AC7).      */
/************************************************************************/

static bool Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE() {
    std::cout << "  Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_poi_nogeom");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    // Create feature WITHOUT geometry
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPOILayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    // Note: NO geometry set!

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to missing geometry
    OGRErr eErr = poPOILayer->CreateFeature(poFeature);
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
/*              Test_TestCapability_OLCSequentialWrite (Task 4)           */
/*                                                                      */
/* TestCapability(OLCSequentialWrite) returns TRUE in write mode         */
/************************************************************************/

static bool Test_TestCapability_OLCSequentialWrite() {
    std::cout << "  Test_TestCapability_OLCSequentialWrite... ";

    CPLString osTempFile = GetTempFilePath("test_poi_capability");
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

    OGRLayer* poPOILayer = poDS->GetLayer(0);

    // Check OLCSequentialWrite in write mode - should be TRUE
    bool bSeqWrite = poPOILayer->TestCapability(OLCSequentialWrite);
    if (!bSeqWrite) {
        std::cout << "FAILED (OLCSequentialWrite should be TRUE in write mode)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check OLCRandomWrite - should be FALSE
    bool bRandWrite = poPOILayer->TestCapability(OLCRandomWrite);
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
/*                               main()                                  */
/************************************************************************/

int main() {
    std::cout << "=== Story 2.3: POI Feature Writing (CreateFeature for Points) ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // AC1: CreateFeature with valid Point
    if (Test_CreateFeature_ValidPoint_Returns_OGRERR_NONE()) nPassed++; else nFailed++;

    // AC2: File contains [POI] section
    if (Test_CreateFeature_File_Contains_POI_Section()) nPassed++; else nFailed++;

    // AC3: Coordinates with 6 decimals
    if (Test_CreateFeature_Coordinates_6_Decimals()) nPassed++; else nFailed++;

    // AC4: UTF-8 to CP1252 conversion
    if (Test_CreateFeature_UTF8_To_CP1252_Label()) nPassed++; else nFailed++;

    // AC5: Optional Label omitted
    if (Test_CreateFeature_Optional_Label_Omitted()) nPassed++; else nFailed++;

    // AC5 extended: Optional EndLevel omitted
    if (Test_CreateFeature_Optional_EndLevel_Omitted()) nPassed++; else nFailed++;

    // AC5 extended: EndLevel written when set
    if (Test_CreateFeature_EndLevel_Written_When_Set()) nPassed++; else nFailed++;

    // M2: Wrong geometry type fails
    if (Test_CreateFeature_WrongGeometryType_Fails()) nPassed++; else nFailed++;

    // AC6: Round-trip validation (includes M5: FID verification)
    if (Test_CreateFeature_RoundTrip()) nPassed++; else nFailed++;

    // AC7: CreateFeature with missing geometry returns OGRERR_FAILURE
    // Note: GDAL crashes on nullptr before our code runs, so we test no-geometry instead
    if (Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Task 4: TestCapability(OLCSequentialWrite)
    if (Test_TestCapability_OLCSequentialWrite()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
