//! Integration tests for Story 9.5: JSON Report + Rules + SRS end-to-end
//!
//! Tests:
//! - AC1: Report JSON written to disk with complete schema
//! - AC2: Pipeline rules + SRS end-to-end (GeoPackage EPSG:2154 → .mp WGS84)
//! - AC3: Report statistics coherence (matched + ignored == features processed via rules)
//! - AC4: Backward compatibility without rules or SRS

use gdal::spatial_ref::SpatialRef;
use gdal::vector::{FieldDefn, LayerAccess, OGRFieldType};
use gdal::DriverManager;
use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// ============================================================================
// Helpers
// ============================================================================

/// Create minimal BuildArgs for testing, with optional report path.
fn make_args(report: Option<String>) -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    }
}

/// Create a GeoPackage in EPSG:2154 (Lambert 93) with route + poi layers.
/// Route layer: LineString features with NATURE, CL_ADMIN, NUMERO attributes.
/// POI layer: Point features with NATURE attribute.
fn create_bdtopo_gpkg(dir: &Path) -> PathBuf {
    let gpkg_path = dir.join("bdtopo_test.gpkg");
    let driver = DriverManager::get_driver_by_name("GPKG").unwrap();
    let mut ds = driver
        .create_vector_only(gpkg_path.to_str().unwrap())
        .unwrap();

    let mut srs = SpatialRef::from_epsg(2154).unwrap();
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);

    // --- Route layer (LineString) ---
    {
        let layer = ds
            .create_layer(gdal::vector::LayerOptions {
                name: "TRONCON_DE_ROUTE",
                srs: Some(&srs),
                ty: gdal::vector::OGRwkbGeometryType::wkbLineString,
                ..Default::default()
            })
            .unwrap();

        for (name, width) in &[("NATURE", 100), ("CL_ADMIN", 100), ("NUMERO", 20)] {
            let fd = FieldDefn::new(name, OGRFieldType::OFTString).unwrap();
            fd.set_width(*width);
            fd.add_to_layer(&layer).unwrap();
        }

        let defn = layer.defn();

        // Feature 1: Autoroute A6 — should match rule → Type=0x01
        let mut f = gdal::vector::Feature::new(defn).unwrap();
        f.set_field_string(0, "Autoroute").unwrap();
        f.set_field_string(1, "Autoroute").unwrap();
        f.set_field_string(2, "A6").unwrap();
        // Line near Paris in Lambert 93
        let geom = gdal::vector::Geometry::from_wkt(
            "LINESTRING (600000 6800000, 601000 6801000)",
        )
        .unwrap();
        f.set_geometry(geom).unwrap();
        f.create(&layer).unwrap();

        // Feature 2: Départementale — should match rule → Type=0x02
        let mut f2 = gdal::vector::Feature::new(defn).unwrap();
        f2.set_field_string(0, "Route").unwrap();
        f2.set_field_string(1, "Nationale").unwrap();
        f2.set_field_string(2, "N7").unwrap();
        let geom2 = gdal::vector::Geometry::from_wkt(
            "LINESTRING (600500 6800500, 601500 6801500)",
        )
        .unwrap();
        f2.set_geometry(geom2).unwrap();
        f2.create(&layer).unwrap();
    }

    // --- POI layer (Point) ---
    {
        let layer = ds
            .create_layer(gdal::vector::LayerOptions {
                name: "LIEU_DIT",
                srs: Some(&srs),
                ty: gdal::vector::OGRwkbGeometryType::wkbPoint,
                ..Default::default()
            })
            .unwrap();

        let fd = FieldDefn::new("NATURE", OGRFieldType::OFTString).unwrap();
        fd.set_width(100);
        fd.add_to_layer(&layer).unwrap();

        let defn = layer.defn();

        // Feature: Mairie — should match POI rule
        let mut f = gdal::vector::Feature::new(defn).unwrap();
        f.set_field_string(0, "Mairie").unwrap();
        let geom = gdal::vector::Geometry::from_wkt("POINT (600500 6800500)").unwrap();
        f.set_geometry(geom).unwrap();
        f.create(&layer).unwrap();
    }

    gpkg_path
}

