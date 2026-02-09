/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.4 - POLYLINE Feature Writing (CreateFeature for LineStrings)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - CreateFeature() with valid LineString returns OGRERR_NONE (AC1)
 * - Coordinates formatted on Data0 with "(lat,lon),(lat,lon)" format (AC2)
 * - File contains [POLYLINE] sections after close (AC3)
 * - Levels and EndLevel fields written if present (AC4)
 * - POLYLINE with 50+ points (AC5)
 * - Round-trip validation (AC6)
 * - UTF-8 to CP1252 conversion for Label (AC7)
 * - TestCapability(OLCSequentialWrite) returns TRUE for POLYLINE (AC8)
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

// Suppress warn_unused_result in tests where CreateFeature return is intentionally unchecked
#if defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-result"
#endif

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
/*     Test_CreateFeature_ValidLineString_Returns_OGRERR_NONE (AC1)      */
/*                                                                      */
/* CreateFeature() with valid LineString geometry returns OGRERR_NONE    */
/************************************************************************/

static bool Test_CreateFeature_ValidLineString_Returns_OGRERR_NONE() {
    std::cout << "  Test_CreateFeature_ValidLineString_Returns_OGRERR_NONE... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_create");
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

    // Get POLYLINE layer (index 1)
    OGRLayer* poPolylineLayer = poDS->GetLayer(1);
    if (poPolylineLayer == nullptr) {
        std::cout << "FAILED (GetLayer(1) returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify it's the POLYLINE layer
    if (strcmp(poPolylineLayer->GetName(), "POLYLINE") != 0) {
        std::cout << "FAILED (layer 1 is not POLYLINE)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Create a feature with LineString geometry (3 points)
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    if (poFeature == nullptr) {
        std::cout << "FAILED (CreateFeature returned nullptr)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Set fields (AC1: Type="0x16", Label="Trail", EndLevel=3)
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Label", "Trail");
    poFeature->SetField("EndLevel", 3);

    // Create LineString geometry with 3 points (X=lon, Y=lat)
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);  // Paris coordinates
    oLine.addPoint(2.3533, 48.8577);
    oLine.addPoint(2.3544, 48.8588);
    poFeature->SetGeometry(&oLine);

    // Call CreateFeature
    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);

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
/*          Test_CreateFeature_Data0_OneLine_Format (AC2)                 */
/*                                                                      */
/* Coordinates formatted on Data0 with "(lat,lon),(lat,lon)" format      */
/************************************************************************/

static bool Test_CreateFeature_Data0_OneLine_Format() {
    std::cout << "  Test_CreateFeature_Data0_OneLine_Format... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_data0");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");

    // AC2: LineString with coordinates [(48.8566,2.3522), (48.8577,2.3533), (48.8588,2.3544)]
    // OGR: X=lon, Y=lat
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    oLine.addPoint(2.3544, 48.8588);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // AC2: Data0=(48.856600,2.352200),(48.857700,2.353300),(48.858800,2.354400)
    // ALL coordinates on ONE line, separated by commas
    if (osContent.find("Data0=(48.856600,2.352200),(48.857700,2.353300),(48.858800,2.354400)") == std::string::npos) {
        std::cout << "FAILED (expected Data0=(48.856600,2.352200),(48.857700,2.353300),(48.858800,2.354400))" << std::endl;
        std::cout << "  File content: " << osContent << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    CleanupTempFile(osTempFile);

    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          Test_CreateFeature_File_Contains_POLYLINE_Section (AC3)       */
/*                                                                      */
/* File contains [POLYLINE] sections after close                          */
/************************************************************************/

static bool Test_CreateFeature_File_Contains_POLYLINE_Section() {
    std::cout << "  Test_CreateFeature_File_Contains_POLYLINE_Section... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_section");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    // Create a feature
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Label", "Test Trail");

    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    // Close to flush
    GDALClose(poDS);

    // Read file content
    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POLYLINE] section
    if (osContent.find("[POLYLINE]") == std::string::npos) {
        std::cout << "FAILED (missing [POLYLINE] section)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify [END] marker after POLYLINE section
    // Count [END] markers - should have at least 2: one for header, one for POLYLINE
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
    if (osContent.find("Type=0x16") == std::string::npos) {
        std::cout << "FAILED (missing Type=0x16)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    if (osContent.find("Label=Test Trail") == std::string::npos) {
        std::cout << "FAILED (missing Label=Test Trail)" << std::endl;
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
/*          Test_CreateFeature_Coordinates_6_Decimals (AC2)              */
/*                                                                      */
/* Coordinates formatted with 6 decimal precision                        */
/************************************************************************/

static bool Test_CreateFeature_Coordinates_6_Decimals() {
    std::cout << "  Test_CreateFeature_Coordinates_6_Decimals... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_coords");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");

    // Specific coordinates to test 6-decimal formatting
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);  // lon, lat
    oLine.addPoint(2.3533, 48.8577);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Polish Map format: Data0=(lat,lon),(lat,lon) with 6 decimal precision
    // Check for 6 decimals
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
/*          Test_CreateFeature_UTF8_To_CP1252_Label (AC7)                 */
/*                                                                      */
/* Label field converted from UTF-8 to CP1252                            */
/************************************************************************/

static bool Test_CreateFeature_UTF8_To_CP1252_Label() {
    std::cout << "  Test_CreateFeature_UTF8_To_CP1252_Label... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_utf8");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    // AC7: UTF-8 "Sentier de la Forêt" = contains é and ê
    poFeature->SetField("Label", "Sentier de la For\xC3\xAAt");  // Forêt in UTF-8

    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
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

    // In CP1252: ê = 0xEA
    // So "Forêt" should be "For\xEAt"
    std::string osExpectedCP1252 = "Label=Sentier de la For\xEAt";

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
/*          Test_CreateFeature_Optional_Label_Omitted (AC4 related)       */
/*                                                                      */
/* Optional Label field omitted if not set                               */
/************************************************************************/

static bool Test_CreateFeature_Optional_Label_Omitted() {
    std::cout << "  Test_CreateFeature_Optional_Label_Omitted... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_nolabel");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    // Note: Label NOT set

    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify [POLYLINE] section exists
    if (osContent.find("[POLYLINE]") == std::string::npos) {
        std::cout << "FAILED (missing [POLYLINE] section)" << std::endl;
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
/*          Test_CreateFeature_RoundTrip (AC6)                           */
/*                                                                      */
/* Round-trip: Create -> Write -> Close -> Open -> Read -> Verify        */
/************************************************************************/

static bool Test_CreateFeature_RoundTrip() {
    std::cout << "  Test_CreateFeature_RoundTrip... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_roundtrip");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);  // POLYLINE layer (index 1)

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Label", "Test Trail");
    poFeature->SetField("EndLevel", 3);
    poFeature->SetField("Levels", "0-3");

    // Create LineString with 3 points (X=lon, Y=lat)
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);  // lon, lat - Paris
    oLine.addPoint(2.3533, 48.8577);  // lon, lat
    oLine.addPoint(2.3544, 48.8588);  // lon, lat
    poFeature->SetGeometry(&oLine);

    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
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

    poPolylineLayer = poDS->GetLayerByName("POLYLINE");
    if (poPolylineLayer == nullptr) {
        std::cout << "FAILED (POLYLINE layer not found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    poPolylineLayer->ResetReading();
    OGRFeature* poReadFeature = poPolylineLayer->GetNextFeature();

    if (poReadFeature == nullptr) {
        std::cout << "FAILED (no features found in reopened file)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Type field
    const char* pszType = poReadFeature->GetFieldAsString("Type");
    if (pszType == nullptr || strcmp(pszType, "0x16") != 0) {
        std::cout << "FAILED (Type mismatch: " << (pszType ? pszType : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Label field
    const char* pszLabel = poReadFeature->GetFieldAsString("Label");
    if (pszLabel == nullptr || strcmp(pszLabel, "Test Trail") != 0) {
        std::cout << "FAILED (Label mismatch: " << (pszLabel ? pszLabel : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify EndLevel field
    int nEndLevel = poReadFeature->GetFieldAsInteger("EndLevel");
    if (nEndLevel != 3) {
        std::cout << "FAILED (EndLevel mismatch: " << nEndLevel << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify Levels field
    const char* pszLevels = poReadFeature->GetFieldAsString("Levels");
    if (pszLevels == nullptr || strcmp(pszLevels, "0-3") != 0) {
        std::cout << "FAILED (Levels mismatch: " << (pszLevels ? pszLevels : "null") << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify geometry
    OGRGeometry* poGeom = poReadFeature->GetGeometryRef();
    if (poGeom == nullptr || wkbFlatten(poGeom->getGeometryType()) != wkbLineString) {
        std::cout << "FAILED (geometry is not LineString)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLineString* poReadLine = poGeom->toLineString();
    if (poReadLine->getNumPoints() != 3) {
        std::cout << "FAILED (expected 3 points, got " << poReadLine->getNumPoints() << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check coordinates (tolerance for 6 decimal formatting)
    // Point 0
    if (std::abs(poReadLine->getY(0) - 48.8566) > 1e-5 ||
        std::abs(poReadLine->getX(0) - 2.3522) > 1e-5) {
        std::cout << "FAILED (point 0 mismatch: " << poReadLine->getX(0) << "," << poReadLine->getY(0) << ")" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Point 1
    if (std::abs(poReadLine->getY(1) - 48.8577) > 1e-5 ||
        std::abs(poReadLine->getX(1) - 2.3533) > 1e-5) {
        std::cout << "FAILED (point 1 mismatch)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Point 2
    if (std::abs(poReadLine->getY(2) - 48.8588) > 1e-5 ||
        std::abs(poReadLine->getX(2) - 2.3544) > 1e-5) {
        std::cout << "FAILED (point 2 mismatch)" << std::endl;
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
/*          Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE         */
/*                                                                      */
/* CreateFeature with no geometry returns OGRERR_FAILURE + CPLError      */
/************************************************************************/

static bool Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE() {
    std::cout << "  Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_nogeom");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    // Create feature WITHOUT geometry
    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    // Note: NO geometry set!

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to missing geometry
    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
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
/*          Test_CreateFeature_SinglePoint_Returns_OGRERR_FAILURE        */
/*                                                                      */
/* CreateFeature with LineString containing only 1 point fails           */
/************************************************************************/

static bool Test_CreateFeature_SinglePoint_Returns_OGRERR_FAILURE() {
    std::cout << "  Test_CreateFeature_SinglePoint_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_1point");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");

    // Create LineString with only 1 point - invalid for POLYLINE
    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);  // Only 1 point
    poFeature->SetGeometry(&oLine);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to < 2 points
    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for single point, got " << eErr << ")" << std::endl;
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
/*          Test_CreateFeature_50Plus_Points (AC5)                        */
/*                                                                      */
/* POLYLINE with 50+ points                                               */
/************************************************************************/

static bool Test_CreateFeature_50Plus_Points() {
    std::cout << "  Test_CreateFeature_50Plus_Points... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_50pts");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Label", "Long Trail");

    // Create LineString with 50+ points
    OGRLineString oLine;
    for (int i = 0; i < 55; i++) {
        oLine.addPoint(2.3522 + i * 0.001, 48.8566 + i * 0.001);  // lon, lat
    }
    poFeature->SetGeometry(&oLine);

    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (CreateFeature returned error for 50+ points)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    GDALClose(poDS);

    // Reopen and verify
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    if (poDS == nullptr) {
        std::cout << "FAILED (cannot reopen file)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    poPolylineLayer = poDS->GetLayerByName("POLYLINE");
    poPolylineLayer->ResetReading();
    OGRFeature* poReadFeature = poPolylineLayer->GetNextFeature();

    if (poReadFeature == nullptr) {
        std::cout << "FAILED (no features found)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRGeometry* poGeom = poReadFeature->GetGeometryRef();
    if (poGeom == nullptr || wkbFlatten(poGeom->getGeometryType()) != wkbLineString) {
        std::cout << "FAILED (geometry is not LineString)" << std::endl;
        OGRFeature::DestroyFeature(poReadFeature);
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    OGRLineString* poReadLine = poGeom->toLineString();
    if (poReadLine->getNumPoints() != 55) {
        std::cout << "FAILED (expected 55 points, got " << poReadLine->getNumPoints() << ")" << std::endl;
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
/*          Test_TestCapability_OLCSequentialWrite (AC8)                  */
/*                                                                      */
/* TestCapability(OLCSequentialWrite) returns TRUE for POLYLINE          */
/************************************************************************/

static bool Test_TestCapability_OLCSequentialWrite() {
    std::cout << "  Test_TestCapability_OLCSequentialWrite... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_capability");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    // Check OLCSequentialWrite in write mode - should be TRUE for POLYLINE
    bool bSeqWrite = poPolylineLayer->TestCapability(OLCSequentialWrite);
    if (!bSeqWrite) {
        std::cout << "FAILED (OLCSequentialWrite should be TRUE for POLYLINE in write mode)" << std::endl;
        GDALClose(poDS);
        CleanupTempFile(osTempFile);
        return false;
    }

    // Check OLCRandomWrite - should be FALSE
    bool bRandWrite = poPolylineLayer->TestCapability(OLCRandomWrite);
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
/*          Test_CreateFeature_Levels_And_EndLevel_Written (AC4)          */
/*                                                                      */
/* Levels and EndLevel fields written when present                        */
/************************************************************************/

static bool Test_CreateFeature_Levels_And_EndLevel_Written() {
    std::cout << "  Test_CreateFeature_Levels_And_EndLevel_Written... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_levels");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");
    poFeature->SetField("Levels", "0-3");
    poFeature->SetField("EndLevel", 3);

    OGRLineString oLine;
    oLine.addPoint(2.3522, 48.8566);
    oLine.addPoint(2.3533, 48.8577);
    poFeature->SetGeometry(&oLine);

    (void)poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    GDALClose(poDS);

    std::string osContent = ReadFileContent(osTempFile.c_str());

    // Verify Levels field
    if (osContent.find("Levels=0-3") == std::string::npos) {
        std::cout << "FAILED (missing Levels=0-3)" << std::endl;
        CleanupTempFile(osTempFile);
        return false;
    }

    // Verify EndLevel field
    if (osContent.find("EndLevel=3") == std::string::npos) {
        std::cout << "FAILED (missing EndLevel=3)" << std::endl;
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
/* Story 2.6: CreateFeature with Point geometry on POLYLINE layer is     */
/* auto-dispatched to POI writer (for ogr2ogr compatibility).            */
/************************************************************************/

static bool Test_CreateFeature_WrongGeometryType_Returns_OGRERR_FAILURE() {
    std::cout << "  Test_CreateFeature_GeometryAutoDispatch... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_autodispatch");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);  // POLYLINE layer

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "AutoDispatch Test");

    // Story 2.6: Set POINT geometry on POLYLINE layer - will be auto-dispatched to POI
    OGRPoint oPoint(2.3522, 48.8566);
    poFeature->SetGeometry(&oPoint);

    // Clear previous errors
    CPLErrorReset();

    // Story 2.6: Call CreateFeature - should succeed with auto-dispatch
    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_NONE) {
        std::cout << "FAILED (expected OGRERR_NONE for auto-dispatched geometry, got " << eErr << ")" << std::endl;
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
/*          Test_CreateFeature_EmptyLineString_Returns_OGRERR_FAILURE    */
/*                                                                      */
/* CreateFeature with empty LineString (0 points) fails                  */
/************************************************************************/

static bool Test_CreateFeature_EmptyLineString_Returns_OGRERR_FAILURE() {
    std::cout << "  Test_CreateFeature_EmptyLineString_Returns_OGRERR_FAILURE... ";

    CPLString osTempFile = GetTempFilePath("test_polyline_empty");
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

    OGRLayer* poPolylineLayer = poDS->GetLayer(1);

    OGRFeature* poFeature = OGRFeature::CreateFeature(poPolylineLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x16");

    // Create empty LineString (0 points)
    OGRLineString oLine;  // Empty - no points added
    poFeature->SetGeometry(&oLine);

    // Clear previous errors
    CPLErrorReset();

    // Call CreateFeature - should fail due to < 2 points
    OGRErr eErr = poPolylineLayer->CreateFeature(poFeature);
    OGRFeature::DestroyFeature(poFeature);

    if (eErr != OGRERR_FAILURE) {
        std::cout << "FAILED (expected OGRERR_FAILURE for empty LineString, got " << eErr << ")" << std::endl;
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
    std::cout << "=== Story 2.4: POLYLINE Feature Writing (CreateFeature for LineStrings) ===" << std::endl;
    std::cout << std::endl;

    SetupTest();

    int nPassed = 0;
    int nFailed = 0;

    std::cout << "Running tests:" << std::endl;

    // AC1: CreateFeature with valid LineString
    if (Test_CreateFeature_ValidLineString_Returns_OGRERR_NONE()) nPassed++; else nFailed++;

    // AC2: Data0 format with all coordinates on one line
    if (Test_CreateFeature_Data0_OneLine_Format()) nPassed++; else nFailed++;

    // AC3: File contains [POLYLINE] section
    if (Test_CreateFeature_File_Contains_POLYLINE_Section()) nPassed++; else nFailed++;

    // AC2: Coordinates with 6 decimals
    if (Test_CreateFeature_Coordinates_6_Decimals()) nPassed++; else nFailed++;

    // AC7: UTF-8 to CP1252 conversion
    if (Test_CreateFeature_UTF8_To_CP1252_Label()) nPassed++; else nFailed++;

    // AC4 related: Optional Label omitted
    if (Test_CreateFeature_Optional_Label_Omitted()) nPassed++; else nFailed++;

    // AC6: Round-trip validation
    if (Test_CreateFeature_RoundTrip()) nPassed++; else nFailed++;

    // No geometry case
    if (Test_CreateFeature_NoGeometry_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Single point invalid case
    if (Test_CreateFeature_SinglePoint_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Empty LineString (0 points) invalid case
    if (Test_CreateFeature_EmptyLineString_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // Wrong geometry type (Point instead of LineString)
    if (Test_CreateFeature_WrongGeometryType_Returns_OGRERR_FAILURE()) nPassed++; else nFailed++;

    // AC5: 50+ points
    if (Test_CreateFeature_50Plus_Points()) nPassed++; else nFailed++;

    // AC8: TestCapability(OLCSequentialWrite) = TRUE for POLYLINE
    if (Test_TestCapability_OLCSequentialWrite()) nPassed++; else nFailed++;

    // AC4: Levels and EndLevel fields
    if (Test_CreateFeature_Levels_And_EndLevel_Written()) nPassed++; else nFailed++;

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << nPassed << std::endl;
    std::cout << "Failed: " << nFailed << std::endl;

    return (nFailed == 0) ? 0 : 1;
}
