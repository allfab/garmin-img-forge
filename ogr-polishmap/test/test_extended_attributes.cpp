/******************************************************************************
 * Project:  OGR PolishMap Driver
 * Purpose:  Tests for extended attributes (read, write, alias mapping)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Copyright (c) 2026, mpforge contributors
 *
 * Tests:
 * 1. Read POI with extended attributes (CityName, RegionName, etc.)
 * 2. POI schema contains all POI-specific fields
 * 3. Read POLYLINE with extended attributes (DirIndicator, RoadID, SpeedType)
 * 4. POLYLINE schema has POLYLINE fields but NOT POI-specific fields
 * 5. Read POLYGON with extended attributes (CityName, RegionName, CountryName)
 * 6. POLYGON schema does NOT have StreetDesc, DirIndicator, RoadID, SpeedType, Zip
 * 7. Round-trip write/read POI with extended attributes
 * 8. Round-trip write/read POLYLINE with extended attributes
 * 9. CreateField alias mapping (NOM->Label, VILLE->CityName, PAYS->CountryName)
 * 10. CreateField: DirIndicator ignored on POI, accepted on POLYLINE
 * 11. Backward compatibility: existing files still readable
 ****************************************************************************/

#include <iostream>
#include <cstring>
#include <string>
#include "gdal_priv.h"
#include "ogrsf_frmts.h"
#include "cpl_conv.h"
#include "cpl_string.h"
#include "cpl_vsi.h"

extern "C" void RegisterOGRPolishMap();

#ifndef TEST_DATA_DIR
#define TEST_DATA_DIR "test/data"
#endif

static int g_nPassed = 0;
static int g_nFailed = 0;

#define CHECK(cond, msg) do { \
    if (!(cond)) { \
        std::cout << "[FAIL] " << (msg) << std::endl; \
        return false; \
    } else { \
        std::cout << "[OK] " << (msg) << std::endl; \
    } \
} while(0)

static void RunTest(const char* pszName, bool (*pfnTest)()) {
    std::cout << "\n=== Test: " << pszName << " ===" << std::endl;
    if (pfnTest()) {
        std::cout << "  PASSED" << std::endl;
        g_nPassed++;
    } else {
        std::cout << "  FAILED" << std::endl;
        g_nFailed++;
    }
}

/************************************************************************/
/* Test 1: Read POI with extended attributes                           */
/************************************************************************/
static bool Test_Read_POI_Extended_Attributes() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/poi-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open poi-extended-attrs.mp");

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    CHECK(poLayer != nullptr, "Get POI layer");

    OGRFeature* poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get first feature");

    CHECK(std::string(poFeature->GetFieldAsString("Type")) == "0x2C00", "Type is 0x2C00");
    CHECK(std::string(poFeature->GetFieldAsString("Label")) == "Restaurant Le Paris", "Label correct");
    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Paris", "CityName is Paris");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "Ile-de-France", "RegionName is Ile-de-France");
    CHECK(std::string(poFeature->GetFieldAsString("CountryName")) == "France", "CountryName is France");
    CHECK(std::string(poFeature->GetFieldAsString("StreetDesc")) == "Rue de Rivoli", "StreetDesc is Rue de Rivoli");
    CHECK(std::string(poFeature->GetFieldAsString("HouseNumber")) == "42", "HouseNumber is 42");
    CHECK(std::string(poFeature->GetFieldAsString("Zip")) == "75001", "Zip is 75001");
    CHECK(std::string(poFeature->GetFieldAsString("PhoneNumber")) == "+33 1 42 60 00 00", "PhoneNumber correct");

    OGRFeature::DestroyFeature(poFeature);

    // Second feature: partial attributes
    poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get second feature");
    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Lyon", "CityName is Lyon");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "Rhone-Alpes", "RegionName is Rhone-Alpes");
    CHECK(std::string(poFeature->GetFieldAsString("StreetDesc")) == "Place Bellecour", "StreetDesc correct");
    // CountryName not set on second feature
    CHECK(std::string(poFeature->GetFieldAsString("CountryName")) == "", "CountryName empty (not set)");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 2: POI schema contains all POI-specific fields                 */
