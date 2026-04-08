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
// label_case integration tests
// ============================================================================

#[test]
fn test_load_rules_with_label_case() {
    let rules_path = fixture_path("valid_rules_label_case.yaml");
    let rules_file = rules::load_rules(&rules_path).unwrap();
    assert_eq!(rules_file.rulesets.len(), 2);

    // Ruleset-level label_case
    assert_eq!(
        rules_file.rulesets[0].label_case,
        Some(rules::LabelCase::Title)
    );
    // Rule-level override
    assert_eq!(
        rules_file.rulesets[0].rules[0].label_case,
        Some(rules::LabelCase::Upper)
    );
    // Rule without override
    assert_eq!(rules_file.rulesets[0].rules[1].label_case, None);

    // Ruleset without label_case
    assert_eq!(rules_file.rulesets[1].label_case, None);
    // Rule with label_case on ruleset without
    assert_eq!(
        rules_file.rulesets[1].rules[0].label_case,
        Some(rules::LabelCase::Upper)
    );
}

#[test]
fn test_label_case_evaluate_feature_integration() {
    let rules_path = fixture_path("valid_rules_label_case.yaml");
    let rules_file = rules::load_rules(&rules_path).unwrap();

    // Test rule override (Montagne → upper, ruleset is title)
    let attrs = std::collections::HashMap::from([
        ("CLASSE".to_string(), "Montagne".to_string()),
        ("GRAPHIE".to_string(), "mont blanc".to_string()),
    ]);
    let result = rules::evaluate_feature(&rules_file.rulesets[0], &attrs)
        .expect("rule evaluation should succeed")
        .expect("should match Montagne rule");
    assert_eq!(result.get("Label").expect("Label field should exist"), "MONT BLANC");

    // Test ruleset default (Cours d'eau → title from ruleset)
    let attrs2 = std::collections::HashMap::from([
        ("CLASSE".to_string(), "Cours d'eau".to_string()),
        ("GRAPHIE".to_string(), "la durance".to_string()),
    ]);
    let result2 = rules::evaluate_feature(&rules_file.rulesets[0], &attrs2)
        .expect("rule evaluation should succeed")
        .expect("should match Cours d'eau rule");
    assert_eq!(result2.get("Label").expect("Label field should exist"), "La Durance");
}

#[test]
fn test_backward_compat_existing_rules_without_label_case() {
    // Existing valid_rules.yaml has no label_case at all
    let rules_path = fixture_path("valid_rules.yaml");
    let rules_file = rules::load_rules(&rules_path).unwrap();

    assert_eq!(rules_file.rulesets[0].label_case, None);
    for rule in &rules_file.rulesets[0].rules {
        assert_eq!(rule.label_case, None);
    }

    // Labels should pass through unchanged
    let attrs = std::collections::HashMap::from([
        ("CL_ADMIN".to_string(), "Autoroute".to_string()),
        ("NUMERO".to_string(), "A7".to_string()),
    ]);
    let result = rules::evaluate_feature(&rules_file.rulesets[0], &attrs)
        .unwrap()
        .unwrap();
    assert_eq!(result.get("Label").unwrap(), "A7");
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