/// Create a simple WGS84 GeoPackage with two points (non-degenerate bbox needed for tiling).
fn create_simple_wgs84_gpkg(dir: &Path) -> PathBuf {
    let gpkg_path = dir.join("simple_wgs84.gpkg");
    let driver = DriverManager::get_driver_by_name("GPKG").unwrap();
    let mut ds = driver
        .create_vector_only(gpkg_path.to_str().unwrap())
        .unwrap();

    let mut srs = SpatialRef::from_epsg(4326).unwrap();
    srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);

    let layer = ds
        .create_layer(gdal::vector::LayerOptions {
            name: "test_layer",
            srs: Some(&srs),
            ty: gdal::vector::OGRwkbGeometryType::wkbPoint,
            ..Default::default()
        })
        .unwrap();

    let fd = FieldDefn::new("NAME", OGRFieldType::OFTString).unwrap();
    fd.set_width(100);
    fd.add_to_layer(&layer).unwrap();

    let defn = layer.defn();

    // Point 1: Paris
    let mut f = gdal::vector::Feature::new(defn).unwrap();
    f.set_field_string(0, "Paris").unwrap();
    let geom = gdal::vector::Geometry::from_wkt("POINT (2.35 48.85)").unwrap();
    f.set_geometry(geom).unwrap();
    f.create(&layer).unwrap();

    // Point 2: slightly offset (ensures non-degenerate bbox for tile generation)
    let mut f2 = gdal::vector::Feature::new(defn).unwrap();
    f2.set_field_string(0, "Lyon").unwrap();
    let geom2 = gdal::vector::Geometry::from_wkt("POINT (4.83 45.76)").unwrap();
    f2.set_geometry(geom2).unwrap();
    f2.create(&layer).unwrap();

    gpkg_path
}

/// Create a YAML rules file for end-to-end testing.
/// 2 rulesets: TRONCON_DE_ROUTE (2 rules) + LIEU_DIT (1 rule).
fn create_rules_yaml(dir: &Path) -> PathBuf {
    let rules_path = dir.join("rules.yaml");
    fs::write(
        &rules_path,
        r#"version: 1

rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set:
          Type: "0x01"
          EndLevel: "3"
          Label: "${NUMERO}"
      - match:
          CL_ADMIN: "Nationale"
        set:
          Type: "0x06"
          EndLevel: "1"

  - name: "Lieux-dits"
    source_layer: "LIEU_DIT"
    rules:
      - match:
          NATURE: "Mairie"
        set:
          Type: "0x2F00"
          EndLevel: "2"
          Label: "Mairie"
"#,
    )
    .unwrap();
    rules_path
}

/// Build a Config from YAML string.
fn parse_config(yaml: &str) -> Config {
    serde_yml::from_str(yaml).expect("Failed to parse test config")
}

// ============================================================================
// AC1: Report JSON written to disk (Tasks 1.2, 1.3, 1.4)
// ============================================================================

#[test]
fn test_ac1_report_json_written_to_disk() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_simple_wgs84_gpkg(temp_dir.path());
    let report_path = temp_dir.path().join("rapport.json");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
"#,
        gpkg_path.display(),
        temp_dir.path().join("output").display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(Some(report_path.to_str().unwrap().to_string()));

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // AC1: Report file exists on disk
    assert!(
        report_path.exists(),
        "Report JSON file should exist at: {}",
        report_path.display()
    );

    // AC1: Complete JSON schema verification
    let content = fs::read_to_string(&report_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content)
        .expect("Report should be valid JSON");

    // Required top-level fields
    assert!(json.get("status").is_some(), "Missing 'status' field");
    assert!(json.get("tiles_generated").is_some(), "Missing 'tiles_generated' field");
    assert!(json.get("tiles_failed").is_some(), "Missing 'tiles_failed' field");
    assert!(json.get("tiles_skipped").is_some(), "Missing 'tiles_skipped' field");
    assert!(json.get("features_processed").is_some(), "Missing 'features_processed' field");
    assert!(json.get("duration_seconds").is_some(), "Missing 'duration_seconds' field");
    assert!(json.get("errors").is_some(), "Missing 'errors' field");

    // Verify success
    assert_eq!(json["status"], "success");
    assert!(json["duration_seconds"].as_f64().unwrap() > 0.0);
    assert!(json["errors"].as_array().unwrap().is_empty());
}

#[test]
fn test_ac1_report_schema_with_rules_stats() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_bdtopo_gpkg(temp_dir.path());
    let rules_path = create_rules_yaml(temp_dir.path());
    let report_path = temp_dir.path().join("rapport_rules.json");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{gpkg}"
    layers:
      - "TRONCON_DE_ROUTE"
      - "LIEU_DIT"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
output:
  directory: "{out}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
rules: "{rules}"
"#,
        gpkg = gpkg_path.display(),
        out = temp_dir.path().join("output").display(),
        rules = rules_path.display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(Some(report_path.to_str().unwrap().to_string()));

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let content = fs::read_to_string(&report_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // AC1: rules_stats present when rules are used
    assert!(
        json.get("rules_stats").is_some(),
        "Report should contain rules_stats when rules are configured"
    );

    let stats = &json["rules_stats"];
    assert!(stats.get("matched").is_some(), "Missing rules_stats.matched");
    assert!(stats.get("ignored").is_some(), "Missing rules_stats.ignored");
    assert!(stats.get("errors").is_some(), "Missing rules_stats.errors");
    assert!(stats.get("by_ruleset").is_some(), "Missing rules_stats.by_ruleset");
}

