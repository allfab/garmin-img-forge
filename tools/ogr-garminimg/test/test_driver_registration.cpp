/******************************************************************************
 * Test: Driver Registration
 * Verify GarminIMG driver registers correctly with GDAL driver manager.
 ****************************************************************************/

#include "ogrgarminimgdriver.h"
#include "gdal_priv.h"

#include <cstdio>
#include <cstdlib>

static int g_nTests = 0;
static int g_nPassed = 0;

#define TEST(cond, msg) do { \
    g_nTests++; \
    if (cond) { g_nPassed++; printf("  PASS: %s\n", msg); } \
    else { printf("  FAIL: %s\n", msg); } \
} while(0)

int main() {
    printf("=== Test: Driver Registration ===\n");

    // Register the driver
    RegisterOGRGarminIMG();

    // Test 1: Driver is found by name
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("GarminIMG");
    TEST(poDriver != nullptr, "GDALGetDriverByName('GarminIMG') != NULL");

    if (poDriver) {
        // Test 2: Long name
        const char* pszLongName = poDriver->GetMetadataItem(GDAL_DMD_LONGNAME);
        TEST(pszLongName != nullptr && strcmp(pszLongName, "Garmin IMG Format") == 0,
             "Long name is 'Garmin IMG Format'");

        // Test 3: Extension
        const char* pszExt = poDriver->GetMetadataItem(GDAL_DMD_EXTENSION);
        TEST(pszExt != nullptr && strcmp(pszExt, "img") == 0,
             "Extension is 'img'");

        // Test 4: Vector capability
        const char* pszVec = poDriver->GetMetadataItem(GDAL_DCAP_VECTOR);
        TEST(pszVec != nullptr && strcmp(pszVec, "YES") == 0,
             "GDAL_DCAP_VECTOR = YES");

        // Test 5: Create capability
        const char* pszCreate = poDriver->GetMetadataItem(GDAL_DCAP_CREATE);
        TEST(pszCreate != nullptr && strcmp(pszCreate, "YES") == 0,
             "GDAL_DCAP_CREATE = YES");

        // Test 6: Virtual IO
        const char* pszVIO = poDriver->GetMetadataItem(GDAL_DCAP_VIRTUALIO);
        TEST(pszVIO != nullptr && strcmp(pszVIO, "YES") == 0,
             "GDAL_DCAP_VIRTUALIO = YES");
    }

    // Test 7: Double registration is safe (no-op)
    RegisterOGRGarminIMG();
    int nDriverCount = 0;
    for (int i = 0; i < GetGDALDriverManager()->GetDriverCount(); i++) {
        if (strcmp(GetGDALDriverManager()->GetDriver(i)->GetDescription(),
                   "GarminIMG") == 0) {
            nDriverCount++;
        }
    }
    TEST(nDriverCount == 1, "Double registration is a no-op (only 1 driver)");

    printf("\n=== Results: %d/%d passed ===\n", g_nPassed, g_nTests);
    return (g_nPassed == g_nTests) ? 0 : 1;
}
