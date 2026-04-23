// Build intermediate : génère les artefacts TRE/RGN/LBL de l'overview produits par
// `build_overview_map` avec des tiles factices couvrant D038 (Isère), et les écrit
// dans `<repo-root>/tmp/overview/intermediate/` pour validation hex byte-à-byte
// contre SUD.
//
// Usage : `cargo run -p imgforge --example dump_overview_artifacts`
// Le path de sortie est résolu via `CARGO_MANIFEST_DIR` pour être indépendant du
// répertoire courant (l'example était initialement CWD-dépendant et écrivait dans
// `tools/imgforge/tmp/` quand lancé depuis le crate).

use imgforge::img::assembler::TileSubfiles;
use imgforge::img::overview_map::build_overview_map;
use std::fs;
use std::path::PathBuf;

fn make_tile(n: i32, e: i32, s: i32, w: i32) -> TileSubfiles {
    let mut tre = vec![0u8; 200];
    tre[0] = 188;
    tre[1] = 0;
    tre[2..12].copy_from_slice(b"GARMIN TRE");
    tre[21..24].copy_from_slice(&n.to_le_bytes()[..3]);
    tre[24..27].copy_from_slice(&e.to_le_bytes()[..3]);
    tre[27..30].copy_from_slice(&s.to_le_bytes()[..3]);
    tre[30..33].copy_from_slice(&w.to_le_bytes()[..3]);
    TileSubfiles {
        map_number: "00380004".to_string(),
        description: "D038 dump".to_string(),
        tre,
        rgn: vec![0x02; 300],
        lbl: vec![0x03; 196],
        net: None,
        nod: None,
        dem: None,
    }
}

fn main() {
    // Bounds approximatifs de l'Isère (D038) en unités Garmin (deg * 2^24 / 360)
    // Lat ~ 44.7..45.9, Lon ~ 4.7..6.1 — mêmes ordres de grandeur que ce que
    // build-garmin-map.sh produit.
    let tiles = vec![
        make_tile(2143196, 287000, 2130000, 268000), // NW-ish
        make_tile(2148000, 300000, 2143196, 287000), // NE-ish
        make_tile(2130000, 287000, 2110000, 268000), // SW-ish
        make_tile(2143196, 300000, 2130000, 287000), // SE-ish
    ];

    let fid: u16 = 1100;
    let overview_id: u32 = (fid as u32) * 10000 + 1855; // 11001855
    // Phase 1 bounding-box uniquement (overview_features.rs supprimé)
    let ov = build_overview_map(&tiles, overview_id, 1252);

    // CARGO_MANIFEST_DIR = .../tools/imgforge ; remonter 2× pour atteindre repo-root.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("CARGO_MANIFEST_DIR doit avoir 2 ancêtres (tools/imgforge → repo-root)")
        .to_path_buf();
    let root = repo_root.join("tmp/overview/intermediate");
    fs::create_dir_all(&root).expect("mkdir intermediate");
    let stem = format!("{:08}", overview_id);
    for (ext, data) in [("TRE", &ov.tre), ("RGN", &ov.rgn), ("LBL", &ov.lbl)] {
        let path = root.join(format!("{}.{}", stem, ext));
        fs::write(&path, data).expect("write");
        println!("wrote {} ({} B)", path.display(), data.len());
    }
    println!("map_number = {}", ov.map_number);
}
