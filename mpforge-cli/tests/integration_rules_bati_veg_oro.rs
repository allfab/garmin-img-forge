//! Integration tests for bâti, végétation & orographie rulesets (Story 10.3)
//!
//! Validates that the BDTOPO bâti+végétation+orographie rules in bdtopo-garmin-rules.yaml
//! produce the correct Garmin attributes matching the FME source of truth:
//!   - 06-IGN-BDTOPO-ESRI-SHP-TO-ESRI-SHP-GARMIN-BATI.fmw
//!   - 05-IGN-BDTOPO-ESRI-SHP-TO-ESRI-SHP-GARMIN-VEGETATION.fmw

mod common;

use common::{assert_transform, attrs, load_ruleset, rules_path};
use mpforge_cli::rules;

// ============================================================================
// Task 2.2: AC1 — BATIMENT NATURE="Chapelle" → Type=0x10f09, EndLevel=2, pas de Label
// FME: 06-BATI.fmw / AttributeManager BATIMENT
// ============================================================================

#[test]
fn test_ac1_batiment_chapelle() {
    let ruleset = load_ruleset("BATIMENT");
    let feature = attrs(&[("NATURE", "Chapelle")]);
    assert_transform(&ruleset, &feature, "0x10f09", "2", None);
}

// ============================================================================
// Task 2.3: AC2 — BATIMENT NATURE non reconnue → catch-all 0x1101c
// FME: DEFAULT_VALUE = 0x1101c (Indifférenciée)
// ============================================================================

#[test]
fn test_ac2_batiment_nature_inconnue_catchall() {
    let ruleset = load_ruleset("BATIMENT");
    let feature = attrs(&[("NATURE", "Préfecture")]);
    assert_transform(&ruleset, &feature, "0x1101c", "2", None);
}

// ============================================================================
// Task 2.4: AC3 — CIMETIERE NATURE="Militaire" → Type=0x10f13, EndLevel=4
// ============================================================================

#[test]
fn test_ac3_cimetiere_militaire() {
    let ruleset = load_ruleset("CIMETIERE");
    let feature = attrs(&[("NATURE", "Militaire")]);
    assert_transform(&ruleset, &feature, "0x10f13", "4", None);
}

// ============================================================================
// Task 2.5: AC4 — CIMETIERE NATURE inconnue → pas de match
// FME: DEFAULT_VALUE = FME_NULL_VALUE → feature ignorée
// ============================================================================

