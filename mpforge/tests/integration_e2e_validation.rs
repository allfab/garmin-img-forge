//! Integration tests: end-to-end validation of the complete rules file (Story 10.5)
//!
//! Validates structural integrity of bdtopo-garmin-rules.yaml:
//!   - All 21 rulesets parse without error
//!   - Each ruleset has valid source_layer and at least 1 rule
//!   - Total rule count matches expected (≥ 274)
//!   - All Type values are valid hex format
//!   - All EndLevel values are in range 1-9

mod common;

use common::rules_path;
use mpforge::rules;

// ============================================================================
// Task 2.1: Load all 21 rulesets and verify structural integrity
// ============================================================================

#[test]
fn test_rules_file_loads_without_error() {
    let rules_file = rules::load_rules(&rules_path()).expect("Rules file should parse without error");
    assert_eq!(rules_file.version, 1, "Rules file version should be 1");
    assert_eq!(
        rules_file.rulesets.len(),
        21,
        "Should have exactly 21 rulesets"
    );
}

#[test]
fn test_each_ruleset_has_valid_source_layer_and_rules() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    for rs in &rules_file.rulesets {
        assert!(
            !rs.source_layer.is_empty(),
            "Ruleset {:?} has empty source_layer",
            rs.name
        );
        assert!(
            !rs.rules.is_empty(),
            "Ruleset {} ({:?}) has no rules",
            rs.source_layer,
            rs.name
        );
    }
}

// ============================================================================
// Task 2.2: Coverage — each expected BDTOPO layer has a corresponding ruleset
// ============================================================================

#[test]
fn test_source_layer_coverage_all_21_bdtopo_layers() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let source_layers: Vec<&str> = rules_file
        .rulesets
        .iter()
        .map(|rs| rs.source_layer.as_str())
        .collect();

    let expected_layers = [
        // Transport (FME 01)
        "TRONCON_DE_ROUTE",
        "TRONCON_DE_VOIE_FERREE",
        "PISTE_D_AERODROME",
        "TRANSPORT_PAR_CABLE",
        // Administratif (FME 01 Chef-lieu)
        "COMMUNE",
        "ZONE_D_HABITATION",
        // Hydrographie (FME 04)
        "TRONCON_HYDROGRAPHIQUE",
        "SURFACE_HYDROGRAPHIQUE",
        "DETAIL_HYDROGRAPHIQUE",
        // Bati (FME 06)
        "BATIMENT",
        "CIMETIERE",
        "CONSTRUCTION_LINEAIRE",
        "CONSTRUCTION_PONCTUELLE",
        "PYLONE",
        "TERRAIN_DE_SPORT",
        // Orographie (FME 07)
        "LIGNE_OROGRAPHIQUE",
        // Vegetation (FME 05)
        "ZONE_DE_VEGETATION",
        // Services (FME 03)
        "ZONE_D_ACTIVITE_OU_D_INTERET",
        "LIGNE_ELECTRIQUE",
        // Zones reglementees (FME 02)
        "FORET_PUBLIQUE",
        // Toponymie (FME 08)
        "TOPONYMIE",
    ];

    assert_eq!(
        expected_layers.len(),
        21,
        "Expected layers list should have 21 entries"
    );

    for layer in &expected_layers {
        assert!(
            source_layers.contains(layer),
            "Missing ruleset for BDTOPO layer: {}",
            layer
        );
    }
}

// ============================================================================
// Task 2.3: Total rule count ≥ 274
// ============================================================================

#[test]
fn test_total_rule_count_at_least_274() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let total: usize = rules_file.rulesets.iter().map(|rs| rs.rules.len()).sum();
    assert!(
        total >= 274,
        "Total rule count should be >= 274, got {}",
        total
    );
}

// ============================================================================
// Task 2.4: All Type values match hex pattern ^0x[0-9a-fA-F]+$
// ============================================================================

#[test]
fn test_all_type_values_are_valid_hex() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let hex_pattern = regex::Regex::new(r"^0x[0-9a-fA-F]+$").unwrap();

    for rs in &rules_file.rulesets {
        for (i, rule) in rs.rules.iter().enumerate() {
            if let Some(type_val) = rule.set.get("Type") {
                assert!(
                    hex_pattern.is_match(type_val),
                    "Invalid Type hex '{}' in ruleset {} rule #{}",
                    type_val,
                    rs.source_layer,
                    i + 1
                );
            } else {
                panic!(
                    "Missing Type in ruleset {} rule #{} (set: {:?})",
                    rs.source_layer,
                    i + 1,
                    rule.set
                );
            }
        }
    }
}

// ============================================================================
// Task 2.5: All EndLevel values in range 1-9
// ============================================================================

#[test]
fn test_all_endlevel_values_in_range_1_to_9() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");

    for rs in &rules_file.rulesets {
        for (i, rule) in rs.rules.iter().enumerate() {
            if let Some(endlevel_str) = rule.set.get("EndLevel") {
                let endlevel: u32 = endlevel_str.parse().unwrap_or_else(|_| {
                    panic!(
                        "EndLevel '{}' is not a number in ruleset {} rule #{}",
                        endlevel_str,
                        rs.source_layer,
                        i + 1
                    )
                });
                assert!(
                    (1..=9).contains(&endlevel),
                    "EndLevel {} out of range 1-9 in ruleset {} rule #{}",
                    endlevel,
                    rs.source_layer,
                    i + 1
                );
            } else {
                panic!(
                    "Missing EndLevel in ruleset {} rule #{} (set: {:?})",
                    rs.source_layer,
                    i + 1,
                    rule.set
                );
            }
        }
    }
}

// ============================================================================
// Summary: Per-ruleset rule counts
// ============================================================================

#[test]
fn test_per_ruleset_rule_counts() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");

    // Actual counts from production rules file (source of truth)
    let expected_counts: Vec<(&str, usize)> = vec![
        ("TRONCON_DE_ROUTE", 17),
        ("TRONCON_DE_VOIE_FERREE", 7),
        ("PISTE_D_AERODROME", 2),
        ("TRANSPORT_PAR_CABLE", 1),
        ("COMMUNE", 1),
        ("ZONE_D_HABITATION", 1),
        ("TRONCON_HYDROGRAPHIQUE", 2),
        ("SURFACE_HYDROGRAPHIQUE", 4),
        ("DETAIL_HYDROGRAPHIQUE", 13),
        ("BATIMENT", 13),
        ("CIMETIERE", 3),
        ("CONSTRUCTION_LINEAIRE", 12),
        ("CONSTRUCTION_PONCTUELLE", 12),
        ("PYLONE", 1),
        ("TERRAIN_DE_SPORT", 6),
        ("LIGNE_OROGRAPHIQUE", 3),
        ("ZONE_DE_VEGETATION", 11),
        ("ZONE_D_ACTIVITE_OU_D_INTERET", 1),
        ("LIGNE_ELECTRIQUE", 1),
        ("FORET_PUBLIQUE", 1),
        ("TOPONYMIE", 162),
    ];

    for (layer, expected) in &expected_counts {
        let rs = rules_file
            .rulesets
            .iter()
            .find(|r| r.source_layer == *layer)
            .unwrap_or_else(|| panic!("Ruleset not found: {}", layer));
        assert_eq!(
            rs.rules.len(),
            *expected,
            "Ruleset {} has {} rules, expected exactly {}",
            layer,
            rs.rules.len(),
            expected
        );
    }
}
