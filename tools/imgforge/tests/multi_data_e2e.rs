//! End-to-end multi-level Data fixture (AC11 / F12).
//!
//! Vérifie que la chaîne complète (parser → encoder RGN par niveau) honore
//! la sélection par bucket : la géométrie réellement encodée pour un subdiv
//! au niveau L provient du bucket DataL (ou du fallback Politique B).
//!
//! Ne dépend d'aucun parseur IMG externe : on s'appuie sur la fonction
//! publique `filter_features_for_level` exposée via lib pour comparer la
//! sortie au bucket attendu, et on confirme via `build_img` que le pipeline
//! complet ne plante pas.

use imgforge::img::writer::build_img;
use imgforge::parser;

const FIXTURE: &str = include_str!("fixtures/multi_data_synthetic.mp");

#[test]
fn multi_data_fixture_parses_three_buckets() {
    let mp = parser::parse_mp(FIXTURE).unwrap();
    assert_eq!(mp.polylines.len(), 2);
    assert_eq!(mp.polygons.len(), 1);

    // Polyline #0 : trois buckets distincts.
    let pl0 = &mp.polylines[0];
    assert_eq!(pl0.geometries.len(), 3, "Data0/Data1/Data2 attendus");
    assert_eq!(pl0.geometries[&0].len(), 5);
    assert_eq!(pl0.geometries[&1].len(), 3);
    assert_eq!(pl0.geometries[&2].len(), 2);

    // Polyline #1 : Data0 only.
    let pl1 = &mp.polylines[1];
    assert_eq!(pl1.geometries.len(), 1);
    assert_eq!(pl1.geometries[&0].len(), 3);

    // Polygon #0 : deux buckets.
    let pg0 = &mp.polygons[0];
    assert_eq!(pg0.geometries.len(), 2);
    assert_eq!(pg0.geometries[&0].len(), 5);
    assert_eq!(pg0.geometries[&1].len(), 4);
}

#[test]
fn multi_data_fixture_geometry_for_level_routes_correctly() {
    let mp = parser::parse_mp(FIXTURE).unwrap();
    let pl0 = &mp.polylines[0];
    // Bucket exact à chaque niveau présent.
    assert_eq!(pl0.geometry_for_level(0).len(), 5);
    assert_eq!(pl0.geometry_for_level(1).len(), 3);
    assert_eq!(pl0.geometry_for_level(2).len(), 2);

    // Polyline #1 (Data0 only) : tous les niveaux retombent sur le bucket 0.
    let pl1 = &mp.polylines[1];
    assert_eq!(pl1.geometry_for_level(0).len(), 3);
    assert_eq!(pl1.geometry_for_level(1).len(), 3);
    assert_eq!(pl1.geometry_for_level(2).len(), 3);

    // Polygon #0 : niveau 2 absent → fallback vers plus grossier disponible (1).
    let pg0 = &mp.polygons[0];
    assert_eq!(pg0.geometry_for_level(0).len(), 5);
    assert_eq!(pg0.geometry_for_level(1).len(), 4);
    assert_eq!(pg0.geometry_for_level(2).len(), 4, "fallback Politique B vers Data1");
}

#[test]
fn multi_data_fixture_builds_complete_img() {
    let mp = parser::parse_mp(FIXTURE).unwrap();
    let img = build_img(&mp).unwrap();
    // Sanity: header DSKIMG + signature trailer.
    assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
    assert_eq!(img[0x1FE], 0x55);
    assert_eq!(img[0x1FF], 0xAA);
    assert!(img.len() > 1024);
}

#[test]
fn multi_data_fixture_differs_from_data0_only_equivalent() {
    // Si la sélection par bucket fonctionne, l'IMG produit avec multi-Data
    // est forcément différent d'un IMG construit uniquement avec Data0
    // (la simplification par niveau n'a alors rien à sélectionner).
    let mp_multi = parser::parse_mp(FIXTURE).unwrap();
    let img_multi = build_img(&mp_multi).unwrap();

    // Variante "Data0 only" — on conserve seulement le bucket 0 de chaque feature.
    let mut mp_d0_only = mp_multi.clone();
    for pl in mp_d0_only.polylines.iter_mut() {
        pl.geometries.retain(|&n, _| n == 0);
    }
    for pg in mp_d0_only.polygons.iter_mut() {
        pg.geometries.retain(|&n, _| n == 0);
    }
    let img_d0 = build_img(&mp_d0_only).unwrap();

    assert_ne!(
        img_multi, img_d0,
        "Les IMGs doivent différer : la version multi-Data utilise des buckets simplifiés \
         pour les niveaux 1 et 2, ce qui produit un RGN différent."
    );
}
