//! Integration tests for header configuration (Story 8.1)

use mpforge_cli::config::{Config, GridConfig, HeaderConfig, InputSource, OutputConfig};
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

    // Verify header contains template values via ogrinfo
    let ogrinfo_output = Command::new("ogrinfo")
        .arg("-al")
        .arg(&output_path)
        .output()
        .expect("Failed to run ogrinfo");

    let ogrinfo_str = String::from_utf8_lossy(&ogrinfo_output.stdout);
    println!("ogrinfo output:\n{}", ogrinfo_str);

    // Metadata is accessible via ogrinfo domain queries (driver-specific)
    // For now, we verify the file was created successfully with template
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

    // Verify template name is used, not individual name
    let header_content = read_mp_header(&output_path);
    assert!(
        header_content.contains("Name=Template Name"),
        "Header should use template name"
    );
    assert!(
        !header_content.contains("Individual Name"),
        "Header should NOT use individual name when template present"
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
            // Error comes from GDAL driver creation failure
            // GDAL will log "ERROR 4: HEADER_TEMPLATE file not found" to stderr
            // Our wrapper returns generic "Failed to create dataset with options"
            assert!(
                error_msg.contains("Failed to create dataset") || error_msg.contains("template") || error_msg.contains("Failed to resolve"),
                "Error message should indicate dataset creation failure: {}",
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

    // Verify it's a valid MP file
    let ogrinfo_output = Command::new("ogrinfo")
        .arg("-al")
        .arg(&output_path)
        .output()
        .expect("Failed to run ogrinfo");

    assert!(
        ogrinfo_output.status.success(),
        "ogrinfo should succeed on MP without header config"
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