/************************************************************************/
static bool Test_POI_Schema_Contains_Extended_Fields() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/poi-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open file");

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    CHECK(poLayer != nullptr, "Get POI layer");

    OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
    CHECK(poDefn->GetFieldCount() == 16, "POI has 16 fields");

    // POI-specific fields must be present
    CHECK(poDefn->GetFieldIndex("City") >= 0, "City field exists");
    CHECK(poDefn->GetFieldIndex("StreetDesc") >= 0, "StreetDesc field exists");
    CHECK(poDefn->GetFieldIndex("HouseNumber") >= 0, "HouseNumber field exists");
    CHECK(poDefn->GetFieldIndex("PhoneNumber") >= 0, "PhoneNumber field exists");
    CHECK(poDefn->GetFieldIndex("Highway") >= 0, "Highway field exists");
    CHECK(poDefn->GetFieldIndex("CityName") >= 0, "CityName field exists");
    CHECK(poDefn->GetFieldIndex("RegionName") >= 0, "RegionName field exists");
    CHECK(poDefn->GetFieldIndex("CountryName") >= 0, "CountryName field exists");
    CHECK(poDefn->GetFieldIndex("Zip") >= 0, "Zip field exists");
    CHECK(poDefn->GetFieldIndex("SubType") >= 0, "SubType field exists");
    CHECK(poDefn->GetFieldIndex("Marine") >= 0, "Marine field exists");

    // POLYLINE-specific fields must NOT be in POI schema
    CHECK(poDefn->GetFieldIndex("DirIndicator") < 0, "DirIndicator NOT in POI");
    CHECK(poDefn->GetFieldIndex("RoadID") < 0, "RoadID NOT in POI");
    CHECK(poDefn->GetFieldIndex("SpeedType") < 0, "SpeedType NOT in POI");

    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 3: Read POLYLINE with extended attributes                      */
/************************************************************************/
static bool Test_Read_POLYLINE_Extended_Attributes() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/polyline-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open polyline-extended-attrs.mp");

    OGRLayer* poLayer = poDS->GetLayerByName("POLYLINE");
    CHECK(poLayer != nullptr, "Get POLYLINE layer");

    OGRFeature* poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get first feature");

    CHECK(poFeature->GetFieldAsInteger("DirIndicator") == 1, "DirIndicator is 1");
    CHECK(std::string(poFeature->GetFieldAsString("RoadID")) == "RN7", "RoadID is RN7");
    CHECK(poFeature->GetFieldAsInteger("SpeedType") == 3, "SpeedType is 3");
    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Valence", "CityName is Valence");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "Drome", "RegionName is Drome");
    CHECK(std::string(poFeature->GetFieldAsString("Zip")) == "26000", "Zip is 26000");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 4: POLYLINE schema has POLYLINE fields but NOT POI-specific    */
/************************************************************************/
static bool Test_POLYLINE_Schema_No_POI_Fields() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/polyline-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open file");

    OGRLayer* poLayer = poDS->GetLayerByName("POLYLINE");
    CHECK(poLayer != nullptr, "Get POLYLINE layer");

    OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
    CHECK(poDefn->GetFieldCount() == 14, "POLYLINE has 14 fields");

    // POLYLINE-specific fields
    CHECK(poDefn->GetFieldIndex("DirIndicator") >= 0, "DirIndicator exists");
    CHECK(poDefn->GetFieldIndex("RoadID") >= 0, "RoadID exists");
    CHECK(poDefn->GetFieldIndex("SpeedType") >= 0, "SpeedType exists");
    CHECK(poDefn->GetFieldIndex("Zip") >= 0, "Zip exists");

    // POI-specific fields NOT in POLYLINE
    CHECK(poDefn->GetFieldIndex("City") < 0, "City NOT in POLYLINE");
    CHECK(poDefn->GetFieldIndex("StreetDesc") < 0, "StreetDesc NOT in POLYLINE");
    CHECK(poDefn->GetFieldIndex("HouseNumber") < 0, "HouseNumber NOT in POLYLINE");
    CHECK(poDefn->GetFieldIndex("PhoneNumber") < 0, "PhoneNumber NOT in POLYLINE");
    CHECK(poDefn->GetFieldIndex("Highway") < 0, "Highway NOT in POLYLINE");

    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 5: Read POLYGON with extended attributes                       */
/************************************************************************/
static bool Test_Read_POLYGON_Extended_Attributes() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/polygon-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open polygon-extended-attrs.mp");

    OGRLayer* poLayer = poDS->GetLayerByName("POLYGON");
    CHECK(poLayer != nullptr, "Get POLYGON layer");

    OGRFeature* poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get first feature");

    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Lyon", "CityName is Lyon");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "Rhone-Alpes", "RegionName is Rhone-Alpes");
    CHECK(std::string(poFeature->GetFieldAsString("CountryName")) == "France", "CountryName is France");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 6: POLYGON schema does NOT have layer-specific fields          */
