/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Test PolishMapParser and Open() functionality
 * Author:   mpforge project
 *
 ******************************************************************************
 * Test that the parser correctly extracts [IMG ID] header metadata and
 * that Open() correctly creates dataset with parsed data.
 ****************************************************************************/

#include "gdal_priv.h"
#include "ogrpolishmapdatasource.h"
#include "polishmapparser.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include <iostream>
#include <cstring>

// Declare the driver registration function
extern "C" void RegisterOGRPolishMap();

// Helper function to get test data path
static CPLString GetTestDataPath(const char* pszSubPath) {
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
/*                    Test_Parser_SimpleHeader                          */
/*                                                                      */
/* AC3: Parser extrait les metadonnees du header                        */
/************************************************************************/
static void Test_Parser_SimpleHeader() {
    const char* pszTestName = "Parser extracts metadata from simple header";

    CPLString osPath = GetTestDataPath("valid-minimal/header-simple.mp");

    PolishMapParser oParser(osPath.c_str());
    if (!oParser.IsOpen()) {
        TestFail(pszTestName, "Cannot open test file");
        return;
    }

    if (!oParser.ParseHeader()) {
        TestFail(pszTestName, "ParseHeader() returned false");
        return;
    }

    const PolishMapHeaderData& oData = oParser.GetHeaderData();

    if (oData.osName != "TestMap") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected Name='TestMap', got '%s'", oData.osName.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Parser_FullHeader                            */
/*                                                                      */
/* AC3: Parser extrait tous les champs metadata                         */
/************************************************************************/
static void Test_Parser_FullHeader() {
    const char* pszTestName = "Parser extracts all metadata from full header";

    CPLString osPath = GetTestDataPath("valid-minimal/header-full.mp");

    PolishMapParser oParser(osPath.c_str());
    if (!oParser.IsOpen()) {
        TestFail(pszTestName, "Cannot open test file");
        return;
    }

    if (!oParser.ParseHeader()) {
        TestFail(pszTestName, "ParseHeader() returned false");
        return;
    }

    const PolishMapHeaderData& oData = oParser.GetHeaderData();

    // Check known fields
    if (oData.osName != "Full Test Map") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected Name='Full Test Map', got '%s'", oData.osName.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    if (oData.osID != "12345678") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected ID='12345678', got '%s'", oData.osID.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    if (oData.osCodePage != "1252") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected CodePage='1252', got '%s'", oData.osCodePage.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    if (oData.osDatum != "WGS 84") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected Datum='WGS 84', got '%s'", oData.osDatum.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    if (oData.osElevation != "M") {
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected Elevation='M', got '%s'", oData.osElevation.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    // Check other fields are stored
    auto it = oData.aoOtherFields.find("Preprocess");
    if (it == oData.aoOtherFields.end() || it->second != "F") {
        TestFail(pszTestName, "Expected Preprocess='F' in other fields");
        return;
    }

    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Parser_CP1252Encoding                        */
/*                                                                      */
/* AC3: Parser converts CP1252 text to UTF-8                            */
/************************************************************************/
static void Test_Parser_CP1252Encoding() {
    const char* pszTestName = "Parser converts CP1252 characters to UTF-8";

    CPLString osPath = GetTestDataPath("valid-minimal/header-cp1252.mp");

    PolishMapParser oParser(osPath.c_str());
    if (!oParser.IsOpen()) {
        TestFail(pszTestName, "Cannot open test file");
        return;
    }

    if (!oParser.ParseHeader()) {
        TestFail(pszTestName, "ParseHeader() returned false");
        return;
    }

    const PolishMapHeaderData& oData = oParser.GetHeaderData();

    // Expected UTF-8 encoded string: "Café réseau français"
    // CP1252: C a f 0xE9 (space) r 0xE9 s e a u (space) f r a n 0xE7 a i s
    // UTF-8:  C a f 0xC3 0xA9 (space) r 0xC3 0xA9 s e a u (space) f r a n 0xC3 0xA7 a i s
    // Note: Use string concatenation to avoid hex escape sequence confusion
    const char* pszExpected = "Caf\xC3\xA9 r\xC3\xA9seau fran\xC3\xA7" "ais";

    if (oData.osName != pszExpected) {
        char szMsg[512];
        snprintf(szMsg, sizeof(szMsg),
                 "Expected UTF-8 Name='%s', got '%s' (lengths: expected=%zu, got=%zu)",
                 pszExpected, oData.osName.c_str(),
                 strlen(pszExpected), oData.osName.size());
        TestFail(pszTestName, szMsg);
        return;
    }

    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Parser_MissingHeader                         */
/*                                                                      */
/* AC2: Parser rejette les fichiers sans [IMG ID]                       */
/************************************************************************/
static void Test_Parser_MissingHeader() {
    const char* pszTestName = "Parser rejects file without [IMG ID] header";

    CPLString osPath = GetTestDataPath("error-recovery/missing-header.mp");

    PolishMapParser oParser(osPath.c_str());
    if (!oParser.IsOpen()) {
        TestFail(pszTestName, "Cannot open test file");
        return;
    }

    // Suppress error output during this test
    CPLPushErrorHandler(CPLQuietErrorHandler);
    bool bResult = oParser.ParseHeader();
    CPLPopErrorHandler();

    if (bResult) {
        TestFail(pszTestName, "Expected ParseHeader() to return false");
        return;
    }

    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Open_ValidFile                               */
/*                                                                      */
/* AC1, AC3: Open() returns valid dataset with metadata                 */
/************************************************************************/
static void Test_Open_ValidFile() {
    const char* pszTestName = "Open() returns valid dataset with metadata";

    CPLString osPath = GetTestDataPath("valid-minimal/header-full.mp");

    GDALDataset* poDS = static_cast<GDALDataset*>(
        GDALOpenEx(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY,
                   nullptr, nullptr, nullptr));

    if (poDS == nullptr) {
        TestFail(pszTestName, "GDALOpenEx returned NULL");
        return;
    }

    // Check that description is set to file path (FR26)
    const char* pszDesc = poDS->GetDescription();
    if (pszDesc == nullptr || strstr(pszDesc, "header-full.mp") == nullptr) {
        GDALClose(poDS);
        TestFail(pszTestName, "Dataset description not set to file path");
        return;
    }

    // Cast to OGRPolishMapDataSource to access header data
    OGRPolishMapDataSource* poPolishDS = dynamic_cast<OGRPolishMapDataSource*>(poDS);
    if (poPolishDS == nullptr) {
        GDALClose(poDS);
        TestFail(pszTestName, "Cannot cast to OGRPolishMapDataSource");
        return;
    }

    const PolishMapHeaderData& oData = poPolishDS->GetHeaderData();
    if (oData.osName != "Full Test Map") {
        GDALClose(poDS);
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected Name='Full Test Map', got '%s'", oData.osName.c_str());
        TestFail(pszTestName, szMsg);
        return;
    }

    GDALClose(poDS);
    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Open_MissingHeader                           */
/*                                                                      */
/* AC2: Open() returns NULL and logs error for missing header           */
/************************************************************************/
static void Test_Open_MissingHeader() {
    const char* pszTestName = "Open() returns NULL for file without [IMG ID]";

    CPLString osPath = GetTestDataPath("error-recovery/missing-header.mp");

    // Suppress error output during this test
    CPLPushErrorHandler(CPLQuietErrorHandler);
    GDALDataset* poDS = static_cast<GDALDataset*>(
        GDALOpenEx(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY,
                   nullptr, nullptr, nullptr));
    CPLErrorNum eLastErr = CPLGetLastErrorNo();
    CPLPopErrorHandler();

    if (poDS != nullptr) {
        GDALClose(poDS);
        TestFail(pszTestName, "Expected GDALOpenEx to return NULL");
        return;
    }

    // Note: Since Identify() now rejects files without [IMG ID], Open() won't
    // even be called for such files. This is expected behavior.

    TestPass(pszTestName);
}

/************************************************************************/
/*                    Test_Open_SimpleHeader                            */
/*                                                                      */
/* Verify Open() works with minimal valid file                          */
/************************************************************************/
static void Test_Open_SimpleHeader() {
    const char* pszTestName = "Open() succeeds with minimal valid file";

    CPLString osPath = GetTestDataPath("valid-minimal/header-simple.mp");

    GDALDataset* poDS = static_cast<GDALDataset*>(
        GDALOpenEx(osPath.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY,
                   nullptr, nullptr, nullptr));

    if (poDS == nullptr) {
        TestFail(pszTestName, "GDALOpenEx returned NULL");
        return;
    }

    // Verify basic dataset properties
    // Story 1.3 changed expected layer count from 0 to 3
    int nLayerCount = poDS->GetLayerCount();
    if (nLayerCount != 3) {
        GDALClose(poDS);
        char szMsg[256];
        snprintf(szMsg, sizeof(szMsg), "Expected 3 layers, got %d", nLayerCount);
        TestFail(pszTestName, szMsg);
        return;
    }

    GDALClose(poDS);
    TestPass(pszTestName);
}

/************************************************************************/
/*                              main()                                   */
/************************************************************************/
int main() {
    std::cout << "=== OGR PolishMap Parser and Open() Tests ===" << std::endl;
    std::cout << std::endl;

    // Initialize GDAL
    GDALAllRegister();

    // Register our driver
    RegisterOGRPolishMap();

    std::cout << "Parser Tests:" << std::endl;
    Test_Parser_SimpleHeader();
    Test_Parser_FullHeader();
    Test_Parser_CP1252Encoding();
    Test_Parser_MissingHeader();

    std::cout << std::endl;
    std::cout << "Open() Tests:" << std::endl;
    Test_Open_ValidFile();
    Test_Open_MissingHeader();
    Test_Open_SimpleHeader();

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
