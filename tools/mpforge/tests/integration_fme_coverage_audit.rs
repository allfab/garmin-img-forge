//! FME coverage audit: exhaustive traceability of FME branches → YAML rules (Story 10.5)
//!
//! Validates that every conditional branch from the 8 FME projects has a corresponding
//! rule in bdtopo-garmin-rules.yaml. This is a regression test on rules completeness,
//! not a functional test of the rules engine.
//!
//! FME source of truth:
//!   01-TRANSPORT.fmw         → Routes, VF, Aéro, Câbles, Chef-lieu, Zones habitation
//!   02-ZONES-REGLEMENTEES.fmw → FORET_PUBLIQUE
//!   03-SERVICES-ACTIVITES.fmw  → ZONE_D_ACTIVITE, LIGNE_ELECTRIQUE
//!   04-HYDROGRAPHIE.fmw       → 3 couches hydro
//!   05-VEGETATION.fmw         → ZONE_DE_VEGETATION
//!   06-BATI.fmw               → 7 couches bâti
//!   07-OROGRAPHIE.fmw         → LIGNE_OROGRAPHIQUE
//!   08-TOPONYMIE.fmw          → TOPONYMIE (10 catégories CLASSE)

mod common;

use common::{attrs, load_ruleset, rules_path};
use mpforge::rules;

/// Helper: verify a ruleset has at least N rules and uses expected match fields
fn audit_ruleset(source_layer: &str, min_rules: usize, expected_match_fields: &[&str]) {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let rs = rules_file
        .rulesets
        .iter()
        .find(|r| r.source_layer == source_layer)
        .unwrap_or_else(|| panic!("Ruleset missing for layer: {}", source_layer));

    assert!(
        rs.rules.len() >= min_rules,
        "Ruleset {} has {} rules, expected >= {}",
        source_layer,
        rs.rules.len(),
        min_rules
    );

    for field in expected_match_fields {
        let found = rs
            .rules
            .iter()
            .any(|r| r.match_conditions.contains_key(*field));
        assert!(
            found,
            "Ruleset {} doesn't use match field '{}'",
            source_layer, field
        );
    }
}

/// Helper: verify a specific match condition produces expected Type and EndLevel
fn audit_branch(
    source_layer: &str,
    feature_attrs: &[(&str, &str)],
    expected_type: &str,
    expected_endlevel: &str,
) {
    let ruleset = load_ruleset(source_layer);
    let fa = attrs(feature_attrs);
    let result = rules::evaluate_feature(&ruleset, &fa)
        .expect("Rule evaluation error")
        .unwrap_or_else(|| {
            panic!(
                "No rule matched for {} with {:?}",
                source_layer, feature_attrs
            )
        });
    assert_eq!(
        result.get("Type").unwrap(),
        expected_type,
        "Type mismatch for {} {:?}",
        source_layer,
        feature_attrs
    );
    assert_eq!(
        result.get("EndLevel").unwrap(),
        expected_endlevel,
        "EndLevel mismatch for {} {:?}",
        source_layer,
        feature_attrs
    );
}

// ============================================================================
// FME 01 — TRANSPORT: TRONCON_DE_ROUTE (17 branches CL_ADMIN + NATURE)
// ============================================================================

#[test]
fn test_fme01_troncon_de_route_structure() {
    audit_ruleset("TRONCON_DE_ROUTE", 17, &["CL_ADMIN", "NATURE"]);
}

#[test]
fn test_fme01_troncon_de_route_cl_admin_branches() {
    // CL_ADMIN priority branches (6 rules)
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Autoroute"), ("NATURE", "Route à 2 chaussées")],
        "0x01",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[
            ("CL_ADMIN", "Nationale"),
            ("NATURE", "Route à 1 chaussée"),
        ],
        "0x04",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Nationale"), ("NATURE", "Rond-point")],
        "0x04",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[
            ("CL_ADMIN", "Départementale"),
            ("NATURE", "Route à 1 chaussée"),
        ],
        "0x05",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", "Départementale"), ("NATURE", "Rond-point")],
        "0x04",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[
            ("CL_ADMIN", "Route intercommunale"),
            ("NATURE", "Route à 1 chaussée"),
        ],
        "0x05",
        "7",
    );
}

