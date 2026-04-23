// Tests d'intégration pour l'overview map multi-niveaux (parité SUD Alpha 100)
// Cf. docs/implementation-artifacts/tech-spec-overview-multilevel-wide-zoom.md

use imgforge::img::assembler::{
    build_gmapsupp_with_overview, compute_overview_map_id, GmapsuppMeta, Packaging, TileSubfiles,
};
use imgforge::img::overview_map::build_overview_map;

fn make_tile(map_number: &str, north: i32, east: i32, south: i32, west: i32) -> TileSubfiles {
    let mut tre = vec![0u8; 200];
    tre[0] = 188;
    tre[1] = 0;
    tre[2..12].copy_from_slice(b"GARMIN TRE");
    tre[21..24].copy_from_slice(&north.to_le_bytes()[..3]);
    tre[24..27].copy_from_slice(&east.to_le_bytes()[..3]);
    tre[27..30].copy_from_slice(&south.to_le_bytes()[..3]);
    tre[30..33].copy_from_slice(&west.to_le_bytes()[..3]);
    TileSubfiles {
        map_number: map_number.to_string(),
        description: "Test".to_string(),
        tre,
        rgn: vec![0x02; 300],
        lbl: vec![0x03; 196],
        net: None,
        nod: None,
        dem: None,
    }
}

fn default_meta(fid: u16) -> GmapsuppMeta {
    GmapsuppMeta {
        family_id: fid,
        product_id: 1,
        family_name: "Test".to_string(),
        series_name: "Test".to_string(),
        area_name: String::new(),
        codepage: 1252,
        typ_basename: None,
        packaging: Packaging::Legacy,
    }
}

/// Parse FAT entries : renvoie liste (name, ext) dans l'ordre FAT (part=0 uniquement).
fn list_fat_entries(img: &[u8]) -> Vec<(String, String)> {
    let dir_start = 2 * 512;
    let mut out = Vec::new();
    let mut pos = dir_start;
    while pos + 512 <= img.len() {
        let entry = &img[pos..pos + 512];
        if entry[0] == 0x01 {
            let part = u16::from_le_bytes([entry[0x11], entry[0x12]]);
            if part == 0 {
                let name = std::str::from_utf8(&entry[1..9]).unwrap_or("").trim().to_string();
                let ext = std::str::from_utf8(&entry[9..12]).unwrap_or("").trim().to_string();
                // Ignorer l'entrée spéciale header
                let flag = entry[0x10];
                if flag != 0x03 && !name.is_empty() {
                    out.push((name, ext));
                }
            }
        }
        pos += 512;
    }
    out
}

#[test]
fn test_overview_present_in_gmapsupp() {
    let tiles = vec![
        make_tile("11000001", 2143196, 262632, 2138930, 255409),
        make_tile("11000002", 2148000, 270000, 2143196, 262632),
    ];
    let meta = default_meta(1100);
    let overview_id = compute_overview_map_id(meta.family_id);
    let overview = build_overview_map(&tiles, &[], overview_id, meta.codepage);

    let img = build_gmapsupp_with_overview(&tiles, "Test", &meta, None, Some(&overview))
        .expect("build failed");

    let entries = list_fat_entries(&img);
    let ov_name = format!("{:08}", overview_id);

    // MPS en tête
    assert_eq!(entries[0], ("MAKEGMAP".to_string(), "MPS".to_string()));
    // Overview immédiatement après : TRE/RGN/LBL
    assert_eq!(entries[1], (ov_name.clone(), "TRE".to_string()));
    assert_eq!(entries[2], (ov_name.clone(), "RGN".to_string()));
    assert_eq!(entries[3], (ov_name, "LBL".to_string()));
}

#[test]
fn test_overview_map_levels_match_sud_parity() {
    let tiles = vec![make_tile("11000001", 2143196, 262632, 2138930, 255409)];
    let meta = default_meta(1100);
    let overview_id = compute_overview_map_id(meta.family_id);
    let overview = build_overview_map(&tiles, &[], overview_id, meta.codepage);

    // Parse directement la TRE produite
    let tre = &overview.tre;
    let levels_off = u32::from_le_bytes([tre[0x21], tre[0x22], tre[0x23], tre[0x24]]) as usize;
    let levels_size = u32::from_le_bytes([tre[0x25], tre[0x26], tre[0x27], tre[0x28]]) as usize;
    assert_eq!(levels_size, 8, "2 paliers × 4 bytes");
    // Level 1 inherited bits=14
    assert_eq!(tre[levels_off], 0x81);
    assert_eq!(tre[levels_off + 1], 14);
    // Level 0 leaf bits=16
    assert_eq!(tre[levels_off + 4], 0x00);
    assert_eq!(tre[levels_off + 5], 16);
}

