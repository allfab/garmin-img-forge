//! Tech-spec overview wide-zoom — tests d'intégration mpforge pour l'opt-in
//! `overview_levels`. Couvre :
//!   - AC1 : compat descendante (sans `overview_levels:` → header 7L standard,
//!     pas de `Level7/8/9`, pas de `Data7/8/9`).
//!   - AC6/7/8/9/10/11 : avec `overview_levels:` activé, le header émis est
//!     `Levels=10`, les features promues reçoivent `Data7/8/9` contigus, les
//!     features d'une branche non promue (Communale) s'arrêtent à `Data6`,
//!     les couches absentes du catalogue (BATIMENT) ne reçoivent que `Data0`.
//!
//! Le byte-identité strict contre un golden committé n'est pas testé ici
//! (couverture via les autres tests d'intégration qui comparent produit/
//! baseline). Ce test se concentre sur les garanties comportementales
//! observables directement sur le `.mp` émis.

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

/// Shapefile TRONCON_DE_ROUTE avec 2 features : une Autoroute (matchée par
/// `promote_to: 9`) et une Communale (non matchée). Les deux portent
/// `CL_ADMIN` pour permettre au dispatch `when[]` du profil de router chaque
/// feature sur la branche adéquate.
fn write_routes_shapefile(dir: &Path) -> PathBuf {
    let shp_path = dir.join("TRONCON_DE_ROUTE.shp");
    let driver = DriverManager::get_driver_by_name("ESRI Shapefile").unwrap();
    let mut ds: Dataset = driver.create_vector_only(&shp_path).unwrap();
    let layer = ds
        .create_layer(LayerOptions {
            name: "TRONCON_DE_ROUTE",
            srs: None,
            ty: OGRwkbGeometryType::wkbLineString,
            options: None,
        })
        .unwrap();
    FieldDefn::new("CL_ADMIN", OGRFieldType::OFTString)
        .unwrap()
        .add_to_layer(&layer)
        .unwrap();
    let cl_idx = layer.defn().field_index("CL_ADMIN").unwrap();
    let defn = layer.defn().clone();

    for (offset_y, cl_admin) in [(0.0, "Autoroute"), (0.01, "Communale")] {
        let coords: Vec<Coord<f64>> = (0..15)
            .map(|i| Coord {
                x: 2.0 + i as f64 * 0.005,
                y: 48.0 + offset_y + (i as f64 * 0.01).sin() * 0.001,
            })
            .collect();
        let geom = LineString::new(coords).to_gdal().unwrap();
        let mut f = gdal::vector::Feature::new(&defn).unwrap();
        f.set_geometry(geom).unwrap();
        f.set_field_string(cl_idx, cl_admin).unwrap();
        f.create(&layer).unwrap();
    }
    drop(layer);
    drop(ds);
    shp_path
}

