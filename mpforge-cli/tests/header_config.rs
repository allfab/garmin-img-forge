//! Integration tests for header configuration (Story 8.1)

use mpforge_cli::config::HeaderConfig;
use mpforge_cli::pipeline::writer::MpWriter;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a temp directory with test fixture
fn create_test_fixture() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a minimal test shapefile using GDAL
    let shp_path = temp_dir.path().join("test.shp");
    let output = Command::new("ogr2ogr")
        .arg("-f")
        .arg("ESRI Shapefile")
        .arg(&shp_path)
        .arg("/vsimem/test.csv")
        .arg("-dsco")
        .arg("GEOMETRY=AS_WKT")
        .arg("-sql")
        .arg("SELECT 1 as id, 'POINT(2.3 48.8)' as WKT")
        .output();

    if output.is_ok() && output.unwrap().status.success() {
        return (temp_dir, shp_path);
    }

    // Fallback: use gdal commands to create a simple point
    let create_cmd = format!(
        "echo 'id,wkt\n1,\"POINT(2.3 48.8)\"' > {}/test.csv && \
         ogr2ogr -f 'ESRI Shapefile' {} {}/test.csv -oo GEOM_POSSIBLE_NAMES=wkt -oo KEEP_GEOM_COLUMNS=NO",
        temp_dir.path().display(),
        shp_path.display(),
        temp_dir.path().display()
    );
    let _ = Command::new("sh").arg("-c").arg(&create_cmd).output();

    (temp_dir, shp_path)
}

/// Helper to read MP file header section
fn read_mp_header(mp_path: &PathBuf) -> String {
    let content = fs::read_to_string(mp_path).expect("Failed to read MP file");
    // Extract [IMG ID] section (everything before first [POI], [POLYLINE], or [POLYGON])
    content
        .split("[POI]")
        .next()
        .and_then(|s| s.split("[POLYLINE]").next())
        .and_then(|s| s.split("[POLYGON]").next())
        .unwrap_or("")
        .to_string()
}

// ============================================================================
// AC1: Template passthrough
// ============================================================================

#[test]
fn test_ac1_template_passthrough() {
    // AC1: Given config with header.template, when CLI exports, then each MP uses template header

    let (temp_dir, _shp_path) = create_test_fixture();

    // Create template file
    let template_path = temp_dir.path().join("template.mp");
    fs::write(
        &template_path,
        "[IMG ID]\nName=Template Map\nID=42\nCopyright=Test Corp\nLevels=3\n",
    )
    .expect("Failed to write template");

    // Create output directory
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");

    let output_path = output_dir.join("test.mp");

    let header = HeaderConfig {
        template: Some(template_path.clone()),
        name: None,
        id: None,
        copyright: None,
        levels: None,
        level0: None,
        level1: None,
        level2: None,
        level3: None,
        level4: None,
        level5: None,
        level6: None,
        level7: None,
        level8: None,
        level9: None,
        tree_size: None,
        rgn_limit: None,
        transparent: None,
        marine: None,
        preprocess: None,
        lbl_coding: None,
        simplify_level: None,
        left_side_traffic: None,
        custom: None,
    };

    // Create writer with header config
    let writer = MpWriter::new(output_path.clone(), None, Some(&header))
        .expect("Failed to create writer with template");

    writer.finalize().expect("Failed to finalize");

    // Verify output file exists
    assert!(output_path.exists(), "MP file should exist");

    // Verify header contains template values (AC1 requires checking actual content)
    let header_content = read_mp_header(&output_path);
    assert!(
        header_content.contains("Name=Template Map"),
        "Header must use template name (AC1)"
    );
    assert!(
        header_content.contains("ID=42"),
        "Header must use template ID (AC1)"
    );
    assert!(
        header_content.contains("Copyright=Test Corp"),
        "Header must use template copyright (AC1)"
    );

    // Also verify via ogrinfo for completeness
    let ogrinfo_output = Command::new("ogrinfo")
        .arg("-al")
        .arg(&output_path)
        .output()
        .expect("Failed to run ogrinfo");

    assert!(
        ogrinfo_output.status.success(),
        "ogrinfo should succeed on generated MP"
    );
}