// AC 8 — Phase 2 : 4 paliers TRE (bits 10/12/14/16) quand features overview présentes
#[test]
fn test_overview_phase2_4levels() {
    use imgforge::img::coord::Coord;
    use imgforge::img::overview_features::OverviewFeature;

    let tiles = vec![make_tile("11000001", 2143196, 262632, 2138930, 255409)];
    let meta = default_meta(1100);
    let overview_id = compute_overview_map_id(meta.family_id);

    let features = vec![OverviewFeature {
        type_code: 0x4A,
        end_level: 7,
        geometry: vec![
            Coord::new(2138930, 255409),
            Coord::new(2143196, 255409),
            Coord::new(2143196, 262632),
        ],
        is_polygon: true,
        palier_index: 0,
    }];

    let overview = build_overview_map(&tiles, &features, overview_id, meta.codepage);
    let tre = &overview.tre;

    let levels_off = u32::from_le_bytes([tre[0x21], tre[0x22], tre[0x23], tre[0x24]]) as usize;
    let levels_size = u32::from_le_bytes([tre[0x25], tre[0x26], tre[0x27], tre[0x28]]) as usize;

    assert_eq!(levels_size, 16, "AC 8 : 4 paliers × 4 bytes = 16 B");
    assert_eq!(tre[levels_off],      0x83, "level 3 inherited bits=10");
    assert_eq!(tre[levels_off + 1],  10);
    assert_eq!(tre[levels_off + 4],  0x82, "level 2 inherited bits=12");
    assert_eq!(tre[levels_off + 5],  12);
    assert_eq!(tre[levels_off + 8],  0x81, "level 1 inherited bits=14");
    assert_eq!(tre[levels_off + 9],  14);
    assert_eq!(tre[levels_off + 12], 0x00, "level 0 leaf bits=16");
    assert_eq!(tre[levels_off + 13], 16);
}

#[test]
fn test_no_overview_when_none() {
    let tiles = vec![make_tile("11000001", 2143196, 262632, 2138930, 255409)];
    let meta = default_meta(1100);
    let overview_id = compute_overview_map_id(meta.family_id);

    let img = build_gmapsupp_with_overview(&tiles, "Test", &meta, None, None)
        .expect("build failed");

    let entries = list_fat_entries(&img);
    let ov_name = format!("{:08}", overview_id);
    assert!(
        !entries.iter().any(|(n, _)| n == &ov_name),
        "aucun sous-fichier d'ID overview ne doit être présent (kill-switch)"
    );
}

// AC1 — compute_detail_level_count : détection dynamique du split bits<16
#[test]
fn test_compute_detail_level_count_dynamic() {
    use imgforge::img::overview_features::compute_detail_level_count;
    assert_eq!(compute_detail_level_count(&[24, 23, 22, 21, 20, 18, 16, 14, 12, 10]), 7);
    assert_eq!(compute_detail_level_count(&[24, 22, 20, 18, 16]),              5);
    assert_eq!(compute_detail_level_count(&[24, 20, 18, 16, 14, 12]),          4);
    assert_eq!(compute_detail_level_count(&[24, 23, 22, 21, 20, 18, 16]),      7); // standard 7L
    assert_eq!(compute_detail_level_count(&[]),                                0); // vide
}

// AC2 — garde nb_overview_levels == 0 dans extract_overview_features
#[test]
fn test_extract_overview_features_nb_overview_zero() {
    use imgforge::img::overview_features::extract_overview_features;
    use imgforge::parser::mp_types::{MpFile, MpHeader};
    let mut header = MpHeader::default();
    header.levels = vec![24, 23, 22, 21, 20, 18, 16];
    let mp = MpFile { header, points: vec![], polylines: vec![], polygons: vec![] };
    let features = extract_overview_features(&mp, 7, 0);
    assert!(features.is_empty(), "nb_overview_levels=0 → garde → Vec::new()");
}

