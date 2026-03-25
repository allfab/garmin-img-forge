//! Integration tests for transport rulesets (Story 10.1)
//!
//! Validates that the BDTOPO transport rules in bdtopo-garmin-rules.yaml
//! produce the correct Garmin attributes (Type, EndLevel, Label) matching
//! the FME 07-VOIRIE.fmw source of truth.

mod common;

use common::{assert_transform, attrs, load_ruleset, rules_path};
use mpforge_cli::rules;

// ============================================================================
// Task 2.1: Validate production rules file loads correctly
// ============================================================================

#[test]
fn test_load_production_rules_file() {
    let rules_file = rules::load_rules(&rules_path()).unwrap();
    assert_eq!(rules_file.version, 1);
    // Verify the 4 transport rulesets exist with correct rule counts
    // (no assertion on total rulesets.len() — other stories add more rulesets)
    let transport_layers = [
        ("TRONCON_DE_ROUTE", 17),
        ("TRONCON_DE_VOIE_FERREE", 7),
        ("PISTE_D_AERODROME", 2),
        ("TRANSPORT_PAR_CABLE", 1),
    ];
    for (layer, expected_count) in &transport_layers {
        let rs = rules_file
            .rulesets
            .iter()
            .find(|rs| rs.source_layer == *layer)
            .unwrap_or_else(|| panic!("Transport ruleset {} not found", layer));
        assert_eq!(
            rs.rules.len(),
            *expected_count,
            "Rule count mismatch for {}",
            layer
        );
    }
}

// ============================================================================
// Task 2.2: AC1 — TRONCON_DE_ROUTE CL_ADMIN="Autoroute"
// ============================================================================

#[test]
fn test_ac1_autoroute_with_numero() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    let feature = attrs(&[
        ("CL_ADMIN", "Autoroute"),
        ("NATURE", "Type autoroutier"),
        ("NUMERO", "A48"),
    ]);
    assert_transform(&ruleset, &feature, "0x01", "7", Some("~[0x04]A48"));
}

#[test]
fn test_ac1_autoroute_without_numero() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    let feature = attrs(&[
        ("CL_ADMIN", "Autoroute"),
        ("NATURE", "Type autoroutier"),
    ]);
    // NUMERO absent -> Label = "~[0x04]" (empty substitution)
    assert_transform(&ruleset, &feature, "0x01", "7", Some("~[0x04]"));
}

// ============================================================================
// Task 2.3: AC2 — TRONCON_DE_ROUTE NATURE="Sentier"
// ============================================================================

#[test]
fn test_ac2_sentier_without_cl_admin() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    let feature = attrs(&[("NATURE", "Sentier")]);
    assert_transform(&ruleset, &feature, "0x10", "1", None);
}

#[test]
fn test_ac2_sentier_with_empty_cl_admin() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    let feature = attrs(&[("CL_ADMIN", ""), ("NATURE", "Sentier")]);
    assert_transform(&ruleset, &feature, "0x10", "1", None);
}

// ============================================================================
// Task 2.4: AC3 — TRONCON_DE_VOIE_FERREE NATURE="LGV", POS_SOL="0"
// ============================================================================