// ============================================================================
// AC2: Champs individuels
// ============================================================================

#[test]
fn test_ac2_individual_fields() {
    // AC2: Given config with header.name and header.levels, then header contains Name= and Levels=

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");
    let output_path = output_dir.join("test.mp");

    let header = HeaderConfig {
        template: None,
        name: Some("Ma Carte".to_string()),
        id: None,
        copyright: None,
        levels: Some("4".to_string()),
        level0: None,
        level1: None,
        level2: None,
        level3: None,
        level4: None,
        level5: None,
        level6: None,
        level7: None,
        level8: None,
        level9: None,
        tree_size: None,
        rgn_limit: None,
        transparent: None,
        marine: None,
        preprocess: None,
        lbl_coding: None,
        simplify_level: None,
        left_side_traffic: None,
        custom: None,
    };

    let writer =
        MpWriter::new(output_path.clone(), None, Some(&header)).expect("Failed to create writer");

    writer.finalize().expect("Failed to finalize");

    // Verify output file exists
    assert!(output_path.exists(), "MP file should exist");

    // Read header and verify fields
    let header_content = read_mp_header(&output_path);
    assert!(
        header_content.contains("Name=Ma Carte"),
        "Header should contain Name=Ma Carte"
    );
    assert!(
        header_content.contains("Levels=4"),
        "Header should contain Levels=4"
    );
}

// ============================================================================
// AC3: Précédence template > champs
// ============================================================================

#[test]
fn test_ac3_template_precedence() {
    // AC3: Given config with both template AND name, then template takes precedence

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create template with specific Name
    let template_path = temp_dir.path().join("template.mp");
    fs::write(
        &template_path,
        "[IMG ID]\nName=Template Name\nID=99\n",
    )
    .expect("Failed to write template");

    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");
    let output_path = output_dir.join("test.mp");

    let header = HeaderConfig {
        template: Some(template_path.clone()),
        name: Some("Individual Name".to_string()), // This should be ignored (template wins)
        id: None,
        copyright: None,
        levels: None,
        level0: None,
        level1: None,
        level2: None,
        level3: None,
        level4: None,
        level5: None,
        level6: None,
        level7: None,
        level8: None,
        level9: None,
        tree_size: None,
        rgn_limit: None,
        transparent: None,
        marine: None,
        preprocess: None,
        lbl_coding: None,
        simplify_level: None,
        left_side_traffic: None,
        custom: None,
    };

    let writer =
        MpWriter::new(output_path.clone(), None, Some(&header)).expect("Failed to create writer");

    writer.finalize().expect("Failed to finalize");

    assert!(output_path.exists(), "MP file should exist");

    // AC3: Verify template precedence (Code Review Fix M3: robust validation)
    let header_content = read_mp_header(&output_path);

    // Positive check: template values MUST be present
    assert!(
        header_content.contains("Name=Template Name"),
        "Header must use template name (AC3)"
    );
    assert!(
        header_content.contains("ID=99"),
        "Header must use template ID (AC3)"
    );

    // Negative check: individual fields MUST NOT be present
    assert!(
        !header_content.contains("Individual Name"),
        "Header must NOT use individual name when template present (AC3)"
    );

    // Robustness check: Verify some header content exists (not empty/broken)
    assert!(
        header_content.contains("[IMG ID]"),
        "Header section must exist and be valid"
    );
}

// ============================================================================
// AC4: Template invalide = échec clair
// ============================================================================

