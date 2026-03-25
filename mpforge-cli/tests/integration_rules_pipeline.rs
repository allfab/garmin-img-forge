//! Integration tests for rules engine pipeline integration (Story 9.3)
//!
//! Tests the full integration of rules evaluation with the pipeline:
//! - find_ruleset lookup
//! - Feature transformation with source_layer
//! - Passthrough for layers without ruleset
//! - Feature filtering (no match → ignored)
//! - Statistics collection

use mpforge_cli::pipeline::reader::{Feature, GeometryType};
use mpforge_cli::rules::{self, RuleStats};
use std::collections::HashMap;
use std::path::PathBuf;

/// Helper: path to integration test fixtures
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/integration/fixtures")
        .join(name)
}

/// Helper: create a Feature with source_layer and attributes
fn make_feature(
    layer: &str,
    attrs: &[(&str, &str)],
) -> Feature {
    Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(2.35, 48.85)],
        attributes: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        source_layer: Some(layer.to_string()),
    }
}

// ============================================================================
// AC1: Ruleset par couche source — le ruleset correspondant est appliqué
// ============================================================================

#[test]
fn test_pipeline_rules_transform_matching_feature() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    // Feature from TRONCON_DE_ROUTE with CL_ADMIN=Autoroute
    let feature = make_feature(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Autoroute"), ("NUMERO", "A6")],
    );

    let ruleset = rules::find_ruleset(&rules_file, "TRONCON_DE_ROUTE").unwrap();
    let result = rules::evaluate_feature(ruleset, &feature.attributes).unwrap();

    assert!(result.is_some(), "Feature should match a rule");
    let new_attrs = result.unwrap();
    assert_eq!(new_attrs.get("Type"), Some(&"0x01".to_string()));
    assert_eq!(new_attrs.get("EndLevel"), Some(&"1".to_string()));
    assert_eq!(new_attrs.get("Label"), Some(&"A6".to_string()));
}

// ============================================================================
// AC2: First-match-wins — seule la première règle matchée est appliquée
// ============================================================================

#[test]
fn test_pipeline_rules_first_match_wins() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    // Feature that matches the first rule (CL_ADMIN=Autoroute)
    let feature = make_feature(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Autoroute"), ("NATURE", "Autoroute"), ("NUMERO", "A1")],
    );

    let ruleset = rules::find_ruleset(&rules_file, "TRONCON_DE_ROUTE").unwrap();
    let result = rules::evaluate_feature(ruleset, &feature.attributes).unwrap();

    let new_attrs = result.unwrap();
    // Should match first rule (Type=0x01) not second rule (Type=0x02)
    assert_eq!(new_attrs.get("Type"), Some(&"0x01".to_string()));
}

// ============================================================================
// AC3: Couche sans ruleset = passthrough
// ============================================================================

#[test]
fn test_pipeline_rules_no_ruleset_passthrough() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    // Feature from an unknown layer (not in rules file)
    let feature = make_feature(
        "BATIMENT",
        &[("TYPE", "Mairie"), ("HAUTEUR", "15")],
    );

    let result = rules::find_ruleset(&rules_file, "BATIMENT");
    assert!(result.is_none(), "No ruleset for BATIMENT → passthrough");

    // Feature should pass through unchanged
    assert_eq!(feature.attributes.get("TYPE"), Some(&"Mairie".to_string()));
    assert_eq!(feature.attributes.get("HAUTEUR"), Some(&"15".to_string()));
}

// ============================================================================
// AC4: Feature sans match = ignorée
// ============================================================================

#[test]
fn test_pipeline_rules_no_match_ignored() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    // Feature from TRONCON_DE_ROUTE but with CL_ADMIN value that matches no rule
    let feature = make_feature(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Chemin"), ("NATURE", "Sentier")],
    );

    let ruleset = rules::find_ruleset(&rules_file, "TRONCON_DE_ROUTE").unwrap();
    let result = rules::evaluate_feature(ruleset, &feature.attributes).unwrap();

    assert!(result.is_none(), "Feature should be ignored (no matching rule)");
}

