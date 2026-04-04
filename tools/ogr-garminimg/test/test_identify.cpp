/******************************************************************************
 * Test: Identify
 * Verify the driver correctly identifies valid/invalid IMG files.
 ****************************************************************************/

#include "ogrgarminimgdriver.h"
#include "gdal_priv.h"
#include "cpl_vsi.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>

static int g_nTests = 0;
static int g_nPassed = 0;

#define TEST(cond, msg) do { \
    g_nTests++; \
    if (cond) { g_nPassed++; printf("  PASS: %s\n", msg); } \
    else { printf("  FAIL: %s\n", msg); } \
} while(0)

int main() {
    printf("=== Test: Identify ===\n");

    RegisterOGRGarminIMG();

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("GarminIMG");
    TEST(poDriver != nullptr, "Driver found");

    // Create a valid IMG file in memory
    {
        const char* pszPath = "/vsimem/test_valid.img";
        VSILFILE* fp = VSIFOpenL(pszPath, "wb");
        if (fp) {
            // Create minimal 512-byte header
            uint8_t header[512];
            memset(header, 0, sizeof(header));

            // Magic "DSKIMG\0" at offset 0x10
            memcpy(header + 0x10, "DSKIMG\0", 7);

            // "GARMIN\0" at offset 0x41
            memcpy(header + 0x41, "GARMIN\0", 7);

            // Block size exponents at 0x61-0x62
            header[0x61] = 0x09;  // exp1
            header[0x62] = 0x00;  // exp2 -> block_size = 2^9 = 512

            // Partition signature
            header[0x1FE] = 0x55;
            header[0x1FF] = 0xAA;

            VSIFWriteL(header, 1, sizeof(header), fp);
            VSIFCloseL(fp);

            // Test identification
            GDALOpenInfo oInfo(pszPath, GA_ReadOnly);
            int nResult = poDriver->pfnIdentify(&oInfo);
            TEST(nResult == TRUE, "Valid IMG file identified as TRUE");

            VSIUnlink(pszPath);
        }
    }

    // Create an invalid file (not IMG)
    {
        const char* pszPath = "/vsimem/test_invalid.img";
        VSILFILE* fp = VSIFOpenL(pszPath, "wb");
        if (fp) {
            const char* pszData = "This is not an IMG file";
            VSIFWriteL(pszData, 1, strlen(pszData), fp);
            VSIFCloseL(fp);

            GDALOpenInfo oInfo(pszPath, GA_ReadOnly);
            int nResult = poDriver->pfnIdentify(&oInfo);
            TEST(nResult == FALSE, "Non-IMG file identified as FALSE");

            VSIUnlink(pszPath);
        }
    }

    // Wrong extension
    {
        const char* pszPath = "/vsimem/test_file.txt";
        VSILFILE* fp = VSIFOpenL(pszPath, "wb");
        if (fp) {
            uint8_t header[512];
            memset(header, 0, sizeof(header));
            memcpy(header + 0x10, "DSKIMG\0", 7);
            VSIFWriteL(header, 1, sizeof(header), fp);
            VSIFCloseL(fp);

            GDALOpenInfo oInfo(pszPath, GA_ReadOnly);
            int nResult = poDriver->pfnIdentify(&oInfo);
            TEST(nResult == FALSE, "Wrong extension (.txt) identified as FALSE");

            VSIUnlink(pszPath);
        }
    }

    printf("\n=== Results: %d/%d passed ===\n", g_nPassed, g_nTests);
    return (g_nPassed == g_nTests) ? 0 : 1;
}
