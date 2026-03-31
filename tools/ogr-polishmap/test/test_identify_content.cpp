/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Test Identify() with content validation ([IMG ID] header check)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Test that Identify() correctly validates Polish Map files by checking
 * for [IMG ID] header content, not just .mp extension.
 ****************************************************************************/

#include "gdal_priv.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include <iostream>
#include <chrono>
#include <cstring>

// Declare the driver registration function
extern "C" void RegisterOGRPolishMap();

// Helper function to get test data path
static CPLString GetTestDataPath(const char* pszSubPath) {
    // Get the directory containing this test executable
    // Tests are run from build directory, test data is in source tree
    CPLString osPath = CPLGetDirname(__FILE__);
    if (osPath.empty()) {
        osPath = ".";
    }
    osPath = CPLFormFilename(osPath.c_str(), "data", nullptr);
    osPath = CPLFormFilename(osPath.c_str(), pszSubPath, nullptr);
    return osPath;
}

// Test counter
static int g_nTestsPassed = 0;
static int g_nTestsFailed = 0;

static void TestPass(const char* pszTestName) {
    std::cout << "  [PASS] " << pszTestName << std::endl;
    g_nTestsPassed++;
}

static void TestFail(const char* pszTestName, const char* pszReason) {
    std::cerr << "  [FAIL] " << pszTestName << ": " << pszReason << std::endl;
    g_nTestsFailed++;
}