// F7 — chemin dynamique complet pour MP standard 7L : nb_overview calculé → 0 → Vec::new()
#[test]
fn test_dynamic_path_standard_7l_gives_empty_overview() {
    use imgforge::img::overview_features::{compute_detail_level_count, extract_overview_features};
    use imgforge::parser::mp_types::{MpFile, MpHeader};
    let mut header = MpHeader::default();
    header.levels = vec![24, 23, 22, 21, 20, 18, 16];
    let mp = MpFile { header, points: vec![], polylines: vec![], polygons: vec![] };

    let detail_count = compute_detail_level_count(&mp.header.levels);
    let nb_overview = (mp.header.levels.len() as u8).saturating_sub(detail_count);

    assert_eq!(detail_count, 7, "7L standard → detail_count=7");
    assert_eq!(nb_overview, 0, "7L standard → nb_overview=0");

    let features = extract_overview_features(&mp, detail_count, nb_overview);
    assert!(features.is_empty(), "chemin dynamique 7L standard → Vec::new()");
}

// AC3 — TRE tuile de détail : exactement 7 niveaux (bits>=16) après truncate
#[test]
fn test_detail_tre_7_levels_after_truncate() {
    use imgforge::img::overview_features::compute_detail_level_count;
    use imgforge::img::writer;
    // Utilise parse_mp pour obtenir un MP minimal valide (build_subfiles nécessite ≥1 feature)
    let mp_str = "[IMG ID]\nID=99990001\nName=Test\nLevels=7\n[END-IMG ID]\n\
                  [POLYGON]\nType=0x03\nLabel=Test\n\
                  Data0=(2.000,7.700),(2.010,7.700),(2.010,7.800)\n[END]\n";
    let mut mp = imgforge::parser::parse_mp(mp_str).expect("parse mp minimal");
    // Forcer 10 niveaux overview-demo : simule un MP produit par mpforge avec overview_levels
    mp.header.levels = vec![24, 23, 22, 21, 20, 18, 16, 14, 12, 10];

    let detail_level_count = compute_detail_level_count(&mp.header.levels);
    assert_eq!(detail_level_count, 7);
    mp.header.levels.truncate(detail_level_count as usize);
    assert_eq!(mp.header.levels.len(), 7);
    assert!(mp.header.levels.iter().all(|&b| b >= 16));

    let tile = writer::build_subfiles(&mp).expect("build_subfiles ne doit pas échouer");
    let tre = &tile.tre;
    // Section niveaux TRE : offset @0x21, taille @0x25 ; chaque enregistrement = 4 bytes
    let levels_off = u32::from_le_bytes([tre[0x21], tre[0x22], tre[0x23], tre[0x24]]) as usize;
    let levels_size = u32::from_le_bytes([tre[0x25], tre[0x26], tre[0x27], tre[0x28]]) as usize;
    let level_count = levels_size / 4;
    assert_eq!(level_count, 7, "AC3 : TRE détail = 7 niveaux exactement après truncate");
    for i in 0..level_count {
        let bits = tre[levels_off + i * 4 + 1];
        assert!(bits >= 16, "niveau {i} : bits={bits} doit être >= 16 (détail uniquement)");
    }
}

// F12 — cas limite : bits==16 en première position et slice entièrement < 16
#[test]
fn test_compute_detail_level_count_boundary() {
    use imgforge::img::overview_features::compute_detail_level_count;
    assert_eq!(compute_detail_level_count(&[16]),          1, "[16] → 1 niveau détail");
    assert_eq!(compute_detail_level_count(&[15]),          0, "[15] → 0 niveau détail");
    assert_eq!(compute_detail_level_count(&[14, 12, 10]),  0, "tous overview → 0");
    assert_eq!(compute_detail_level_count(&[17, 15, 16]),  1, "premier < 16 en index 1 → 1");
}