#[test]
fn test_fme01_troncon_de_route_nature_fallback_branches() {
    // NATURE fallback branches (11 rules: 7-17)
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Route à 1 chaussée")],
        "0x06",
        "2",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Route à 2 chaussées")],
        "0x06",
        "2",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Rond-point")],
        "0x06",
        "2",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Route empierrée")],
        "0x07",
        "1",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Bretelle")],
        "0x09",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Type autoroutier")],
        "0x09",
        "7",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Chemin")],
        "0x0a",
        "1",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Escalier")],
        "0x0f",
        "1",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Piste cyclable")],
        "0x0e",
        "1",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Sentier")],
        "0x10",
        "1",
    );
    audit_branch(
        "TRONCON_DE_ROUTE",
        &[("CL_ADMIN", ""), ("NATURE", "Bac ou liaison maritime")],
        "0x1b",
        "1",
    );
}

// ============================================================================
// FME 01 — TRANSPORT: TRONCON_DE_VOIE_FERREE (7 branches POS_SOL + NATURE)
// ============================================================================

#[test]
fn test_fme01_troncon_de_voie_ferree_structure() {
    audit_ruleset("TRONCON_DE_VOIE_FERREE", 7, &["POS_SOL", "NATURE"]);
}

#[test]
fn test_fme01_voie_ferree_branches() {
    // POS_SOL=-1 tunnel override (rule 1)
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[("POS_SOL", "-1"), ("NATURE", "Voie ferrée principale")],
        "0x10e06",
        "5",
    );
    // NATURE-specific branches (rules 2-7, all with POS_SOL != -1)
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[
            ("POS_SOL", "0"),
            ("NATURE", "Voie ferrée principale"),
        ],
        "0x10c00",
        "5",
    );
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[("POS_SOL", "0"), ("NATURE", "LGV")],
        "0x10e02",
        "5",
    );
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[("POS_SOL", "0"), ("NATURE", "Voie de service")],
        "0x10e03",
        "5",
    );
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[("POS_SOL", "0"), ("NATURE", "Tramway")],
        "0x10e04",
        "5",
    );
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[
            ("POS_SOL", "0"),
            ("NATURE", "Funiculaire ou crémaillère"),
        ],
        "0x10e05",
        "5",
    );
    audit_branch(
        "TRONCON_DE_VOIE_FERREE",
        &[("POS_SOL", "0"), ("NATURE", "Sans objet")],
        "0x10c00",
        "5",
    );
}

// ============================================================================
// FME 01 — TRANSPORT: PISTE_D_AERODROME (2), TRANSPORT_PAR_CABLE (1)
// ============================================================================

#[test]
fn test_fme01_piste_aerodrome_structure() {
    audit_ruleset("PISTE_D_AERODROME", 2, &["NATURE"]);
}

#[test]
fn test_fme01_piste_aerodrome_branches() {
    audit_branch(
        "PISTE_D_AERODROME",
        &[("NATURE", "Piste en dur")],
        "0x1090b",
        "4",
    );
    audit_branch(
        "PISTE_D_AERODROME",
        &[("NATURE", "Piste en herbe")],
        "0x10f17",
        "4",
    );
}

#[test]
fn test_fme01_transport_par_cable_wildcard() {
    audit_ruleset("TRANSPORT_PAR_CABLE", 1, &[]);
    audit_branch(
        "TRANSPORT_PAR_CABLE",
        &[("NATURE", "Téléphérique")],
        "0x10f0b",
        "2",
    );
}

// ============================================================================
// FME 01 — Chef-lieu: COMMUNE (1), ZONE_D_HABITATION (1)
// ============================================================================

#[test]
fn test_fme01_commune_structure() {
    audit_ruleset("COMMUNE", 1, &["NOM"]);
    audit_branch(
        "COMMUNE",
        &[("NOM", "Grenoble"), ("CODE_POST", "38000")],
        "0x54",
        "7",
    );
}

#[test]
fn test_fme01_zone_habitation_structure() {
    audit_ruleset("ZONE_D_HABITATION", 1, &["NATURE"]);
    audit_branch(
        "ZONE_D_HABITATION",
        &[("NATURE", "Lieu-dit habité"), ("TOPONYME", "Le Hameau")],
        "0x03",
        "4",
    );
}

