/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Test driver registration
 * Author:   mpforge project
 *
 ******************************************************************************
 * Test that the PolishMap driver can be registered and is available.
 ****************************************************************************/

#include "gdal_priv.h"
#include "cpl_conv.h"
#include <iostream>

// Declare the driver registration function
extern "C" void RegisterOGRPolishMap();

int main() {
    std::cout << "=== OGR PolishMap Driver Registration Test ===" << std::endl;

    // Initialize GDAL
    GDALAllRegister();

    // Register our driver
    std::cout << "Registering PolishMap driver..." << std::endl;
    RegisterOGRPolishMap();

    // Check if driver is registered
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        std::cerr << "ERROR: PolishMap driver not found!" << std::endl;
        return 1;
    }

    std::cout << "[OK] PolishMap driver registered successfully" << std::endl;

    // Display driver metadata
    std::cout << "\nDriver Metadata:" << std::endl;
    std::cout << "  Description: " << poDriver->GetDescription() << std::endl;

    const char* pszLongName = poDriver->GetMetadataItem(GDAL_DMD_LONGNAME);
    if (pszLongName) {
        std::cout << "  Long Name: " << pszLongName << std::endl;
    }

    const char* pszExtension = poDriver->GetMetadataItem(GDAL_DMD_EXTENSION);
    if (pszExtension) {
        std::cout << "  Extension: " << pszExtension << std::endl;
    }

    const char* pszVectorCap = poDriver->GetMetadataItem(GDAL_DCAP_VECTOR);
    if (pszVectorCap && EQUAL(pszVectorCap, "YES")) {
        std::cout << "  Vector Capability: YES" << std::endl;
    }

    // Test Identify() method with a .mp file
    std::cout << "\nTesting Identify() method:" << std::endl;

    // Create a temporary .mp file for testing (cross-platform)
    CPLString osTempFile = CPLGenerateTempFilename("ogr_polishmap_test");
    osTempFile += ".mp";
    const char* pszTestFile = osTempFile.c_str();

    FILE* fp = fopen(pszTestFile, "w");
    if (fp) {
        fprintf(fp, "[IMG ID]\nTest=1\n");
        fclose(fp);
    }

    GDALOpenInfo oOpenInfo(pszTestFile, GDAL_OF_VECTOR | GDAL_OF_READONLY);

    int nIdentify = poDriver->pfnIdentify(&oOpenInfo);
    if (nIdentify) {
        std::cout << "  [OK] Identify() correctly recognizes .mp extension" << std::endl;
    } else {
        std::cerr << "  ERROR: Identify() failed to recognize .mp extension" << std::endl;
        VSIUnlink(pszTestFile);
        return 1;
    }

    // Test with non-.mp file (cross-platform)
    CPLString osTempFileTxt = CPLGenerateTempFilename("ogr_polishmap_test");
    osTempFileTxt += ".txt";
    const char* pszWrongFile = osTempFileTxt.c_str();

    FILE* fp2 = fopen(pszWrongFile, "w");
    if (fp2) {
        fprintf(fp2, "Not a Polish Map file\n");
        fclose(fp2);
    }

    GDALOpenInfo oOpenInfo2(pszWrongFile, GDAL_OF_VECTOR | GDAL_OF_READONLY);
    int nIdentify2 = poDriver->pfnIdentify(&oOpenInfo2);
    if (!nIdentify2) {
        std::cout << "  [OK] Identify() correctly rejects non-.mp extension" << std::endl;
    } else {
        std::cerr << "  ERROR: Identify() incorrectly accepted non-.mp extension" << std::endl;
        VSIUnlink(pszTestFile);
        VSIUnlink(pszWrongFile);
        return 1;
    }

    // Cleanup temporary files (cross-platform)
    VSIUnlink(pszTestFile);
    VSIUnlink(pszWrongFile);

    std::cout << "\n=== All Tests PASSED ===" << std::endl;
    return 0;
}
