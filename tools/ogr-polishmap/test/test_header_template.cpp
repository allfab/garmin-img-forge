/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for Story 2.2.8 - HEADER_TEMPLATE Dataset Creation Option
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * - AC1: Template valide copié correctement
 * - AC2: Fichier template manquant → erreur
 * - AC3: Template header invalide → erreur
 * - AC4: Précédence template > metadata
 * - AC5: Template vide → fallback metadata
 * - AC6: Round-trip préservation custom fields
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

// Test counters
static int nPassedTests = 0;
static int nFailedTests = 0;

// Test helper: Register driver
static void SetupTest() {
    GDALAllRegister();
    RegisterOGRPolishMap();
}

// Test helper: Create valid template file
static void CreateValidTemplate(const char* pszPath) {
    VSILFILE* fp = VSIFOpenL(pszPath, "wb");
    assert(fp != nullptr && "Failed to create template file");

    VSIFPrintfL(fp, "[IMG ID]\n");
    VSIFPrintfL(fp, "ID=12345678\n");
    VSIFPrintfL(fp, "Name=Template Map\n");
    VSIFPrintfL(fp, "Elevation=M\n");
    VSIFPrintfL(fp, "Preprocess=F\n");
    VSIFPrintfL(fp, "LBLcoding=9\n");
    VSIFPrintfL(fp, "CodePage=1252\n");
    VSIFPrintfL(fp, "[END]\n\n");
    VSIFCloseL(fp);
}

// Test helper: Read file content
static CPLString ReadFile(const char* pszPath) {
    VSILFILE* fp = VSIFOpenL(pszPath, "rb");
    if (!fp) return "";

    VSIFSeekL(fp, 0, SEEK_END);
    vsi_l_offset nSize = VSIFTellL(fp);
    VSIFSeekL(fp, 0, SEEK_SET);

    char* pszContent = static_cast<char*>(CPLMalloc(static_cast<size_t>(nSize) + 1));
    VSIFReadL(pszContent, 1, static_cast<size_t>(nSize), fp);
    pszContent[nSize] = '\0';
    VSIFCloseL(fp);

    CPLString osContent(pszContent);
    CPLFree(pszContent);

    return osContent;
}

// Test helper: Check if header contains field=value
static bool HeaderContains(const char* pszPath, const char* pszField, const char* pszValue) {
    CPLString osContent = ReadFile(pszPath);
    CPLString osExpected = CPLString(pszField) + "=" + pszValue;
    return osContent.find(osExpected.c_str()) != std::string::npos;
}

// Test helper: Cleanup files
static void CleanupFiles() {
    VSIUnlink("/vsimem/template.mp");
    VSIUnlink("/vsimem/output.mp");
    VSIUnlink("/vsimem/template_custom.mp");
    VSIUnlink("/vsimem/invalid_template.mp");
}

/************************************************************************/
/*                     AC1: Template valide copié                       */
/************************************************************************/