// ============================================================================
// FME 02 — ZONES REGLEMENTEES: FORET_PUBLIQUE (1 wildcard)
// ============================================================================

#[test]
fn test_fme02_foret_publique_wildcard() {
    audit_ruleset("FORET_PUBLIQUE", 1, &[]);
    audit_branch(
        "FORET_PUBLIQUE",
        &[("NATURE", "Domaniale"), ("TOPONYME", "Forêt de Chartreuse")],
        "0x10a03",
        "3",
    );
}

// ============================================================================
// FME 03 — SERVICES: ZONE_D_ACTIVITE (1), LIGNE_ELECTRIQUE (1)
// ============================================================================

#[test]
fn test_fme03_zone_activite_wildcard() {
    audit_ruleset("ZONE_D_ACTIVITE_OU_D_INTERET", 1, &[]);
    audit_branch(
        "ZONE_D_ACTIVITE_OU_D_INTERET",
        &[("NATURE", "Quelconque"), ("TOPONYME", "Zone industrielle")],
        "0x0c",
        "4",
    );
}

#[test]
fn test_fme03_ligne_electrique_wildcard() {
    audit_ruleset("LIGNE_ELECTRIQUE", 1, &[]);
    audit_branch(
        "LIGNE_ELECTRIQUE",
        &[("NATURE", "Haute tension"), ("VOLTAGE", "400kV")],
        "0x29",
        "2",
    );
}

// ============================================================================
// FME 04 — HYDROGRAPHIE: TRONCON (2), SURFACE (4), DETAIL (13)
// ============================================================================

#[test]
fn test_fme04_troncon_hydrographique_structure() {
    audit_ruleset("TRONCON_HYDROGRAPHIQUE", 2, &["PERSISTANC"]);
}

#[test]
fn test_fme04_troncon_hydrographique_branches() {
    audit_branch(
        "TRONCON_HYDROGRAPHIQUE",
        &[("PERSISTANC", "Intermittent"), ("NOM_C_EAU", "Ruisseau des Prés")],
        "0x26",
        "2",
    );
    // Catch-all (Permanent, Inconnue, vide)
    audit_branch(
        "TRONCON_HYDROGRAPHIQUE",
        &[("PERSISTANC", "Permanent"), ("NOM_C_EAU", "L'Isère")],
        "0x18",
        "2",
    );
}

#[test]
fn test_fme04_surface_hydrographique_structure() {
    audit_ruleset("SURFACE_HYDROGRAPHIQUE", 4, &["NATURE", "PERSISTANC"]);
}

#[test]
fn test_fme04_surface_hydrographique_branches() {
    // NATURE priority: Glacier, névé
    audit_branch(
        "SURFACE_HYDROGRAPHIQUE",
        &[("NATURE", "Glacier, névé"), ("PERSISTANC", "Permanent")],
        "0x4d",
        "2",
    );
    // PERSISTANC branches
    audit_branch(
        "SURFACE_HYDROGRAPHIQUE",
        &[("NATURE", "Eau libre"), ("PERSISTANC", "Intermittent")],
        "0x4c",
        "2",
    );
    audit_branch(
        "SURFACE_HYDROGRAPHIQUE",
        &[("NATURE", "Eau libre"), ("PERSISTANC", "Inconnue")],
        "0x4c",
        "2",
    );
    // Catch-all (Permanent, vide)
    audit_branch(
        "SURFACE_HYDROGRAPHIQUE",
        &[("NATURE", "Eau libre"), ("PERSISTANC", "Permanent")],
        "0x3f",
        "2",
    );
}

#[test]
fn test_fme04_detail_hydrographique_structure() {
    audit_ruleset("DETAIL_HYDROGRAPHIQUE", 13, &["NATURE"]);
}

#[test]
fn test_fme04_detail_hydrographique_branches() {
    // 12 explicit NATURE values + 1 catch-all
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Arroyo")], "0x06501", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Baie")], "0x06503", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Cascade")], "0x06508", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Crique")], "0x06507", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Fontaine")], "0x06509", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Glacier")], "0x0650a", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Lac")], "0x0650d", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Marais")], "0x06513", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Mer")], "0x06510", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Réservoir")], "0x0650f", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Source")], "0x06511", "2");
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Source captée")], "0x06511", "2");
    // Catch-all (Citerne, Lavoir, Perte, Point d'eau, Résurgence)
    audit_branch("DETAIL_HYDROGRAPHIQUE", &[("NATURE", "Citerne")], "0x06414", "2");
}

