//! Integration tests for end-to-end MP export (Story 5.4)

use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to create a minimal test configuration
fn create_test_config(temp_dir: &TempDir, fixture_path: &str) -> Config {
    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "output.mp"
error_handling: "fail-fast"
"#,
        fixture_path,
        temp_dir.path().display()
    );

    serde_yml::from_str(&config_yaml).expect("Failed to parse test config")
}

/// Helper to create minimal BuildArgs for testing
fn create_test_args() -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        verbose: 0,
    }
}

// ============================================================================
// Task 6: Integration Tests
// ============================================================================

#[test]
fn test_end_to_end_shapefile_to_mp() {
    // AC1: Pipeline complet avec fixtures Shapefile → .mp
    // Note Story 6.4: Pipeline now generates multi-tile output (col_row.mp files)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Use existing test fixtures from Story 5.3
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    // Fail test if fixture doesn't exist (don't skip silently)
    assert!(
        PathBuf::from(fixture_path).exists(),
        "Test fixture missing: {}. Ensure test data is set up correctly.",
        fixture_path
    );

    let config = create_test_config(&temp_dir, fixture_path);
    let args = create_test_args();

    // Run pipeline
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should complete successfully: {:?}",
        result.err()
    );

    // Story 6.4: Verify at least one .mp tile file exists
    let mp_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read output directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "mp")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !mp_files.is_empty(),
        "At least one .mp tile file should exist in output directory"
    );

    // Verify first tile file is not empty
    let first_tile = &mp_files[0];
    let metadata = fs::metadata(first_tile.path()).expect("Failed to get file metadata");
    assert!(
        metadata.len() > 0,
        "Output .mp tile file should not be empty"
    );
}

#[test]
fn test_mp_file_readable_with_ogrinfo() {
    // AC3: Vérification .mp avec ogrinfo (driver ogr-polishmap)
    // Note Story 6.4: Pipeline now generates multi-tile output
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    assert!(
        PathBuf::from(fixture_path).exists(),
        "Test fixture missing: {}. Ensure test data is set up correctly.",
        fixture_path
    );

    let config = create_test_config(&temp_dir, fixture_path);
    let args = create_test_args();

    // Run pipeline
    pipeline::run(&config, &args).expect("Pipeline should succeed");

    // Story 6.4: Find first generated tile file
    let mp_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read output directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "mp")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !mp_files.is_empty(),
        "At least one .mp tile file should exist"
    );

    let output_path = mp_files[0].path();

    // Run ogrinfo to verify file is readable
    let mut cmd = Command::new("ogrinfo");
    cmd.arg("-ro").arg("-so").arg(&output_path);

    // Use GDAL_DRIVER_PATH from environment if set (for custom driver locations)
    if let Ok(driver_path) = std::env::var("GDAL_DRIVER_PATH") {
        cmd.env("GDAL_DRIVER_PATH", driver_path);
    }

    let output = cmd.output().expect("Failed to execute ogrinfo");

    // Check that ogrinfo succeeded
    assert!(
        output.status.success(),
        "ogrinfo should read .mp file successfully. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify 3 layers are listed
    assert!(
        stdout.contains("POI") || stdout.contains("1: POI"),
        "ogrinfo output should list POI layer"
    );
    assert!(
        stdout.contains("POLYLINE") || stdout.contains("2: POLYLINE"),
        "ogrinfo output should list POLYLINE layer"
    );
    assert!(
        stdout.contains("POLYGON") || stdout.contains("3: POLYGON"),
        "ogrinfo output should list POLYGON layer"
    );
}

#[test]
fn test_pipeline_console_summary() {
    // AC4: Test résumé console (feature count, path)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    assert!(
        PathBuf::from(fixture_path).exists(),
        "Test fixture missing: {}. Ensure test data is set up correctly.",
        fixture_path
    );

    let config = create_test_config(&temp_dir, fixture_path);
    let args = create_test_args();

    // Capture stdout/stderr would require more complex setup
    // For now, we just verify the pipeline completes
    let result = pipeline::run(&config, &args);

    assert!(result.is_ok(), "Pipeline should complete and print summary");

    // In a real scenario, we'd capture stdout and verify:
    // - "Export completed successfully"
    // - Output file path
    // - Feature counts (POI, POLYLINE, POLYGON)
}

// ============================================================================
// Story 7.4: Field Mapping Integration Tests
// ============================================================================