/************************************************************************/
static bool Test_POLYGON_Schema_No_Specific_Fields() {
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/polygon-extended-attrs.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open file");

    OGRLayer* poLayer = poDS->GetLayerByName("POLYGON");
    CHECK(poLayer != nullptr, "Get POLYGON layer");

    OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
    CHECK(poDefn->GetFieldCount() == 10, "POLYGON has 10 fields");

    // Common fields present
    CHECK(poDefn->GetFieldIndex("CityName") >= 0, "CityName exists");
    CHECK(poDefn->GetFieldIndex("RegionName") >= 0, "RegionName exists");
    CHECK(poDefn->GetFieldIndex("CountryName") >= 0, "CountryName exists");

    // NOT present in POLYGON
    CHECK(poDefn->GetFieldIndex("StreetDesc") < 0, "StreetDesc NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("DirIndicator") < 0, "DirIndicator NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("RoadID") < 0, "RoadID NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("SpeedType") < 0, "SpeedType NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("Zip") < 0, "Zip NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("City") < 0, "City NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("HouseNumber") < 0, "HouseNumber NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("PhoneNumber") < 0, "PhoneNumber NOT in POLYGON");
    CHECK(poDefn->GetFieldIndex("Highway") < 0, "Highway NOT in POLYGON");

    GDALClose(poDS);
    return true;
}

/************************************************************************/
/* Test 7: Round-trip write/read POI with extended attributes           */
/************************************************************************/
static bool Test_Roundtrip_POI_Extended() {
    CPLString osTempFile = CPLGenerateTempFilename("test_ext_poi");
    osTempFile += ".mp";

    // Create and write
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    CHECK(poDriver != nullptr, "Get PolishMap driver");

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    CHECK(poDS != nullptr, "Create output dataset");

    OGRLayer* poLayer = poDS->CreateLayer("POI", nullptr, wkbPoint);
    CHECK(poLayer != nullptr, "Create POI layer");

    OGRFeature* poFeature = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x2C00");
    poFeature->SetField("Label", "Test POI");
    poFeature->SetField("CityName", "Marseille");
    poFeature->SetField("RegionName", "PACA");
    poFeature->SetField("CountryName", "France");
    poFeature->SetField("StreetDesc", "La Canebiere");
    poFeature->SetField("HouseNumber", "10");
    poFeature->SetField("Zip", "13001");
    poFeature->SetField("PhoneNumber", "+33 4 91 00 00 00");

    OGRPoint oPoint(5.3698, 43.2965);
    poFeature->SetGeometry(&oPoint);
    CHECK(poLayer->CreateFeature(poFeature) == OGRERR_NONE, "CreateFeature succeeds");
    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);

    // Read back
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Reopen output file");

    poLayer = poDS->GetLayerByName("POI");
    CHECK(poLayer != nullptr, "Get POI layer on reread");

    poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get feature on reread");

    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Marseille", "CityName roundtrip: Marseille");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "PACA", "RegionName roundtrip: PACA");
    CHECK(std::string(poFeature->GetFieldAsString("CountryName")) == "France", "CountryName roundtrip: France");
    CHECK(std::string(poFeature->GetFieldAsString("StreetDesc")) == "La Canebiere", "StreetDesc roundtrip");
    CHECK(std::string(poFeature->GetFieldAsString("HouseNumber")) == "10", "HouseNumber roundtrip");
    CHECK(std::string(poFeature->GetFieldAsString("Zip")) == "13001", "Zip roundtrip");
    CHECK(std::string(poFeature->GetFieldAsString("PhoneNumber")) == "+33 4 91 00 00 00", "PhoneNumber roundtrip");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    VSIUnlink(osTempFile.c_str());
    return true;
}