// ============================================================================
// FME 05 — VEGETATION: ZONE_DE_VEGETATION (11 branches)
// ============================================================================

#[test]
fn test_fme05_zone_de_vegetation_structure() {
    audit_ruleset("ZONE_DE_VEGETATION", 11, &["NATURE"]);
}

#[test]
fn test_fme05_vegetation_branches() {
    // 10 explicit NATURE + 1 catch-all
    audit_branch("ZONE_DE_VEGETATION", &[("NATURE", "Bois")], "0x11005", "6");
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Forêt fermée de conifères")],
        "0x10f1f",
        "6",
    );
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Forêt fermée de feuillus")],
        "0x10f1e",
        "6",
    );
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Forêt fermée mixte")],
        "0x4e",
        "6",
    );
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Forêt ouverte")],
        "0x11000",
        "6",
    );
    audit_branch("ZONE_DE_VEGETATION", &[("NATURE", "Haie")], "0x11002", "4");
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Lande ligneuse")],
        "0x11003",
        "4",
    );
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Peupleraie")],
        "0x11001",
        "4",
    );
    audit_branch("ZONE_DE_VEGETATION", &[("NATURE", "Verger")], "0x11004", "4");
    audit_branch("ZONE_DE_VEGETATION", &[("NATURE", "Vigne")], "0x11004", "4");
    // Catch-all
    audit_branch(
        "ZONE_DE_VEGETATION",
        &[("NATURE", "Bambouseraie")],
        "0x11005",
        "4",
    );
}

// ============================================================================
// FME 06 — BATI: BATIMENT (13), CIMETIERE (3), CONSTRUCTION_LINEAIRE (12),
//   CONSTRUCTION_PONCTUELLE (12), PYLONE (1), TERRAIN_DE_SPORT (6)
// ============================================================================

#[test]
fn test_fme06_batiment_structure() {
    audit_ruleset("BATIMENT", 13, &["NATURE"]);
}

#[test]
fn test_fme06_batiment_key_branches() {
    audit_branch("BATIMENT", &[("NATURE", "Arène ou théâtre antique")], "0x10f08", "2");
    audit_branch("BATIMENT", &[("NATURE", "Chapelle")], "0x10f09", "2");
    audit_branch("BATIMENT", &[("NATURE", "Château")], "0x10f0a", "2");
    audit_branch("BATIMENT", &[("NATURE", "Eglise")], "0x10f0b", "2");
    audit_branch("BATIMENT", &[("NATURE", "Fort, blockhaus, casemate")], "0x10f0c", "2");
    audit_branch("BATIMENT", &[("NATURE", "Indifférenciée")], "0x1101c", "2");
    audit_branch("BATIMENT", &[("NATURE", "Industriel, agricole ou commercial")], "0x10f04", "2");
    audit_branch("BATIMENT", &[("NATURE", "Monument")], "0x10f0d", "2");
    audit_branch("BATIMENT", &[("NATURE", "Serre")], "0x10f05", "2");
    audit_branch("BATIMENT", &[("NATURE", "Silo")], "0x10f06", "2");
    audit_branch("BATIMENT", &[("NATURE", "Tour, donjon")], "0x10f11", "2");
    audit_branch("BATIMENT", &[("NATURE", "Tribune")], "0x10f12", "2");
    // Catch-all
    audit_branch("BATIMENT", &[("NATURE", "Préfecture")], "0x1101c", "2");
}

#[test]
fn test_fme06_cimetiere_structure() {
    audit_ruleset("CIMETIERE", 3, &["NATURE"]);
}

#[test]
fn test_fme06_cimetiere_branches() {
    audit_branch("CIMETIERE", &[("NATURE", "Civil")], "0x1a", "4");
    audit_branch("CIMETIERE", &[("NATURE", "Militaire")], "0x10f13", "4");
    audit_branch("CIMETIERE", &[("NATURE", "Militaire étranger")], "0x10f13", "4");
}

#[test]
fn test_fme06_construction_lineaire_structure() {
    audit_ruleset("CONSTRUCTION_LINEAIRE", 12, &["NATURE"]);
}