// Option B — tuile purement overview (bits 14/12/10) : TRE intacte, aucun truncate
#[test]
fn test_pure_overview_tile_levels_preserved() {
    use imgforge::img::writer;
    let mp_str = "[IMG ID]\nID=39000001\nName=OV-B\nLevels=3\nLevel0=14\nLevel1=12\nLevel2=10\n\
                  [END-IMG ID]\n\
                  [POLYGON]\nType=0x50\nLabel=Foret\n\
                  Data0=(2.000,7.700),(2.010,7.700),(2.010,7.800)\n[END]\n";
    let mut mp = imgforge::parser::parse_mp(mp_str).expect("parse mp overview-b");

    // Simule le chemin imgforge Build : detail_level_count=0 → pas de truncate
    use imgforge::img::overview_features::compute_detail_level_count;
    let detail_level_count = compute_detail_level_count(&mp.header.levels);
    assert_eq!(detail_level_count, 0, "tuile overview pure : aucun niveau ≥ 16");

    // nb_overview = 0 quand detail_level_count == 0 (pas de split)
    let nb_overview: u8 = if detail_level_count == 0 { 0 } else {
        (mp.header.levels.len().min(255) as u8).saturating_sub(detail_level_count)
    };
    assert_eq!(nb_overview, 0);

    // Pas de truncate → levels inchangés
    if detail_level_count > 0 && nb_overview > 0 {
        mp.header.levels.truncate(detail_level_count as usize);
    }
    assert_eq!(mp.header.levels, vec![14, 12, 10], "levels inchangés après no-op truncate");

    // build_subfiles doit réussir et produire un TRE 3 niveaux bits<16
    let tile = writer::build_subfiles(&mp).expect("build_subfiles tuile overview-b");
    let tre = &tile.tre;
    let levels_off  = u32::from_le_bytes([tre[0x21], tre[0x22], tre[0x23], tre[0x24]]) as usize;
    let levels_size = u32::from_le_bytes([tre[0x25], tre[0x26], tre[0x27], tre[0x28]]) as usize;
    let nb = levels_size / 4;
    assert_eq!(nb, 3, "TRE tuile overview-b : exactement 3 niveaux");
    let bits: Vec<u8> = (0..nb).map(|i| tre[levels_off + i * 4 + 1]).collect();
    assert!(bits.iter().all(|&b| b < 16), "tous les niveaux overview-b ont bits < 16 : {bits:?}");
}

// Option B — gmapsupp combiné : détail (7L) + overview-b (3L) sans collision ni rejet
#[test]
fn test_option_b_combined_packaging() {
    // Tuile détail 7L standard
    let detail = make_tile("38000001", 2143196, 262632, 2138930, 255409);
    // Tuile overview-b 3L (bits 14/12/10)
    let mut ov_b_tre = vec![0u8; 200];
    ov_b_tre[0] = 188; ov_b_tre[1] = 0;
    ov_b_tre[2..12].copy_from_slice(b"GARMIN TRE");
    ov_b_tre[21..24].copy_from_slice(&2143196i32.to_le_bytes()[..3]);
    ov_b_tre[24..27].copy_from_slice(&262632i32.to_le_bytes()[..3]);
    ov_b_tre[27..30].copy_from_slice(&2138930i32.to_le_bytes()[..3]);
    ov_b_tre[30..33].copy_from_slice(&255409i32.to_le_bytes()[..3]);
    let ov_b = TileSubfiles {
        map_number: "39000001".to_string(),
        description: "OV-B".to_string(),
        tre: ov_b_tre,
        rgn: vec![0x02; 300],
        lbl: vec![0x03; 196],
        net: None, nod: None, dem: None,
    };

    let meta = default_meta(1100);
    let overview_id = compute_overview_map_id(meta.family_id);
    let all_tiles = vec![detail, ov_b];
    let overview = build_overview_map(&all_tiles, &[], overview_id, meta.codepage);

    let img = build_gmapsupp_with_overview(&all_tiles, "Test", &meta, None, Some(&overview))
        .expect("packaging combiné détail + overview-b");

    let entries = list_fat_entries(&img);
    // Les deux tuiles doivent être présentes (IDs distincts)
    assert!(entries.iter().any(|(n, e)| n == "38000001" && e == "TRE"),
        "tuile détail 38000001 absente");
    assert!(entries.iter().any(|(n, e)| n == "39000001" && e == "TRE"),
        "tuile overview-b 39000001 absente");
}