/// Shapefile BATIMENT minimal — sert à vérifier qu'une couche ABSENTE du
/// catalogue overview reste mono-Data (AC7).
fn write_bati_shapefile(dir: &Path) -> PathBuf {
    let shp_path = dir.join("BATIMENT.shp");
    let driver = DriverManager::get_driver_by_name("ESRI Shapefile").unwrap();
    let mut ds: Dataset = driver.create_vector_only(&shp_path).unwrap();
    {
        let layer = ds
            .create_layer(LayerOptions {
                name: "BATIMENT",
                srs: None,
                ty: OGRwkbGeometryType::wkbPolygon,
                options: None,
            })
            .unwrap();
        let defn = layer.defn().clone();
        use geo::Polygon;
        let poly = Polygon::new(
            LineString::new(vec![
                Coord { x: 2.05, y: 48.02 },
                Coord { x: 2.06, y: 48.02 },
                Coord { x: 2.06, y: 48.03 },
                Coord { x: 2.05, y: 48.03 },
                Coord { x: 2.05, y: 48.02 },
            ]),
            vec![],
        );
        let geom = poly.to_gdal().unwrap();
        let mut f = gdal::vector::Feature::new(&defn).unwrap();
        f.set_geometry(geom).unwrap();
        f.create(&layer).unwrap();
    }
    drop(ds);
    shp_path
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

fn prefer_local_polishmap_plugin() {
    let crate_dir = env!("CARGO_MANIFEST_DIR");
    let build_dir = PathBuf::from(crate_dir).join("../ogr-polishmap/build");
    let so = build_dir.join("ogr_PolishMap.so");
    if !so.exists() {
        panic!(
            "fresh ogr-polishmap plugin not found at {}.\n\
             Run `cmake --build tools/ogr-polishmap/build --target ogr_PolishMap` first.",
            so.display()
        );
    }
    std::env::set_var("GDAL_DRIVER_PATH", build_dir);
}

fn collect_mp_bodies(tiles_dir: &Path) -> Vec<String> {
    fs::read_dir(tiles_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("mp"))
        .map(|p| fs::read_to_string(&p).unwrap_or_default())
        .collect()
}

/// Header standard 7L partagé (sans opt-in overview).
fn standard_header_7l() -> &'static str {
    r#"header:
  levels: "7"
  level0: "24"
  level1: "23"
  level2: "22"
  level3: "21"
  level4: "20"
  level5: "18"
  level6: "16"
"#
}

/// Profil 7L minimal pour TRONCON_DE_ROUTE — sans n=7..9. Utilisé dans le
/// scénario OFF pour garantir la compat descendante.
fn write_profiles_7l(dir: &Path) -> PathBuf {
    let p = dir.join("generalize-profiles.yaml");
    fs::write(
        &p,
        r#"profiles:
  TRONCON_DE_ROUTE:
    levels:
      - { n: 0, simplify: 0.00001 }
      - { n: 1, simplify: 0.00002 }
      - { n: 2, simplify: 0.00003 }
      - { n: 3, simplify: 0.00004 }
      - { n: 4, simplify: 0.00005 }
      - { n: 5, simplify: 0.00008 }
      - { n: 6, simplify: 0.00013 }
"#,
    )
    .unwrap();
    p
}

/// Profil 10L couvrant les 3 couches démo avec `when[]` gatant n=7..9 aux
/// seules branches promues (Autoroute/Nationale/Départementale).
fn write_profiles_10l(dir: &Path) -> PathBuf {
    let p = dir.join("generalize-profiles.yaml");
    fs::write(
        &p,
        r#"profiles:
  TRONCON_DE_ROUTE:
    when:
      - field: CL_ADMIN
        values: [Autoroute, Nationale]
        levels:
          - { n: 0, simplify: 0.00001 }
          - { n: 1, simplify: 0.00002 }
          - { n: 2, simplify: 0.00003 }
          - { n: 3, simplify: 0.00004 }
          - { n: 4, simplify: 0.00005 }
          - { n: 5, simplify: 0.00008 }
          - { n: 6, simplify: 0.00013 }
          - { n: 7, simplify: 0.0005 }
          - { n: 8, simplify: 0.0015 }
          - { n: 9, simplify: 0.005 }
      - field: CL_ADMIN
        values: [Communale]
        levels:
          - { n: 0, simplify: 0.00005 }
          - { n: 1, simplify: 0.00006 }
          - { n: 2, simplify: 0.00007 }
          - { n: 3, simplify: 0.00009 }
          - { n: 4, simplify: 0.00011 }
          - { n: 5, simplify: 0.00015 }
          - { n: 6, simplify: 0.00020 }
    levels:
      - { n: 0, simplify: 0.00005 }
"#,
    )
    .unwrap();
    p
}