#[test]
fn test_fme06_construction_lineaire_key_branches() {
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Autre ligne descriptive")], "0x10c04", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Barrage")], "0x10f08", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Clôture")], "0x13309", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Mur")], "0x13308", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Mur anti-bruit")], "0x10e13", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Mur de soutènement")], "0x10e18", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Pont")], "0x10e14", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Quai")], "0x10e16", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Ruines")], "0x10e15", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Sport de montagne")], "0x10f0c", "2");
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Tunnel")], "0x10e08", "2");
    // Catch-all
    audit_branch("CONSTRUCTION_LINEAIRE", &[("NATURE", "Inconnu")], "0x10c04", "2");
}

#[test]
fn test_fme06_construction_ponctuelle_structure() {
    audit_ruleset("CONSTRUCTION_PONCTUELLE", 12, &["NATURE"]);
}

#[test]
fn test_fme06_construction_ponctuelle_key_branches() {
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Antenne")], "0x11503", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Autre construction élevée")], "0x06402", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Calvaire")], "0x11507", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Cheminée")], "0x11504", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Clocher")], "0x10d0e", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Croix")], "0x11507", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Eolienne")], "0x11505", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Minaret")], "0x10d0d", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Phare")], "0x10101", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Puits d'hydrocarbures")], "0x0640d", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Torchère")], "0x11108", "1");
    audit_branch("CONSTRUCTION_PONCTUELLE", &[("NATURE", "Transformateur")], "0x11506", "1");
}

#[test]
fn test_fme06_pylone_wildcard() {
    audit_ruleset("PYLONE", 1, &[]);
    audit_branch("PYLONE", &[("NATURE", "Quelconque")], "0x11503", "2");
}

#[test]
fn test_fme06_terrain_de_sport_structure() {
    audit_ruleset("TERRAIN_DE_SPORT", 6, &["NATURE"]);
}

#[test]
fn test_fme06_terrain_de_sport_key_branches() {
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Bassin de natation")], "0x10f1d", "2");
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Grand terrain de sport")], "0x1090d", "4");
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Petit terrain multi-sports")], "0x1100a", "2");
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Piste de sport")], "0x10f1b", "2");
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Terrain de tennis")], "0x10f1c", "2");
    // Catch-all
    audit_branch("TERRAIN_DE_SPORT", &[("NATURE", "Autre")], "0x1090d", "2");
}

// ============================================================================
// FME 07 — OROGRAPHIE: LIGNE_OROGRAPHIQUE (3 branches)
// ============================================================================

#[test]
fn test_fme07_ligne_orographique_structure() {
    audit_ruleset("LIGNE_OROGRAPHIQUE", 3, &["NATURE"]);
}

#[test]
fn test_fme07_ligne_orographique_branches() {
    audit_branch("LIGNE_OROGRAPHIQUE", &[("NATURE", "Carrière")], "0x10e1a", "2");
    audit_branch("LIGNE_OROGRAPHIQUE", &[("NATURE", "Levée")], "0x10e17", "2");
    audit_branch("LIGNE_OROGRAPHIQUE", &[("NATURE", "Talus")], "0x10e19", "2");
}

// ============================================================================
// FME 08 — TOPONYMIE: 162 branches sur 10 catégories CLASSE
// ============================================================================

#[test]
fn test_fme08_toponymie_structure() {
    audit_ruleset("TOPONYMIE", 162, &["CLASSE", "NATURE"]);
}

#[test]
fn test_fme08_toponymie_classe_categories() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let rs = rules_file
        .rulesets
        .iter()
        .find(|r| r.source_layer == "TOPONYMIE")
        .expect("TOPONYMIE ruleset missing");

    let mut classes: Vec<&str> = rs
        .rules
        .iter()
        .filter_map(|r| r.match_conditions.get("CLASSE").map(|s| s.as_str()))
        .collect();
    classes.sort();
    classes.dedup();

    let expected_classes = [
        "Aérodrome",
        "Cimetière",
        "Construction linéaire",
        "Construction ponctuelle",
        "Construction surfacique",
        "Détail orographique",
        "Forêt publique",
        "Lieu-dit non habité",
        "Zone d'activité ou d'intérêt",
        "Zone d'habitation",
    ];

    assert_eq!(
        classes.len(),
        expected_classes.len(),
        "Expected {} CLASSE categories, found {}: {:?}",
        expected_classes.len(),
        classes.len(),
        classes
    );

    for expected in &expected_classes {
        assert!(
            classes.contains(expected),
            "Missing CLASSE category: {}",
            expected
        );
    }
}