#[test]
fn test_ac4_cimetiere_nature_inconnue_no_match() {
    let ruleset = load_ruleset("CIMETIERE");
    let feature = attrs(&[("NATURE", "Animalier")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "CIMETIERE NATURE='Animalier' should not match (no catch-all)"
    );
}

// ============================================================================
// Task 2.6: AC5 — CONSTRUCTION_LINEAIRE NATURE="Barrage" → Type=0x10f08, EndLevel=2, Label=TOPONYME
// ============================================================================

#[test]
fn test_ac5_construction_lineaire_barrage() {
    let ruleset = load_ruleset("CONSTRUCTION_LINEAIRE");
    let feature = attrs(&[("NATURE", "Barrage"), ("TOPONYME", "Barrage de Monteynard")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10f08",
        "2",
        Some("Barrage de Monteynard"),
    );
}

// ============================================================================
// Task 2.7: AC6 — CONSTRUCTION_LINEAIRE NATURE inconnue → catch-all 0x10c04
// ============================================================================

#[test]
fn test_ac6_construction_lineaire_nature_inconnue_catchall() {
    let ruleset = load_ruleset("CONSTRUCTION_LINEAIRE");
    let feature = attrs(&[("NATURE", "Passerelle"), ("TOPONYME", "Passerelle du Drac")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10c04",
        "2",
        Some("Passerelle du Drac"),
    );
}

// ============================================================================
// Task 2.8: AC7 — CONSTRUCTION_PONCTUELLE NATURE="Antenne" → Type=0x11503, EndLevel=1, Label=TOPONYME
// ============================================================================

#[test]
fn test_ac7_construction_ponctuelle_antenne() {
    let ruleset = load_ruleset("CONSTRUCTION_PONCTUELLE");
    let feature = attrs(&[("NATURE", "Antenne"), ("TOPONYME", "Antenne TDF Chamrousse")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x11503",
        "1",
        Some("Antenne TDF Chamrousse"),
    );
}

// ============================================================================
// Task 2.9: AC8 — CONSTRUCTION_PONCTUELLE NATURE inconnue → pas de match
// FME: DEFAULT_VALUE = FME_NULL_VALUE → feature ignorée
// ============================================================================

#[test]
fn test_ac8_construction_ponctuelle_nature_inconnue_no_match() {
    let ruleset = load_ruleset("CONSTRUCTION_PONCTUELLE");
    let feature = attrs(&[("NATURE", "Radar"), ("TOPONYME", "Radar météo")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "CONSTRUCTION_PONCTUELLE NATURE='Radar' should not match (no catch-all)"
    );
}

// ============================================================================
// Task 2.10: AC9 — PYLONE wildcard → Type=0x11503, EndLevel=2, pas de Label
// FME: AttributeCreator statique, pas de champ NAME
// ============================================================================

#[test]
fn test_ac9_pylone_wildcard() {
    let ruleset = load_ruleset("PYLONE");
    let feature = attrs(&[("NATURE", "Pylône électrique")]);
    assert_transform(&ruleset, &feature, "0x11503", "2", None);
}

// ============================================================================
// Task 2.11: AC10 — TERRAIN_DE_SPORT NATURE="Bassin de natation" → Type=0x10f1d, EndLevel=2
// ============================================================================

#[test]
fn test_ac10_terrain_sport_bassin_natation() {
    let ruleset = load_ruleset("TERRAIN_DE_SPORT");
    let feature = attrs(&[("NATURE", "Bassin de natation"), ("NAT_DETAIL", "Piscine")]);
    assert_transform(&ruleset, &feature, "0x10f1d", "2", Some("Piscine"));
}

// ============================================================================
// Task 2.12: AC11 — TERRAIN_DE_SPORT NATURE="Grand terrain de sport" → Type=0x1090d, EndLevel=4
// ============================================================================

#[test]
fn test_ac11_terrain_sport_grand_terrain_endlevel4() {
    let ruleset = load_ruleset("TERRAIN_DE_SPORT");
    let feature = attrs(&[
        ("NATURE", "Grand terrain de sport"),
        ("NAT_DETAIL", "Football"),
    ]);
    assert_transform(&ruleset, &feature, "0x1090d", "4", Some("Football"));
}

// ============================================================================
// Task 2.13: AC12 — LIGNE_OROGRAPHIQUE NATURE="Talus" → Type=0x10e19, EndLevel=2, Label=NATURE
// FME: NATURE renommé→NAME, le label est la valeur NATURE
// ============================================================================

#[test]
fn test_ac12_ligne_orographique_talus() {
    let ruleset = load_ruleset("LIGNE_OROGRAPHIQUE");
    let feature = attrs(&[("NATURE", "Talus")]);
    assert_transform(&ruleset, &feature, "0x10e19", "2", Some("Talus"));
}

// ============================================================================
// Task 2.14: AC13 — LIGNE_OROGRAPHIQUE NATURE inconnue → pas de match
// ============================================================================

#[test]
fn test_ac13_ligne_orographique_nature_inconnue_no_match() {
    let ruleset = load_ruleset("LIGNE_OROGRAPHIQUE");
    let feature = attrs(&[("NATURE", "Falaise")]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "LIGNE_OROGRAPHIQUE NATURE='Falaise' should not match (no catch-all)"
    );
}

// ============================================================================
// Task 2.15: AC14 — ZONE_DE_VEGETATION NATURE="Forêt fermée de feuillus"
//   → Type=0x10f1e, EndLevel=6, Label="Forêt de feuillus" (raccourci FME)
// ============================================================================

#[test]
fn test_ac14_vegetation_foret_feuillus() {
    let ruleset = load_ruleset("ZONE_DE_VEGETATION");
    let feature = attrs(&[("NATURE", "Forêt fermée de feuillus")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10f1e",
        "6",
        Some("Forêt de feuillus"),
    );
}

// ============================================================================
// Task 2.16: AC15 — ZONE_DE_VEGETATION NATURE="Haie" → Type=0x11002, EndLevel=4, pas de Label
// ============================================================================

#[test]
fn test_ac15_vegetation_haie_no_label() {
    let ruleset = load_ruleset("ZONE_DE_VEGETATION");
    let feature = attrs(&[("NATURE", "Haie")]);
    assert_transform(&ruleset, &feature, "0x11002", "4", None);
}

// ============================================================================
// Task 3.1: Validation couverture FME complete — tests parametres
// Traceability: 06-BATI.fmw, 05-VEGETATION.fmw
// ============================================================================

/// FME coverage: BATIMENT — 12 explicit NATURE + catch-all
#[test]
fn test_fme_coverage_batiment_all_13_branches() {
    let ruleset = load_ruleset("BATIMENT");

    // FME: 06-BATI.fmw / BATIMENT — 12 NATURE + catch-all
    let test_cases: Vec<(&str, &str)> = vec![
        ("Arène ou théâtre antique", "0x10f08"),
        ("Chapelle", "0x10f09"),
        ("Château", "0x10f0a"),
        ("Eglise", "0x10f0b"),
        ("Fort, blockhaus, casemate", "0x10f0c"),
        ("Indifférenciée", "0x1101c"),
        ("Industriel, agricole ou commercial", "0x10f04"),
        ("Monument", "0x10f0d"),
        ("Serre", "0x10f05"),
        ("Silo", "0x10f06"),
        ("Tour, donjon", "0x10f11"),
        ("Tribune", "0x10f12"),
        // Catch-all: NATURE absentes du FME (Préfecture, Sous-préfecture, Construction légère)
        ("Préfecture", "0x1101c"),
        ("Sous-préfecture", "0x1101c"),
        ("Construction légère", "0x1101c"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature)]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("BATIMENT #{} ({}): rule error: {}", i + 1, nature, e))
            .unwrap_or_else(|| {
                panic!("BATIMENT #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "BATIMENT #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "BATIMENT #{} ({}): EndLevel should be 2",
            i + 1,
            nature
        );
        assert!(
            !result.contains_key("Label"),
            "BATIMENT #{} ({}): should have no Label (FME suppresses TOPONYME)",
            i + 1,
            nature
        );
    }
}

/// FME coverage: CIMETIERE — 3 NATURE, no catch-all
#[test]
fn test_fme_coverage_cimetiere_all_3_branches() {
    let ruleset = load_ruleset("CIMETIERE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Civil", "0x1a"),
        ("Militaire", "0x10f13"),
        ("Militaire étranger", "0x10f13"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature)]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("CIMETIERE #{} ({}): rule error: {}", i + 1, nature, e))
            .unwrap_or_else(|| {
                panic!("CIMETIERE #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "CIMETIERE #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "4",
            "CIMETIERE #{} ({}): EndLevel should be 4",
            i + 1,
            nature
        );
        assert!(
            !result.contains_key("Label"),
            "CIMETIERE #{} ({}): should have no Label",
            i + 1,
            nature
        );
    }
}

/// FME coverage: CONSTRUCTION_LINEAIRE — 11 NATURE + catch-all
#[test]
fn test_fme_coverage_construction_lineaire_all_12_branches() {
    let ruleset = load_ruleset("CONSTRUCTION_LINEAIRE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Autre ligne descriptive", "0x10c04"),
        ("Barrage", "0x10f08"),
        ("Clôture", "0x13309"),
        ("Mur", "0x13308"),
        ("Mur anti-bruit", "0x10e13"),
        ("Mur de soutènement", "0x10e18"),
        ("Pont", "0x10e14"),
        ("Quai", "0x10e16"),
        ("Ruines", "0x10e15"),
        ("Sport de montagne", "0x10f0c"),
        ("Tunnel", "0x10e08"),
        // Catch-all
        ("NatureInconnue", "0x10c04"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature), ("TOPONYME", "T")]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| {
                panic!("CONSTR_LIN #{} ({}): rule error: {}", i + 1, nature, e)
            })
            .unwrap_or_else(|| {
                panic!("CONSTR_LIN #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "CONSTR_LIN #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "CONSTR_LIN #{} ({}): EndLevel should be 2",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "T",
            "CONSTR_LIN #{} ({}): Label mismatch",
            i + 1,
            nature
        );
    }
}

/// FME coverage: CONSTRUCTION_PONCTUELLE — 12 NATURE, no catch-all
#[test]
fn test_fme_coverage_construction_ponctuelle_all_12_branches() {
    let ruleset = load_ruleset("CONSTRUCTION_PONCTUELLE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Antenne", "0x11503"),
        ("Autre construction élevée", "0x06402"),
        ("Calvaire", "0x11507"),
        ("Cheminée", "0x11504"),
        ("Clocher", "0x10d0e"),
        ("Croix", "0x11507"),
        ("Eolienne", "0x11505"),
        ("Minaret", "0x10d0d"),
        ("Phare", "0x10101"),
        ("Puits d'hydrocarbures", "0x0640d"),
        ("Torchère", "0x11108"),
        ("Transformateur", "0x11506"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature), ("TOPONYME", "T")]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| {
                panic!("CONSTR_PONC #{} ({}): rule error: {}", i + 1, nature, e)
            })
            .unwrap_or_else(|| {
                panic!("CONSTR_PONC #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "CONSTR_PONC #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "1",
            "CONSTR_PONC #{} ({}): EndLevel should be 1",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "T",
            "CONSTR_PONC #{} ({}): Label mismatch",
            i + 1,
            nature
        );
    }
}

/// FME coverage: PYLONE — 1 wildcard
#[test]
fn test_fme_coverage_pylone_1_branch() {
    let ruleset = load_ruleset("PYLONE");
    // Any feature matches the wildcard
    let feature = attrs(&[("NATURE", "Quelconque")]);
    let result = rules::evaluate_feature(&ruleset, &feature)
        .unwrap()
        .unwrap();
    assert_eq!(result.get("Type").unwrap(), "0x11503");
    assert_eq!(result.get("EndLevel").unwrap(), "2");
    assert!(!result.contains_key("Label"), "PYLONE should have no Label");
}

/// FME coverage: TERRAIN_DE_SPORT — 5 NATURE + catch-all, EndLevel variable
#[test]
fn test_fme_coverage_terrain_sport_all_6_branches() {
    let ruleset = load_ruleset("TERRAIN_DE_SPORT");

    // (NATURE, expected_type, expected_end_level)
    let test_cases: Vec<(&str, &str, &str)> = vec![
        ("Bassin de natation", "0x10f1d", "2"),
        ("Grand terrain de sport", "0x1090d", "4"),
        ("Petit terrain multi-sports", "0x1100a", "2"),
        ("Piste de sport", "0x10f1b", "2"),
        ("Terrain de tennis", "0x10f1c", "2"),
        // Catch-all: Type=0x1090d but EndLevel=2 (not 4)
        ("NatureInconnue", "0x1090d", "2"),
    ];

    for (i, (nature, expected_type, expected_end_level)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature), ("NAT_DETAIL", "D")]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| {
                panic!("TERRAIN #{} ({}): rule error: {}", i + 1, nature, e)
            })
            .unwrap_or_else(|| {
                panic!("TERRAIN #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "TERRAIN #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            expected_end_level,
            "TERRAIN #{} ({}): EndLevel mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "D",
            "TERRAIN #{} ({}): Label mismatch",
            i + 1,
            nature
        );
    }
}

/// FME coverage: LIGNE_OROGRAPHIQUE — 3 NATURE, no catch-all
#[test]
fn test_fme_coverage_ligne_orographique_all_3_branches() {
    let ruleset = load_ruleset("LIGNE_OROGRAPHIQUE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Carrière", "0x10e1a"),
        ("Levée", "0x10e17"),
        ("Talus", "0x10e19"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[("NATURE", nature)]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| {
                panic!("OROGRAPHIQUE #{} ({}): rule error: {}", i + 1, nature, e)
            })
            .unwrap_or_else(|| {
                panic!("OROGRAPHIQUE #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "OROGRAPHIQUE #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "OROGRAPHIQUE #{} ({}): EndLevel should be 2",
            i + 1,
            nature
        );
        // Label = NATURE value (FME renames NATURE→NAME)
        assert_eq!(
            result.get("Label").unwrap(),
            nature,
            "OROGRAPHIQUE #{} ({}): Label should equal NATURE value",
            i + 1,
            nature
        );
    }
}

/// FME coverage: ZONE_DE_VEGETATION — 10 NATURE + catch-all, EndLevel variable, labels spéciaux
#[test]
fn test_fme_coverage_vegetation_all_11_branches() {
    let ruleset = load_ruleset("ZONE_DE_VEGETATION");

    // (NATURE, expected_type, expected_end_level, expected_label)
    let test_cases: Vec<(&str, &str, &str, Option<&str>)> = vec![
        ("Bois", "0x11005", "6", Some("Bois")),
        (
            "Forêt fermée de conifères",
            "0x10f1f",
            "6",
            Some("Forêt de conifères"),
        ),
        (
            "Forêt fermée de feuillus",
            "0x10f1e",
            "6",
            Some("Forêt de feuillus"),
        ),
        ("Forêt fermée mixte", "0x4e", "6", None),
        ("Forêt ouverte", "0x11000", "6", None),
        ("Haie", "0x11002", "4", None),
        ("Lande ligneuse", "0x11003", "4", None),
        ("Peupleraie", "0x11001", "4", Some("Peupleraie")),
        ("Verger", "0x11004", "4", Some("Verger")),
        ("Vigne", "0x11004", "4", Some("Vigne")),
        // Catch-all
        ("NatureInconnue", "0x11005", "4", None),
    ];

    for (i, (nature, expected_type, expected_end_level, expected_label)) in
        test_cases.iter().enumerate()
    {
        let feature = attrs(&[("NATURE", nature)]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| {
                panic!("VEGETATION #{} ({}): rule error: {}", i + 1, nature, e)
            })
            .unwrap_or_else(|| {
                panic!("VEGETATION #{} ({}): no match", i + 1, nature)
            });

        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "VEGETATION #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            expected_end_level,
            "VEGETATION #{} ({}): EndLevel mismatch",
            i + 1,
            nature
        );

        match expected_label {
            Some(label) => assert_eq!(
                result.get("Label").unwrap(),
                label,
                "VEGETATION #{} ({}): Label mismatch",
                i + 1,
                nature
            ),
            None => assert!(
                !result.contains_key("Label"),
                "VEGETATION #{} ({}): should have no Label, got {:?}",
                i + 1,
                nature,
                result.get("Label")
            ),
        }
    }
}

// ============================================================================
// Total rules count verification (Story 10.3)
// ============================================================================

#[test]
fn test_total_bati_veg_oro_rules_count() {
    let rules_file = rules::load_rules(&rules_path()).unwrap();

    let bati_veg_oro_layers = [
        ("BATIMENT", 13),
        ("CIMETIERE", 3),
        ("CONSTRUCTION_LINEAIRE", 12),
        ("CONSTRUCTION_PONCTUELLE", 12),
        ("PYLONE", 1),
        ("TERRAIN_DE_SPORT", 6),
        ("LIGNE_OROGRAPHIQUE", 3),
        ("ZONE_DE_VEGETATION", 11),
    ];

    for (layer, expected_count) in &bati_veg_oro_layers {
        let rs = rules_file
            .rulesets
            .iter()
            .find(|rs| rs.source_layer == *layer)
            .unwrap_or_else(|| panic!("Ruleset {} not found", layer));
        assert_eq!(
            rs.rules.len(),
            *expected_count,
            "Rule count mismatch for {}",
            layer
        );
    }

    let total: usize = rules_file
        .rulesets
        .iter()
        .filter(|rs| bati_veg_oro_layers.iter().any(|(l, _)| *l == rs.source_layer))
        .map(|rs| rs.rules.len())
        .sum();
    assert_eq!(total, 61, "Total bâti+végétation+orographie rules should be 61");
}
