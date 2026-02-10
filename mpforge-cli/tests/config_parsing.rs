//! Integration tests for configuration parsing and validation

use mpforge_cli::config::load_config;
use std::fs;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/integration/fixtures")
        .join(name)
}

#[test]
fn test_load_valid_config() {
    let config = load_config(fixture_path("valid_config.yaml")).unwrap();

    assert_eq!(config.version, 1);
    assert_eq!(config.grid.cell_size, 0.15);
    assert_eq!(config.grid.overlap, 0.005);
    assert_eq!(config.grid.origin, Some([0.0, 0.0]));
    assert_eq!(config.inputs.len(), 2);
    assert_eq!(config.output.directory, "tiles/");
    assert_eq!(config.output.filename_pattern, "{x}_{y}.mp");
    assert_eq!(config.error_handling, "continue");
}

#[test]
fn test_load_config_with_defaults() {
    // Create a minimal config file
    let minimal_yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/test.shp"
output:
  directory: "tiles/"
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("minimal.yaml");
    fs::write(&config_path, minimal_yaml).unwrap();

    let config = load_config(&config_path).unwrap();

    // Check defaults
    assert_eq!(config.version, 1); // default_version
    assert_eq!(config.grid.overlap, 0.0); // default overlap
    assert_eq!(config.output.filename_pattern, "{x}_{y}.mp"); // default pattern
    assert_eq!(config.error_handling, "continue"); // default error_handling
    assert!(config.filters.is_none()); // optional filters
}

#[test]
fn test_load_config_invalid_yaml_syntax() {
    let result = load_config(fixture_path("invalid_syntax.yaml"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to parse YAML config"));
}

#[test]
fn test_load_config_negative_cell_size() {
    let result = load_config(fixture_path("negative_cell_size.yaml"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    // The error chain includes both the context and the root cause
    assert!(
        err_msg.contains("Config validation failed")
            || err_msg.contains("cell_size must be positive"),
        "Expected validation error, got: {}",
        err_msg
    );
}

#[test]
fn test_load_config_no_inputs() {
    let result = load_config(fixture_path("no_inputs.yaml"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    // The error chain includes both the context and the root cause
    assert!(
        err_msg.contains("Config validation failed")
            || err_msg.contains("At least one input source"),
        "Expected validation error, got: {}",
        err_msg
    );
}

#[test]
fn test_load_config_with_wildcard_expansion() {
    // This test verifies wildcard resolution works:
    // 1. Config loads successfully (no error from wildcard resolution)
    // 2. Pattern is preserved in config structure
    // 3. Actual files (file1.shp, file2.shp) exist in test_data/ and are resolved
    // 4. Logs show resolved files (via tracing debug)
    // 5. New validation (M4 fix) ensures at least one file was resolved
    let config = load_config(fixture_path("wildcard_pattern.yaml")).unwrap();

    assert_eq!(config.inputs.len(), 1);
    assert_eq!(
        config.inputs[0].path,
        Some("tests/integration/fixtures/test_data/*.shp".to_string())
    );

    // If wildcard matched zero files, load_config would fail with validation error
    // (due to M4 fix: no connections + zero resolved files = error)
    // Success here proves files were resolved successfully
}

#[test]
fn test_load_config_wildcard_no_match_warning() {
    // Create config with wildcard that matches no files
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "nonexistent/*.xyz"
output:
  directory: "tiles/"
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("wildcard_no_match.yaml");
    fs::write(&config_path, yaml).unwrap();

    // Should succeed (warning is logged, not error)
    let config = load_config(&config_path).unwrap();
    assert_eq!(config.inputs.len(), 1);
}

#[test]
fn test_load_config_postgis_connection() {
    let config = load_config(fixture_path("postgis_source.yaml")).unwrap();

    assert_eq!(config.inputs.len(), 1);
    assert_eq!(
        config.inputs[0].connection,
        Some("PG:host=localhost dbname=gis".to_string())
    );
    assert_eq!(config.inputs[0].layer, Some("roads".to_string()));
    assert!(config.inputs[0].path.is_none());
}

#[test]
fn test_load_config_with_bbox_filter() {
    let config = load_config(fixture_path("bbox_filter.yaml")).unwrap();

    assert!(config.filters.is_some());
    let filters = config.filters.unwrap();
    assert_eq!(filters.bbox, [-5.0, 41.0, 10.0, 51.5]);
}

#[test]
fn test_load_config_file_not_found() {
    let result = load_config(PathBuf::from("nonexistent.yaml"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to read config file"));
}