/************************************************************************/
/*                    Test_Identify_ValidHeader_Simple                  */
/*                                                                      */
/* AC1: Identify() valide les fichiers .mp via contenu                  */
/************************************************************************/
static void Test_Identify_ValidHeader_Simple() {
    const char* pszTestName = "Identify() recognizes valid .mp with [IMG ID] header";

    CPLString osPath = GetTestDataPath("valid-minimal/header-simple.mp");

    // Check test file exists
    VSIStatBufL sStat;
    if (VSIStatL(osPath.c_str(), &sStat) != 0) {
        TestFail(pszTestName, "Test file not found");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nResult = poDriver->pfnIdentify(&oOpenInfo);
    if (nResult == TRUE) {
        TestPass(pszTestName);
    } else {
        TestFail(pszTestName, "Expected TRUE, got FALSE");
    }
}

/************************************************************************/
/*                    Test_Identify_ValidHeader_Full                    */
/*                                                                      */
/* AC1: Identify() valide les fichiers .mp via contenu (full metadata)  */
/************************************************************************/
static void Test_Identify_ValidHeader_Full() {
    const char* pszTestName = "Identify() recognizes valid .mp with full metadata";

    CPLString osPath = GetTestDataPath("valid-minimal/header-full.mp");

    VSIStatBufL sStat;
    if (VSIStatL(osPath.c_str(), &sStat) != 0) {
        TestFail(pszTestName, "Test file not found");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nResult = poDriver->pfnIdentify(&oOpenInfo);
    if (nResult == TRUE) {
        TestPass(pszTestName);
    } else {
        TestFail(pszTestName, "Expected TRUE, got FALSE");
    }
}

/************************************************************************/
/*                    Test_Identify_MissingHeader                       */
/*                                                                      */
/* AC4: Identify() rejette rapidement les fichiers invalides            */
/************************************************************************/
static void Test_Identify_MissingHeader() {
    const char* pszTestName = "Identify() rejects .mp without [IMG ID] header";

    CPLString osPath = GetTestDataPath("error-recovery/missing-header.mp");

    VSIStatBufL sStat;
    if (VSIStatL(osPath.c_str(), &sStat) != 0) {
        TestFail(pszTestName, "Test file not found");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nResult = poDriver->pfnIdentify(&oOpenInfo);
    if (nResult == FALSE) {
        TestPass(pszTestName);
    } else {
        TestFail(pszTestName, "Expected FALSE, got TRUE - file without [IMG ID] should be rejected");
    }
}

/************************************************************************/
/*                    Test_Identify_InvalidFormat                       */
/*                                                                      */
/* AC4: Identify() rejette rapidement les fichiers invalides (binaire)  */
/************************************************************************/
static void Test_Identify_InvalidFormat() {
    const char* pszTestName = "Identify() rejects binary .mp file";

    CPLString osPath = GetTestDataPath("error-recovery/invalid-format.mp");

    VSIStatBufL sStat;
    if (VSIStatL(osPath.c_str(), &sStat) != 0) {
        TestFail(pszTestName, "Test file not found");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nResult = poDriver->pfnIdentify(&oOpenInfo);
    if (nResult == FALSE) {
        TestPass(pszTestName);
    } else {
        TestFail(pszTestName, "Expected FALSE, got TRUE - binary content should be rejected");
    }
}

/************************************************************************/
/*                    Test_Identify_Performance                         */
/*                                                                      */
/* AC4: Identify() rejette rapidement les fichiers invalides (< 10ms)   */
/************************************************************************/
static void Test_Identify_Performance() {
    const char* pszTestName = "Identify() completes in < 10ms";

    CPLString osPath = GetTestDataPath("valid-minimal/header-full.mp");

    VSIStatBufL sStat;
    if (VSIStatL(osPath.c_str(), &sStat) != 0) {
        TestFail(pszTestName, "Test file not found");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    // Measure execution time
    auto start = std::chrono::high_resolution_clock::now();

    // Run multiple times for better measurement
    for (int i = 0; i < 100; i++) {
        CPL_IGNORE_RET_VAL(poDriver->pfnIdentify(&oOpenInfo));
    }

    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);

    // Average time per call in microseconds
    double avgTimeUs = static_cast<double>(duration.count()) / 100.0;

    // Should be < 10ms (10000 microseconds)
    if (avgTimeUs < 10000.0) {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "%s (avg: %.2f us)", pszTestName, avgTimeUs);
        TestPass(szMsg);
    } else {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Average time %.2f us exceeds 10ms limit", avgTimeUs);
        TestFail(pszTestName, szMsg);
    }
}

/************************************************************************/
/*                    Test_Identify_NonMpExtension                      */
/*                                                                      */
/* Ensure non-.mp files are still rejected                              */
/************************************************************************/
static void Test_Identify_NonMpExtension() {
    const char* pszTestName = "Identify() rejects non-.mp extension";

    // Create a temporary .txt file with valid [IMG ID] content
    CPLString osTempFile = CPLGenerateTempFilename("ogr_polishmap_test");
    osTempFile += ".txt";

    VSILFILE* fp = VSIFOpenL(osTempFile.c_str(), "wb");
    if (fp) {
        const char* pszContent = "[IMG ID]\nName=Test\n[END-IMG ID]\n";
        VSIFWriteL(pszContent, 1, strlen(pszContent), fp);
        VSIFCloseL(fp);
    } else {
        TestFail(pszTestName, "Cannot create temp file");
        return;
    }

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        VSIUnlink(osTempFile.c_str());
        TestFail(pszTestName, "Driver not registered");
        return;
    }

    GDALOpenInfo oOpenInfo(osTempFile.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nResult = poDriver->pfnIdentify(&oOpenInfo);

    VSIUnlink(osTempFile.c_str());

    if (nResult == FALSE) {
        TestPass(pszTestName);
    } else {
        TestFail(pszTestName, "Expected FALSE, got TRUE - non-.mp extension should be rejected");
    }
}

/************************************************************************/
/*                              main()                                   */
/************************************************************************/
int main() {
    std::cout << "=== OGR PolishMap Identify() Content Validation Tests ===" << std::endl;
    std::cout << std::endl;

    // Initialize GDAL
    GDALAllRegister();

    // Register our driver
    RegisterOGRPolishMap();

    std::cout << "Running tests..." << std::endl;

    // Run all tests
    Test_Identify_ValidHeader_Simple();
    Test_Identify_ValidHeader_Full();
    Test_Identify_MissingHeader();
    Test_Identify_InvalidFormat();
    Test_Identify_Performance();
    Test_Identify_NonMpExtension();

    std::cout << std::endl;
    std::cout << "=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << g_nTestsPassed << std::endl;
    std::cout << "Failed: " << g_nTestsFailed << std::endl;

    if (g_nTestsFailed > 0) {
        std::cout << "\n=== TESTS FAILED ===" << std::endl;
        return 1;
    }

    std::cout << "\n=== All Tests PASSED ===" << std::endl;
    return 0;
}