static bool Test_ValidTemplate() {
    std::cout << "  Test_ValidTemplate... ";

    CleanupFiles();

    // Create valid template
    CreateValidTemplate("/vsimem/template.mp");

    // Get driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        return false;
    }

    // Create dataset with HEADER_TEMPLATE option
    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "/vsimem/template.mp");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    if (poDS == nullptr) {
        std::cout << "FAILED (dataset creation failed)" << std::endl;
        CleanupFiles();
        return false;
    }

    GDALClose(poDS);

    // Verify header fields copied
    if (!HeaderContains("/vsimem/output.mp", "ID", "12345678")) {
        std::cout << "FAILED (ID field not copied)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "Name", "Template Map")) {
        std::cout << "FAILED (Name field not copied)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "Elevation", "M")) {
        std::cout << "FAILED (Elevation field not copied)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "LBLcoding", "9")) {
        std::cout << "FAILED (LBLcoding field not copied)" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*                AC2: Fichier template manquant → erreur              */
/************************************************************************/

static bool Test_MissingFile() {
    std::cout << "  Test_MissingFile... ";

    CleanupFiles();

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        return false;
    }

    CPLPushErrorHandler(CPLQuietErrorHandler);

    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "/vsimem/nonexistent.mp");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    CPLPopErrorHandler();

    if (poDS != nullptr) {
        std::cout << "FAILED (dataset creation should have failed)" << std::endl;
        GDALClose(poDS);
        CleanupFiles();
        return false;
    }

    // Verify error message
    const char* pszError = CPLGetLastErrorMsg();
    if (pszError == nullptr || strstr(pszError, "HEADER_TEMPLATE file not found") == nullptr) {
        std::cout << "FAILED (wrong error message: " << (pszError ? pszError : "null") << ")" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*             AC3: Template header invalide → erreur                   */
/************************************************************************/

static bool Test_InvalidHeader() {
    std::cout << "  Test_InvalidHeader... ";

    CleanupFiles();

    // Create template WITHOUT [IMG ID] section
    VSILFILE* fp = VSIFOpenL("/vsimem/invalid_template.mp", "wb");
    assert(fp != nullptr);
    VSIFPrintfL(fp, "[POI]\n");
    VSIFPrintfL(fp, "Type=0x2C00\n");
    VSIFPrintfL(fp, "[END]\n");
    VSIFCloseL(fp);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        CleanupFiles();
        return false;
    }

    CPLPushErrorHandler(CPLQuietErrorHandler);

    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "/vsimem/invalid_template.mp");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    CPLPopErrorHandler();

    if (poDS != nullptr) {
        std::cout << "FAILED (dataset creation should have failed with invalid header)" << std::endl;
        GDALClose(poDS);
        CleanupFiles();
        return false;
    }

    // Verify error message
    const char* pszError = CPLGetLastErrorMsg();
    if (pszError == nullptr || strstr(pszError, "invalid [IMG ID] section") == nullptr) {
        std::cout << "FAILED (wrong error message: " << (pszError ? pszError : "null") << ")" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*              AC4: Précédence template > metadata                     */
/************************************************************************/

static bool Test_TemplatePrecedence() {
    std::cout << "  Test_TemplatePrecedence... ";

    CleanupFiles();

    CreateValidTemplate("/vsimem/template.mp");

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        CleanupFiles();
        return false;
    }

    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "/vsimem/template.mp");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    if (poDS == nullptr) {
        std::cout << "FAILED (dataset creation failed)" << std::endl;
        CleanupFiles();
        return false;
    }

    // Try to override with SetMetadataItem (should be ignored)
    poDS->SetMetadataItem("Name", "Override Name");
    poDS->SetMetadataItem("ID", "99999999");

    GDALClose(poDS);

    // Verify template values used, NOT metadata overrides
    if (!HeaderContains("/vsimem/output.mp", "Name", "Template Map")) {
        std::cout << "FAILED (template Name not used)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "ID", "12345678")) {
        std::cout << "FAILED (template ID not used)" << std::endl;
        CleanupFiles();
        return false;
    }

    // Verify override values NOT present
    if (HeaderContains("/vsimem/output.mp", "Name", "Override Name")) {
        std::cout << "FAILED (metadata override was incorrectly used)" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*             AC5: Template vide → fallback metadata                   */
/************************************************************************/

static bool Test_EmptyTemplateFallback() {
    std::cout << "  Test_EmptyTemplateFallback... ";

    CleanupFiles();

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        CleanupFiles();
        return false;
    }

    // Use empty string for HEADER_TEMPLATE
    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    if (poDS == nullptr) {
        std::cout << "FAILED (dataset creation failed)" << std::endl;
        CleanupFiles();
        return false;
    }

    // Set metadata (should be used since template is empty)
    poDS->SetMetadataItem("Name", "Metadata Name");
    poDS->SetMetadataItem("ID", "87654321");

    GDALClose(poDS);

    // Verify metadata values used
    if (!HeaderContains("/vsimem/output.mp", "Name", "Metadata Name")) {
        std::cout << "FAILED (metadata Name not used)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "ID", "87654321")) {
        std::cout << "FAILED (metadata ID not used)" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*          AC6: Round-trip préservation custom fields                  */
/************************************************************************/

static bool Test_RoundTripCustomFields() {
    std::cout << "  Test_RoundTripCustomFields... ";

    CleanupFiles();

    // Create template with custom fields
    VSILFILE* fp = VSIFOpenL("/vsimem/template_custom.mp", "wb");
    assert(fp != nullptr);
    VSIFPrintfL(fp, "[IMG ID]\n");
    VSIFPrintfL(fp, "ID=123456\n");
    VSIFPrintfL(fp, "Name=Custom Template\n");
    VSIFPrintfL(fp, "CustomField=CustomValue\n");
    VSIFPrintfL(fp, "AnotherCustom=TestData\n");
    VSIFPrintfL(fp, "CodePage=1252\n");
    VSIFPrintfL(fp, "[END]\n\n");
    VSIFCloseL(fp);

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cout << "FAILED (driver not found)" << std::endl;
        CleanupFiles();
        return false;
    }

    char** papszOptions = CSLSetNameValue(nullptr, "HEADER_TEMPLATE", "/vsimem/template_custom.mp");
    GDALDataset* poDS = poDriver->Create("/vsimem/output.mp", 0, 0, 0, GDT_Unknown, papszOptions);
    CSLDestroy(papszOptions);

    if (poDS == nullptr) {
        std::cout << "FAILED (dataset creation failed)" << std::endl;
        CleanupFiles();
        return false;
    }

    GDALClose(poDS);

    // Verify custom fields preserved
    if (!HeaderContains("/vsimem/output.mp", "CustomField", "CustomValue")) {
        std::cout << "FAILED (CustomField not preserved)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "AnotherCustom", "TestData")) {
        std::cout << "FAILED (AnotherCustom not preserved)" << std::endl;
        CleanupFiles();
        return false;
    }

    // Verify standard fields also preserved
    if (!HeaderContains("/vsimem/output.mp", "Name", "Custom Template")) {
        std::cout << "FAILED (Name not preserved)" << std::endl;
        CleanupFiles();
        return false;
    }

    if (!HeaderContains("/vsimem/output.mp", "ID", "123456")) {
        std::cout << "FAILED (ID not preserved)" << std::endl;
        CleanupFiles();
        return false;
    }

    CleanupFiles();
    std::cout << "PASSED" << std::endl;
    return true;
}

/************************************************************************/
/*                             Main                                     */
/************************************************************************/

int main(int /* argc */, char** /* argv */) {
    std::cout << "=== Story 2.2.8: HEADER_TEMPLATE Dataset Creation Option ===" << std::endl;

    // Set GDAL_DRIVER_PATH to avoid loading system plugins
    CPLSetConfigOption("GDAL_DRIVER_PATH", "/nonexistent");

    SetupTest();

    // Run tests
    if (Test_ValidTemplate()) nPassedTests++; else nFailedTests++;
    if (Test_MissingFile()) nPassedTests++; else nFailedTests++;
    if (Test_InvalidHeader()) nPassedTests++; else nFailedTests++;
    if (Test_TemplatePrecedence()) nPassedTests++; else nFailedTests++;
    if (Test_EmptyTemplateFallback()) nPassedTests++; else nFailedTests++;
    if (Test_RoundTripCustomFields()) nPassedTests++; else nFailedTests++;

    std::cout << "========================================" << std::endl;
    std::cout << "Passed: " << nPassedTests << std::endl;
    std::cout << "Failed: " << nFailedTests << std::endl;
    std::cout << "========================================" << std::endl;

    GDALDestroyDriverManager();

    return (nFailedTests == 0) ? 0 : 1;
}
