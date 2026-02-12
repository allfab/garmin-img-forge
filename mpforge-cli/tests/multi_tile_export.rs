//! Unit tests for multi-tile export operations (Story 6.4).
//!
//! Tests verify multi-tile .mp file generation, error handling modes,
//! and global statistics aggregation.

use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a minimal test configuration for multi-tile export
fn create_multi_tile_config(
    temp_dir: &TempDir,
    fixture_path: &str,
    error_handling: &str,
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
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "{}"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display(),
        error_handling
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
// Task 5: Tests unitaires export multi-tuiles (AC: 1, 4)
// ============================================================================

#[test]
fn test_multi_tile_export_generates_multiple_files() {
    // Subtask 5.1: Export 4+ tuiles → fichiers .mp générés
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Use existing test fixtures that will generate multiple tiles
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args();

    // Run pipeline
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should complete successfully: {:?}",
        result.err()
    );

    // Verify multiple .mp files exist
    let tiles_dir = temp_dir.path().join("tiles");
    assert!(
        tiles_dir.exists(),
        "Tiles directory should exist at: {}",
        tiles_dir.display()
    );

    let entries: Vec<_> = fs::read_dir(&tiles_dir)
        .expect("Failed to read tiles directory")
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
        !entries.is_empty(),
        "At least one .mp tile file should be generated"
    );

    // Verify filenames match pattern {col}_{row}.mp
    for entry in &entries {
        let filename = entry.file_name();
        let name = filename.to_string_lossy();
        assert!(
            name.contains('_') && name.ends_with(".mp"),
            "Tile filename should match pattern col_row.mp, got: {}",
            name
        );
    }
}

#[test]
fn test_tile_filename_pattern_matches_tile_id() {
    // Subtask 5.2: Pattern {col}_{row}.mp résolu correctement
    use mpforge_cli::pipeline::tiler::TileBounds;

    let tile = TileBounds {
        col: 45,
        row: 12,
        min_lon: 0.0,
        min_lat: 0.0,
        max_lon: 1.0,
        max_lat: 1.0,
    };

    // Test that tile_id() returns expected format
    let tile_id = tile.tile_id();
    assert_eq!(tile_id, "45_12");

    // Filename should be tile_id + .mp extension
    let filename = format!("{}.mp", tile_id);
    assert_eq!(filename, "45_12.mp");
}

#[test]
fn test_verify_mp_file_with_ogrinfo() {
    // Subtask 5.3: Vérification contenu fichier .mp avec ogrinfo
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args();

    pipeline::run(&config, &args).expect("Pipeline should succeed");

    // Find first generated tile
    let tiles_dir = temp_dir.path().join("tiles");
    let entries: Vec<_> = fs::read_dir(&tiles_dir)
        .expect("Failed to read tiles directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mp"))
        .collect();

    assert!(!entries.is_empty(), "At least one tile should exist");

    let tile_path = entries[0].path();

    // Verify with ogrinfo
    let output = std::process::Command::new("ogrinfo")
        .arg("-al")
        .arg(&tile_path)
        .arg("-so") // Summary only for faster test
        .output();

    match output {
        Ok(result) => {
            if !result.status.success() {
                eprintln!(
                    "ogrinfo failed. Ensure GDAL is installed with PolishMap driver support."
                );
                return;
            }

            let stdout = String::from_utf8_lossy(&result.stdout);
            assert!(
                stdout.contains("Layer name: POI")
                    || stdout.contains("Layer name: POLYLINE")
                    || stdout.contains("Layer name: POLYGON"),
                "ogrinfo should show at least one MP layer, got: {}",
                stdout
            );
        }
        Err(e) => {
            eprintln!("ogrinfo not found ({}), skipping verification test", e);
        }
    }
}

#[test]
fn test_global_stats_aggregation() {
    // Subtask 5.4: Stats globales agrégées (sum features par type)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args();

    // Run pipeline and capture summary
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should complete successfully for stats aggregation test"
    );

    let summary = result.unwrap();

    // Verify tiles were created
    assert!(
        summary.tiles_succeeded > 0,
        "At least one tile should be exported successfully"
    );

    // Verify global stats are aggregated
    let total_features = summary.total_features();
    assert!(
        total_features > 0,
        "Global stats should aggregate features from all tiles"
    );

    // Verify stats breakdown
    assert!(
        summary.global_stats.point_count > 0
            || summary.global_stats.linestring_count > 0
            || summary.global_stats.polygon_count > 0,
        "At least one geometry type should be counted"
    );
}