#[test]
fn test_fme08_toponymie_sample_branches() {
    // Aérodrome
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Aérodrome"), ("NATURE", "Aérodrome")],
        "0x02d0b",
        "2",
    );
    // Zone d'habitation — Lieu-dit habité
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Zone d'habitation"), ("NATURE", "Lieu-dit habité")],
        "0x00d00",
        "2",
    );
    // Zone d'habitation — Quartier
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Zone d'habitation"), ("NATURE", "Quartier")],
        "0x11513",
        "2",
    );
    // Détail orographique — Col
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Détail orographique"), ("NATURE", "Col")],
        "0x06601",
        "2",
    );
    // Détail orographique — Sommet
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Détail orographique"), ("NATURE", "Sommet")],
        "0x06616",
        "2",
    );
    // Lieu-dit non habité — Bois
    audit_branch(
        "TOPONYMIE",
        &[("CLASSE", "Lieu-dit non habité"), ("NATURE", "Bois")],
        "0x0660a",
        "2",
    );
}

// ============================================================================
// Global: Total 283 rules across 8 FME projects + courbes de niveau = 100% coverage
// ============================================================================

#[test]
fn test_global_283_rules_total_fme_100_percent() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let total: usize = rules_file.rulesets.iter().map(|rs| rs.rules.len()).sum();
    assert_eq!(
        total, 283,
        "Total rules should be exactly 283 (FME 100% + courbes de niveau), got {}",
        total
    );
}

#[test]
fn test_global_all_8_fme_projects_covered() {
    let rules_file = rules::load_rules(&rules_path()).expect("Failed to load rules");
    let source_layers: Vec<&str> = rules_file
        .rulesets
        .iter()
        .map(|r| r.source_layer.as_str())
        .collect();

    // FME 01 — Transport + Chef-lieu
    let fme01 = [
        "TRONCON_DE_ROUTE",
        "TRONCON_DE_VOIE_FERREE",
        "PISTE_D_AERODROME",
        "TRANSPORT_PAR_CABLE",
        "COMMUNE",
        "ZONE_D_HABITATION",
    ];
    for layer in &fme01 {
        assert!(source_layers.contains(layer), "FME 01 missing: {}", layer);
    }

    // FME 02 — Zones réglementées
    assert!(
        source_layers.contains(&"FORET_PUBLIQUE"),
        "FME 02 missing: FORET_PUBLIQUE"
    );

    // FME 03 — Services
    let fme03 = ["ZONE_D_ACTIVITE_OU_D_INTERET", "LIGNE_ELECTRIQUE"];
    for layer in &fme03 {
        assert!(source_layers.contains(layer), "FME 03 missing: {}", layer);
    }

    // FME 04 — Hydrographie
    let fme04 = [
        "TRONCON_HYDROGRAPHIQUE",
        "SURFACE_HYDROGRAPHIQUE",
        "DETAIL_HYDROGRAPHIQUE",
    ];
    for layer in &fme04 {
        assert!(source_layers.contains(layer), "FME 04 missing: {}", layer);
    }

    // FME 05 — Végétation
    assert!(
        source_layers.contains(&"ZONE_DE_VEGETATION"),
        "FME 05 missing: ZONE_DE_VEGETATION"
    );

    // FME 06 — Bâti
    let fme06 = [
        "BATIMENT",
        "CIMETIERE",
        "CONSTRUCTION_LINEAIRE",
        "CONSTRUCTION_PONCTUELLE",
        "PYLONE",
        "TERRAIN_DE_SPORT",
    ];
    for layer in &fme06 {
        assert!(source_layers.contains(layer), "FME 06 missing: {}", layer);
    }

    // FME 07 — Orographie
    assert!(
        source_layers.contains(&"LIGNE_OROGRAPHIQUE"),
        "FME 07 missing: LIGNE_OROGRAPHIQUE"
    );

    // FME 08 — Toponymie
    assert!(
        source_layers.contains(&"TOPONYMIE"),
        "FME 08 missing: TOPONYMIE"
    );
}
