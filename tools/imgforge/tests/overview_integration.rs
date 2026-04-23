// Tests d'intégration pour l'overview map Phase 1 bounding-box (parité SUD Alpha 100)
// Cf. docs/implementation-artifacts/tech-spec-imgforge-overview-refactor-phase2-purge.md

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
    let overview = build_overview_map(&tiles, overview_id, meta.codepage);

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
    let overview = build_overview_map(&tiles, overview_id, meta.codepage);

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