#[test]
fn test_ac3_lgv_surface() {
    let ruleset = load_ruleset("TRONCON_DE_VOIE_FERREE");
    let feature = attrs(&[("NATURE", "LGV"), ("POS_SOL", "0"), ("TOPONYME", "LGV Sud-Est")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10e02",
        "5",
        Some("LGV Sud-Est"),
    );
}

// ============================================================================
// Task 2.5: AC4 — TRONCON_DE_VOIE_FERREE POS_SOL="-1" (souterrain)
// ============================================================================

#[test]
fn test_ac4_voie_ferree_souterrain_override_nature() {
    let ruleset = load_ruleset("TRONCON_DE_VOIE_FERREE");
    // LGV en souterrain -> souterrain prend le dessus
    let feature = attrs(&[
        ("NATURE", "LGV"),
        ("POS_SOL", "-1"),
        ("TOPONYME", "Tunnel LGV"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10e06",
        "5",
        Some("Tunnel LGV"),
    );
}

#[test]
fn test_ac4_voie_ferree_souterrain_principale() {
    let ruleset = load_ruleset("TRONCON_DE_VOIE_FERREE");
    let feature = attrs(&[
        ("NATURE", "Voie ferrée principale"),
        ("POS_SOL", "-1"),
        ("TOPONYME", "Tunnel"),
    ]);
    assert_transform(&ruleset, &feature, "0x10e06", "5", Some("Tunnel"));
}

// ============================================================================
// Task 2.6: AC5 — PISTE_D_AERODROME NATURE="Piste en dur"
// ============================================================================

#[test]
fn test_ac5_piste_aerodrome_en_dur() {
    let ruleset = load_ruleset("PISTE_D_AERODROME");
    let feature = attrs(&[
        ("NATURE", "Piste en dur"),
        ("TOPONYME", "Piste 09/27"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x1090b",
        "4",
        Some("Piste 09/27"),
    );
}

#[test]
fn test_ac5_piste_aerodrome_en_herbe() {
    let ruleset = load_ruleset("PISTE_D_AERODROME");
    let feature = attrs(&[
        ("NATURE", "Piste en herbe"),
        ("TOPONYME", "Piste herbe"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10f17",
        "4",
        Some("Piste herbe"),
    );
}

#[test]
fn test_ac5_piste_aerodrome_nature_inconnue_ignored() {
    let ruleset = load_ruleset("PISTE_D_AERODROME");
    let feature = attrs(&[("NATURE", "Piste mixte")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "Unknown NATURE should not match (FME _FME_NO_OP_)"
    );
}

// ============================================================================
// Task 2.7: AC6 — TRANSPORT_PAR_CABLE (wildcard)
// ============================================================================

#[test]
fn test_ac6_transport_par_cable_wildcard() {
    let ruleset = load_ruleset("TRANSPORT_PAR_CABLE");
    let feature = attrs(&[
        ("NATURE", "Téléphérique"),
        ("TOPONYME", "Téléphérique du Mt Blanc"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10f0b",
        "2",
        Some("Téléphérique du Mt Blanc"),
    );
}

#[test]
fn test_ac6_transport_par_cable_empty_nature() {
    let ruleset = load_ruleset("TRANSPORT_PAR_CABLE");
    let feature = attrs(&[("TOPONYME", "Funiculaire")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10f0b",
        "2",
        Some("Funiculaire"),
    );
}

// ============================================================================
// Negative test: TRONCON_DE_ROUTE unknown NATURE → feature dropped (FME _FME_NO_OP_)
// ============================================================================

#[test]
fn test_troncon_de_route_unknown_nature_ignored() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    let feature = attrs(&[("NATURE", "Inconnu")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "Unknown NATURE without CL_ADMIN should not match (FME _FME_NO_OP_)"
    );
}

// ============================================================================
// Task 2.8: First-match-wins — Nationale Rond-point override
// ============================================================================

#[test]
fn test_first_match_wins_nationale_rond_point() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    // Nationale + Rond-point -> CL_ADMIN Nationale takes priority, Type=0x04 (not 0x06 generic)
    let feature = attrs(&[
        ("CL_ADMIN", "Nationale"),
        ("NATURE", "Rond-point"),
    ]);
    assert_transform(&ruleset, &feature, "0x04", "7", None);
}

#[test]
fn test_first_match_wins_autoroute_rond_point() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    // Autoroute + Rond-point -> CL_ADMIN Autoroute takes priority (wildcard NATURE), Type=0x01
    let feature = attrs(&[
        ("CL_ADMIN", "Autoroute"),
        ("NATURE", "Rond-point"),
        ("NUMERO", "A480"),
    ]);
    assert_transform(&ruleset, &feature, "0x01", "7", Some("~[0x04]A480"));
}

#[test]
fn test_first_match_wins_departementale_rond_point() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");
    // Departementale + Rond-point -> override to Type=0x04 (not 0x05 Departementale, not 0x06 generic)
    let feature = attrs(&[
        ("CL_ADMIN", "Départementale"),
        ("NATURE", "Rond-point"),
    ]);
    assert_transform(&ruleset, &feature, "0x04", "7", None);
}

// ============================================================================
// Task 3: Validation couverture FME complete — tests parametres
// ============================================================================

/// All 27 FME transport branches: (source_layer, attrs, expected_type, expected_end_level, expected_label)
/// Tracability: FME 07-VOIRIE.fmw AttributeManager conditionals
#[test]
fn test_fme_coverage_troncon_de_route_all_17_branches() {
    let ruleset = load_ruleset("TRONCON_DE_ROUTE");

    // FME branch table for TRONCON_DE_ROUTE (17 rules)
    // Type alias to reduce complexity
    type FmeBranch<'a> = (Vec<(&'a str, &'a str)>, &'a str, &'a str, Option<&'a str>);
    let test_cases: Vec<FmeBranch> = vec![
        // # | CL_ADMIN | NATURE | Type | EndLevel | Label
        // 1. Autoroute
        (
            vec![("CL_ADMIN", "Autoroute"), ("NUMERO", "A6")],
            "0x01",
            "7",
            Some("~[0x04]A6"),
        ),
        // 2. Nationale (hors Rond-point)
        (
            vec![("CL_ADMIN", "Nationale"), ("NATURE", "Route à 1 chaussée"), ("NUMERO", "N7")],
            "0x04",
            "7",
            Some("~[0x05]N7"),
        ),
        // 3. Nationale Rond-point
        (
            vec![("CL_ADMIN", "Nationale"), ("NATURE", "Rond-point")],
            "0x04",
            "7",
            None,
        ),
        // 4. Departementale (hors Rond-point)
        (
            vec![("CL_ADMIN", "Départementale"), ("NATURE", "Route à 1 chaussée"), ("NUMERO", "D1075")],
            "0x05",
            "7",
            Some("~[0x06]D1075"),
        ),
        // 5. Departementale Rond-point
        (
            vec![("CL_ADMIN", "Départementale"), ("NATURE", "Rond-point")],
            "0x04",
            "7",
            None,
        ),
        // 6. Route intercommunale
        (
            vec![("CL_ADMIN", "Route intercommunale"), ("NUMERO", "C12")],
            "0x05",
            "7",
            Some("C12"),
        ),
        // 7. Route a 1 chaussee (fallback NATURE)
        (
            vec![("NATURE", "Route à 1 chaussée")],
            "0x06",
            "2",
            None,
        ),
        // 8. Route a 2 chaussees
        (
            vec![("NATURE", "Route à 2 chaussées")],
            "0x06",
            "2",
            None,
        ),
        // 9. Rond-point (generique)
        (
            vec![("NATURE", "Rond-point")],
            "0x06",
            "2",
            None,
        ),
        // 10. Route empierree
        (
            vec![("NATURE", "Route empierrée")],
            "0x07",
            "1",
            None,
        ),
        // 11. Bretelle
        (
            vec![("NATURE", "Bretelle")],
            "0x09",
            "7",
            None,
        ),
        // 12. Type autoroutier
        (
            vec![("NATURE", "Type autoroutier"), ("NUMERO", "A43")],
            "0x09",
            "7",
            Some("~[0x04]A43"),
        ),
        // 13. Chemin
        (
            vec![("NATURE", "Chemin")],
            "0x0a",
            "1",
            None,
        ),
        // 14. Escalier
        (
            vec![("NATURE", "Escalier")],
            "0x0f",
            "1",
            None,
        ),
        // 15. Piste cyclable
        (
            vec![("NATURE", "Piste cyclable")],
            "0x0e",
            "1",
            None,
        ),
        // 16. Sentier
        (
            vec![("NATURE", "Sentier")],
            "0x10",
            "1",
            None,
        ),
        // 17. Bac ou liaison maritime
        (
            vec![("NATURE", "Bac ou liaison maritime")],
            "0x1b",
            "1",
            None,
        ),
    ];

    for (i, (attr_pairs, expected_type, expected_end_level, expected_label)) in
        test_cases.iter().enumerate()
    {
        let feature = attrs(attr_pairs);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("FME branch #{}: rule error: {}", i + 1, e))
            .unwrap_or_else(|| panic!("FME branch #{}: no rule matched for {:?}", i + 1, feature));

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "FME branch #{}: Type mismatch for {:?}",
            i + 1,
            feature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            expected_end_level,
            "FME branch #{}: EndLevel mismatch for {:?}",
            i + 1,
            feature
        );

        match expected_label {
            Some(label) => assert_eq!(
                result.get("Label").unwrap(),
                label,
                "FME branch #{}: Label mismatch for {:?}",
                i + 1,
                feature
            ),
            None => assert!(
                !result.contains_key("Label"),
                "FME branch #{}: Expected no Label for {:?}, got {:?}",
                i + 1,
                feature,
                result.get("Label")
            ),
        }
    }
}

#[test]
fn test_fme_coverage_troncon_de_voie_ferree_all_7_branches() {
    let ruleset = load_ruleset("TRONCON_DE_VOIE_FERREE");

    // FME branch table for TRONCON_DE_VOIE_FERREE (7 rules)
    type VfBranch<'a> = (Vec<(&'a str, &'a str)>, &'a str, &'a str);
    let test_cases: Vec<VfBranch> = vec![
        // 1. Souterrain (POS_SOL=-1, any NATURE)
        (vec![("NATURE", "LGV"), ("POS_SOL", "-1"), ("TOPONYME", "T")], "0x10e06", "5"),
        // 2. Voie ferree principale
        (vec![("NATURE", "Voie ferrée principale"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10c00", "5"),
        // 3. LGV
        (vec![("NATURE", "LGV"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10e02", "5"),
        // 4. Voie de service
        (vec![("NATURE", "Voie de service"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10e03", "5"),
        // 5. Tramway
        (vec![("NATURE", "Tramway"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10e04", "5"),
        // 6. Funiculaire ou cremaillere
        (vec![("NATURE", "Funiculaire ou crémaillère"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10e05", "5"),
        // 7. Sans objet
        (vec![("NATURE", "Sans objet"), ("POS_SOL", "0"), ("TOPONYME", "T")], "0x10c00", "5"),
    ];

    for (i, (attr_pairs, expected_type, expected_end_level)) in test_cases.iter().enumerate() {
        let feature = attrs(attr_pairs);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("VF branch #{}: rule error: {}", i + 1, e))
            .unwrap_or_else(|| panic!("VF branch #{}: no rule matched for {:?}", i + 1, feature));

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "VF branch #{}: Type mismatch",
            i + 1
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            expected_end_level,
            "VF branch #{}: EndLevel mismatch",
            i + 1
        );
        // All VF rules have Label=${TOPONYME}
        assert_eq!(
            result.get("Label").unwrap(),
            "T",
            "VF branch #{}: Label mismatch",
            i + 1
        );
    }
}

#[test]
fn test_fme_coverage_piste_aerodrome_all_2_branches() {
    let ruleset = load_ruleset("PISTE_D_AERODROME");

    let test_cases = [
        (vec![("NATURE", "Piste en dur"), ("TOPONYME", "P")], "0x1090b", "4"),
        (vec![("NATURE", "Piste en herbe"), ("TOPONYME", "P")], "0x10f17", "4"),
    ];

    for (i, (attr_pairs, expected_type, expected_end_level)) in test_cases.iter().enumerate() {
        let feature = attrs(attr_pairs);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Aero branch #{}: rule error: {}", i + 1, e))
            .unwrap_or_else(|| panic!("Aero branch #{}: no rule matched for {:?}", i + 1, feature));

        assert_eq!(result.get("Type").unwrap(), expected_type, "Aero branch #{}", i + 1);
        assert_eq!(result.get("EndLevel").unwrap(), expected_end_level, "Aero branch #{}", i + 1);
        assert_eq!(result.get("Label").unwrap(), "P", "Aero branch #{}", i + 1);
    }
}

#[test]
fn test_fme_coverage_transport_par_cable_1_branch() {
    let ruleset = load_ruleset("TRANSPORT_PAR_CABLE");
    let feature = attrs(&[("NATURE", "Télésiège"), ("TOPONYME", "C")]);
    let result = rules::evaluate_feature(&ruleset, &feature)
        .unwrap()
        .unwrap();
    assert_eq!(result.get("Type").unwrap(), "0x10f0b");
    assert_eq!(result.get("EndLevel").unwrap(), "2");
    assert_eq!(result.get("Label").unwrap(), "C");
}

// ============================================================================
// Total FME branch count verification
// ============================================================================

#[test]
fn test_total_transport_rules_count() {
    let rules_file = rules::load_rules(&rules_path()).unwrap();
    // Count only the 4 transport rulesets (first 4 in file)
    let transport_layers = [
        "TRONCON_DE_ROUTE",
        "TRONCON_DE_VOIE_FERREE",
        "PISTE_D_AERODROME",
        "TRANSPORT_PAR_CABLE",
    ];
    let total: usize = rules_file
        .rulesets
        .iter()
        .filter(|rs| transport_layers.contains(&rs.source_layer.as_str()))
        .map(|rs| rs.rules.len())
        .sum();
    // 17 (routes) + 7 (VF) + 2 (aero) + 1 (cable) = 27 branches FME
    assert_eq!(total, 27, "Total transport rules should match 27 FME branches");
}
