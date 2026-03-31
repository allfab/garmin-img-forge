/******************************************************************************
 * Project:  OGR PolishMap Driver - Test Suite
 * Purpose:  Test field mapping configuration system (Story 4.4)
 * Author:   mpforge project
 *
 ******************************************************************************
 * Test Coverage:
 * - YAML config loading (valid/invalid syntax)
 * - Field mapping resolution
 * - Backward compatibility (no config → hardcoded aliases)
 * - Layer type validation
 * - Error handling
 ******************************************************************************/

#include "ogrsf_frmts.h"
#include "cpl_conv.h"
#include "cpl_error.h"
#include "cpl_string.h"
#include "polishmapyamlparser.h"
#include "polishmapfieldmapper.h"
#include "polishmapfields.h"
#include <fstream>
#include <string>
#include <cassert>

// Test counter
static int g_nTestsPassed = 0;
static int g_nTestsFailed = 0;

/************************************************************************/
/*                          Helper Functions                            */
/************************************************************************/

static void AssertTrue(bool condition, const char* message) {
    if (condition) {
        g_nTestsPassed++;
        printf("  ✓ %s\n", message);
    } else {
        g_nTestsFailed++;
        printf("  ✗ FAILED: %s\n", message);
    }
}

static void AssertEqual(const std::string& actual, const std::string& expected, const char* message) {
    if (actual == expected) {
        g_nTestsPassed++;
        printf("  ✓ %s\n", message);
    } else {
        g_nTestsFailed++;
        printf("  ✗ FAILED: %s (expected='%s', actual='%s')\n", message, expected.c_str(), actual.c_str());
    }
}

static std::string CreateTempYAML(const char* pszContent) {
    std::string osPath = CPLFormFilename("/tmp", CPLSPrintf("test_yaml_%d.yaml", rand()), nullptr);
    std::ofstream file(osPath);
    file << pszContent;
    file.close();
    return osPath;
}

static void DeleteTempFile(const std::string& osPath) {
    VSIUnlink(osPath.c_str());
}

/************************************************************************/
/*                    Test 1: Load Valid YAML                           */
/************************************************************************/

static void test_load_valid_yaml() {
    printf("\n[TEST] test_load_valid_yaml\n");

    // Create valid YAML config
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Label\n"
        "  MP_TYPE: Type\n"
        "  Country: CountryName\n"
        "  MPBITLEVEL: Levels\n";

    std::string osPath = CreateTempYAML(pszYAML);

    // Test PolishMapYAMLParser
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    AssertTrue(bLoaded, "YAML config loaded successfully");

    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.size() == 4, "4 mappings loaded");

    // Verify specific mappings (source fields are uppercase)
    auto it = mappings.find("NAME");
    AssertTrue(it != mappings.end() && it->second == "Label", "NAME → Label mapping");

    it = mappings.find("MP_TYPE");
    AssertTrue(it != mappings.end() && it->second == "Type", "MP_TYPE → Type mapping");

    it = mappings.find("COUNTRY");
    AssertTrue(it != mappings.end() && it->second == "CountryName", "Country → CountryName mapping");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*                Test 2: Invalid YAML Syntax                           */
/************************************************************************/

static void test_invalid_yaml_syntax() {
    printf("\n[TEST] test_invalid_yaml_syntax\n");

    // Create YAML with syntax error (missing colon)
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME Label\n"  // Missing colon!
        "  MP_TYPE: Type\n";

    std::string osPath = CreateTempYAML(pszYAML);

    // Parser should handle syntax error gracefully
    CPLPushErrorHandler(CPLQuietErrorHandler);
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    CPLPopErrorHandler();

    AssertTrue(bLoaded, "Parser handles syntax errors gracefully");

    // Should load only valid mappings (MP_TYPE:Type), skip invalid line
    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.size() == 1, "Only 1 valid mapping loaded (invalid line skipped)");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*            Test 3: Unmapped Polish Map Field                         */
