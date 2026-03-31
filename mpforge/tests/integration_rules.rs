//! Integration tests for rules file parsing and validation (Story 9.1)

use mpforge::rules;
use std::path::PathBuf;

/// Helper: path to integration test fixtures
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/integration/fixtures")
        .join(name)
}

// ============================================================================
// AC1: Config YAML avec champ rules → fichier parsé
// ============================================================================

#[test]
fn test_config_with_valid_rules_path() {
    let rules_path = fixture_path("valid_rules.yaml");
    let yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
rules: "{}"
"#,
        rules_path.display()
    );
    let config: mpforge::config::Config = serde_yml::from_str(&yaml).unwrap();
    assert!(config.rules.is_some());
    assert_eq!(config.rules.as_ref().unwrap(), &rules_path);
}

#[test]
fn test_load_valid_rules_file() {
    let rules_path = fixture_path("valid_rules.yaml");
    let rules_file = rules::load_rules(&rules_path).unwrap();
    assert_eq!(rules_file.version, 1);
    assert_eq!(rules_file.rulesets.len(), 2);
    assert_eq!(rules_file.rulesets[0].source_layer, "TRONCON_DE_ROUTE");
    assert_eq!(rules_file.rulesets[0].rules.len(), 2);
    assert_eq!(rules_file.rulesets[1].source_layer, "COURS_D_EAU");
    assert_eq!(rules_file.rulesets[1].rules.len(), 1);
}

// ============================================================================
// AC3: Erreur fichier invalide → message clair
// ============================================================================

#[test]
fn test_load_invalid_rules_no_source_layer() {
    let rules_path = fixture_path("invalid_rules_no_source_layer.yaml");
    let result = rules::load_rules(&rules_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("missing source_layer"),
        "Expected 'missing source_layer' in error, got: {}",
        err
    );
}

#[test]
fn test_load_invalid_rules_no_set() {
    let rules_path = fixture_path("invalid_rules_no_set.yaml");
    let result = rules::load_rules(&rules_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("missing set"),
        "Expected 'missing set' in error, got: {}",
        err
    );
}

#[test]
fn test_load_malformed_rules_yaml() {
    let rules_path = fixture_path("malformed_rules.yaml");
    let result = rules::load_rules(&rules_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Failed to parse YAML rules file"),
        "Expected YAML parse error, got: {}",
        err
    );
}

// ============================================================================
// AC4: Config sans champ rules → aucune régression
// ============================================================================

#[test]
fn test_config_without_rules_backward_compat() {
    let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
    let config: mpforge::config::Config = serde_yml::from_str(yaml).unwrap();
    assert!(config.rules.is_none());
    assert!(config.validate().is_ok());
}

// ============================================================================
// AC3: rules pointe vers fichier inexistant → erreur claire
// ============================================================================

#[test]
fn test_load_rules_nonexistent_file() {
    let rules_path = PathBuf::from("/nonexistent/path/rules.yaml");
    let result = rules::load_rules(&rules_path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Failed to read rules file"),
        "Expected 'Failed to read rules file' in error, got: {}",
        err
    );
}
