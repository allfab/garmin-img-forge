/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tech-spec #2 Task 5 — CTest coverage for multi-geometry fields.
 *
 * Covers (cf. tech-spec-mpforge-multi-data-bdtopo-profiles.md Task 5):
 *   1. Happy path POLYLINE — Data0/Data1/Data2 emitted with correct coords
 *   2. Happy path POLYGON — Data0/Data1/Data2 emitted with closing rings
 *   3. POI mono-geom preserved even with MULTI_GEOM_FIELDS=YES
 *   4. Non-contiguous geoms — gaps in geom field indices produce no DataN
 *   5. Backward-compat — same output as mono-geom when CSL option absent
 *   6. Round-trip — write N geoms, read back with MULTI_GEOM_FIELDS=YES
 *   7. Error config — MAX_DATA_LEVEL=0 or 10 makes Create() fail
 *
 * Maps to tech-spec #2 AC3, AC7, AC8.
 ****************************************************************************/

#include <cstring>
#include <iostream>
#include <string>

#include "cpl_conv.h"
#include "cpl_string.h"
#include "cpl_vsi.h"
#include "gdal_priv.h"
#include "ogrsf_frmts.h"

#if defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wunused-result"
#endif

extern "C" void RegisterOGRPolishMap();

static int g_nPassed = 0;
static int g_nFailed = 0;

static void SetupTest() {
    // Prevent GDAL from loading the installed ogr_PolishMap.so plugin (which
    // may be an older build missing the multi-geom CSL options). We want to
    // exercise the linked-in sources registered below.
    CPLSetConfigOption("GDAL_DRIVER_PATH", "");
    RegisterOGRPolishMap();
    GDALAllRegister();
}

static CPLString TempFile(const char* pszPrefix) {
    CPLString os = CPLGenerateTempFilename(pszPrefix);
    os += ".mp";
    return os;
}

static std::string Slurp(const char* pszPath) {
    VSILFILE* fp = VSIFOpenL(pszPath, "rb");
    if (!fp) return {};
    VSIFSeekL(fp, 0, SEEK_END);
    vsi_l_offset n = VSIFTellL(fp);
    VSIFSeekL(fp, 0, SEEK_SET);
    std::string s; s.resize(static_cast<size_t>(n));
    VSIFReadL(&s[0], 1, static_cast<size_t>(n), fp);
    VSIFCloseL(fp);
    return s;
}

#define ASSERT_TRUE(cond, msg) do { \
    if (!(cond)) { std::cout << "    [FAIL] " << msg << std::endl; return false; } \
} while (0)

static char** MultiGeomOptions(int nMaxLevel) {
    char** opts = nullptr;
    opts = CSLSetNameValue(opts, "MULTI_GEOM_FIELDS", "YES");
    opts = CSLSetNameValue(opts, "MAX_DATA_LEVEL", CPLSPrintf("%d", nMaxLevel));
    return opts;
}

/* ----- Test 1 : happy path POLYLINE 3 geoms ----- */

static bool Test1_HappyPolyline() {
    std::cout << "  Test 1: HappyPolyline... ";
    CPLString osFile = TempFile("t1_polyline_multigeom");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    ASSERT_TRUE(poDriver != nullptr, "PolishMap driver not found");

    char** papszOpts = MultiGeomOptions(2);
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts);
    CSLDestroy(papszOpts);
    ASSERT_TRUE(poDS != nullptr, "Create() returned NULL with MULTI_GEOM_FIELDS=YES MAX_DATA_LEVEL=2");

    OGRLayer* poLayer = poDS->GetLayer(1);  // POLYLINE
    ASSERT_TRUE(poLayer != nullptr, "POLYLINE layer not found");
    ASSERT_TRUE(poLayer->GetLayerDefn()->GetGeomFieldCount() == 3,
                "POLYLINE defn should have 3 geom fields (primary + 2 additional)");

    OGRFeature* poFeat = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeat->SetField("Type", "0x16");

    OGRLineString oL0;
    oL0.addPoint(2.0, 48.0);
    oL0.addPoint(2.1, 48.1);
    oL0.addPoint(2.2, 48.2);
    poFeat->SetGeometry(&oL0);

    OGRLineString oL1;
    oL1.addPoint(2.0, 48.0);
    oL1.addPoint(2.2, 48.2);
    poFeat->SetGeomField(1, &oL1);

    OGRLineString oL2;
    oL2.addPoint(2.0, 48.0);
    oL2.addPoint(2.2, 48.2);
    poFeat->SetGeomField(2, &oL2);

    ASSERT_TRUE(poLayer->CreateFeature(poFeat) == OGRERR_NONE, "CreateFeature failed");
    OGRFeature::DestroyFeature(poFeat);
    GDALClose(poDS);

    std::string osContent = Slurp(osFile.c_str());
    ASSERT_TRUE(osContent.find("Data0=(48.000000,2.000000),(48.100000,2.100000),(48.200000,2.200000)")
                != std::string::npos, "Data0 line missing or wrong");
    ASSERT_TRUE(osContent.find("Data1=(48.000000,2.000000),(48.200000,2.200000)")
                != std::string::npos, "Data1 line missing or wrong");
    ASSERT_TRUE(osContent.find("Data2=(48.000000,2.000000),(48.200000,2.200000)")
                != std::string::npos, "Data2 line missing or wrong");

    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 2 : happy path POLYGON 3 geoms ----- */