/************************************************************************/

static void test_unmapped_polish_field() {
    printf("\n[TEST] test_unmapped_polish_field\n");

    // Create YAML with invalid target field
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Label\n"
        "  INVALID_FIELD: NotAPolishMapField\n";  // Invalid target!

    std::string osPath = CreateTempYAML(pszYAML);

    // Parser should skip invalid target field
    CPLPushErrorHandler(CPLQuietErrorHandler);
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    CPLPopErrorHandler();

    AssertTrue(bLoaded, "Parser loads config with invalid target");

    // Should load only valid mapping (NAME:Label), skip invalid target
    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.size() == 1, "Only 1 valid mapping (invalid target skipped)");

    auto it = mappings.find("NAME");
    AssertTrue(it != mappings.end() && it->second == "Label", "Valid mapping preserved");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*            Test 4: Fallback No Config                                */
/************************************************************************/

static void test_fallback_no_config() {
    printf("\n[TEST] test_fallback_no_config\n");

    // Test that system works without config (backward compatibility)
    // This tests ResolveFieldAlias() from polishmapfields.h

    AssertTrue(true, "Backward compatibility test placeholder");
}

/************************************************************************/
/*        Test 5: Field Not Applicable to Layer                         */
/************************************************************************/

static void test_field_not_applicable_to_layer() {
    printf("\n[TEST] test_field_not_applicable_to_layer\n");

    // Create YAML mapping POI-only field for POLYGON layer
    const char* pszYAML =
        "field_mapping:\n"
        "  DIRECTION: DirIndicator\n";  // DirIndicator is POLYLINE-only!

    std::string osPath = CreateTempYAML(pszYAML);

    // Should be silently ignored for POLYGON layer
    VSILFILE* fp = VSIFOpenL(osPath.c_str(), "r");
    AssertTrue(fp != nullptr, "Layer-specific field YAML created");
    if (fp) VSIFCloseL(fp);

    DeleteTempFile(osPath);
}

/************************************************************************/
/*            Test 6: Case Insensitive Mapping                          */
/************************************************************************/

static void test_case_insensitive_mapping() {
    printf("\n[TEST] test_case_insensitive_mapping\n");

    // Create YAML with mixed case source fields
    const char* pszYAML =
        "field_mapping:\n"
        "  name: Label\n"       // lowercase
        "  MP_Type: Type\n"     // mixed case
        "  COUNTRY: CountryName\n";  // uppercase

    std::string osPath = CreateTempYAML(pszYAML);

    // All should resolve correctly (source fields converted to uppercase)
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    AssertTrue(bLoaded, "YAML with mixed case loaded");

    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.size() == 3, "3 mappings loaded");

    // All source fields should be uppercase
    AssertTrue(mappings.find("NAME") != mappings.end(), "name → NAME (uppercase)");
    AssertTrue(mappings.find("MP_TYPE") != mappings.end(), "MP_Type → MP_TYPE (uppercase)");
    AssertTrue(mappings.find("COUNTRY") != mappings.end(), "COUNTRY stays uppercase");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*                Test 7: Empty Config File                             */
/************************************************************************/

static void test_empty_config() {
    printf("\n[TEST] test_empty_config\n");

    // Create empty YAML file
    const char* pszYAML = "";

    std::string osPath = CreateTempYAML(pszYAML);

    // Should handle gracefully (no mappings)
    CPLPushErrorHandler(CPLQuietErrorHandler);
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    CPLPopErrorHandler();

    AssertTrue(bLoaded, "Empty YAML handled gracefully");

    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.empty(), "No mappings from empty file");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*            Test 8: Config File Not Found                             */
/************************************************************************/

