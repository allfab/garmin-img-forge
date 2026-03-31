//! Integration tests for export control options (Story 8.3).
//!
//! Tests verify --skip-existing, --dry-run, overwrite config, and their combinations.

use mpforge::cli::BuildArgs;
use mpforge::config::Config;
use mpforge::pipeline;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a test config for export control testing
fn create_test_config(temp_dir: &TempDir, fixture_path: &str) -> Config {
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
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display()
    );

    serde_yml::from_str(&config_yaml).expect("Failed to parse test config")
}

/// Helper to create a test config with overwrite setting
fn create_test_config_with_overwrite(
    temp_dir: &TempDir,
    fixture_path: &str,
    overwrite: bool,
) -> Config {
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
  filename_pattern: "{{col}}_{{row}}.mp"
  overwrite: {}
error_handling: "continue"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display(),
        overwrite
    );

    serde_yml::from_str(&config_yaml).expect("Failed to parse test config")
}

/// Helper to create BuildArgs for testing
fn create_test_args() -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 2, // suppress progress bar
    }
}

/// Fixture path for test data
fn fixture_path() -> &'static str {
    "tests/integration/fixtures/test_data/file1.shp"
}

// ============================================================================
// AC1: Skip existing via CLI
// ============================================================================

#[test]
fn test_ac1_skip_existing_skips_already_exported_tiles() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);

    // First export: normal (creates files)
    let args = create_test_args();
    let result1 = pipeline::run(&config, &args);
    assert!(result1.is_ok(), "First export should succeed: {:?}", result1.err());
    let summary1 = result1.unwrap();
    assert!(summary1.tiles_succeeded > 0, "Should export at least 1 tile");

    // Collect exported file paths and their modification times
    let tiles_dir = temp_dir.path().join("tiles");
    let mp_files: Vec<_> = fs::read_dir(&tiles_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "mp"))
        .collect();
    assert!(!mp_files.is_empty(), "Should have .mp files after first export");

    let original_times: Vec<_> = mp_files
        .iter()
        .map(|f| (f.path(), fs::metadata(f.path()).unwrap().modified().unwrap()))
        .collect();

    // Wait a small amount to ensure timestamps would differ
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second export: with --skip-existing
    let args_skip = BuildArgs {
        skip_existing: true,
        ..create_test_args()
    };
    let result2 = pipeline::run(&config, &args_skip);
    assert!(result2.is_ok(), "Skip-existing export should succeed: {:?}", result2.err());
    let summary2 = result2.unwrap();

    // All tiles should be skipped (they all exist from first export)
    assert!(
        summary2.tiles_skipped > 0,
        "Should have skipped tiles, got tiles_skipped={}, tiles_succeeded={}",
        summary2.tiles_skipped,
        summary2.tiles_succeeded
    );

    // Verify file timestamps unchanged (files not rewritten)
    for (path, original_time) in &original_times {
        let new_time = fs::metadata(path).unwrap().modified().unwrap();
        assert_eq!(
            *original_time, new_time,
            "File {} should not have been modified",
            path.display()
        );
    }
}

// ============================================================================
// AC2: Skip existing dans rapport JSON
// ============================================================================

#[test]
fn test_ac2_skip_existing_in_json_report() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);

    // First export: normal
    let args = create_test_args();
    let result1 = pipeline::run(&config, &args);
    assert!(result1.is_ok());

    // Second export: --skip-existing --report
    let report_path = temp_dir.path().join("report.json");
    let args_skip = BuildArgs {
        skip_existing: true,
        report: Some(report_path.to_str().unwrap().to_string()),
        ..create_test_args()
    };
    let result2 = pipeline::run(&config, &args_skip);
    assert!(result2.is_ok());

    // Verify JSON report contains tiles_skipped
    let report_content = fs::read_to_string(&report_path).unwrap();
    let report_json: serde_json::Value = serde_json::from_str(&report_content).unwrap();

    assert!(
        report_json["tiles_skipped"].as_u64().unwrap() > 0,
        "JSON report should have tiles_skipped > 0, got: {}",
        report_json["tiles_skipped"]
    );
}

// ============================================================================
// AC3: Dry-run sans écriture
// ============================================================================

#[test]
fn test_ac3_dry_run_no_files_created() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);
    let tiles_dir = temp_dir.path().join("tiles");

    // Run in dry-run mode
    let args = BuildArgs {
        dry_run: true,
        ..create_test_args()
    };
    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Dry-run should succeed: {:?}", result.err());
    let summary = result.unwrap();

    // Verify output directory is NOT created in dry-run (no side effects)
    assert!(
        !tiles_dir.exists(),
        "Dry-run should not create output directory"
    );

    // Verify summary is coherent (tiles counted)
    assert!(
        summary.tiles_succeeded > 0,
        "Dry-run should report tiles that would be exported"
    );
    assert!(
        summary.total_features() > 0,
        "Dry-run should report features that would be processed"
    );
}