static bool Test2_HappyPolygon() {
    std::cout << "  Test 2: HappyPolygon... ";
    CPLString osFile = TempFile("t2_polygon_multigeom");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    char** papszOpts = MultiGeomOptions(2);
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts);
    CSLDestroy(papszOpts);
    ASSERT_TRUE(poDS != nullptr, "Create() failed");

    OGRLayer* poLayer = poDS->GetLayer(2);  // POLYGON
    ASSERT_TRUE(poLayer->GetLayerDefn()->GetGeomFieldCount() == 3, "POLYGON defn should have 3 geom fields");

    OGRFeature* poFeat = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeat->SetField("Type", "0x4C");

    auto makeTri = [](double dfBase) {
        OGRPolygon* p = new OGRPolygon();
        OGRLinearRing* r = new OGRLinearRing();
        r->addPoint(2.0 + dfBase, 48.0);
        r->addPoint(2.1 + dfBase, 48.0);
        r->addPoint(2.05 + dfBase, 48.1);
        r->addPoint(2.0 + dfBase, 48.0);  // closed
        p->addRingDirectly(r);
        return p;
    };
    OGRPolygon* p0 = makeTri(0.0);
    OGRPolygon* p1 = makeTri(0.0);
    OGRPolygon* p2 = makeTri(0.0);
    poFeat->SetGeometryDirectly(p0);
    poFeat->SetGeomFieldDirectly(1, p1);
    poFeat->SetGeomFieldDirectly(2, p2);

    ASSERT_TRUE(poLayer->CreateFeature(poFeat) == OGRERR_NONE, "CreateFeature failed");
    OGRFeature::DestroyFeature(poFeat);
    GDALClose(poDS);

    std::string os = Slurp(osFile.c_str());
    ASSERT_TRUE(os.find("Data0=") != std::string::npos, "Data0 missing");
    ASSERT_TRUE(os.find("Data1=") != std::string::npos, "Data1 missing");
    ASSERT_TRUE(os.find("Data2=") != std::string::npos, "Data2 missing");

    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 3 : POI stays mono-geom ----- */

static bool Test3_PoiMonoGeomPreserved() {
    std::cout << "  Test 3: PoiMonoGeomPreserved... ";
    CPLString osFile = TempFile("t3_poi_mono");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    char** papszOpts = MultiGeomOptions(3);
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts);
    CSLDestroy(papszOpts);
    ASSERT_TRUE(poDS != nullptr, "Create() failed");

    OGRLayer* poPOI  = poDS->GetLayer(0);
    OGRLayer* poPL   = poDS->GetLayer(1);
    OGRLayer* poPG   = poDS->GetLayer(2);
    ASSERT_TRUE(poPOI->GetLayerDefn()->GetGeomFieldCount() == 1,
                "POI must stay mono-geom (MP spec §4.4.3.1)");
    ASSERT_TRUE(poPL->GetLayerDefn()->GetGeomFieldCount() == 4,  // 1 + 3
                "POLYLINE should expose primary + 3 additional");
    ASSERT_TRUE(poPG->GetLayerDefn()->GetGeomFieldCount() == 4,
                "POLYGON should expose primary + 3 additional");

    GDALClose(poDS);
    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 4 : non-contiguous geom fields ----- */

