//! Common test helpers for rules integration tests.
//!
//! Shared utilities used by integration_rules_transport.rs,
//! integration_rules_admin_hydro.rs, and future rules test files.

use mpforge_cli::rules;
use std::collections::HashMap;
use std::path::PathBuf;

/// Path to the production rules file
pub fn rules_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/bdtopo-garmin-rules.yaml")
}

/// Helper: load rules and find a specific ruleset by source_layer
pub fn load_ruleset(source_layer: &str) -> rules::Ruleset {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules file");
    rules_file
        .rulesets
        .into_iter()
        .find(|rs| rs.source_layer == source_layer)
        .unwrap_or_else(|| panic!("Ruleset not found for layer: {}", source_layer))
}

/// Helper: build feature attributes from key-value pairs
pub fn attrs(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Helper: evaluate a feature and assert Type, EndLevel, and optionally Label
pub fn assert_transform(
    ruleset: &rules::Ruleset,
    feature_attrs: &HashMap<String, String>,
    expected_type: &str,
    expected_end_level: &str,
    expected_label: Option<&str>,
) {
    let result = rules::evaluate_feature(ruleset, feature_attrs)
        .expect("Rule evaluation should not error")
        .expect("Feature should match a rule");

    assert_eq!(
        result.get("Type").unwrap(),
        expected_type,
        "Type mismatch for attrs {:?}",
        feature_attrs
    );
    assert_eq!(
        result.get("EndLevel").unwrap(),
        expected_end_level,
        "EndLevel mismatch for attrs {:?}",
        feature_attrs
    );

    if let Some(label) = expected_label {
        assert_eq!(
            result.get("Label").unwrap(),
            label,
            "Label mismatch for attrs {:?}",
            feature_attrs
        );
    } else {
        assert!(
            !result.contains_key("Label"),
            "Expected no Label for attrs {:?}, got {:?}",
            feature_attrs,
            result.get("Label")
        );
    }
}