/************************************************************************/
/* Test 8: Round-trip write/read POLYLINE with extended attributes      */
/************************************************************************/
static bool Test_Roundtrip_POLYLINE_Extended() {
    CPLString osTempFile = CPLGenerateTempFilename("test_ext_polyline");
    osTempFile += ".mp";

    // Create and write
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    CHECK(poDriver != nullptr, "Get PolishMap driver");

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    CHECK(poDS != nullptr, "Create output dataset");

    OGRLayer* poLayer = poDS->CreateLayer("POLYLINE", nullptr, wkbLineString);
    CHECK(poLayer != nullptr, "Create POLYLINE layer");

    OGRFeature* poFeature = OGRFeature::CreateFeature(poLayer->GetLayerDefn());
    poFeature->SetField("Type", "0x0002");
    poFeature->SetField("Label", "Test Road");
    poFeature->SetField("DirIndicator", 1);
    poFeature->SetField("RoadID", "D42");
    poFeature->SetField("SpeedType", 4);
    poFeature->SetField("CityName", "Valence");
    poFeature->SetField("Zip", "26000");

    OGRLineString oLine;
    oLine.addPoint(4.891700, 44.933300);
    oLine.addPoint(4.898800, 44.942200);
    poFeature->SetGeometry(&oLine);
    CHECK(poLayer->CreateFeature(poFeature) == OGRERR_NONE, "CreateFeature succeeds");
    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);

    // Read back
    poDS = GDALDataset::Open(osTempFile.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Reopen output file");

    poLayer = poDS->GetLayerByName("POLYLINE");
    CHECK(poLayer != nullptr, "Get POLYLINE layer on reread");

    poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get feature on reread");

    CHECK(poFeature->GetFieldAsInteger("DirIndicator") == 1, "DirIndicator roundtrip: 1");
    CHECK(std::string(poFeature->GetFieldAsString("RoadID")) == "D42", "RoadID roundtrip: D42");
    CHECK(poFeature->GetFieldAsInteger("SpeedType") == 4, "SpeedType roundtrip: 4");
    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "Valence", "CityName roundtrip: Valence");
    CHECK(std::string(poFeature->GetFieldAsString("Zip")) == "26000", "Zip roundtrip: 26000");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    VSIUnlink(osTempFile.c_str());
    return true;
}

/************************************************************************/
/* Test 9: CreateField alias mapping                                   */
/************************************************************************/
static bool Test_CreateField_Alias_Mapping() {
    CPLString osTempFile = CPLGenerateTempFilename("test_alias");
    osTempFile += ".mp";

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    CHECK(poDriver != nullptr, "Get PolishMap driver");

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    CHECK(poDS != nullptr, "Create output dataset");

    OGRLayer* poLayer = poDS->CreateLayer("POI", nullptr, wkbPoint);
    CHECK(poLayer != nullptr, "Create POI layer");

    // Test alias mapping: NOM -> Label
    OGRFieldDefn oFieldNom("NOM", OFTString);
    CHECK(poLayer->CreateField(&oFieldNom) == OGRERR_NONE, "CreateField NOM accepted");

    // Test alias mapping: VILLE -> CityName
    OGRFieldDefn oFieldVille("VILLE", OFTString);
    CHECK(poLayer->CreateField(&oFieldVille) == OGRERR_NONE, "CreateField VILLE accepted");

    // Test alias mapping: PAYS -> CountryName
    OGRFieldDefn oFieldPays("PAYS", OFTString);
    CHECK(poLayer->CreateField(&oFieldPays) == OGRERR_NONE, "CreateField PAYS accepted");

    // Test alias mapping: TELEPHONE -> PhoneNumber
    OGRFieldDefn oFieldTel("TELEPHONE", OFTString);
    CHECK(poLayer->CreateField(&oFieldTel) == OGRERR_NONE, "CreateField TELEPHONE accepted");

    // Test alias mapping: CODE_POSTAL -> Zip
    OGRFieldDefn oFieldZip("CODE_POSTAL", OFTString);
    CHECK(poLayer->CreateField(&oFieldZip) == OGRERR_NONE, "CreateField CODE_POSTAL accepted");

    // Unknown field: silently ignored
    OGRFieldDefn oFieldUnknown("RANDOM_FIELD", OFTString);
    CHECK(poLayer->CreateField(&oFieldUnknown) == OGRERR_NONE, "CreateField RANDOM_FIELD accepted (ignored)");

    GDALClose(poDS);
    VSIUnlink(osTempFile.c_str());
    return true;
}