static bool Test4_NonContiguousGeoms() {
    std::cout << "  Test 4: NonContiguousGeoms... ";
    CPLString osFile = TempFile("t4_noncontig");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    char** papszOpts = MultiGeomOptions(3);
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts);
    CSLDestroy(papszOpts);
    OGRLayer* poLayer = poDS->GetLayer(1);

    OGRFeature* poFeat = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeat->SetField("Type", "0x16");

    OGRLineString oL0; oL0.addPoint(0.0, 0.0); oL0.addPoint(1.0, 1.0);
    OGRLineString oL2; oL2.addPoint(0.0, 0.0); oL2.addPoint(2.0, 2.0);
    poFeat->SetGeometry(&oL0);
    poFeat->SetGeomField(2, &oL2);  // index 1 intentionally left empty

    ASSERT_TRUE(poLayer->CreateFeature(poFeat) == OGRERR_NONE, "CreateFeature failed");
    OGRFeature::DestroyFeature(poFeat);
    GDALClose(poDS);

    std::string os = Slurp(osFile.c_str());
    ASSERT_TRUE(os.find("Data0=") != std::string::npos, "Data0 missing");
    ASSERT_TRUE(os.find("Data1=") == std::string::npos, "Data1 should NOT be emitted (gap)");
    ASSERT_TRUE(os.find("Data2=") != std::string::npos, "Data2 missing");

    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 5 : backward-compat sans option = 1 geom ----- */