#[test]
fn test_ac1_no_report_when_not_requested() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_simple_wgs84_gpkg(temp_dir.path());

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
"#,
        gpkg_path.display(),
        temp_dir.path().join("output").display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(None); // No --report

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // AC1 (negative): No report file should exist
    let json_files: Vec<_> = fs::read_dir(temp_dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();

    assert!(
        json_files.is_empty(),
        "No JSON report files should exist when --report is not specified"
    );
}

// ============================================================================
// AC2: Pipeline rules + SRS end-to-end
// ============================================================================

#[test]
fn test_ac2_pipeline_rules_srs_end_to_end() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_bdtopo_gpkg(temp_dir.path());
    let rules_path = create_rules_yaml(temp_dir.path());
    let output_dir = temp_dir.path().join("output");
    let report_path = temp_dir.path().join("rapport.json");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{gpkg}"
    layers:
      - "TRONCON_DE_ROUTE"
      - "LIEU_DIT"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
output:
  directory: "{out}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
rules: "{rules}"
"#,
        gpkg = gpkg_path.display(),
        out = output_dir.display(),
        rules = rules_path.display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(Some(report_path.to_str().unwrap().to_string()));

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Find generated .mp files
    let mp_files: Vec<PathBuf> = fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mp"))
        .collect();

    assert!(
        !mp_files.is_empty(),
        "At least one .mp file should be generated"
    );

    // Read all .mp content concatenated
    let all_mp_content: String = mp_files
        .iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect::<Vec<_>>()
        .join("\n");

    // AC2: Verify Garmin attributes from rules transformation
    // Route autoroute → Type=0x01, EndLevel=3
    assert!(
        all_mp_content.contains("Type=0x01"),
        "Should contain Type=0x01 (Autoroute rule)"
    );
    assert!(
        all_mp_content.contains("EndLevel=3"),
        "Should contain EndLevel=3 (Autoroute rule)"
    );

    // Route nationale → Type=0x06, EndLevel=1
    assert!(
        all_mp_content.contains("Type=0x06"),
        "Should contain Type=0x06 (Nationale rule)"
    );

    // POI Mairie → Type=0x2F00, EndLevel=2
    assert!(
        all_mp_content.contains("Type=0x2F00"),
        "Should contain Type=0x2F00 (Mairie POI rule)"
    );

    // AC2: Verify Label attributes from rules transformation
    // Autoroute rule: Label="${NUMERO}" → should produce Label=A6
    assert!(
        all_mp_content.contains("Label=A6"),
        "Should contain Label=A6 (Autoroute rule with ${{NUMERO}} substitution)"
    );
    // POI Mairie rule: Label="Mairie" → static label
    assert!(
        all_mp_content.contains("Label=Mairie"),
        "Should contain Label=Mairie (POI rule static label)"
    );

    // AC2: Verify WGS84 coordinates (Lambert 93 ~600000,6800000 → WGS84 ~2.35,48.85)
    // Parse Data0 lines and check coordinate ranges
    for line in all_mp_content.lines() {
        if line.starts_with("Data0=") {
            // Extract coordinate pairs from Data0 line
            // Format: Data0=(lat,lon),(lat,lon),...
            let coords_str = line.trim_start_matches("Data0=");
            for pair in coords_str.split("),(") {
                let pair = pair.trim_matches(|c| c == '(' || c == ')');
                let parts: Vec<&str> = pair.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(lat), Ok(lon)) = (
                        parts[0].trim().parse::<f64>(),
                        parts[1].trim().parse::<f64>(),
                    ) {
                        // France WGS84 bounds: lon ∈ [-5, 10], lat ∈ [41, 52]
                        assert!(
                            lon > -5.0 && lon < 10.0,
                            "Longitude should be in France WGS84 range [-5,10], got: {}",
                            lon
                        );
                        assert!(
                            lat > 41.0 && lat < 52.0,
                            "Latitude should be in France WGS84 range [41,52], got: {}",
                            lat
                        );
                    }
                }
            }
        }
    }
}

// ============================================================================
// AC3: Report statistics coherence
// ============================================================================

#[test]
fn test_ac3_report_statistics_coherence() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_bdtopo_gpkg(temp_dir.path());
    let rules_path = create_rules_yaml(temp_dir.path());
    let report_path = temp_dir.path().join("rapport.json");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{gpkg}"
    layers:
      - "TRONCON_DE_ROUTE"
      - "LIEU_DIT"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
output:
  directory: "{out}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