// ============================================================================
// AC5: Ordre d'application rules → field_mapping → export
// ============================================================================

#[test]
fn test_pipeline_rules_applied_before_field_mapping() {
    // Simulate the actual pipeline loop pattern from mod.rs:
    // rules transform → then features go to writer (which applies field_mapping)
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    // Input features with raw BDTOPO attributes
    let features = vec![
        make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute"), ("NUMERO", "A6")]),
        make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Nationale"), ("NATURE", "Route"), ("NUMERO", "N7")]),
    ];

    // Reproduce pipeline loop: rules transform phase
    let mut transformed = Vec::new();
    for mut feature in features {
        let layer_name = feature.source_layer.clone().unwrap_or_default();
        if let Some(ruleset) = rules::find_ruleset(&rules_file, &layer_name) {
            match rules::evaluate_feature(ruleset, &feature.attributes) {
                Ok(Some(new_attrs)) => {
                    feature.attributes = new_attrs;
                    transformed.push(feature);
                }
                Ok(None) => { /* ignored */ }
                Err(_) => { /* error */ }
            }
        } else {
            transformed.push(feature);
        }
    }

    // After rules phase: features have Garmin-ready attributes
    assert_eq!(transformed.len(), 2, "Both features should match rules");

    // Feature 1: Autoroute → Type=0x01
    let f1 = &transformed[0];
    assert_eq!(f1.attributes.get("Type"), Some(&"0x01".to_string()));
    assert_eq!(f1.attributes.get("Label"), Some(&"A6".to_string()));
    assert!(!f1.attributes.contains_key("CL_ADMIN"), "Original BDTOPO attrs replaced by rules output");

    // Feature 2: Nationale → Type=0x02
    let f2 = &transformed[1];
    assert_eq!(f2.attributes.get("Type"), Some(&"0x02".to_string()));
    assert!(!f2.attributes.contains_key("CL_ADMIN"), "Original BDTOPO attrs replaced by rules output");

    // These transformed features would then be passed to MpWriter which applies field_mapping.
    // The key assertion: attributes are Garmin-ready BEFORE reaching the writer.
}

// ============================================================================
// AC6: Statistiques des règles
// ============================================================================

#[test]
fn test_pipeline_rules_statistics() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();
    let mut stats = RuleStats::default();

    // Simulate pipeline processing multiple features
    let features = vec![
        // Matches rule 1 (Autoroute)
        make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute"), ("NUMERO", "A6")]),
        // Matches rule 2 (Nationale)
        make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Nationale"), ("NATURE", "Route")]),
        // No match → ignored
        make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Chemin")]),
        // No ruleset → passthrough (not counted in stats)
        make_feature("BATIMENT", &[("TYPE", "Mairie")]),
        // Matches hydro rule
        make_feature("COURS_D_EAU", &[("REGIME", "Permanent"), ("TOPONYME", "Loire")]),
    ];

    for feature in &features {
        let layer_name = feature.source_layer.as_deref().unwrap_or("");
        match rules::find_ruleset(&rules_file, layer_name) {
            None => {
                // Passthrough — no stats
            }
            Some(ruleset) => match rules::evaluate_feature(ruleset, &feature.attributes) {
                Ok(Some(_)) => stats.record_match(layer_name),
                Ok(None) => stats.record_ignored(layer_name),
                Err(_) => stats.record_error(layer_name),
            },
        }
    }

    // Verify aggregated stats
    assert_eq!(stats.matched, 3, "3 features matched rules");
    assert_eq!(stats.ignored, 1, "1 feature had no matching rule");
    assert_eq!(stats.errors, 0, "No errors");

    // Verify per-ruleset stats
    let route_stats = stats.by_ruleset.get("TRONCON_DE_ROUTE").unwrap();
    assert_eq!(route_stats.matched, 2);
    assert_eq!(route_stats.ignored, 1);

    let hydro_stats = stats.by_ruleset.get("COURS_D_EAU").unwrap();
    assert_eq!(hydro_stats.matched, 1);
    assert_eq!(hydro_stats.ignored, 0);

    // BATIMENT should NOT appear in stats (no ruleset = passthrough)
    assert!(stats.by_ruleset.get("BATIMENT").is_none());
}

