//! Integration tests for administratif & hydrographie rulesets (Story 10.2)
//!
//! Validates that the BDTOPO admin+hydro rules in bdtopo-garmin-rules.yaml
//! produce the correct Garmin attributes matching the FME source of truth:
//!   - 01-COMMUNE-ZONEURBA.fmw (COMMUNE, ZONE_D_HABITATION)
//!   - 04-COMMUNE-HYDROGRAPHIE.fmw (TRONCON/SURFACE/DETAIL_HYDROGRAPHIQUE)

mod common;

use common::{assert_transform, attrs, load_ruleset, rules_path};
use mpforge::rules;

// ============================================================================
// Task 2.2: AC1 — COMMUNE → Type=0x54, EndLevel=7, Country, CityName, Zip
// FME: 01-COMMUNE-ZONEURBA.fmw / mapping statique
// ============================================================================

#[test]
fn test_ac1_commune_type_endlevel_label() {
    let ruleset = load_ruleset("COMMUNE");
    let feature = attrs(&[("NOM", "Grenoble"), ("CODE_POST", "38000")]);
    assert_transform(&ruleset, &feature, "0x54", "7", Some("Grenoble"));
}

#[test]
fn test_ac1_commune_country_cityname_zip() {
    let ruleset = load_ruleset("COMMUNE");
    let feature = attrs(&[("NOM", "Grenoble"), ("CODE_POST", "38000")]);
    let result = rules::evaluate_feature(&ruleset, &feature)
        .unwrap()
        .unwrap();

    assert_eq!(
        result.get("Country").unwrap(),
        "France~[0x1d]FRA",
        "Country should contain shield separator"
    );
    assert_eq!(
        result.get("CityName").unwrap(),
        "Grenoble",
        "CityName should be substituted from NOM"
    );
    assert_eq!(
        result.get("Zip").unwrap(),
        "38000",
        "Zip should be substituted from CODE_POST"
    );
}

// ============================================================================
// Task 2.3: AC2 — ZONE_D_HABITATION NATURE="Lieu-dit habité"
// FME: 01-COMMUNE-ZONEURBA.fmw / Tester NATURE
// ============================================================================

#[test]
fn test_ac2_zone_habitation_lieu_dit_habite() {
    let ruleset = load_ruleset("ZONE_D_HABITATION");
    let feature = attrs(&[("NATURE", "Lieu-dit habité"), ("TOPONYME", "Les Granges")]);
    assert_transform(&ruleset, &feature, "0x03", "4", Some("Les Granges"));
}

// ============================================================================
// Task 2.4: AC3 — TRONCON_HYDROGRAPHIQUE PERSISTANC="Permanent"
// FME: 04-HYDROGRAPHIE.fmw / PERSISTANC only (pas NATURE)
// ============================================================================

#[test]
fn test_ac3_troncon_hydro_permanent() {
    let ruleset = load_ruleset("TRONCON_HYDROGRAPHIQUE");
    let feature = attrs(&[("PERSISTANC", "Permanent"), ("NOM_C_EAU", "L'Isère")]);
    assert_transform(&ruleset, &feature, "0x18", "2", Some("L'Isère"));
}

