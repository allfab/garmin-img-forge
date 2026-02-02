/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Test driver metadata for GDAL integration (Story 2.6 AC1, AC6)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Tests:
 * - GDAL_DMD_LONGNAME = "Polish Map Format"
 * - GDAL_DMD_EXTENSION = "mp"
 * - GDAL_DMD_HELPTOPIC = documentation URL
 * - GDAL_DCAP_VECTOR = "YES"
 * - GDAL_DCAP_CREATE = "YES"
 * - GDAL_DCAP_VIRTUALIO = "YES"
 * - GDAL_DMD_CREATIONFIELDDATATYPES = supported types
 * - GDAL_DMD_SUPPORTED_SQL_DIALECTS = "OGRSQL"
 ****************************************************************************/

#include "gdal_priv.h"
#include "cpl_conv.h"
#include <iostream>
#include <cstring>

// Declare the driver registration function
extern "C" void RegisterOGRPolishMap();

// Test counter
static int nTestsPassed = 0;
static int nTestsFailed = 0;

void TestMetadataItem(GDALDriver* poDriver, const char* pszItem,
                      const char* pszExpected, const char* pszTestName) {
    const char* pszValue = poDriver->GetMetadataItem(pszItem);

    if (pszValue == nullptr) {
        std::cerr << "[FAIL] " << pszTestName << ": metadata item not found" << std::endl;
        nTestsFailed++;
        return;
    }

    if (EQUAL(pszValue, pszExpected)) {
        std::cout << "[OK] " << pszTestName << ": " << pszValue << std::endl;
        nTestsPassed++;
    } else {
        std::cerr << "[FAIL] " << pszTestName << ": expected '" << pszExpected
                  << "', got '" << pszValue << "'" << std::endl;
        nTestsFailed++;
    }
}

void TestMetadataItemExists(GDALDriver* poDriver, const char* pszItem,
                            const char* pszTestName) {
    const char* pszValue = poDriver->GetMetadataItem(pszItem);

    if (pszValue != nullptr && strlen(pszValue) > 0) {
        std::cout << "[OK] " << pszTestName << ": " << pszValue << std::endl;
        nTestsPassed++;
    } else {
        std::cerr << "[FAIL] " << pszTestName << ": metadata item not found or empty" << std::endl;
        nTestsFailed++;
    }
}

int main() {
    std::cout << "=== OGR PolishMap Driver Metadata Tests (Story 2.6) ===" << std::endl;
    std::cout << "Testing AC1: GetMetadata() returns required GDAL_DMD_* fields" << std::endl;
    std::cout << "Testing AC6: Driver metadata complete with all standard fields" << std::endl;
    std::cout << std::endl;

    // Initialize GDAL
    GDALAllRegister();

    // Register our driver
    RegisterOGRPolishMap();

    // Get driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cerr << "ERROR: PolishMap driver not found!" << std::endl;
        return 1;
    }

    std::cout << "--- AC1: Required Metadata Fields ---" << std::endl;

    // AC1: GDAL_DMD_LONGNAME = "Polish Map Format"
    TestMetadataItem(poDriver, GDAL_DMD_LONGNAME, "Polish Map Format",
                     "AC1.1 GDAL_DMD_LONGNAME");

    // AC1: GDAL_DMD_EXTENSION = "mp"
    TestMetadataItem(poDriver, GDAL_DMD_EXTENSION, "mp",
                     "AC1.2 GDAL_DMD_EXTENSION");

    // AC1: GDAL_DMD_HELPTOPIC must exist and contain documentation URL
    TestMetadataItemExists(poDriver, GDAL_DMD_HELPTOPIC,
                           "AC1.3 GDAL_DMD_HELPTOPIC");

    std::cout << std::endl << "--- AC6: Driver Capabilities ---" << std::endl;

    // AC6: GDAL_DCAP_VECTOR = "YES"
    TestMetadataItem(poDriver, GDAL_DCAP_VECTOR, "YES",
                     "AC6.1 GDAL_DCAP_VECTOR");

    // AC6: GDAL_DCAP_CREATE = "YES" (from Story 2.1)
    TestMetadataItem(poDriver, GDAL_DCAP_CREATE, "YES",
                     "AC6.2 GDAL_DCAP_CREATE");

    // AC6: GDAL_DCAP_VIRTUALIO = "YES"
    TestMetadataItem(poDriver, GDAL_DCAP_VIRTUALIO, "YES",
                     "AC6.3 GDAL_DCAP_VIRTUALIO");

    std::cout << std::endl << "--- Additional Metadata Fields ---" << std::endl;

    // GDAL_DMD_CREATIONFIELDDATATYPES - supported field types for writing
    TestMetadataItemExists(poDriver, GDAL_DMD_CREATIONFIELDDATATYPES,
                           "Task 1.5 GDAL_DMD_CREATIONFIELDDATATYPES");

    // GDAL_DMD_SUPPORTED_SQL_DIALECTS - SQL dialects supported
    TestMetadataItem(poDriver, GDAL_DMD_SUPPORTED_SQL_DIALECTS, "OGRSQL",
                     "Task 1.8 GDAL_DMD_SUPPORTED_SQL_DIALECTS");

    std::cout << std::endl << "--- Driver Description Check ---" << std::endl;

    // Check driver description (short name)
    const char* pszDesc = poDriver->GetDescription();
    if (pszDesc && EQUAL(pszDesc, "PolishMap")) {
        std::cout << "[OK] Driver short name: " << pszDesc << std::endl;
        nTestsPassed++;
    } else {
        std::cerr << "[FAIL] Driver short name: expected 'PolishMap', got '"
                  << (pszDesc ? pszDesc : "null") << "'" << std::endl;
        nTestsFailed++;
    }

    // Summary
    std::cout << std::endl << "======================================" << std::endl;
    std::cout << "Driver Metadata Test Summary:" << std::endl;
    std::cout << "  Passed: " << nTestsPassed << std::endl;
    std::cout << "  Failed: " << nTestsFailed << std::endl;
    std::cout << "======================================" << std::endl;

    if (nTestsFailed == 0) {
        std::cout << "=== All Metadata Tests PASSED ===" << std::endl;
        return 0;
    } else {
        std::cerr << "=== Some Metadata Tests FAILED ===" << std::endl;
        return 1;
    }
}
