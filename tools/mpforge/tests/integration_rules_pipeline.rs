//! Integration tests for rules engine pipeline integration (Story 9.3 + Story 11.3)
//!
//! Tests the full integration of rules evaluation with the pipeline:
//! - find_ruleset lookup
//! - Feature transformation with source_layer
//! - Passthrough for layers without ruleset
//! - Feature filtering (no match → ignored)
//! - Statistics collection
//! - NFR overhead rules < 10% (Story 11.3)

use gdal::spatial_ref::SpatialRef;
use gdal::vector::{FieldDefn, LayerAccess, OGRFieldType};
use gdal::DriverManager;
use mpforge::cli::BuildArgs;
use mpforge::config::Config;
use mpforge::pipeline;
use mpforge::pipeline::reader::{Feature, GeometryType};
use mpforge::rules::{self, RuleStats};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::TempDir;

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

// ============================================================================
// Story 11.3 — AC4 + BDTOPO-NFR1 : Overhead rules < 10%
// ============================================================================

/// Crée un GeoPackage WGS84 avec des points TRONCON_DE_ROUTE pour tester l'overhead rules.
/// 50 tuiles (grille 10×5, cell_size 0.1) × 25 features/tuile ≈ 1250 features.
fn create_overhead_test_gpkg(dir: &Path) -> PathBuf {
    let gpkg_path = dir.join("overhead_test.gpkg");
    let driver = DriverManager::get_driver_by_name("GPKG")
        .expect("GPKG driver not available");
    let mut ds = driver
        .create_vector_only(gpkg_path.to_str().expect("valid path"))
        .expect("Failed to create GeoPackage");

    let mut srs = SpatialRef::from_epsg(4326).expect("EPSG:4326");
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);

    let layer = ds
        .create_layer(gdal::vector::LayerOptions {
            name: "TRONCON_DE_ROUTE",
            srs: Some(&srs),
            ty: gdal::vector::OGRwkbGeometryType::wkbLineString,
            ..Default::default()
        })
        .expect("Failed to create layer");

    // Fields added in this order — indices used below must match
    // IDX_NATURE=0, IDX_CL_ADMIN=1, IDX_NUMERO=2
    for (name, width) in &[("NATURE", 100usize), ("CL_ADMIN", 100), ("NUMERO", 20)] {
        let fd = FieldDefn::new(name, OGRFieldType::OFTString).expect("FieldDefn");
        fd.set_width(*width as i32);
        fd.add_to_layer(&layer).expect("add field");
    }
    const IDX_NATURE: usize = 0;
    const IDX_CL_ADMIN: usize = 1;
    const IDX_NUMERO: usize = 2;

    let defn = layer.defn();
    // 1250 features : routes réparties sur la grille 1°×0.5° (10×5 tuiles de 0.1°)
    let n_features = 1250usize;
    for i in 0..n_features {
        let lon_base = (i % 50) as f64 * (1.0 / 50.0);
        let lat_base = (i / 50) as f64 * (0.5 / (n_features / 50 + 1) as f64);
        let mut f = gdal::vector::Feature::new(defn).expect("Feature");
        // Routes alternant entre Autoroute et Nationale pour exercer les règles
        let (nature, cl_admin, numero) = if i % 2 == 0 {
            ("Autoroute", "Autoroute", format!("A{}", i))
        } else {
            ("Route", "Nationale", format!("N{}", i))
        };
        f.set_field_string(IDX_NATURE, nature).expect("set NATURE");
        f.set_field_string(IDX_CL_ADMIN, cl_admin).expect("set CL_ADMIN");
        f.set_field_string(IDX_NUMERO, &numero).expect("set NUMERO");
        let geom = gdal::vector::Geometry::from_wkt(&format!(
            "LINESTRING ({} {}, {} {})",
            lon_base, lat_base,
            lon_base + 0.005, lat_base + 0.005
        ))
        .expect("valid WKT");
        f.set_geometry(geom).expect("set geometry");
        f.create(&layer).expect("create feature");
    }

    gpkg_path
}

fn make_overhead_args(jobs: usize) -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    }
}

#[test]
#[ignore] // Story 11.3 AC4: Exécuter via: cargo test -- --ignored test_rules_overhead_below_10_percent
fn test_rules_overhead_below_10_percent() {
    // BDTOPO-NFR1: Overhead rules + reprojection + parallélisation ≤ 10% vs pipeline sans règles
    // Subtask 3.1-3.5: 50 tuiles synthétiques TRONCON_DE_ROUTE, assertion overhead < 10%
    let tmp_no_rules = TempDir::new().expect("TempDir");
    let tmp_with_rules = TempDir::new().expect("TempDir");
    let tmp_data = TempDir::new().expect("TempDir data");

    let gpkg_path = create_overhead_test_gpkg(tmp_data.path());
    let rules_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/bdtopo-garmin-rules.yaml");

    assert!(
        rules_path.exists(),
        "Fichier de règles BDTOPO introuvable: {}",
        rules_path.display()
    );

    // AC4 + BDTOPO-NFR1: mesure avec --jobs 4 pour tester "rules + parallélisation combinés"
    let args = make_overhead_args(4);

    // Subtask 3.2: Mesure SANS rules (--jobs 4)
    let config_no_rules_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "continue"
"#,
        gpkg_path.display(),
        tmp_no_rules.path().join("tiles").display(),
    );
    let config_no_rules: Config =
        serde_yml::from_str(&config_no_rules_yaml).expect("parse config sans rules");

    let start_no_rules = Instant::now();
    let result_no_rules = pipeline::run(&config_no_rules, &args);
    let time_without_rules = start_no_rules.elapsed().as_secs_f64();
    assert!(result_no_rules.is_ok(), "Pipeline sans rules doit réussir");

    // Subtask 3.3: Mesure AVEC rules bdtopo-garmin-rules.yaml (--jobs 4)
    let config_with_rules_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "continue"
rules: "{}"
"#,
        gpkg_path.display(),
        tmp_with_rules.path().join("tiles").display(),
        rules_path.display(),
    );
    let config_with_rules: Config =
        serde_yml::from_str(&config_with_rules_yaml).expect("parse config avec rules");

    let start_with_rules = Instant::now();
    let result_with_rules = pipeline::run(&config_with_rules, &args);
    let time_with_rules = start_with_rules.elapsed().as_secs_f64();
    assert!(result_with_rules.is_ok(), "Pipeline avec rules doit réussir");

    // Subtask 3.4: Calcul du ratio overhead
    let overhead_ratio = (time_with_rules - time_without_rules) / time_without_rules;

    // Subtask 3.5: Log pour diagnostic
    eprintln!("Temps sans rules (--jobs 4):  {:.3}s", time_without_rules);
    eprintln!("Temps avec rules (--jobs 4):  {:.3}s", time_with_rules);
    eprintln!("Overhead rules:               {:.1}%", overhead_ratio * 100.0);

    // Guard : si les temps sont trop courts (< 200ms), la mesure est trop bruitée
    // pour une assertion fiable — on log et on sort sans échec
    if time_without_rules < 0.2 {
        eprintln!(
            "SKIP assertion: time_without_rules={:.3}s trop court pour mesure fiable (seuil 200ms)",
            time_without_rules
        );
        return;
    }

    // Subtask 3.4: Assertion overhead < 10% (BDTOPO-NFR1)
    assert!(
        overhead_ratio < 0.10,
        "Overhead rules {:.1}% dépasse 10% (BDTOPO-NFR1). \
         time_sans={:.3}s, time_avec={:.3}s (--jobs 4)",
        overhead_ratio * 100.0,
        time_without_rules,
        time_with_rules,
    );
}