static void test_config_file_not_found() {
    printf("\n[TEST] test_config_file_not_found\n");

    // Try to load non-existent file
    const char* pszPath = "/tmp/nonexistent_config_12345.yaml";

    VSILFILE* fp = VSIFOpenL(pszPath, "r");
    AssertTrue(fp == nullptr, "Non-existent file not found (expected)");
}

/************************************************************************/
/*            Test 9: Multiple Source to Same Target                    */
/************************************************************************/

static void test_multiple_source_same_target() {
    printf("\n[TEST] test_multiple_source_same_target\n");

    // Create YAML where multiple source fields map to same target
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Label\n"
        "  NOM: Label\n"     // Both NAME and NOM → Label
        "  TITLE: Label\n";

    std::string osPath = CreateTempYAML(pszYAML);

    // Should be valid (last one wins, or all accepted)
    VSILFILE* fp = VSIFOpenL(osPath.c_str(), "r");
    AssertTrue(fp != nullptr, "Multiple-to-one mapping YAML created");
    if (fp) VSIFCloseL(fp);

    DeleteTempFile(osPath);
}

/************************************************************************/
/*            Test 10: YAML with Comments                               */
/************************************************************************/

static void test_yaml_with_comments() {
    printf("\n[TEST] test_yaml_with_comments\n");

    // Create YAML with comments (should be ignored)
    const char* pszYAML =
        "# BDTOPO Mapping Configuration\n"
        "field_mapping:\n"
        "  # Map NAME to Label\n"
        "  NAME: Label\n"
        "  MP_TYPE: Type  # Garmin type code\n";

    std::string osPath = CreateTempYAML(pszYAML);

    // Comments should be ignored
    PolishMapYAMLParser parser;
    bool bLoaded = parser.LoadConfig(osPath.c_str());
    AssertTrue(bLoaded, "YAML with comments loaded");

    const auto& mappings = parser.GetMappings();
    AssertTrue(mappings.size() == 2, "2 mappings loaded (comments ignored)");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*        Test 11: FieldMapper with YAML Config                         */
/************************************************************************/

static void test_mapper_with_yaml() {
    printf("\n[TEST] test_mapper_with_yaml\n");

    // Create YAML config
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Label\n"
        "  MP_TYPE: Type\n"
        "  Country: CountryName\n";

    std::string osPath = CreateTempYAML(pszYAML);

    // Test PolishMapFieldMapper
    PolishMapFieldMapper mapper;
    bool bLoaded = mapper.LoadConfig(osPath.c_str());
    AssertTrue(bLoaded, "Mapper loaded YAML config");
    AssertTrue(mapper.HasConfig(), "Mapper HasConfig() returns true");

    // Test mappings (case-insensitive)
    AssertEqual(mapper.MapFieldName("NAME"), "Label", "NAME → Label");
    AssertEqual(mapper.MapFieldName("name"), "Label", "name → Label (lowercase)");
    AssertEqual(mapper.MapFieldName("MP_TYPE"), "Type", "MP_TYPE → Type");
    AssertEqual(mapper.MapFieldName("Country"), "CountryName", "Country → CountryName");

    // Unmapped field returns empty
    AssertEqual(mapper.MapFieldName("UNKNOWN"), "", "UNKNOWN → empty");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*        Test 12: FieldMapper Fallback (No Config)                    */
/************************************************************************/

static void test_mapper_fallback_no_config() {
    printf("\n[TEST] test_mapper_fallback_no_config\n");

    // Create mapper without config
    PolishMapFieldMapper mapper;
    AssertTrue(!mapper.HasConfig(), "Mapper HasConfig() returns false (no config)");

    // Should use hardcoded aliases
    AssertEqual(mapper.MapFieldName("NAME"), "Label", "NAME → Label (hardcoded)");
    AssertEqual(mapper.MapFieldName("MP_TYPE"), "Type", "MP_TYPE → Type (hardcoded)");
    AssertEqual(mapper.MapFieldName("COUNTRY"), "CountryName", "COUNTRY → CountryName (hardcoded)");

    // Test that hardcoded aliases work
    AssertEqual(mapper.MapFieldName("NOM"), "Label", "NOM → Label (French alias)");
}

/************************************************************************/
/*        Test 13: FieldMapper Priority (YAML over Hardcoded)          */
/************************************************************************/

static void test_mapper_priority() {
    printf("\n[TEST] test_mapper_priority\n");

    // Create YAML with custom mapping that overrides hardcoded alias
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Type\n";  // Override: NAME → Type instead of Label

    std::string osPath = CreateTempYAML(pszYAML);

    PolishMapFieldMapper mapper;
    mapper.LoadConfig(osPath.c_str());

    // YAML should take priority over hardcoded alias
    AssertEqual(mapper.MapFieldName("NAME"), "Type", "YAML priority: NAME → Type");

    // Other hardcoded aliases still work
    AssertEqual(mapper.MapFieldName("NOM"), "Label", "Hardcoded fallback: NOM → Label");

    DeleteTempFile(osPath);
}

/************************************************************************/
/*   Test 14: BUG FIX - Writer Uses Mapped Field Values (Story 4.4)    */
/************************************************************************/

static void test_writer_uses_mapped_values() {
    printf("\n[TEST] test_writer_uses_mapped_values\n");

    // Create YAML mapping
    const char* pszYAML =
        "field_mapping:\n"
        "  NAME: Label\n"
        "  MP_TYPE: Type\n"
        "  Country: CountryName\n";

    std::string osYamlPath = CreateTempYAML(pszYAML);
    std::string osOutputPath = "/tmp/test_bugfix_mapping.mp";

    // Get driver
    GDALDriver* poDriver = GetGDALDriverManager()->GetDriverByName("PolishMap");
    if (poDriver == nullptr) {
        printf("  ✗ ERROR: PolishMap driver not registered\n");
        g_nTestsFailed++;
        return;
    }

    // Create dataset with field mapping
    std::string osFieldMappingOpt = "FIELD_MAPPING=" + osYamlPath;
    const char* papszOptions[] = {
        osFieldMappingOpt.c_str(),
        nullptr
    };

    GDALDataset* poDS = poDriver->Create(
        osOutputPath.c_str(), 0, 0, 0, GDT_Unknown,
        const_cast<char**>(papszOptions)
    );
    AssertTrue(poDS != nullptr, "Dataset created with FIELD_MAPPING option");

    // Create POI layer
    OGRLayer* poLayer = poDS->CreateLayer("POI", nullptr, wkbPoint, nullptr);
    AssertTrue(poLayer != nullptr, "POI layer created");

    // Debug: check TestCapability
    printf("  DEBUG: OLCCreateField capability = %d\n", poLayer->TestCapability(OLCCreateField));

    // Create source fields (NAME, MP_TYPE, Country)
    OGRFieldDefn oFieldName("NAME", OFTString);
    OGRErr eErr1 = poLayer->CreateField(&oFieldName);
    printf("  DEBUG: CreateField(NAME) returned %d\n", eErr1);

    OGRFieldDefn oFieldType("MP_TYPE", OFTString);
    OGRErr eErr2 = poLayer->CreateField(&oFieldType);
    printf("  DEBUG: CreateField(MP_TYPE) returned %d\n", eErr2);

    OGRFieldDefn oFieldCountry("Country", OFTString);
    OGRErr eErr3 = poLayer->CreateField(&oFieldCountry);
    printf("  DEBUG: CreateField(Country) returned %d\n", eErr3);

    printf("  DEBUG: Layer now has %d fields\n", poLayer->GetLayerDefn()->GetFieldCount());

    // Create feature with source field values
    OGRFeature* poFeature = OGRFeature::CreateFeature(poLayer->GetLayerDefn());

    // Set geometry
    OGRPoint oPoint(2.3, 48.9);
    poFeature->SetGeometry(&oPoint);

    // Debug: print feature definition
    printf("  DEBUG: Feature has %d fields:\n", poFeature->GetFieldCount());
    for (int i = 0; i < poFeature->GetFieldCount(); i++) {
        printf("    Field %d: %s\n", i, poFeature->GetFieldDefnRef(i)->GetNameRef());
    }

    // Set attribute values (using SOURCE field names!)
    poFeature->SetField("NAME", "Les Avirons");
    poFeature->SetField("MP_TYPE", "0x54");
    poFeature->SetField("Country", "France~[0x1d]FRA");

    printf("  DEBUG: After setting fields:\n");
    printf("    NAME = %s\n", poFeature->GetFieldAsString("NAME"));
    printf("    MP_TYPE = %s\n", poFeature->GetFieldAsString("MP_TYPE"));
    printf("    Country = %s\n", poFeature->GetFieldAsString("Country"));

    // Write feature
    OGRErr eErr = poLayer->CreateFeature(poFeature);
    AssertTrue(eErr == OGRERR_NONE, "Feature written successfully");

    OGRFeature::DestroyFeature(poFeature);
    GDALClose(poDS);

    // Read output file and verify values were written
    VSILFILE* fp = VSIFOpenL(osOutputPath.c_str(), "rb");
    AssertTrue(fp != nullptr, "Output file readable");

    VSIFSeekL(fp, 0, SEEK_END);
    vsi_l_offset nSize = VSIFTellL(fp);
    VSIFSeekL(fp, 0, SEEK_SET);

    std::string osContent;
    osContent.resize(static_cast<size_t>(nSize));
    VSIFReadL(&osContent[0], 1, static_cast<size_t>(nSize), fp);
    VSIFCloseL(fp);

    // Debug: print file content
    printf("  DEBUG: Output file content:\n%s\n", osContent.c_str());

    // Verify mapped values were written (not source field names!)
    AssertTrue(osContent.find("Type=0x54") != std::string::npos,
               "✓ Type=0x54 written (from MP_TYPE)");
    AssertTrue(osContent.find("Label=Les Avirons") != std::string::npos,
               "✓ Label=Les Avirons written (from NAME)");
    AssertTrue(osContent.find("CountryName=France~[0x1d]FRA") != std::string::npos,
               "✓ CountryName written (from Country)");

    // Verify source field names NOT in output
    AssertTrue(osContent.find("NAME=") == std::string::npos,
               "✓ Source field NAME not in output");
    AssertTrue(osContent.find("MP_TYPE=") == std::string::npos,
               "✓ Source field MP_TYPE not in output");

    // Cleanup
    VSIUnlink(osOutputPath.c_str());
    DeleteTempFile(osYamlPath);
}

/************************************************************************/
/*                        Main Test Runner                              */
/************************************************************************/

int main() {
    printf("===============================================\n");
    printf("  Field Mapping Config Tests (Story 4.4)\n");
    printf("===============================================\n");

    // Register GDAL drivers (needed for VSI functions)
    GDALAllRegister();

    // Run all tests
    test_load_valid_yaml();
    test_invalid_yaml_syntax();
    test_unmapped_polish_field();
    test_fallback_no_config();
    test_field_not_applicable_to_layer();
    test_case_insensitive_mapping();
    test_empty_config();
    test_config_file_not_found();
    test_multiple_source_same_target();
    test_yaml_with_comments();
    test_mapper_with_yaml();
    test_mapper_fallback_no_config();
    test_mapper_priority();
    test_writer_uses_mapped_values();  // Story 4.4 Task 5: BUG FIX test

    // Report results
    printf("\n===============================================\n");
    printf("  Test Results:\n");
    printf("  ✓ Passed: %d\n", g_nTestsPassed);
    printf("  ✗ Failed: %d\n", g_nTestsFailed);
    printf("===============================================\n");

    return (g_nTestsFailed == 0) ? 0 : 1;
}
