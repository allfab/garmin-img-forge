//! Integration tests for JSON report generation (Story 7.3).
//!
//! Tests verify JSON report creation, schema validation, and exit codes.

use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a minimal test configuration
fn create_test_config(temp_dir: &TempDir, fixture_path: &str, error_handling: &str) -> Config {
    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.05
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "{}"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display(),
        error_handling
    );

    serde_yml::from_str(&config_yaml).expect("Failed to parse test config")
}

/// Helper to create BuildArgs with report path
fn create_args_with_report(report_path: &str) -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: Some(report_path.to_string()),
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    }
}

/// Helper to create BuildArgs without report
fn create_args_without_report() -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    }
}

// ============================================================================
// Task 7.1: Test JSON report generation with --report flag (AC2)
// ============================================================================

#[test]
fn test_json_report_generated_when_flag_present() {
    // AC2: --report génère fichier JSON
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let report_path = temp_dir.path().join("report.json");
    let args = create_args_with_report(report_path.to_str().unwrap());

    // Run pipeline
    let result = pipeline::run(&config, &args);
    assert!(
        result.is_ok(),
        "Pipeline should complete: {:?}",
        result.err()
    );

    // Verify JSON file created
    assert!(report_path.exists(), "JSON report file should exist");

    // Verify JSON content structure
    let content = fs::read_to_string(&report_path).expect("Failed to read report");
    let report: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON report");

    // Verify schema fields exist
    assert!(report.get("status").is_some(), "status field missing");
    assert!(
        report.get("tiles_generated").is_some(),
        "tiles_generated field missing"
    );
    assert!(
        report.get("tiles_failed").is_some(),
        "tiles_failed field missing"
    );
    assert!(
        report.get("tiles_skipped").is_some(),
        "tiles_skipped field missing"
    );
    assert!(
        report.get("features_processed").is_some(),
        "features_processed field missing"
    );
    assert!(
        report.get("duration_seconds").is_some(),
        "duration_seconds field missing"
    );
    assert!(report.get("errors").is_some(), "errors field missing");

    // Verify status is "success" for valid data
    assert_eq!(report["status"], "success", "Status should be success");
    assert!(
        report["errors"].as_array().unwrap().is_empty(),
        "Errors should be empty"
    );
}

// ============================================================================
// Task 7.2: Test JSON report NOT generated without --report flag (AC6)
// ============================================================================

#[test]
fn test_json_report_not_generated_when_flag_absent() {
    // AC6: Pas de --report → pas de fichier JSON créé
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let args = create_args_without_report();

    // Run pipeline
    let result = pipeline::run(&config, &args);
    assert!(
        result.is_ok(),
        "Pipeline should complete: {:?}",
        result.err()
    );

    // Verify no JSON files in output directory
    let json_files: Vec<_> = fs::read_dir(temp_dir.path())
        .expect("Failed to read temp dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    assert_eq!(
        json_files.len(),
        0,
        "No JSON files should be created without --report flag"
    );
}

// ============================================================================
// Task 7.3: Test JSON schema matches Epic specification (AC2)
// ============================================================================

#[test]
fn test_json_schema_matches_epic_specification() {
    // AC2: Schema JSON exact match
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let report_path = temp_dir.path().join("report.json");
    let args = create_args_with_report(report_path.to_str().unwrap());

    // Run pipeline
    let _ = pipeline::run(&config, &args);

    // Read and parse JSON
    let content = fs::read_to_string(&report_path).expect("Failed to read report");
    let report: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON report");

    // Verify exact schema fields from Epic spec
    assert_eq!(report["status"].as_str().unwrap(), "success");
    assert!(report["tiles_generated"].is_number());
    assert!(report["tiles_failed"].is_number());
    assert!(report["tiles_skipped"].is_number());
    assert!(report["features_processed"].is_number());
    assert!(report["duration_seconds"].is_number());
    assert!(report["errors"].is_array());

    // Verify no extra fields (schema compliance)
    let expected_keys = [
        "status",
        "tiles_generated",
        "tiles_failed",
        "tiles_skipped",
        "features_processed",
        "duration_seconds",
        "errors",
    ];
    let actual_keys: Vec<String> = report.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        actual_keys.len(),
        expected_keys.len(),
        "Schema should have exactly 7 fields"
    );
}

// ============================================================================
// Task 7.4: Test features_processed count accuracy (AC2)
// ============================================================================

#[test]
fn test_features_processed_count() {
    // AC2: features_processed = sum de toutes features exportées
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let report_path = temp_dir.path().join("report.json");
    let args = create_args_with_report(report_path.to_str().unwrap());

    // Run pipeline
    let _ = pipeline::run(&config, &args);

    // Verify features_processed > 0
    let content = fs::read_to_string(&report_path).expect("Failed to read report");
    let report: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON report");

    let features_processed = report["features_processed"].as_u64().unwrap();
    assert!(
        features_processed > 0,
        "features_processed should be > 0 for valid data"
    );

    // Verify consistency: tiles_generated > 0 implies features_processed > 0
    let tiles_generated = report["tiles_generated"].as_u64().unwrap();
    if tiles_generated > 0 {
        assert!(
            features_processed > 0,
            "If tiles generated, features must be processed"
        );
    }
}

// ============================================================================
// Task 7.5: Test duration_seconds precision (AC2)
// ============================================================================

#[test]
fn test_duration_seconds_precision() {
    // AC2: duration_seconds is f64 for precision
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let report_path = temp_dir.path().join("report.json");
    let args = create_args_with_report(report_path.to_str().unwrap());

    // Run pipeline
    let _ = pipeline::run(&config, &args);

    // Verify duration_seconds is float
    let content = fs::read_to_string(&report_path).expect("Failed to read report");
    let report: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON report");

    let duration = report["duration_seconds"].as_f64();
    assert!(duration.is_some(), "duration_seconds should be f64");
    assert!(duration.unwrap() > 0.0, "duration_seconds should be > 0");
}

// ============================================================================
// M1 Fix: Test AC3 - Errors array structure validation
// ============================================================================

#[test]
fn test_errors_array_structure() {
    // AC3: Validate errors array has correct structure (tile_id + error message)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_test_config(&temp_dir, fixture_path, "continue");
    let report_path = temp_dir.path().join("report.json");
    let args = create_args_with_report(report_path.to_str().unwrap());

    // Run pipeline
    let _ = pipeline::run(&config, &args);

    // Read and validate JSON schema
    let content = fs::read_to_string(&report_path).expect("Failed to read report");
    let report: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse JSON report");

    // Verify errors is an array
    assert!(report["errors"].is_array(), "errors field must be an array");

    // If there are errors, validate structure (AC3)
    if let Some(errors_array) = report["errors"].as_array() {
        for error_entry in errors_array {
            // Each error must have 'tile' and 'error' fields
            assert!(
                error_entry.get("tile").is_some(),
                "Error entry missing 'tile' field"
            );
            assert!(
                error_entry.get("error").is_some(),
                "Error entry missing 'error' field"
            );

            // Fields must be strings
            assert!(
                error_entry["tile"].is_string(),
                "'tile' field must be string"
            );
            assert!(
                error_entry["error"].is_string(),
                "'error' field must be string"
            );

            // Tile ID should not be empty
            let tile_id = error_entry["tile"].as_str().unwrap();
            assert!(!tile_id.is_empty(), "tile ID should not be empty");
        }
    }

    // Note: This test validates error structure (AC3).
    // Testing actual error collection requires fixtures with invalid geometries,
    // which is covered by the unit tests in pipeline/mod.rs.
}