static bool Test5_BackwardCompat() {
    std::cout << "  Test 5: BackwardCompat... ";
    CPLString osFile = TempFile("t5_compat");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    OGRLayer* poLayer = poDS->GetLayer(1);
    ASSERT_TRUE(poLayer->GetLayerDefn()->GetGeomFieldCount() == 1,
                "Without MULTI_GEOM_FIELDS, POLYLINE must have exactly 1 geom field");

    OGRFeature* poFeat = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeat->SetField("Type", "0x16");
    OGRLineString oL0; oL0.addPoint(2.0, 48.0); oL0.addPoint(2.1, 48.1);
    poFeat->SetGeometry(&oL0);
    ASSERT_TRUE(poLayer->CreateFeature(poFeat) == OGRERR_NONE, "CreateFeature failed");
    OGRFeature::DestroyFeature(poFeat);
    GDALClose(poDS);

    std::string os = Slurp(osFile.c_str());
    ASSERT_TRUE(os.find("Data0=") != std::string::npos, "Data0 missing");
    ASSERT_TRUE(os.find("Data1=") == std::string::npos,
                "Data1 emitted in legacy mode — backward-compat broken");

    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 6 : round-trip write → read multi-geom ----- */

static bool Test6_RoundTrip() {
    std::cout << "  Test 6: RoundTrip... ";
    CPLString osFile = TempFile("t6_roundtrip");
    VSIUnlink(osFile.c_str());

    // Écriture
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    char** papszOpts = MultiGeomOptions(2);
    GDALDataset* poDS = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts);
    CSLDestroy(papszOpts);
    OGRLayer* poLayer = poDS->GetLayer(1);

    // We need a valid [IMG ID] header to re-open. The driver requires ID.
    // Rely on default (Create provides Name but may miss ID — safeguard below).
    poDS->SetMetadataItem("Name", "Test");
    poDS->SetMetadataItem("ID", "12345");
    poDS->SetMetadataItem("CodePage", "1252");

    OGRFeature* poFeat = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeat->SetField("Type", "0x16");
    OGRLineString oL0; oL0.addPoint(2.0, 48.0); oL0.addPoint(2.5, 48.5);
    OGRLineString oL1; oL1.addPoint(2.0, 48.0); oL1.addPoint(2.5, 48.5);
    OGRLineString oL2; oL2.addPoint(2.0, 48.0); oL2.addPoint(2.5, 48.5);
    poFeat->SetGeometry(&oL0);
    poFeat->SetGeomField(1, &oL1);
    poFeat->SetGeomField(2, &oL2);
    ASSERT_TRUE(poLayer->CreateFeature(poFeat) == OGRERR_NONE, "CreateFeature failed");
    OGRFeature::DestroyFeature(poFeat);
    GDALClose(poDS);

    // Relecture en mode multi-geom explicite
    char** papszOpenOpts = nullptr;
    papszOpenOpts = CSLSetNameValue(papszOpenOpts, "MULTI_GEOM_FIELDS", "YES");
    papszOpenOpts = CSLSetNameValue(papszOpenOpts, "MAX_DATA_LEVEL", "2");
    GDALDataset* poIn = (GDALDataset*)GDALOpenEx(
        osFile.c_str(), GDAL_OF_VECTOR | GDAL_OF_READONLY,
        nullptr, papszOpenOpts, nullptr);
    CSLDestroy(papszOpenOpts);
    ASSERT_TRUE(poIn != nullptr, "GDALOpenEx failed (header probably missing ID)");

    OGRLayer* poPL = poIn->GetLayer(1);
    ASSERT_TRUE(poPL->GetLayerDefn()->GetGeomFieldCount() == 3,
                "Re-opened layer should have 3 geom fields");
    OGRFeature* poF = poPL->GetNextFeature();
    ASSERT_TRUE(poF != nullptr, "No feature read back");
    ASSERT_TRUE(poF->GetGeomFieldRef(0) != nullptr && !poF->GetGeomFieldRef(0)->IsEmpty(),
                "Primary geom empty");
    ASSERT_TRUE(poF->GetGeomFieldRef(1) != nullptr && !poF->GetGeomFieldRef(1)->IsEmpty(),
                "Additional geom 1 empty");
    ASSERT_TRUE(poF->GetGeomFieldRef(2) != nullptr && !poF->GetGeomFieldRef(2)->IsEmpty(),
                "Additional geom 2 empty");
    OGRFeature::DestroyFeature(poF);
    GDALClose(poIn);

    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

/* ----- Test 7 : erreur config ----- */

static bool Test7_ErrorConfig() {
    std::cout << "  Test 7: ErrorConfig... ";
    CPLString osFile = TempFile("t7_error");
    VSIUnlink(osFile.c_str());

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");

    // Silence expected error for this negative test
    CPLPushErrorHandler(CPLQuietErrorHandler);

    char** papszOpts1 = nullptr;
    papszOpts1 = CSLSetNameValue(papszOpts1, "MULTI_GEOM_FIELDS", "YES");
    papszOpts1 = CSLSetNameValue(papszOpts1, "MAX_DATA_LEVEL", "0");
    GDALDataset* poDS1 = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts1);
    CSLDestroy(papszOpts1);
    ASSERT_TRUE(poDS1 == nullptr, "MAX_DATA_LEVEL=0 should fail Create()");

    char** papszOpts2 = nullptr;
    papszOpts2 = CSLSetNameValue(papszOpts2, "MULTI_GEOM_FIELDS", "YES");
    papszOpts2 = CSLSetNameValue(papszOpts2, "MAX_DATA_LEVEL", "10");
    GDALDataset* poDS2 = poDriver->Create(osFile.c_str(), 0, 0, 0, GDT_Unknown, papszOpts2);
    CSLDestroy(papszOpts2);
    ASSERT_TRUE(poDS2 == nullptr, "MAX_DATA_LEVEL=10 should fail Create()");

    CPLPopErrorHandler();
    VSIUnlink(osFile.c_str());
    std::cout << "PASSED" << std::endl;
    return true;
}

static void Run(bool (*fn)()) {
    if (fn()) g_nPassed++;
    else       g_nFailed++;
}

int main() {
    SetupTest();
    std::cout << "=== Tech-spec #2 Task 5 — multi-geometry fields ===" << std::endl;
    Run(Test1_HappyPolyline);
    Run(Test2_HappyPolygon);
    Run(Test3_PoiMonoGeomPreserved);
    Run(Test4_NonContiguousGeoms);
    Run(Test5_BackwardCompat);
    Run(Test6_RoundTrip);
    Run(Test7_ErrorConfig);

    std::cout << "\n=== Test Summary ===" << std::endl;
    std::cout << "Passed: " << g_nPassed << std::endl;
    std::cout << "Failed: " << g_nFailed << std::endl;
    return (g_nFailed == 0) ? 0 : 1;
}