// ============================================================================
// AC4: Dry-run + skip-existing combinés
// ============================================================================

#[test]
fn test_ac4_dry_run_combined_with_skip_existing() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);

    // First: normal export to create some files
    let args_normal = create_test_args();
    let result1 = pipeline::run(&config, &args_normal);
    assert!(result1.is_ok());
    let summary1 = result1.unwrap();
    let original_succeeded = summary1.tiles_succeeded;

    // Second: dry-run + skip-existing
    let args_combined = BuildArgs {
        dry_run: true,
        skip_existing: true,
        ..create_test_args()
    };
    let result2 = pipeline::run(&config, &args_combined);
    assert!(result2.is_ok(), "Combined dry-run+skip-existing should succeed");
    let summary2 = result2.unwrap();

    // All originally exported tiles should be counted as skipped
    assert!(
        summary2.tiles_skipped > 0,
        "Combined mode should report skipped tiles"
    );

    // No new files should be created (dry-run)
    let tiles_dir = temp_dir.path().join("tiles");
    let mp_files: Vec<_> = fs::read_dir(&tiles_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "mp"))
        .collect();
    // File count should be same as after first export
    assert_eq!(
        mp_files.len(),
        original_succeeded,
        "Dry-run should not create new files"
    );
}

// ============================================================================
// AC5: Rétrocompatibilité sans flags
// ============================================================================

#[test]
fn test_ac5_no_flags_overwrites_existing() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);

    // First export
    let args = create_test_args();
    let result1 = pipeline::run(&config, &args);
    assert!(result1.is_ok());

    // Get timestamps before second export
    let tiles_dir = temp_dir.path().join("tiles");
    let mp_files: Vec<_> = fs::read_dir(&tiles_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "mp"))
        .collect();
    assert!(!mp_files.is_empty());

    let original_times: Vec<_> = mp_files
        .iter()
        .map(|f| (f.path(), fs::metadata(f.path()).unwrap().modified().unwrap()))
        .collect();

    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second export: no flags (should overwrite)
    let result2 = pipeline::run(&config, &args);
    assert!(result2.is_ok());
    let summary2 = result2.unwrap();

    // Should succeed with tiles, NOT skipped
    assert!(summary2.tiles_succeeded > 0, "Should overwrite existing tiles");

    // Verify files were actually rewritten (timestamps changed)
    for (path, original_time) in &original_times {
        let new_time = fs::metadata(path).unwrap().modified().unwrap();
        assert_ne!(
            *original_time, new_time,
            "File {} should have been overwritten (timestamp unchanged)",
            path.display()
        );
    }
}

// ============================================================================
// AC6: Config YAML overwrite: false
// ============================================================================

#[test]
fn test_ac6_config_overwrite_false_skips_existing() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    // First export: normal config (overwrite=true default)
    let config_normal = create_test_config(&temp_dir, fp);
    let args = create_test_args();
    let result1 = pipeline::run(&config_normal, &args);
    assert!(result1.is_ok());
    let summary1 = result1.unwrap();
    assert!(summary1.tiles_succeeded > 0);

    // Second export: config with overwrite: false (no CLI flag)
    let config_no_overwrite = create_test_config_with_overwrite(&temp_dir, fp, false);
    let result2 = pipeline::run(&config_no_overwrite, &args);
    assert!(result2.is_ok());
    let summary2 = result2.unwrap();

    // Should skip all existing tiles
    assert!(
        summary2.tiles_skipped > 0,
        "overwrite: false should skip existing tiles, got tiles_skipped={}",
        summary2.tiles_skipped
    );
}

// ============================================================================
// M4 Fix: Dry-run flag in JSON report
// ============================================================================

#[test]
fn test_dry_run_flag_in_json_report() {
    let temp_dir = TempDir::new().unwrap();
    let fp = fixture_path();

    if !PathBuf::from(fp).exists() {
        return;
    }

    let config = create_test_config(&temp_dir, fp);
    let report_path = temp_dir.path().join("report.json");

    // Run with --dry-run --report
    let args = BuildArgs {
        dry_run: true,
        report: Some(report_path.to_str().unwrap().to_string()),
        ..create_test_args()
    };
    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Dry-run with report should succeed");

    // Verify JSON report contains dry_run: true
    let report_content = fs::read_to_string(&report_path).unwrap();
    let report_json: serde_json::Value = serde_json::from_str(&report_content).unwrap();

    assert_eq!(
        report_json["dry_run"],
        serde_json::Value::Bool(true),
        "JSON report should contain dry_run: true when --dry-run is active"
    );
}