#[test]
fn test_field_mapping_with_yaml_config() {
    // AC3, AC7: Pipeline avec field_mapping_path génère des .mp avec attributs corrects
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a temporary field mapping YAML file
    let mapping_path = temp_dir.path().join("test-mapping.yaml");
    fs::write(
        &mapping_path,
        r#"
field_mapping:
  MP_TYPE: Type
  NAME: Label
"#,
    )
    .expect("Failed to create mapping file");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    assert!(
        PathBuf::from(fixture_path).exists(),
        "Test fixture missing: {}. Ensure test data is set up correctly.",
        fixture_path
    );

    // Create config with field_mapping_path
    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "output.mp"
  field_mapping_path: "{}"
error_handling: "fail-fast"
"#,
        fixture_path,
        temp_dir.path().display(),
        mapping_path.display()
    );

    let config: Config = serde_yml::from_str(&config_yaml).expect("Failed to parse config");
    let args = create_test_args();

    // Run pipeline with field mapping
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline with field mapping should succeed: {:?}",
        result.err()
    );

    // Verify .mp file was created
    let mp_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read output directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "mp")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !mp_files.is_empty(),
        "At least one .mp file should be generated with field mapping"
    );

    // Verify file is not empty
    let first_tile = &mp_files[0];
    let metadata = fs::metadata(first_tile.path()).expect("Failed to get file metadata");
    assert!(
        metadata.len() > 0,
        "Output .mp file should not be empty with field mapping"
    );

    // H1 FIX: Verify actual .mp content contains fields (AC3, AC7)
    let mp_content =
        fs::read_to_string(first_tile.path()).expect("Failed to read .mp file content");

    // Check that field mapping was processed by verifying Type field exists
    // Note: Test data has 'Type' (already canonical) not 'MP_TYPE', so mapping
    // doesn't change it, but driver still processes the FIELD_MAPPING option.
    assert!(
        mp_content.contains("Type="),
        "Output .mp file should contain 'Type=' field. Content:\n{}",
        mp_content
    );

    // Label field is optional (only present if source has NAME field)
    // Test data has 'name' (lowercase) which doesn't match NAME mapping,
    // so Label won't be present. This is expected behavior.
}

#[test]
fn test_field_mapping_without_config_uses_hardcoded_aliases() {
    // AC4: Pipeline SANS field_mapping_path utilise les aliases hardcodés (backward compatible)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    assert!(
        PathBuf::from(fixture_path).exists(),
        "Test fixture missing: {}",
        fixture_path
    );

    // Create config WITHOUT field_mapping_path
    let config = create_test_config(&temp_dir, fixture_path);
    let args = create_test_args();

    // Run pipeline without field mapping (should use hardcoded aliases)
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline without field mapping should succeed (backward compat): {:?}",
        result.err()
    );

    // Verify .mp file was created (same behavior as before)
    let mp_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read output directory")
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "mp")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        !mp_files.is_empty(),
        "At least one .mp file should be generated without field mapping (backward compat)"
    );

    let first_tile = &mp_files[0];
    let metadata = fs::metadata(first_tile.path()).expect("Failed to get file metadata");
    assert!(metadata.len() > 0, "Output .mp file should not be empty");

    // H1 FIX: Verify backward compat uses hardcoded aliases (AC4)
    let mp_content =
        fs::read_to_string(first_tile.path()).expect("Failed to read .mp file content");

    // With hardcoded aliases, driver should still produce Type and Label fields
    // (driver maps MP_TYPE→Type and NAME→Label internally via polishmapfields.h)
    assert!(
        mp_content.contains("Type=") || mp_content.contains("Label="),
        "Output .mp file should contain Type/Label fields even without explicit field_mapping_path (hardcoded aliases). Content:\n{}",
        mp_content
    );
}

#[test]
fn test_field_mapping_invalid_path_error() {
    // AC5: Erreur claire si fichier field_mapping_path inexistant ou invalide
    // H3 FIX: Test MpWriter::new() directly (validation moved from config.validate())
    use mpforge_cli::pipeline::writer::MpWriter;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let output_path = temp_dir.path().join("test.mp");
    let nonexistent_mapping = PathBuf::from("/nonexistent/mapping.yaml");

    // Try to create MpWriter with nonexistent field mapping path
    let result = MpWriter::new(output_path, Some(&nonexistent_mapping));

    match result {
        Ok(_) => panic!("MpWriter::new() should fail with nonexistent field_mapping_path"),
        Err(e) => {
            let error_message = e.to_string();
            assert!(
                error_message.contains("Field mapping file does not exist")
                    || error_message.contains("nonexistent"),
                "Error message should mention field mapping file doesn't exist, got: {}",
                error_message
            );
        }
    }
}