/// AC1 — déterminisme / byte-identité. On lance deux fois le pipeline sur la
/// même fixture OFF et on vérifie que chaque `.mp` produit est strictement
/// identique entre les deux runs. C'est un proxy défensif du golden : si le
/// code introduit une régression non-déterministe ou change silencieusement
/// la sortie, ce test échoue.
#[test]
fn overview_off_is_deterministic_across_runs() {
    prefer_local_polishmap_plugin();

    fn run_once(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        let tiles = root.join("tiles");
        fs::create_dir_all(&tiles).unwrap();
        let shp = write_routes_shapefile(root);
        let _ = write_profiles_7l(root);
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
{header}
"#,
            shp = shp.display(),
            tiles = tiles.display(),
            header = standard_header_7l()
        );
        let cfg_path = root.join("sources.yaml");
        fs::write(&cfg_path, sources).unwrap();
        let cfg = config::load_config(&cfg_path).unwrap();
        pipeline::run(&cfg, &build_args(&cfg_path)).unwrap();

        let mut out = std::collections::BTreeMap::new();
        for entry in fs::read_dir(&tiles).unwrap().flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mp") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                out.insert(name, fs::read(&path).unwrap());
            }
        }
        out
    }

    let tmp1 = TempDir::new().unwrap();
    let tmp2 = TempDir::new().unwrap();
    let run1 = run_once(tmp1.path());
    let run2 = run_once(tmp2.path());
    assert_eq!(
        run1.keys().collect::<Vec<_>>(),
        run2.keys().collect::<Vec<_>>(),
        "OFF runs produced different tile sets"
    );
    for (name, bytes1) in &run1 {
        let bytes2 = run2.get(name).expect("tile missing in run2");
        assert_eq!(
            bytes1, bytes2,
            "byte-identity violated for {name} between two OFF runs"
        );
    }
}

#[test]
fn overview_off_produces_standard_7l_header() {
    prefer_local_polishmap_plugin();
    let tmp = TempDir::new().unwrap();
    let tiles = tmp.path().join("tiles");
    fs::create_dir_all(&tiles).unwrap();

    let shp = write_routes_shapefile(tmp.path());
    let _ = write_profiles_7l(tmp.path());

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
{header}
"#,
        shp = shp.display(),
        tiles = tiles.display(),
        header = standard_header_7l()
    );
    let cfg_path = tmp.path().join("sources.yaml");
    fs::write(&cfg_path, sources).unwrap();

    let cfg = config::load_config(&cfg_path).expect("load_config (OFF)");
    assert!(
        cfg.overview_levels.is_none(),
        "OFF scenario must leave overview_levels unset"
    );
    pipeline::run(&cfg, &build_args(&cfg_path)).expect("pipeline run");

    let bodies = collect_mp_bodies(&tiles);
    assert!(!bodies.is_empty(), "no .mp produced");
    for body in &bodies {
        assert!(
            body.contains("Levels=7"),
            "OFF: header should declare Levels=7, got:\n{body}"
        );
        for banned in ["Level7=", "Level8=", "Level9=", "Data7=", "Data8=", "Data9="] {
            assert!(
                !body.contains(banned),
                "OFF: banned token '{banned}' found in .mp:\n{body}"
            );
        }
    }
}

