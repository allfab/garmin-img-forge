//! Integration tests for services, zones réglementées & toponymie rulesets (Story 10.4)
//!
//! Validates that the BDTOPO services+zones+toponymie rules in bdtopo-garmin-rules.yaml
//! produce the correct Garmin attributes matching the FME source of truth:
//!   - 02-IGN-BDTOPO-ESRI-SHP-TO-ESRI-SHP-GARMIN-ZONES-REGLEMENTEES.fmw
//!   - 03-IGN-BDTOPO-ESRI-SHP-TO-ESRI-SHP-GARMIN-SERVICES-ACTIVITES.fmw
//!   - 08-IGN-BDTOPO-ESRI-SHP-TO-ESRI-SHP-GARMIN-TOPONYMIE.fmw

mod common;

use common::{assert_transform, attrs, load_ruleset, rules_path};
use mpforge::rules;

// ============================================================================
// Task 3.2: AC1 — ZONE_D_ACTIVITE_OU_D_INTERET wildcard
// FME: 03-SERVICES-ACTIVITES.fmw / Type=0x0c, EndLevel=4, Label=${TOPONYME}
// ============================================================================

#[test]
fn test_ac1_zone_activite_wildcard() {
    let ruleset = load_ruleset("ZONE_D_ACTIVITE_OU_D_INTERET");
    let feature = attrs(&[("NATURE", "Quelconque"), ("TOPONYME", "Zone industrielle Nord")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x0c",
        "4",
        Some("Zone industrielle Nord"),
    );
}

// ============================================================================
// Task 3.2: AC2 — LIGNE_ELECTRIQUE wildcard
// FME: 03-SERVICES-ACTIVITES.fmw / Type=0x29, EndLevel=2, Label="Ligne ${VOLTAGE}"
// ============================================================================

#[test]
fn test_ac2_ligne_electrique_wildcard() {
    let ruleset = load_ruleset("LIGNE_ELECTRIQUE");
    let feature = attrs(&[("NATURE", "Quelconque"), ("VOLTAGE", "225kV")]);
    assert_transform(&ruleset, &feature, "0x29", "2", Some("Ligne 225kV"));
}

// ============================================================================
// Task 3.3: AC2 — LIGNE_ELECTRIQUE Label="Ligne 400kV" (concatenation)
// ============================================================================

#[test]
fn test_ac2_ligne_electrique_label_concatenation_400kv() {
    let ruleset = load_ruleset("LIGNE_ELECTRIQUE");
    let feature = attrs(&[("NATURE", "Haute tension"), ("VOLTAGE", "400kV")]);
    assert_transform(&ruleset, &feature, "0x29", "2", Some("Ligne 400kV"));
}

// ============================================================================
// Task 3.2: AC3 — FORET_PUBLIQUE wildcard
// FME: 02-ZONES-REGLEMENTEES.fmw / Type=0x10a03, EndLevel=3, Label=${TOPONYME}
// ============================================================================

#[test]
fn test_ac3_foret_publique_wildcard() {
    let ruleset = load_ruleset("FORET_PUBLIQUE");
    let feature = attrs(&[("NATURE", "Quelconque"), ("TOPONYME", "Forêt de Chartreuse")]);
    assert_transform(
        &ruleset,
        &feature,
        "0x10a03",
        "3",
        Some("Forêt de Chartreuse"),
    );
}

// ============================================================================
// Task 3.4: AC4 — TOPONYMIE Aérodrome Héliport → 0x15500
// FME: 08-TOPONYMIE.fmw / CLASSE="Aérodrome", NATURE="Héliport"
// ============================================================================

#[test]
fn test_ac4_toponymie_aerodrome_heliport() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Aérodrome"),
        ("NATURE", "Héliport"),
        ("GRAPHIE", "Héliport du CHU"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x15500",
        "2",
        Some("Héliport du CHU"),
    );
}

// ============================================================================
// Task 3.4: AC5 — TOPONYMIE Aérodrome catch-all → 0x02d0b
// FME: DEFAULT_VALUE = 0x02d0b
// ============================================================================

#[test]
fn test_ac5_toponymie_aerodrome_catchall() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Aérodrome"),
        ("NATURE", "Hydravion"),
        ("GRAPHIE", "Base hydravion"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x02d0b",
        "2",
        Some("Base hydravion"),
    );
}

// ============================================================================
// Task 3.5: AC6 — TOPONYMIE Détail orographique Col → 0x06601
// FME: CLASSE="Détail orographique", NATURE="Col"
// ============================================================================

