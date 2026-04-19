//! Integration tests for tile naming patterns (Story 8.2)

use mpforge::cli::BuildArgs;
use mpforge::config::Config;
use mpforge::pipeline;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Fixture path for test data
fn fixture_path() -> &'static str {
    "tests/integration/fixtures/test_data/file1.shp"
}

/// Helper to create BuildArgs for testing
fn test_args() -> BuildArgs {
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
        disable_profiles: false,
    }
}

/// Helper to create Config with custom filename pattern
fn config_with_pattern(temp_dir: &TempDir, pattern: &str) -> Config {
    let yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.5
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{}"
error_handling: "continue"
"#,
        fixture_path(),
        temp_dir.path().display(),
        pattern
    );
    serde_yml::from_str(&yaml).expect("Failed to parse test config")
}

/// Collect all .mp files recursively in a directory
fn collect_mp_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_mp_files(&path));
            } else if path.extension().map_or(false, |ext| ext == "mp") {
                files.push(path);
            }
        }
    }
    files
}

// ============================================================================
// AC1: Pattern basique {col}_{row}
// ============================================================================

#[test]
fn test_ac1_basic_col_row_pattern() {
    let fixture = fixture_path();
    if !PathBuf::from(fixture).exists() {
        eprintln!("Skipping: fixture not found: {}", fixture);
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let config = config_with_pattern(&temp_dir, "{col}_{row}.mp");
    let args = test_args();

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    let summary = result.unwrap();
    assert!(summary.tiles_succeeded > 0, "Expected at least one tile to be exported");

    // Verify files match col_row pattern
    let mp_files = collect_mp_files(temp_dir.path());
    assert!(!mp_files.is_empty(), "Should have exported at least one .mp file");

    for file in &mp_files {
        let name = file.file_name().unwrap().to_str().unwrap();
        // Should match N_N.mp pattern
        let parts: Vec<&str> = name.trim_end_matches(".mp").split('_').collect();
        assert_eq!(parts.len(), 2, "Filename should be col_row.mp, got: {}", name);
        assert!(parts[0].parse::<usize>().is_ok(), "col should be numeric: {}", name);
        assert!(parts[1].parse::<usize>().is_ok(), "row should be numeric: {}", name);
    }
}

// ============================================================================
// AC2: Zero-padding
// ============================================================================

#[test]
fn test_ac2_zero_padding_pattern() {
    let fixture = fixture_path();
    if !PathBuf::from(fixture).exists() {
        eprintln!("Skipping: fixture not found: {}", fixture);
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let config = config_with_pattern(&temp_dir, "{col:03}_{row:03}.mp");
    let args = test_args();

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    let summary = result.unwrap();
    assert!(summary.tiles_succeeded > 0, "Expected at least one tile to be exported");

    let mp_files = collect_mp_files(temp_dir.path());
    assert!(!mp_files.is_empty());

    for file in &mp_files {
        let name = file.file_name().unwrap().to_str().unwrap();
        let parts: Vec<&str> = name.trim_end_matches(".mp").split('_').collect();
        assert_eq!(parts.len(), 2, "Should be col_row.mp: {}", name);
        // Each part should be at least 3 chars (zero-padded)
        assert!(parts[0].len() >= 3, "col should be zero-padded to 3: {}", name);
        assert!(parts[1].len() >= 3, "row should be zero-padded to 3: {}", name);
    }
}

// ============================================================================
// AC3: Sequential counter
// ============================================================================

#[test]
fn test_ac3_sequential_pattern() {
    let fixture = fixture_path();
    if !PathBuf::from(fixture).exists() {
        eprintln!("Skipping: fixture not found: {}", fixture);
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let config = config_with_pattern(&temp_dir, "{seq:04}.mp");
    let args = test_args();

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    let summary = result.unwrap();
    assert!(summary.tiles_succeeded > 0, "Expected at least one tile to be exported");

    let mp_files = collect_mp_files(temp_dir.path());
    assert!(!mp_files.is_empty());

    // Verify sequential numbering starts at 1
    let mut seq_numbers: Vec<usize> = mp_files
        .iter()
        .map(|f| {
            f.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_end_matches(".mp")
                .parse::<usize>()
                .expect("seq filename should be numeric")
        })
        .collect();
    seq_numbers.sort();

    assert_eq!(seq_numbers[0], 1, "Sequential counter should start at 1");
    // Verify contiguous sequence
    for (i, &num) in seq_numbers.iter().enumerate() {
        assert_eq!(num, i + 1, "Sequential numbers should be contiguous");
    }

    // Verify zero-padding (4 digits)
    for file in &mp_files {
        let name = file.file_name().unwrap().to_str().unwrap();
        let num_part = name.trim_end_matches(".mp");
        assert!(num_part.len() >= 4, "Should be zero-padded to 4 digits: {}", name);
    }
}

// ============================================================================
// AC4: Sous-dossiers automatiques
// ============================================================================

#[test]
fn test_ac4_subdirectory_pattern() {
    let fixture = fixture_path();
    if !PathBuf::from(fixture).exists() {
        eprintln!("Skipping: fixture not found: {}", fixture);
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let config = config_with_pattern(&temp_dir, "{col}/{row}.mp");
    let args = test_args();

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    let summary = result.unwrap();
    assert!(summary.tiles_succeeded > 0, "Expected at least one tile to be exported");

    // Verify subdirectories were created
    let mp_files = collect_mp_files(temp_dir.path());
    assert!(!mp_files.is_empty());

    for file in &mp_files {
        let relative = file.strip_prefix(temp_dir.path()).unwrap();
        let components: Vec<_> = relative.components().collect();
        // Should be col_dir/row.mp (2 components)
        assert_eq!(
            components.len(),
            2,
            "Should be in subdirectory: {}",
            relative.display()
        );
    }
}

// ============================================================================
// AC5: Default rétrocompatible
// ============================================================================

#[test]
fn test_ac5_default_pattern_retrocompatible() {
    let fixture = fixture_path();
    if !PathBuf::from(fixture).exists() {
        eprintln!("Skipping: fixture not found: {}", fixture);
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    // Config WITHOUT filename_pattern — should use default {col}_{row}.mp
    let yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.5
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
error_handling: "continue"
"#,
        fixture,
        temp_dir.path().display()
    );
    let config: Config = serde_yml::from_str(&yaml).unwrap();
    assert_eq!(config.output.filename_pattern, "{col}_{row}.mp");

    let args = test_args();
    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline failed: {:?}", result.err());

    let summary = result.unwrap();
    assert!(summary.tiles_succeeded > 0, "Expected at least one tile to be exported");

    // Verify files match col_row.mp default pattern
    let mp_files = collect_mp_files(temp_dir.path());
    assert!(!mp_files.is_empty());
    for file in &mp_files {
        let name = file.file_name().unwrap().to_str().unwrap();
        let parts: Vec<&str> = name.trim_end_matches(".mp").split('_').collect();
        assert_eq!(parts.len(), 2, "Default pattern should produce col_row.mp: {}", name);
    }
}

// ============================================================================
// AC6: Validation pattern invalide
// ============================================================================

#[test]
fn test_ac6_invalid_pattern_validation_error() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.5
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{invalid_var}.mp"
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    let result = config.validate();
    assert!(result.is_err(), "Should reject invalid pattern variable");

    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Invalid filename_pattern") || err.contains("Unknown pattern variable"),
        "Error should mention invalid pattern: {}",
        err
    );
}

#[test]
fn test_ac6_multiple_invalid_variables() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.5
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{foo}_{bar}.mp"
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_ac6_unclosed_brace_validation() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.5
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{col.mp"
"#;
    let config: Config = serde_yml::from_str(yaml).unwrap();
    let result = config.validate();
    assert!(result.is_err());
}