rules: "{rules}"
"#,
        gpkg = gpkg_path.display(),
        out = temp_dir.path().join("output").display(),
        rules = rules_path.display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(Some(report_path.to_str().unwrap().to_string()));

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let content = fs::read_to_string(&report_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let stats = &json["rules_stats"];
    let matched = stats["matched"].as_u64().unwrap();
    let ignored = stats["ignored"].as_u64().unwrap();
    let errors = stats["errors"].as_u64().unwrap();

    // AC3: matched + ignored == total features processed by rules engine
    // All 3 features have rulesets (TRONCON_DE_ROUTE + LIEU_DIT), so
    // matched + ignored must equal features_processed
    let features_processed = json["features_processed"].as_u64().unwrap();
    assert_eq!(
        matched + ignored,
        features_processed,
        "AC3 invariant: matched ({}) + ignored ({}) must equal features_processed ({})",
        matched, ignored, features_processed
    );
    assert!(
        matched > 0,
        "At least some features should match rules"
    );
    assert_eq!(errors, 0, "No rule evaluation errors expected");

    // Verify per-ruleset stats present
    let by_ruleset = stats["by_ruleset"].as_object().unwrap();
    assert!(
        by_ruleset.contains_key("TRONCON_DE_ROUTE"),
        "Should have stats for TRONCON_DE_ROUTE ruleset"
    );
    assert!(
        by_ruleset.contains_key("LIEU_DIT"),
        "Should have stats for LIEU_DIT ruleset"
    );

    // Per-ruleset coherence
    let route_stats = &by_ruleset["TRONCON_DE_ROUTE"];
    let route_matched = route_stats["matched"].as_u64().unwrap();
    let route_ignored = route_stats["ignored"].as_u64().unwrap();
    let poi_stats = &by_ruleset["LIEU_DIT"];
    let poi_matched = poi_stats["matched"].as_u64().unwrap();
    let poi_ignored = poi_stats["ignored"].as_u64().unwrap();

    // Sum of per-ruleset matched + ignored should equal global matched + ignored
    assert_eq!(
        route_matched + route_ignored + poi_matched + poi_ignored,
        matched + ignored,
        "Per-ruleset totals should sum to global totals"
    );
}

// ============================================================================
// AC4: Backward compatibility without rules or SRS
// ============================================================================

#[test]
fn test_ac4_backward_compat_no_rules_no_srs() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_simple_wgs84_gpkg(temp_dir.path());
    let report_path = temp_dir.path().join("rapport.json");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
"#,
        gpkg_path.display(),
        temp_dir.path().join("output").display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(Some(report_path.to_str().unwrap().to_string()));

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    let content = fs::read_to_string(&report_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    // AC4: No rules_stats in report when rules are not configured
    assert!(
        json.get("rules_stats").is_none(),
        "Report should NOT contain rules_stats when no rules configured (skip_serializing_if)"
    );

    // AC4: Standard fields still present
    assert_eq!(json["status"], "success");
    assert!(json["features_processed"].as_u64().unwrap() > 0);
}

#[test]
fn test_ac4_coordinates_unchanged_without_srs() {
    let temp_dir = TempDir::new().unwrap();
    let gpkg_path = create_simple_wgs84_gpkg(temp_dir.path());
    let output_dir = temp_dir.path().join("output");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 10.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{col}}_{{row}}.mp"
error_handling: "continue"
"#,
        gpkg_path.display(),
        output_dir.display()
    );

    let config = parse_config(&config_yaml);
    let args = make_args(None);

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed: {:?}", result.err());

    // Read .mp files
    let mp_files: Vec<PathBuf> = fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mp"))
        .collect();

    assert!(!mp_files.is_empty(), "Should have .mp output");

    let all_content: String = mp_files
        .iter()
        .map(|p| fs::read_to_string(p).unwrap())
        .collect::<Vec<_>>()
        .join("\n");

    // AC4: Coordinates should be in France WGS84 range (no reprojection applied)
    // Points: Paris (2.35, 48.85) and Lyon (4.83, 45.76)
    let mut found_data0 = false;
    for line in all_content.lines() {
        if line.starts_with("Data0=") {
            found_data0 = true;
            let coords_str = line.trim_start_matches("Data0=");
            for pair in coords_str.split("),(") {
                let pair = pair.trim_matches(|c| c == '(' || c == ')');
                let parts: Vec<&str> = pair.split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(lat), Ok(lon)) = (
                        parts[0].trim().parse::<f64>(),
                        parts[1].trim().parse::<f64>(),
                    ) {
                        // France WGS84: lon ∈ [-5, 10], lat ∈ [41, 52]
                        assert!(
                            lon > -5.0 && lon < 10.0,
                            "Longitude should be in France WGS84 range, got: {}",
                            lon
                        );
                        assert!(
                            lat > 41.0 && lat < 52.0,
                            "Latitude should be in France WGS84 range, got: {}",
                            lat
                        );
                    }
                }
            }
        }
    }
    assert!(found_data0, "Should find at least one Data0 line in .mp output");
}
