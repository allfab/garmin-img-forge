//! Tech-spec #2 Task 12 — end-to-end integration test for the multi-Data
//! writer. Generates a synthetic shapefile with a single LineString feature
//! (acting as a TRONCON_DE_ROUTE with `CL_ADMIN=Autoroute`), runs mpforge with
//! an inline multi-level generalize profile, and asserts that the produced
//! `.mp` contains both `Data0=` and `Data2=` lines for the feature.
//!
//! Maps to AC3 and AC10 (mpforge leg). The imgforge consumption leg is
//! already covered by Tech-spec #1 `multi_data_e2e` and is out of scope here
//! because running imgforge in this test crate would couple two distinct
//! build pipelines.

use gdal::vector::{
    FieldDefn, LayerAccess, LayerOptions, OGRFieldType, OGRwkbGeometryType, ToGdal,
};
use gdal::{Dataset, DriverManager};
use geo::{Coord, LineString};
use mpforge::cli::BuildArgs;
use mpforge::{config, pipeline};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Produce a shapefile with a single LineString feature carrying the
/// `CL_ADMIN=Autoroute` attribute. The zigzag shape (10 points on a ~10km
/// diagonal) exercises Douglas-Peucker at a tolerance that is low enough to
/// leave most vertices intact at `n=0` and high enough to drop roughly half
/// of them at `n=2`.
fn write_highway_shapefile(dir: &Path) -> PathBuf {
    let shp_path = dir.join("TRONCON_DE_ROUTE.shp");
    let driver = DriverManager::get_driver_by_name("ESRI Shapefile")
        .expect("ESRI Shapefile driver");
    let mut ds: Dataset = driver
        .create_vector_only(&shp_path)
        .expect("create SHP");

    let mut layer = ds
        .create_layer(LayerOptions {
            name: "TRONCON_DE_ROUTE",
            srs: None,
            ty: OGRwkbGeometryType::wkbLineString,
            options: None,
        })
        .expect("create layer");

    let fd = FieldDefn::new("CL_ADMIN", OGRFieldType::OFTString).expect("field defn");
    fd.add_to_layer(&layer).expect("add field");

    let coords: Vec<Coord<f64>> = (0..10)
        .map(|i| Coord {
            x: 2.0 + i as f64 * 0.01,
            y: 48.0 + (i as f64 * 0.01 * 1.3).sin() * 0.002,
        })
        .collect();
    let line = LineString::new(coords);
    let geom = line.to_gdal().expect("geo → gdal");
    let cl_admin_idx = layer
        .defn()
        .field_index("CL_ADMIN")
        .expect("CL_ADMIN idx");
    let defn = layer.defn().clone();
    let mut feat = gdal::vector::Feature::new(&defn).expect("new feature");
    feat.set_geometry(geom).expect("set geom");
    feat.set_field_string(cl_admin_idx, "Autoroute")
        .expect("set field");
    feat.create(&layer).expect("create feature");
    drop(feat);
    drop(layer);
    drop(ds);

    shp_path
}

fn write_sources_yaml(tmp: &TempDir, shp: &Path, tiles_dir: &Path) -> PathBuf {
    // Inline `generalize:` on the layer_alias TRONCON_DE_ROUTE with a single
    // default `levels: [{ n: 0 }]` path is insufficient — we need multi-level.
    // To exercise the external catalog path we write a sibling
    // `generalize-profiles.yaml` and reference it from the sources.yaml.
    let profiles_yaml = tmp.path().join("generalize-profiles.yaml");
    fs::write(
        &profiles_yaml,
        r#"profiles:
  TRONCON_DE_ROUTE:
    levels:
      - { n: 0, simplify: 0.00002 }
      - { n: 2, simplify: 0.00015 }
"#,
    )
    .unwrap();

    let sources = format!(
        r#"version: 1
grid:
  cell_size: 0.5
  overlap: 0.0
generalize_profiles_path: "generalize-profiles.yaml"
inputs:
  - path: "{shp}"
    layer_alias: "TRONCON_DE_ROUTE"
output:
  directory: "{tiles}"
  filename_pattern: "tile_{{col}}_{{row}}.mp"
"#,
        shp = shp.display(),
        tiles = tiles_dir.display(),
    );
    let path = tmp.path().join("sources.yaml");
    fs::write(&path, sources).unwrap();
    path
}

fn build_args(cfg_path: &Path) -> BuildArgs {
    BuildArgs {
        config: cfg_path.to_string_lossy().to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: true,
        report: None,
        skip_existing: false,
        dry_run: false,
        disable_profiles: false,
        verbose: 0,
    }
}

/// Point GDAL to the freshly-built ogr-polishmap plugin sibling to this
/// crate. M8 code review : panic explicite si le plugin frais manque, pour
/// éviter que le test passe silencieusement sur un plugin système stale (qui
/// ignore `MULTI_GEOM_FIELDS`/`MAX_DATA_LEVEL` et rendrait l'assertion sur
/// `Data2=` invalide).
fn prefer_local_polishmap_plugin() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let build_dir = PathBuf::from(crate_dir).join("../ogr-polishmap/build");
    let so = build_dir.join("ogr_PolishMap.so");
    if !so.exists() {
        panic!(
            "fresh ogr-polishmap plugin not found at {}.\n\
             Run `cmake --build tools/ogr-polishmap/build --target ogr_PolishMap` first.\n\
             Without it the test would exercise the stale system plugin that \
             ignores MULTI_GEOM_FIELDS — AC10 assertion would be invalid.",
            so.display()
        );
    }
    std::env::set_var("GDAL_DRIVER_PATH", build_dir);
}

#[test]
fn test_mpforge_emits_data0_and_data2_for_autoroute() {
    prefer_local_polishmap_plugin();
    // Setup
    let tmp = TempDir::new().unwrap();
    let tiles = tmp.path().join("tiles");
    fs::create_dir_all(&tiles).unwrap();
    let shp = write_highway_shapefile(tmp.path());
    let cfg_path = write_sources_yaml(&tmp, &shp, &tiles);

    // Load + run pipeline
    let cfg = config::load_config(&cfg_path).expect("load_config");
    // Sanity : le profil externe a bien été chargé et propagé.
    assert!(
        cfg.profile_map().contains_key("TRONCON_DE_ROUTE"),
        "profile catalog not resolved"
    );
    assert_eq!(
        cfg.profile_map()["TRONCON_DE_ROUTE"].levels.len(),
        2,
        "profile should declare 2 levels"
    );

    let args = build_args(&cfg_path);
    pipeline::run(&cfg, &args).expect("pipeline run");

    // Collect produced .mp files
    let mps: Vec<PathBuf> = fs::read_dir(&tiles)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("mp"))
        .collect();
    assert!(!mps.is_empty(), "no .mp produced under {}", tiles.display());

    // Parse text content — expect at least one tile containing both Data0=
    // and Data2= for the single TRONCON_DE_ROUTE feature.
    let mut seen_data0 = false;
    let mut seen_data2 = false;
    for mp in &mps {
        let body = fs::read_to_string(mp).unwrap();
        if body.contains("\nData0=") || body.starts_with("Data0=") {
            seen_data0 = true;
        }
        if body.contains("\nData2=") || body.starts_with("Data2=") {
            seen_data2 = true;
        }
    }
    assert!(seen_data0, ".mp should contain a Data0= line");
    assert!(
        seen_data2,
        ".mp should contain a Data2= line (multi-Data writer activated)"
    );
}