#[test]
fn test_ac6_toponymie_detail_orographique_col() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Détail orographique"),
        ("NATURE", "Col"),
        ("GRAPHIE", "Col du Galibier"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x06601",
        "2",
        Some("Col du Galibier"),
    );
}

// ============================================================================
// Task 3.5: AC7 — TOPONYMIE Détail orographique NATURE non reconnue → no-match
// FME: DEFAULT = _FME_NO_OP_
// ============================================================================

#[test]
fn test_ac7_toponymie_detail_orographique_no_match() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Détail orographique"),
        ("NATURE", "Falaise"),
        ("GRAPHIE", "Falaise de Presles"),
    ]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "TOPONYMIE Détail orographique NATURE='Falaise' should not match (no catch-all)"
    );
}

// ============================================================================
// Task 3.6: AC8 — TOPONYMIE Zone d'activité Musée → 0x02c02
// FME: CLASSE="Zone d'activité ou d'intérêt", NATURE="Musée"
// ============================================================================

#[test]
fn test_ac8_toponymie_zai_musee() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Zone d'activité ou d'intérêt"),
        ("NATURE", "Musée"),
        ("GRAPHIE", "Musée de Grenoble"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x02c02",
        "2",
        Some("Musée de Grenoble"),
    );
}

// ============================================================================
// Task 3.6: AC9 — TOPONYMIE Zone d'activité Site skiable → no-match
// FME: FME_NULL_VALUE → feature supprimée
// ============================================================================

#[test]
fn test_ac9_toponymie_zai_site_skiable_no_match() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Zone d'activité ou d'intérêt"),
        ("NATURE", "Site skiable"),
        ("GRAPHIE", "Chamrousse"),
    ]);
    let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
    assert!(
        result.is_none(),
        "TOPONYMIE ZAI NATURE='Site skiable' should not match (FME_NULL_VALUE → excluded)"
    );
}

// ============================================================================
// Task 3.7: AC10 — TOPONYMIE Zone d'habitation catch-all → 0x00d00
// FME: DEFAULT_VALUE = 0x00d00
// ============================================================================

#[test]
fn test_ac10_toponymie_zone_habitation_catchall() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Zone d'habitation"),
        ("NATURE", "Hameau"),
        ("GRAPHIE", "Les Granges"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x00d00",
        "2",
        Some("Les Granges"),
    );
}

// ============================================================================
// Task 3.8: AC11 — TOPONYMIE Forêt publique wildcard → 0x0660a
// FME: mapping statique, pas de condition NATURE
// ============================================================================

#[test]
fn test_ac11_toponymie_foret_publique_wildcard() {
    let ruleset = load_ruleset("TOPONYMIE");
    let feature = attrs(&[
        ("CLASSE", "Forêt publique"),
        ("NATURE", "Quelconque"),
        ("GRAPHIE", "Forêt domaniale de Belledonne"),
    ]);
    assert_transform(
        &ruleset,
        &feature,
        "0x0660a",
        "2",
        Some("Forêt domaniale de Belledonne"),
    );
}

// ============================================================================
// Task 3.9: AC13 — TOPONYMIE CLASSE non traitée → no-match
// 11 catégories CLASSE supprimées (gérées par autres rulesets)
// ============================================================================