#[test]
fn overview_on_emits_data7_to_data9_for_promoted_features() {
    prefer_local_polishmap_plugin();
    let tmp = TempDir::new().unwrap();
    let tiles = tmp.path().join("tiles");
    fs::create_dir_all(&tiles).unwrap();

    let routes_shp = write_routes_shapefile(tmp.path());
    let bati_shp = write_bati_shapefile(tmp.path());
    let _ = write_profiles_10l(tmp.path());

    let sources = format!(
        r#"version: 1
grid:
  cell_size: 0.5
  overlap: 0.0
generalize_profiles_path: "generalize-profiles.yaml"
inputs:
  - path: "{routes}"
    layer_alias: "TRONCON_DE_ROUTE"
  - path: "{bati}"
    layer_alias: "BATIMENT"
output:
  directory: "{tiles}"
  filename_pattern: "tile_{{col}}_{{row}}.mp"
{header}
overview_levels:
  header_extension: [14, 12, 10]
  promotion:
    TRONCON_DE_ROUTE:
      - match: {{ CL_ADMIN: [Autoroute, Nationale] }}
        promote_to: 9
"#,
        routes = routes_shp.display(),
        bati = bati_shp.display(),
        tiles = tiles.display(),
        header = standard_header_7l()
    );
    let cfg_path = tmp.path().join("sources.yaml");
    fs::write(&cfg_path, sources).unwrap();

    let cfg = config::load_config(&cfg_path).expect("load_config (ON)");
    assert!(cfg.overview_levels.is_some(), "ON: overview_levels must be set");
    assert_eq!(
        cfg.header.as_ref().and_then(|h| h.levels.as_deref()),
        Some("10"),
        "header.levels must be rewritten to '10' by prepare_overview_levels"
    );
    assert_eq!(
        cfg.header.as_ref().and_then(|h| h.level7.as_deref()),
        Some("14")
    );
    assert_eq!(
        cfg.header.as_ref().and_then(|h| h.level9.as_deref()),
        Some("10")
    );

    pipeline::run(&cfg, &build_args(&cfg_path)).expect("pipeline run");

    let bodies = collect_mp_bodies(&tiles);
    assert!(!bodies.is_empty(), "no .mp produced");

    // Full concatenated body for global header assertions (header lives in
    // exactly one tile when there's a single tile, but we scan all tiles to
    // be robust to grid variations).
    let all_body: String = bodies.join("\n");

    // AC10 : header étendu correctement émis.
    assert!(all_body.contains("Levels=10"), "expected Levels=10 in .mp");
    assert!(all_body.contains("Level7=14"), "expected Level7=14");
    assert!(all_body.contains("Level8=12"), "expected Level8=12");
    assert!(all_body.contains("Level9=10"), "expected Level9=10");

    // AC8/9 : on identifie les blocs POLYLINE par leur profondeur DataN
    // maximale. Fixture = 1 Autoroute (branche when 0..9) + 1 Communale
    // (branche when 0..6). Sans règles de mapping, CL_ADMIN n'est pas écrit
    // dans le `.mp`, donc on discrimine par la présence de Data9.
    let mut polyline_blocks_max_n: Vec<usize> = Vec::new();
    for body in &bodies {
        for block in body.split("[POLYLINE]").skip(1) {
            let (block, _) = block.split_once("[END]").unwrap_or((block, ""));
            let max_n = (0u8..=9)
                .rev()
                .find(|n| block.contains(&format!("Data{n}=")))
                .map(|n| n as usize)
                .unwrap_or(0);
            polyline_blocks_max_n.push(max_n);
            // AC9 : contiguïté stricte Data0..DataN pour chaque bloc.
            for n in 0..=max_n {
                assert!(
                    block.contains(&format!("Data{n}=")),
                    "POLYLINE block missing Data{n}= (max emitted = {max_n}):\n{block}"
                );
            }
        }
    }
    assert!(
        polyline_blocks_max_n.contains(&9),
        "no POLYLINE block reached Data9 (promotion failed):\nmax_n per block = {polyline_blocks_max_n:?}"
    );
    assert!(
        polyline_blocks_max_n.contains(&6),
        "no POLYLINE block capped at Data6 (Communale not preserved):\nmax_n per block = {polyline_blocks_max_n:?}"
    );

    // AC7 : couche BATIMENT absente du catalogue overview → pas de Data7+.
    for body in &bodies {
        for block in body.split("[POLYGON]").skip(1) {
            let (block, _) = block.split_once("[END]").unwrap_or((block, ""));
            for banned in ["Data7=", "Data8=", "Data9="] {
                assert!(
                    !block.contains(banned),
                    "BATIMENT polygon should not contain {banned}"
                );
            }
        }
    }
}
