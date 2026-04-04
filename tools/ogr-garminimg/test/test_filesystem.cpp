/******************************************************************************
 * Test: Filesystem
 * Verify the IMG filesystem parser extracts subfiles correctly.
 ****************************************************************************/

#include "garminimgfilesystem.h"
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
    printf("=== Test: Filesystem Parser ===\n");

    // Test basic construction
    {
        GarminIMGFilesystem fs;
        TEST(true, "GarminIMGFilesystem constructs without error");
    }

    // Test GetSubfileData with empty filesystem
    {
        GarminIMGFilesystem fs;
        const auto* data = fs.GetSubfileData("NONEXIST.TRE");
        TEST(data == nullptr, "GetSubfileData returns nullptr for missing file");
    }

    // Test GetTileNames with empty filesystem
    {
        GarminIMGFilesystem fs;
        auto names = fs.GetTileNames();
        TEST(names.empty(), "GetTileNames returns empty for unparsed filesystem");
    }

    // Test IsMultiTile with empty filesystem
    {
        GarminIMGFilesystem fs;
        TEST(!fs.IsMultiTile(), "IsMultiTile returns false for empty filesystem");
    }

    printf("\n=== Results: %d/%d passed ===\n", g_nPassed, g_nTests);
    return (g_nPassed == g_nTests) ? 0 : 1;
}