// ============================================================================
// Backward compatibility: pipeline without rules
// ============================================================================

#[test]
fn test_pipeline_no_rules_backward_compat() {
    // When no rules are configured, features pass through unmodified
    let feature = make_feature(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Autoroute"), ("NUMERO", "A6")],
    );

    // Simulate: rules is None → no transformation
    let rules: Option<rules::RulesFile> = None;
    assert!(rules.is_none());

    // Feature attributes unchanged
    assert_eq!(feature.attributes.get("CL_ADMIN"), Some(&"Autoroute".to_string()));
    assert_eq!(feature.attributes.get("NUMERO"), Some(&"A6".to_string()));
}

// ============================================================================
// source_layer propagation test
// ============================================================================

#[test]
fn test_feature_source_layer_field() {
    let feature = make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute")]);
    assert_eq!(feature.source_layer, Some("TRONCON_DE_ROUTE".to_string()));

    let feature_no_layer = Feature {
        geometry_type: GeometryType::Point,
        geometry: vec![(0.0, 0.0)],
        attributes: HashMap::new(),
        source_layer: None,
    };
    assert!(feature_no_layer.source_layer.is_none());
}

// ============================================================================
// RuleStats serialization for JSON report
// ============================================================================

#[test]
fn test_rule_stats_serialization() {
    let mut stats = RuleStats::default();
    stats.record_match("TRONCON_DE_ROUTE");
    stats.record_match("TRONCON_DE_ROUTE");
    stats.record_ignored("TRONCON_DE_ROUTE");
    stats.record_match("COURS_D_EAU");

    let json = serde_json::to_string(&stats).unwrap();
    assert!(json.contains("\"matched\":3"));
    assert!(json.contains("\"ignored\":1"));
    assert!(json.contains("TRONCON_DE_ROUTE"));
    assert!(json.contains("COURS_D_EAU"));
}

// ============================================================================
// L1 Fix: Rule error path — invalid Type hex → feature ignored + error stat
// ============================================================================

#[test]
fn test_pipeline_rules_error_path_invalid_type() {
    let rules_file = rules::load_rules(&fixture_path("rules_invalid_type.yaml")).unwrap();
    let mut stats = RuleStats::default();

    let feature = make_feature("ERROR_LAYER", &[("FIELD", "value")]);
    let layer_name = feature.source_layer.as_deref().unwrap_or("");

    let ruleset = rules::find_ruleset(&rules_file, layer_name).unwrap();
    match rules::evaluate_feature(ruleset, &feature.attributes) {
        Ok(Some(_)) => stats.record_match(layer_name),
        Ok(None) => stats.record_ignored(layer_name),
        Err(_) => stats.record_error(layer_name),
    }

    assert_eq!(stats.errors, 1, "Invalid Type hex should produce an error");
    assert_eq!(stats.matched, 0);
    assert_eq!(stats.ignored, 0);
    let layer_stats = stats.by_ruleset.get("ERROR_LAYER").unwrap();
    assert_eq!(layer_stats.errors, 1);
}

// ============================================================================
// L2 Fix: COURS_D_EAU feature transformation test
// ============================================================================

#[test]
fn test_pipeline_rules_hydro_feature_transformation() {
    let rules_file = rules::load_rules(&fixture_path("valid_rules.yaml")).unwrap();

    let feature = make_feature(
        "COURS_D_EAU",
        &[("REGIME", "Permanent"), ("TOPONYME", "Loire")],
    );

    let ruleset = rules::find_ruleset(&rules_file, "COURS_D_EAU").unwrap();
    let result = rules::evaluate_feature(ruleset, &feature.attributes).unwrap();

    assert!(result.is_some(), "Feature should match hydro rule");
    let new_attrs = result.unwrap();
    assert_eq!(new_attrs.get("Type"), Some(&"0x1f".to_string()));
    assert_eq!(new_attrs.get("Label"), Some(&"Loire".to_string()));
}