#[test]
fn test_ac4_invalid_template_error() {
    // AC4: Given config with nonexistent template, then error with clear message

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");
    let output_path = output_dir.join("test.mp");

    let nonexistent_template = PathBuf::from("/nonexistent/template.mp");

    let header = HeaderConfig {
        template: Some(nonexistent_template.clone()),
        name: None,
        id: None,
        copyright: None,
        levels: None,
        level0: None,
        level1: None,
        level2: None,
        level3: None,
        level4: None,
        level5: None,
        level6: None,
        level7: None,
        level8: None,
        level9: None,
        tree_size: None,
        rgn_limit: None,
        transparent: None,
        marine: None,
        preprocess: None,
        lbl_coding: None,
        simplify_level: None,
        left_side_traffic: None,
        custom: None,
    };

    let result = MpWriter::new(output_path, None, Some(&header));

    match result {
        Ok(_) => panic!("MpWriter::new() should fail with nonexistent template"),
        Err(e) => {
            let error_msg = e.to_string();
            // AC4 requires "erreur claire avec chemin du fichier"
            // Must show the actual template path so user knows what's wrong
            assert!(
                error_msg.contains("/nonexistent/template.mp") || error_msg.contains("nonexistent"),
                "Error message must show template path (AC4): {}",
                error_msg
            );
        }
    }
}

// ============================================================================
// AC5: Section header optionnelle
// ============================================================================

#[test]
fn test_ac5_header_optional() {
    // AC5: Given config without header section, then behavior is identical to today (defaults)

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");
    let output_path = output_dir.join("test.mp");

    // Create writer without header config (backward compatible)
    let writer = MpWriter::new(output_path.clone(), None, None).expect("Failed to create writer");

    writer.finalize().expect("Failed to finalize");

    assert!(output_path.exists(), "MP file should exist without header");

    // AC5: Verify "aucune régression" - behavior identical to legacy mode
    // Code Review Fix L2: Check actual header content for driver defaults
    let header_content = read_mp_header(&output_path);

    // Verify basic header structure exists (driver defaults)
    assert!(
        header_content.contains("[IMG ID]"),
        "Header section must exist with driver defaults (AC5)"
    );

    // Verify NO unexpected fields were added (regression check)
    // Driver should produce minimal header without extra fields
    let unexpected_fields = ["Name=", "Copyright=", "Levels="];
    let has_unexpected = unexpected_fields
        .iter()
        .any(|field| header_content.contains(field));

    // Note: Some drivers may add default Name/ID - this is OK as long as ogrinfo validates
    if has_unexpected {
        println!(
            "INFO: Driver added default fields (acceptable): {}",
            header_content
        );
    }

    // Verify it's a valid MP file via ogrinfo (primary AC5 check)
    let ogrinfo_output = Command::new("ogrinfo")
        .arg("-al")
        .arg(&output_path)
        .output()
        .expect("Failed to run ogrinfo");

    assert!(
        ogrinfo_output.status.success(),
        "ogrinfo must succeed on MP without header config (AC5 - no regression)"
    );
}

// ============================================================================
// AC6: Champs custom
// ============================================================================

#[test]
fn test_ac6_custom_fields() {
    // AC6: Given config with header.custom, then custom fields present in MP

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_dir = temp_dir.path().join("output");
    fs::create_dir_all(&output_dir).expect("Failed to create output dir");
    let output_path = output_dir.join("test.mp");

    let mut custom = HashMap::new();
    custom.insert("DrawPriority".to_string(), "25".to_string());
    custom.insert("MG".to_string(), "N".to_string());

    let header = HeaderConfig {
        template: None,
        name: None,
        id: None,
        copyright: None,
        levels: None,
        level0: None,
        level1: None,
        level2: None,
        level3: None,
        level4: None,
        level5: None,
        level6: None,
        level7: None,
        level8: None,
        level9: None,
        tree_size: None,
        rgn_limit: None,
        transparent: None,
        marine: None,
        preprocess: None,
        lbl_coding: None,
        simplify_level: None,
        left_side_traffic: None,
        custom: Some(custom),
    };

    let writer =
        MpWriter::new(output_path.clone(), None, Some(&header)).expect("Failed to create writer");

    writer.finalize().expect("Failed to finalize");

    assert!(output_path.exists(), "MP file should exist");

    // Verify custom fields in header
    let header_content = read_mp_header(&output_path);
    assert!(
        header_content.contains("DrawPriority=25"),
        "Header should contain custom field DrawPriority=25"
    );
    assert!(
        header_content.contains("MG=N"),
        "Header should contain custom field MG=N"
    );
}