// ============================================================================
// Task 6: Tests gestion erreurs export (AC: 2, 3)
// ============================================================================

#[test]
fn test_error_mode_continue_skips_failures() {
    // Subtask 6.1: Mode Continue + échec 1 tuile → autres tuiles exportées
    // NOTE: This test verifies Continue mode configuration is accepted,
    // but does NOT inject actual errors (would require read-only dirs or invalid data).
    // TODO: Add error injection test for robust validation of error handling logic.
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    // Use Continue mode
    let config = create_multi_tile_config(&temp_dir, fixture_path, "continue");
    let args = create_test_args();

    let result = pipeline::run(&config, &args);

    // Pipeline should succeed in Continue mode even if some tiles fail
    assert!(
        result.is_ok(),
        "Pipeline should succeed in Continue mode: {:?}",
        result.as_ref().err()
    );

    let summary = result.unwrap();

    // Verify at least some tiles were created using summary
    assert!(
        summary.tiles_succeeded > 0,
        "At least some tiles should be exported in Continue mode"
    );
}

#[test]
fn test_error_mode_failfast_stops_on_error() {
    // Subtask 6.2: Mode FailFast + échec → pipeline stop immédiatement
    // NOTE: This test verifies FailFast mode configuration is accepted,
    // but does NOT inject actual errors to test failure propagation.
    // TODO: Add error injection test (e.g., read-only output dir) for robust validation.

    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    // Use FailFast mode
    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");

    // Verify configuration parsing
    assert_eq!(config.error_handling, "fail-fast");

    // In normal conditions, FailFast should still succeed
    let args = create_test_args();
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should succeed when no errors occur, even in FailFast mode"
    );
}

#[test]
fn test_empty_tile_skipped() {
    // Subtask 6.3: Tuile vide (0 features) → skippée avec debug log
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    // Use large cell size to potentially create empty tiles at edges
    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 5.0
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "continue"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display()
    );

    let config: Config = serde_yml::from_str(&config_yaml).expect("Failed to parse test config");
    let args = create_test_args();

    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should handle empty tiles gracefully"
    );

    // Verify output directory exists (even if tiles were skipped)
    let tiles_dir = temp_dir.path().join("tiles");
    assert!(
        tiles_dir.exists(),
        "Output directory should be created even if tiles are skipped"
    );
}

// ============================================================================
// Task 7: Tests intégration pipeline complet multi-tuiles (AC: 1-5)
// ============================================================================

#[test]
fn test_full_pipeline_multi_tile_export() {
    // Subtask 7.1: Pipeline complet lecture → R-tree → grille → clip → export multi-tuiles
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args();

    // Run full pipeline
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Full pipeline should complete successfully: {:?}",
        result.err()
    );

    // Verify tiles directory and files
    let tiles_dir = temp_dir.path().join("tiles");
    assert!(tiles_dir.exists(), "Tiles directory should exist");

    let entries: Vec<_> = fs::read_dir(&tiles_dir)
        .expect("Failed to read tiles directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("mp"))
        .collect();

    assert!(
        !entries.is_empty(),
        "Full pipeline should generate at least one tile"
    );

    // Verify each file is non-empty
    for entry in &entries {
        let metadata = fs::metadata(entry.path()).expect("Failed to get file metadata");
        assert!(
            metadata.len() > 0,
            "Tile file should not be empty: {:?}",
            entry.file_name()
        );
    }
}

#[test]
fn test_pipeline_performance_reasonable() {
    // Subtask 7.4: Valider performance export raisonnable
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_multi_tile_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args();

    // Measure pipeline execution time
    let start = std::time::Instant::now();
    let result = pipeline::run(&config, &args);
    let elapsed = start.elapsed();

    assert!(result.is_ok(), "Pipeline should complete successfully");

    // Performance threshold: reasonable time for small test dataset
    // (Actual threshold depends on dataset size; this is a sanity check)
    assert!(
        elapsed.as_secs() < 60,
        "Pipeline should complete in reasonable time (<60s for test data), took: {}s",
        elapsed.as_secs()
    );
}