#[test]
fn test_ac13_toponymie_classe_non_traitee_no_match() {
    let ruleset = load_ruleset("TOPONYMIE");

    let ignored_classes = [
        "Cours d'eau",
        "Cours d'eau GE",
        "Détail hydrographique",
        "Equipement de transport",
        "Parc ou réserve",
        "Plan d'eau",
        "Point du réseau",
        "Poste de transformation",
        "Route",
        "Transport par cable",
        "Voie ferrée",
    ];

    for classe in &ignored_classes {
        let feature = attrs(&[
            ("CLASSE", classe),
            ("NATURE", "Quelconque"),
            ("GRAPHIE", "Test"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature).unwrap();
        assert!(
            result.is_none(),
            "TOPONYMIE CLASSE='{}' should not match (handled by other rulesets)",
            classe
        );
    }
}

// ============================================================================
// Task 3.10: Tests paramétrés — couverture par catégorie CLASSE
// Minimum 1 test par catégorie
// ============================================================================

/// Catégorie Aérodrome: 3 NATURE + catch-all
#[test]
fn test_toponymie_aerodrome_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Aérodrome", "0x02d0b"),
        ("Altiport", "0x15500"),
        ("Héliport", "0x15500"),
        // Catch-all
        ("NatureInconnue", "0x02d0b"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Aérodrome"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Aérodrome/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("Aérodrome/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Aérodrome/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
        assert_eq!(result.get("Label").unwrap(), "G");
    }
}

/// Catégorie Cimetière: wildcard (optimisé depuis 3+catch-all)
#[test]
fn test_toponymie_cimetiere_wildcard() {
    let ruleset = load_ruleset("TOPONYMIE");

    for nature in &["Civil", "Militaire", "Militaire étranger", "NatureInconnue"] {
        let feature = attrs(&[
            ("CLASSE", "Cimetière"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Cimetière/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("Cimetière/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            "0x06403",
            "Cimetière/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }
}

/// Catégorie Construction linéaire: 8 NATURE + catch-all
#[test]
fn test_toponymie_construction_lineaire_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Barrage", "0x06407"),
        ("Mur", "0x06402"),
        ("Mur de soutènement", "0x06402"),
        ("Pont", "0x06401"),
        ("Quai", "0x06402"),
        ("Ruines", "0x11514"),
        ("Sport de montagne", "0x11601"),
        ("Tunnel", "0x06413"),
        // Catch-all
        ("NatureInconnue", "0x06402"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Construction linéaire"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Constr.lin/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("Constr.lin/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Constr.lin/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }
}

/// Catégorie Construction ponctuelle: 5 NATURE + catch-all
#[test]
fn test_toponymie_construction_ponctuelle_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Autre construction élevée", "0x06402"),
        ("Calvaire", "0x11507"),
        ("Clocher", "0x10d0e"),
        ("Croix", "0x11507"),
        ("Eolienne", "0x11505"),
        // Catch-all
        ("NatureInconnue", "0x10101"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Construction ponctuelle"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Constr.ponc/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("Constr.ponc/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Constr.ponc/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }
}

/// Catégorie Construction surfacique: 4 NATURE + catch-all
#[test]
fn test_toponymie_construction_surfacique_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Barrage", "0x06407"),
        ("Dalle", "0x06401"),
        ("Ecluse", "0x06401"),
        ("Pont", "0x06401"),
        // Catch-all
        ("NatureInconnue", "0x06401"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Construction surfacique"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Constr.surf/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("Constr.surf/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Constr.surf/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }
}

/// Catégorie Détail orographique: 23 NATURE, pas de catch-all
#[test]
fn test_toponymie_detail_orographique_all_23_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Cap", "0x06606"),
        ("Cirque", "0x06608"),
        ("Col", "0x06601"),
        ("Crête", "0x06613"),
        ("Dépression", "0x0660b"),
        ("Dune", "0x15502"),
        ("Escarpement", "0x06607"),
        ("Gorge", "0x06611"),
        ("Gouffre", "0x11515"),
        ("Grotte", "0x11515"),
        ("Ile", "0x06501"),
        ("Isthme", "0x0660d"),
        ("Montagne", "0x06601"),
        ("Pic", "0x06616"),
        ("Plage", "0x1160e"),
        ("Plaine", "0x06610"),
        ("Récif", "0x15504"),
        ("Rochers", "0x06614"),
        ("Sommet", "0x06616"),
        ("Terril", "0x15503"),
        ("Vallée", "0x06617"),
        ("Versant", "0x06615"),
        ("Volcan", "0x06608"),
    ];

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[
            ("CLASSE", "Détail orographique"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("Orogr. #{} ({}): error: {}", i + 1, nature, e))
            .unwrap_or_else(|| panic!("Orogr. #{} ({}): no match", i + 1, nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "Orogr. #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
        assert_eq!(result.get("Label").unwrap(), "G");
    }
}

/// Catégorie Forêt publique: 1 wildcard
/// (couvert par test_ac11_toponymie_foret_publique_wildcard — test dédupliqué par code review)

/// Catégorie Lieu-dit non habité: 4 NATURE, pas de catch-all
#[test]
fn test_toponymie_lieu_dit_non_habite_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Arbre", "0x1150c"),
        ("Bois", "0x0660a"),
        ("Pare-feu", "0x11609"),
        ("Lieu-dit non habité", "0x1150e"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Lieu-dit non habité"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("LieuDit/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("LieuDit/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "LieuDit/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }

    // Verify no catch-all: unknown NATURE should not match
    let unknown = attrs(&[
        ("CLASSE", "Lieu-dit non habité"),
        ("NATURE", "Inconnu"),
        ("GRAPHIE", "G"),
    ]);
    let result = rules::evaluate_feature(&ruleset, &unknown).unwrap();
    assert!(
        result.is_none(),
        "Lieu-dit non habité NATURE='Inconnu' should not match"
    );
}

/// Catégorie Zone d'habitation: 6 NATURE + catch-all
#[test]
fn test_toponymie_zone_habitation_all_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Château", "0x1150f"),
        ("Grange", "0x11510"),
        ("Lieu-dit habité", "0x00d00"),
        ("Moulin", "0x11512"),
        ("Quartier", "0x11513"),
        ("Ruines", "0x11514"),
        // Catch-all
        ("NatureInconnue", "0x00d00"),
    ];

    for (nature, expected_type) in &test_cases {
        let feature = attrs(&[
            ("CLASSE", "Zone d'habitation"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("ZoneHab/{}: error: {}", nature, e))
            .unwrap_or_else(|| panic!("ZoneHab/{}: no match", nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "ZoneHab/{}: Type mismatch",
            nature
        );
        assert_eq!(result.get("EndLevel").unwrap(), "2");
    }
}

// ============================================================================
// Task 4: Validation couverture FME complète (AC12)
// Tests paramétrés couvrant TOUTES les branches FME des 10 catégories TOPONYMIE
// ============================================================================

/// FME coverage: Zone d'activité ou d'intérêt — 102 NATURE, pas de catch-all
/// FME: 08-TOPONYMIE.fmw / TestFilter CLASSE="Zone d'activité ou d'intérêt"
/// Traçabilité: chaque entrée correspond à une branche FME NATURE→MP_TYPE
#[test]
fn test_fme_coverage_toponymie_zai_all_102_branches() {
    let ruleset = load_ruleset("TOPONYMIE");

    let test_cases: Vec<(&str, &str)> = vec![
        ("Abri de montagne", "0x02b04"),
        ("Administration centrale de l'Etat", "0x03007"),
        ("Aire d'accueil des gens du voyage", "0x02f0c"),
        ("Aire de détente", "0x02c04"),
        ("Aquaculture", "0x02900"),
        ("Autre équipement sportif", "0x11601"),
        ("Autre établissement d'enseignement", "0x02c05"),
        ("Autre service déconcentré de l'Etat", "0x03007"),
        ("Baignade surveillée", "0x1151f"),
        ("Borne", "0x0660f"),
        ("Borne frontière", "0x0660f"),
        ("Camp militaire non clos", "0x0640b"),
        ("Camping", "0x02b03"),
        ("Capitainerie", "0x02f09"),
        ("Carrière", "0x0640c"),
        ("Caserne", "0x03008"),
        ("Caserne de pompiers", "0x03008"),
        ("Centrale électrique", "0x02900"),
        ("Centre de documentation", "0x02c03"),
        ("Centre équestre", "0x02d0a"),
        ("Champ de tir", "0x0640b"),
        ("Collège", "0x02c05"),
        ("Complexe sportif couvert", "0x11601"),
        ("Construction", "0x06402"),
        ("Culte chrétien", "0x02c0e"),
        ("Culte divers", "0x02c0b"),
        ("Culte israélite", "0x02c10"),
        ("Culte musulman", "0x02c0d"),
        ("Déchèterie", "0x02900"),
        ("Divers agricole", "0x1150e"),
        ("Divers commercial", "0x02e04"),
        ("Divers industriel", "0x02900"),
        ("Divers public ou administratif", "0x03007"),
        ("Ecomusée", "0x11600"),
        ("Elevage", "0x02900"),
        ("Enceinte militaire", "0x0640b"),
        ("Enseignement primaire", "0x02c05"),
        ("Enseignement supérieur", "0x02c05"),
        ("Equipement de cyclisme", "0x1160a"),
        ("Espace public", "0x1160d"),
        ("Etablissement extraterritorial", "0x03007"),
        ("Etablissement hospitalier", "0x03002"),
        ("Etablissement pénitentiaire", "0x02900"),
        ("Etablissement thermal", "0x02900"),
        ("Gendarmerie", "0x03001"),
        ("Golf", "0x02d05"),
        ("Habitation troglodytique", "0x11509"),
        ("Haras", "0x02d0a"),
        ("Hébergement de loisirs", "0x02b04"),
        ("Hippodrome", "0x02c08"),
        ("Hôpital", "0x03002"),
        ("Hôtel de département", "0x03003"),
        ("Hôtel de région", "0x03003"),
        ("Hôtel de collectivité", "0x03003"),
        ("Lycée", "0x02c05"),
        ("Mairie", "0x03003"),
        ("Maison de retraite", "0x02900"),
        ("Maison du parc", "0x06402"),
        ("Maison forestière", "0x14e01"),
        ("Marais salant", "0x15505"),
        ("Marché", "0x02e04"),
        ("Mégalithe", "0x11508"),
        ("Mine", "0x0640c"),
        ("Monument", "0x14e0f"),
        ("Musée", "0x02c02"),
        ("Office de tourisme", "0x02f0c"),
        ("Ouvrage militaire", "0x0640b"),
        ("Palais de justice", "0x03004"),
        ("Parc de loisirs", "0x02c01"),
        ("Parc des expositions", "0x10d09"),
        ("Parc zoologique", "0x02c07"),
        ("Patinoire", "0x02d08"),
        ("Piscine", "0x02d09"),
        ("Point de vue", "0x02c04"),
        ("Police", "0x03001"),
        ("Poste", "0x02f05"),
        ("Préfecture", "0x03007"),
        ("Préfecture de région", "0x03007"),
        ("Refuge", "0x02b04"),
        ("Salle de danse ou de jeux", "0x02d04"),
        ("Salle de spectacle ou conférence", "0x02d01"),
        ("Science", "0x02c05"),
        ("Sentier de découverte", "0x06412"),
        ("Siège d'EPCI", "0x03003"),
        ("Site de vol libre", "0x02d0b"),
        ("Site d'escalade", "0x11601"),
        ("Sous-préfecture", "0x03007"),
        ("Sports en eaux vives", "0x11601"),
        ("Sports mécaniques", "0x11601"),
        ("Sports nautiques", "0x11601"),
        ("Stade", "0x02c08"),
        ("Stand de tir", "0x02900"),
        ("Station de pompage", "0x02900"),
        ("Station d'épuration", "0x02900"),
        ("Structure d'accueil pour personnes handicapées", "0x02900"),
        ("Surveillance maritime", "0x1151f"),
        ("Tombeau", "0x06403"),
        ("Université", "0x02c05"),
        ("Usine", "0x02900"),
        ("Usine de production d'eau potable", "0x02900"),
        ("Vestige archéologique", "0x1150b"),
        ("Zone industrielle", "0x02900"),
    ];

    assert_eq!(
        test_cases.len(),
        102,
        "ZAI coverage: expected 102 NATURE branches"
    );

    for (i, (nature, expected_type)) in test_cases.iter().enumerate() {
        let feature = attrs(&[
            ("CLASSE", "Zone d'activité ou d'intérêt"),
            ("NATURE", nature),
            ("GRAPHIE", "G"),
        ]);
        let result = rules::evaluate_feature(&ruleset, &feature)
            .unwrap_or_else(|e| panic!("ZAI #{} ({}): error: {}", i + 1, nature, e))
            .unwrap_or_else(|| panic!("ZAI #{} ({}): no match", i + 1, nature));
        assert_eq!(
            result.get("Type").unwrap(),
            expected_type,
            "ZAI #{} ({}): Type mismatch",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("EndLevel").unwrap(),
            "2",
            "ZAI #{} ({}): EndLevel should be 2",
            i + 1,
            nature
        );
        assert_eq!(
            result.get("Label").unwrap(),
            "G",
            "ZAI #{} ({}): Label mismatch",
            i + 1,
            nature
        );
    }
}

// ============================================================================
// Task 4.3: Comptage des règles — vérification complétude
// ============================================================================

#[test]
fn test_total_story_10_4_rules_count() {
    let rules_file = rules::load_rules(&rules_path()).unwrap();

    let story_10_4_layers = [
        ("ZONE_D_ACTIVITE_OU_D_INTERET", 1),
        ("LIGNE_ELECTRIQUE", 1),
        ("FORET_PUBLIQUE", 1),
        // TOPONYMIE: 162 rules (optimized from 165: Cimetière 4→1 saves 3)
        ("TOPONYMIE", 162),
    ];

    for (layer, expected_count) in &story_10_4_layers {
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

    // Total Story 10.4: 1+1+1+162 = 165
    let total_10_4: usize = rules_file
        .rulesets
        .iter()
        .filter(|rs| story_10_4_layers.iter().any(|(l, _)| *l == rs.source_layer))
        .map(|rs| rs.rules.len())
        .sum();
    assert_eq!(
        total_10_4, 165,
        "Total Story 10.4 rules should be 165 (168 - 3 Cimetière optimization)"
    );

    // Grand total: Stories 10.1-10.3 (109) + Story 10.4 (165) = 274
    let grand_total: usize = rules_file.rulesets.iter().map(|rs| rs.rules.len()).sum();
    assert_eq!(
        grand_total, 274,
        "Grand total rules should be 274 (109 + 165)"
    );
}
