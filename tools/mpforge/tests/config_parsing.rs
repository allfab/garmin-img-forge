//! Integration tests for configuration parsing and validation

use mpforge::config::load_config;
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
    assert_eq!(config.output.filename_pattern, "{x}_{y}.mp"); // explicit in fixture
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
    assert_eq!(config.output.filename_pattern, "{col}_{row}.mp"); // default pattern (Story 8.2)
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
    // Wildcard patterns are expanded into N InputSource clones,
    // one per resolved file. test_data/ has file1.shp + file2.shp.
    let config = load_config(fixture_path("wildcard_pattern.yaml")).unwrap();

    assert_eq!(config.inputs.len(), 2, "Wildcard should expand to 2 inputs (file1.shp, file2.shp)");
    // Expanded paths should be concrete (no glob characters)
    for input in &config.inputs {
        let path = input.path.as_ref().unwrap();
        assert!(!path.contains('*'), "Expanded path should not contain wildcard: {}", path);
        assert!(path.ends_with(".shp"), "Expanded path should end with .shp: {}", path);
    }
}

#[test]
fn test_load_config_wildcard_no_match_fails_validation() {
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

    // No match → 0 inputs after expansion → validation fails
    let result = load_config(&config_path);
    assert!(result.is_err(), "Config with only unresolvable wildcard should fail validation");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Config validation failed") || err_msg.contains("At least one input"),
        "Expected validation error, got: {}", err_msg
    );
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
fn test_input_with_attribute_filter() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/courbes.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    attribute_filter: "CAST(ALTITUDE AS INTEGER) % 10 = 0"
output:
  directory: "tiles/"
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("attr_filter.yaml");
    fs::write(&config_path, yaml).unwrap();

    let config = load_config(&config_path).unwrap();
    assert_eq!(config.inputs.len(), 1);
    assert_eq!(
        config.inputs[0].attribute_filter,
        Some("CAST(ALTITUDE AS INTEGER) % 10 = 0".to_string())
    );
}

#[test]
fn test_input_with_layer_alias() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data/COURBE_0840_6440.shp"
    layer_alias: "COURBE"
output:
  directory: "tiles/"
"#;

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("layer_alias.yaml");
    fs::write(&config_path, yaml).unwrap();

    let config = load_config(&config_path).unwrap();
    assert_eq!(config.inputs.len(), 1);
    assert_eq!(
        config.inputs[0].layer_alias,
        Some("COURBE".to_string())
    );
}

#[test]
fn test_wildcard_expansion_inherits_properties() {
    // Wildcard expansion must clone attribute_filter, layer_alias, source_srs, target_srs
    let yaml = format!(r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "{}/*.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
    attribute_filter: "ALTITUDE > 0"
    layer_alias: "COURBE"
output:
  directory: "tiles/"
"#, fixture_path("test_data").display());

    let temp_dir = tempfile::TempDir::new().unwrap();
    let config_path = temp_dir.path().join("wildcard_inherit.yaml");
    fs::write(&config_path, yaml).unwrap();

    let config = load_config(&config_path).unwrap();
    assert_eq!(config.inputs.len(), 2, "Should expand to 2 inputs");
    for input in &config.inputs {
        assert_eq!(input.source_srs, Some("EPSG:2154".to_string()));
        assert_eq!(input.target_srs, Some("EPSG:4326".to_string()));
        assert_eq!(input.attribute_filter, Some("ALTITUDE > 0".to_string()));
        assert_eq!(input.layer_alias, Some("COURBE".to_string()));
    }
}

#[test]
fn test_load_config_file_not_found() {
    let result = load_config(PathBuf::from("nonexistent.yaml"));

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Failed to read config file"));
}