// Edge case: TRONCON_HYDROGRAPHIQUE with no PERSISTANC field (vide)
// FME catch-all covers Permanent, Inconnue, and missing/empty → 0x18
#[test]
fn test_troncon_hydro_no_persistanc_field_catches_all() {
    let ruleset = load_ruleset("TRONCON_HYDROGRAPHIQUE");
    let feature = attrs(&[("NOM_C_EAU", "Ruisseau sans persistance")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x18",
        "2",
        Some("Ruisseau sans persistance"),
    );
}

// ============================================================================
// Task 2.5: AC4 — TRONCON_HYDROGRAPHIQUE PERSISTANC="Intermittent"
// ============================================================================

#[test]
fn test_ac4_troncon_hydro_intermittent() {
    let ruleset = load_ruleset("TRONCON_HYDROGRAPHIQUE");
    let feature = attrs(&[("PERSISTANC", "Intermittent"), ("NOM_C_EAU", "Ruisseau sec")]);
    assert_transform(&ruleset, &feature, "0x26", "2", Some("Ruisseau sec"));
}

// ============================================================================
// Task 2.6: AC5 — SURFACE_HYDROGRAPHIQUE NATURE="Glacier, névé"
// FME: Glacier override PERSISTANC (premiere regle)
// ============================================================================

#[test]
fn test_ac5_surface_hydro_glacier() {
    let ruleset = load_ruleset("SURFACE_HYDROGRAPHIQUE");
    let feature = attrs(&[
        ("NATURE", "Glacier, névé"),
        ("PERSISTANC", "Permanent"),
        ("NOM_P_EAU", "Glacier de la Meije"),
    ]);
    assert_transform(&ruleset, &feature, "0x4d", "2", Some("Glacier de la Meije"));
}

#[test]
fn test_ac5_surface_hydro_glacier_intermittent_override() {
    let ruleset = load_ruleset("SURFACE_HYDROGRAPHIQUE");
    // Glacier avec PERSISTANC="Intermittent" -> Glacier prend le dessus
    let feature = attrs(&[
        ("NATURE", "Glacier, névé"),
        ("PERSISTANC", "Intermittent"),
        ("NOM_P_EAU", "Névé"),
    ]);
    assert_transform(&ruleset, &feature, "0x4d", "2", Some("Névé"));
}

// ============================================================================
// Task 2.7: AC6 — SURFACE_HYDROGRAPHIQUE PERSISTANC="Intermittent"
// ============================================================================

#[test]
fn test_ac6_surface_hydro_intermittent() {
    let ruleset = load_ruleset("SURFACE_HYDROGRAPHIQUE");
    let feature = attrs(&[
        ("NATURE", "Lac"),
        ("PERSISTANC", "Intermittent"),
        ("NOM_P_EAU", "Mare temporaire"),
    ]);
    assert_transform(&ruleset, &feature, "0x4c", "2", Some("Mare temporaire"));
}

// ============================================================================
// Task 2.8: AC7 — DETAIL_HYDROGRAPHIQUE NATURE="Source"
// ============================================================================

#[test]
fn test_ac7_detail_hydro_source() {
    let ruleset = load_ruleset("DETAIL_HYDROGRAPHIQUE");
    let feature = attrs(&[("NATURE", "Source"), ("TOPONYME", "Source du Drac")]);
    assert_transform(&ruleset, &feature, "0x06511", "2", Some("Source du Drac"));
}

// ============================================================================
// Task 2.9: AC8 — DETAIL_HYDROGRAPHIQUE NATURE inconnue → catch-all 0x06414
// FME: DEFAULT_VALUE = 0x06414 (couvre Citerne, Lavoir, Perte, Point d'eau, Résurgence)
// ============================================================================

#[test]
fn test_ac8_detail_hydro_nature_inconnue_catchall() {
    let ruleset = load_ruleset("DETAIL_HYDROGRAPHIQUE");
    let feature = attrs(&[("NATURE", "Point d'eau"), ("TOPONYME", "Abreuvoir")]);
    assert_transform(&ruleset, &feature, "0x06414", "2", Some("Abreuvoir"));
}

#[test]
fn test_ac8_detail_hydro_citerne_catchall() {
    let ruleset = load_ruleset("DETAIL_HYDROGRAPHIQUE");
    let feature = attrs(&[("NATURE", "Citerne"), ("TOPONYME", "Citerne communale")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x06414",
        "2",
        Some("Citerne communale"),
    );
}

// ============================================================================
// Task 2.10: Negative test — ZONE_D_HABITATION NATURE="Château" → pas de match
// FME: seule "Lieu-dit habité" est traitée, autres NATURE ignorées
// ============================================================================

#[test]
fn test_negative_zone_habitation_chateau_no_match() {
    let ruleset = load_ruleset("ZONE_D_HABITATION");
    let feature = attrs(&[("NATURE", "Château"), ("TOPONYME", "Château de Vizille")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "NATURE='Château' should not match (FME only processes 'Lieu-dit habité')"
    );
}

// ============================================================================
// Task 3.1: Validation couverture FME complete — tests parametres
// FME: 01-COMMUNE-ZONEURBA.fmw + 04-COMMUNE-HYDROGRAPHIE.fmw
// ============================================================================

/// All admin+hydro FME branches parameterized
/// Traceability: FME 01-COMMUNE-ZONEURBA.fmw, 04-COMMUNE-HYDROGRAPHIE.fmw
#[test]
fn test_fme_coverage_commune_1_branch() {
    // FME: COMMUNE → mapping statique Type=0x54, EndLevel=7
    let ruleset = load_ruleset("COMMUNE");
    let feature = attrs(&[("NOM", "Vizille"), ("CODE_POST", "38220")]);
    let result = rules::evaluate_feature(&ruleset, &feature)
        .unwrap()
        .unwrap();
    assert_eq!(result.get("Type").unwrap(), "0x54");
    assert_eq!(result.get("EndLevel").unwrap(), "7");
    assert_eq!(result.get("Label").unwrap(), "Vizille");
    assert_eq!(result.get("Country").unwrap(), "France~[0x1d]FRA");
    assert_eq!(result.get("CityName").unwrap(), "Vizille");
    assert_eq!(result.get("Zip").unwrap(), "38220");
}

#[test]
fn test_fme_coverage_zone_habitation_1_branch() {
    // FME: ZONE_D_HABITATION NATURE="Lieu-dit habité" → Type=0x03
    let ruleset = load_ruleset("ZONE_D_HABITATION");
    let feature = attrs(&[("NATURE", "Lieu-dit habité"), ("TOPONYME", "Le Bourg")]);
    assert_transform(&ruleset, &feature, "0x03", "4", Some("Le Bourg"));
}

#[test]
fn test_fme_coverage_troncon_hydro_all_3_branches() {
    let ruleset = load_ruleset("TRONCON_HYDROGRAPHIQUE");

    // FME branch table (3 branches via PERSISTANC)
    // Intermittent checked first, then catch-all covers Permanent, Inconnue, vide
    type Branch<'a> = (Vec<(&'a str, &'a str)>, &'a str, &'a str);
    let test_cases: Vec<Branch> = vec![
        // 1. Intermittent → 0x26
        (
            vec![("PERSISTANC", "Intermittent"), ("NOM_C_EAU", "T")],
            "0x26",
            "2",
        ),
        // 2. Permanent → 0x18 (catch-all)
        (
            vec![("PERSISTANC", "Permanent"), ("NOM_C_EAU", "T")],
            "0x18",
            "2",
        ),
        // 3. Inconnue → 0x18 (catch-all)
        (
            vec![("PERSISTANC", "Inconnue"), ("NOM_C_EAU", "T")],
            "0x18",
            "2",
        ),
    ];

    for (i, (attr_pairs, expected_type, expected_end_level)) in test_cases.iter().enumerate() {
        let feature = attrs(attr_pairs);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Hydro branch #{}: rule error: {}", i + 1, e))
            .unwrap_or_else(|| panic!("Hydro branch #{}: no match for {:?}", i + 1, feature));

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Hydro branch #{}: Type mismatch",
            i + 1
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            expected_end_level,
            "Hydro branch #{}: EndLevel mismatch",
            i + 1
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "T",
            "Hydro branch #{}: Label mismatch",
            i + 1
        );
    }
}

#[test]
fn test_fme_coverage_surface_hydro_all_5_branches() {
    let ruleset = load_ruleset("SURFACE_HYDROGRAPHIQUE");

    // FME branch table (5 branches: Glacier, Intermittent, Inconnue, Permanent, vide)
    // Glacier en PREMIER (override PERSISTANC)
    type Branch<'a> = (Vec<(&'a str, &'a str)>, &'a str);
    let test_cases: Vec<Branch> = vec![
        // 1. Glacier, névé → 0x4d
        (
            vec![
                ("NATURE", "Glacier, névé"),
                ("PERSISTANC", "Permanent"),
                ("NOM_P_EAU", "G"),
            ],
            "0x4d",
        ),
        // 2. Intermittent → 0x4c
        (
            vec![
                ("NATURE", "Lac"),
                ("PERSISTANC", "Intermittent"),
                ("NOM_P_EAU", "G"),
            ],
            "0x4c",
        ),
        // 3. Inconnue → 0x4c
        (
            vec![
                ("NATURE", "Lac"),
                ("PERSISTANC", "Inconnue"),
                ("NOM_P_EAU", "G"),
            ],
            "0x4c",
        ),
        // 4. Permanent → 0x3f (catch-all)
        (
            vec![
                ("NATURE", "Lac"),
                ("PERSISTANC", "Permanent"),
                ("NOM_P_EAU", "G"),
            ],
            "0x3f",
        ),
        // 5. Vide (pas de PERSISTANC) → 0x3f (catch-all)
        (vec![("NATURE", "Lac"), ("NOM_P_EAU", "G")], "0x3f"),
    ];

    for (i, (attr_pairs, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(attr_pairs);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Surface branch #{}: rule error: {}", i + 1, e))
            .unwrap_or_else(|| panic!("Surface branch #{}: no match for {:?}", i + 1, feature));

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Surface branch #{}: Type mismatch for {:?}",
            i + 1,
            feature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "Surface branch #{}: EndLevel should be 2",
            i + 1
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "G",
            "Surface branch #{}: Label mismatch",
            i + 1
        );
    }
}

#[test]
fn test_fme_coverage_detail_hydro_all_18_natures() {
    let ruleset = load_ruleset("DETAIL_HYDROGRAPHIQUE");

    // FME branch table: 17 explicit NATURE + 1 DEFAULT_VALUE (catch-all)
    // Traceability: 04-COMMUNE-HYDROGRAPHIE.fmw AttributeManager DETAIL_HYDROGRAPHIQUE
    let test_cases: Vec<(&str, &str)> = vec![
        // 12 explicitly mapped NATURE values
        ("Arroyo", "0x06501"),
        ("Baie", "0x06503"),
        ("Cascade", "0x06508"),
        ("Crique", "0x06507"),
        ("Fontaine", "0x06509"),
        ("Glacier", "0x0650a"),
        ("Lac", "0x0650d"),
        ("Marais", "0x06513"),
        ("Mer", "0x06510"),
        ("Réservoir", "0x0650f"),
        ("Source", "0x06511"),
        ("Source captée", "0x06511"),
        // 5 NATURE covered by catch-all (all map to 0x06414 in FME)
        ("Citerne", "0x06414"),
        ("Lavoir", "0x06414"),
        ("Perte", "0x06414"),
        ("Point d'eau", "0x06414"),
        ("Résurgence", "0x06414"),
        // Unknown NATURE also covered by catch-all
        ("NatureInconnue", "0x06414"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature), ("TOPONYME", "T")]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Detail #{} ({}): rule error: {}", i + 1, nature, e))
            .unwrap_or_else(|| {
                panic!(
                    "Detail #{} ({}): no match for {:?}",
                    i + 1,
                    nature,
                    feature
                )
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Detail #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "Detail #{} ({}): EndLevel should be 2",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "T",
            "Detail #{} ({}): Label mismatch",
            i + 1,
            nature
        );
    }
}

// ============================================================================
// Total rules count verification (Story 10.1 + 10.2)
// ============================================================================

#[test]
fn test_total_admin_hydro_rules_count() {
    let rules_file = rules::load_rules(&rules_path()).unwrap();

    // Count only the 5 admin/hydro rulesets (no assertion on total — other stories add more)
    let admin_hydro_layers = [
        ("COMMUNE", 1),
        ("ZONE_D_HABITATION", 1),
        ("TRONCON_HYDROGRAPHIQUE", 2),
        ("SURFACE_HYDROGRAPHIQUE", 4),
        ("DETAIL_HYDROGRAPHIQUE", 13),
    ];
    for (layer, expected_count) in &admin_hydro_layers {
        let rs = rules_file
            .rulesets
            .iter()
            .find(|rs| rs.source_layer == *layer)
            .unwrap_or_else(|| panic!("Admin/hydro ruleset {} not found", layer));
        assert_eq!(
            rs.rules.len(),
            *expected_count,
            "Rule count mismatch for {}",
            layer
        );
    }

    let total_admin_hydro: usize = rules_file
        .rulesets
        .iter()
        .filter(|rs| admin_hydro_layers.iter().any(|(l, _)| *l == rs.source_layer))
        .map(|rs| rs.rules.len())
        .sum();
    assert_eq!(
        total_admin_hydro, 21,
        "Total admin/hydro rules should be 21"
    );
}