/************************************************************************/
/* Test 10: CreateField layer-specificity                              */
/************************************************************************/
static bool Test_CreateField_Layer_Specificity() {
    CPLString osTempFile = CPLGenerateTempFilename("test_layer_spec");
    osTempFile += ".mp";

    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    CHECK(poDriver != nullptr, "Get PolishMap driver");

    GDALDataset* poDS = poDriver->Create(osTempFile.c_str(), 0, 0, 0, GDT_Unknown, nullptr);
    CHECK(poDS != nullptr, "Create output dataset");

    // POI layer: DirIndicator should be ignored (POLYLINE-only)
    OGRLayer* poPOILayer = poDS->CreateLayer("POI", nullptr, wkbPoint);
    CHECK(poPOILayer != nullptr, "Create POI layer");

    OGRFieldDefn oFieldDir("DirIndicator", OFTInteger);
    CHECK(poPOILayer->CreateField(&oFieldDir) == OGRERR_NONE, "CreateField DirIndicator on POI accepted (but ignored)");

    // POLYLINE layer: DirIndicator should be accepted
    OGRLayer* poPolylineLayer = poDS->CreateLayer("POLYLINE", nullptr, wkbLineString);
    CHECK(poPolylineLayer != nullptr, "Create POLYLINE layer");

    CHECK(poPolylineLayer->CreateField(&oFieldDir) == OGRERR_NONE, "CreateField DirIndicator on POLYLINE accepted");

    // StreetDesc on POLYLINE should be ignored (POI-only)
    OGRFieldDefn oFieldStreet("StreetDesc", OFTString);
    CHECK(poPolylineLayer->CreateField(&oFieldStreet) == OGRERR_NONE, "CreateField StreetDesc on POLYLINE accepted (but ignored)");

    GDALClose(poDS);
    VSIUnlink(osTempFile.c_str());
    return true;
}

/************************************************************************/
/* Test 11: Backward compatibility                                     */
/************************************************************************/
static bool Test_Backward_Compatibility() {
    // Open an existing simple file (no extended attributes) - should still work
    std::string osPath = std::string(TEST_DATA_DIR) + "/valid-minimal/poi-simple.mp";
    GDALDataset* poDS = GDALDataset::Open(osPath.c_str(), GDAL_OF_VECTOR);
    CHECK(poDS != nullptr, "Open existing poi-simple.mp");

    OGRLayer* poLayer = poDS->GetLayerByName("POI");
    CHECK(poLayer != nullptr, "Get POI layer");

    OGRFeatureDefn* poDefn = poLayer->GetLayerDefn();
    CHECK(poDefn->GetFieldCount() == 16, "POI now has 16 fields (extended schema)");

    OGRFeature* poFeature = poLayer->GetNextFeature();
    CHECK(poFeature != nullptr, "Get feature");

    // Core fields still work
    CHECK(std::string(poFeature->GetFieldAsString("Type")) != "", "Type field readable");
    CHECK(std::string(poFeature->GetFieldAsString("Label")) != "", "Label field readable");

    // Extended fields are empty (not set in old file)
    CHECK(std::string(poFeature->GetFieldAsString("CityName")) == "", "CityName empty (not in old file)");
    CHECK(std::string(poFeature->GetFieldAsString("RegionName")) == "", "RegionName empty (not in old file)");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);
    return true;
}

/************************************************************************/
/*                            main()                                    */
/************************************************************************/

int main() {
    GDALAllRegister();
    RegisterOGRPolishMap();

    std::cout << "========================================" << std::endl;
    std::cout << "  OGR PolishMap - Extended Attributes Tests" << std::endl;
    std::cout << "========================================" << std::endl;

    RunTest("Read POI Extended Attributes", Test_Read_POI_Extended_Attributes);
    RunTest("POI Schema Contains Extended Fields", Test_POI_Schema_Contains_Extended_Fields);
    RunTest("Read POLYLINE Extended Attributes", Test_Read_POLYLINE_Extended_Attributes);
    RunTest("POLYLINE Schema No POI Fields", Test_POLYLINE_Schema_No_POI_Fields);
    RunTest("Read POLYGON Extended Attributes", Test_Read_POLYGON_Extended_Attributes);
    RunTest("POLYGON Schema No Specific Fields", Test_POLYGON_Schema_No_Specific_Fields);
    RunTest("Roundtrip POI Extended", Test_Roundtrip_POI_Extended);
    RunTest("Roundtrip POLYLINE Extended", Test_Roundtrip_POLYLINE_Extended);
    RunTest("CreateField Alias Mapping", Test_CreateField_Alias_Mapping);
    RunTest("CreateField Layer Specificity", Test_CreateField_Layer_Specificity);
    RunTest("Backward Compatibility", Test_Backward_Compatibility);

    std::cout << "\n========================================" << std::endl;
    std::cout << "Test Summary:" << std::endl;
    std::cout << "  Passed: " << g_nPassed << std::endl;
    std::cout << "  Failed: " << g_nFailed << std::endl;
    std::cout << "========================================" << std::endl;

    return g_nFailed > 0 ? 1 : 0;
}
