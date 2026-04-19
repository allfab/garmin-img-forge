// Integration tests for the overview sub-map embedded in gmapsupp (Alpha 100 refonte).
//
// These tests build a gmapsupp with an overview sub-map from synthetic TileSubfiles
// and verify the resulting IMG directory exposes the overview sub-map with the expected
// map number (`<family_id>0000`), with TRE/RGN/LBL subfiles and without NET/NOD/DEM.

use imgforge::img::assembler::{
    build_gmapsupp_with_overview, compute_overview_map_id, GmapsuppMeta, TileSubfiles,
};
use imgforge::img::coord::Coord;
use imgforge::img::overview_map::{build_overview_map, OverviewFeature};

/// Build a minimal TRE blob carrying the bounds at offsets 21-32.
fn fake_tre(north: i32, east: i32, south: i32, west: i32) -> Vec<u8> {
    let mut tre = vec![0u8; 200];
    tre[0] = 188; tre[1] = 0;
    tre[2..12].copy_from_slice(b"GARMIN TRE");
    tre[21..24].copy_from_slice(&north.to_le_bytes()[..3]);
    tre[24..27].copy_from_slice(&east.to_le_bytes()[..3]);
    tre[27..30].copy_from_slice(&south.to_le_bytes()[..3]);
    tre[30..33].copy_from_slice(&west.to_le_bytes()[..3]);
    tre
}

fn make_tile(map_number: &str, north: i32, east: i32, south: i32, west: i32) -> TileSubfiles {
    TileSubfiles {
        map_number: map_number.to_string(),
        description: format!("Tile {map_number}"),
        tre: fake_tre(north, east, south, west),
        rgn: vec![0u8; 125],
        lbl: vec![0u8; 196],
        net: None, nod: None, dem: None,
    }
}

#[allow(dead_code)]
struct SubfileEntry {
    name: String,
    ext: String,
    size: usize,
    data_offset: usize,
}

fn parse_img_directory(img: &[u8]) -> Vec<SubfileEntry> {
    let block_exp1 = img[0x61] as u32;
    let block_exp2 = img[0x62] as u32;
    let block_size = 1u32 << (block_exp1 + block_exp2);
    let dir_start = 2usize * 512;

    let mut out = Vec::new();
    let mut pos = dir_start;
    while pos + 512 <= img.len() {
        let entry = &img[pos..pos + 512];
        if entry[0] != 0x01 {
            pos += 512;
            continue;
        }
        let part = u16::from_le_bytes([entry[0x11], entry[0x12]]);
        if part != 0 {
            pos += 512;
            continue;
        }
        let name = std::str::from_utf8(&entry[1..9]).unwrap_or("").trim().to_string();
        let ext = std::str::from_utf8(&entry[9..12]).unwrap_or("").trim().to_string();
        let size = u32::from_le_bytes([
            entry[0x0C], entry[0x0D], entry[0x0E], entry[0x0F],
        ]) as usize;
        let first_block = u16::from_le_bytes([entry[0x20], entry[0x21]]);
        let data_offset = if first_block == 0xFFFF {
            0
        } else {
            first_block as usize * block_size as usize
        };
        // Skip the reserved header entry
        if entry[0x10] != 0x03 && !name.is_empty() {
            out.push(SubfileEntry { name, ext, size, data_offset });
        }
        pos += 512;
    }
    out
}

#[test]
fn overview_submap_embedded_in_gmapsupp() {
    let fid: u16 = 1100;
    let tiles = vec![
        make_tile("11000001", 2143196, 262632, 2138930, 255409),
        make_tile("11000002", 2148000, 270000, 2143196, 262632),
    ];

    // Synthetic overview features: one polyline, one polygon.
    let features = vec![
        OverviewFeature {
            type_code: 0x01,
            is_point: false, is_line: true, is_polygon: false,
            coords: vec![Coord::new(2140000, 258000), Coord::new(2142000, 265000)],
        },
        OverviewFeature {
            type_code: 0x03,
            is_point: false, is_line: false, is_polygon: true,
            coords: vec![
                Coord::new(2140000, 258000),
                Coord::new(2142000, 265000),
                Coord::new(2140500, 260000),
            ],
        },
    ];

    let overview_map_id = compute_overview_map_id(fid);
    let ov_data = build_overview_map(&tiles, &features, overview_map_id, 1252);
    let overview_tile = TileSubfiles {
        map_number: ov_data.map_number.clone(),
        description: "Test overview".to_string(),
        tre: ov_data.tre,
        rgn: ov_data.rgn,
        lbl: ov_data.lbl,
        net: None, nod: None, dem: None,
    };

    let meta = GmapsuppMeta {
        family_id: fid, product_id: 1,
        family_name: "Test".to_string(),
        area_name: String::new(),
        codepage: 1252,
        typ_basename: None,
    };
    let img = build_gmapsupp_with_overview(&tiles, Some(&overview_tile), "Test", &meta, None)
        .expect("gmapsupp build should succeed");

    // Directory scan: find all TRE/RGN/LBL/NET/NOD by name.
    let entries = parse_img_directory(&img);

    // Expected overview map number = family*10000 = 11000000 (convention mkgmap <family>0000).
    let expected_ov = format!("{:08}", overview_map_id);
    assert_eq!(expected_ov, "11000000", "compute_overview_map_id(1100) sanity");

    // TRE/RGN/LBL must all exist for the overview sub-map.
    let ov_exts: Vec<&str> = entries.iter()
        .filter(|e| e.name == expected_ov)
        .map(|e| e.ext.as_str())
        .collect();
    assert!(ov_exts.contains(&"TRE"), "overview TRE missing — found {:?}", ov_exts);
    assert!(ov_exts.contains(&"RGN"), "overview RGN missing");
    assert!(ov_exts.contains(&"LBL"), "overview LBL missing");
    // AC 7: routing must be excluded from the overview.
    assert!(!ov_exts.contains(&"NET"), "overview must not contain NET");
    assert!(!ov_exts.contains(&"NOD"), "overview must not contain NOD");

    // Detail tiles are still there.
    let detail1 = entries.iter().any(|e| e.name == "11000001" && e.ext == "TRE");
    assert!(detail1, "detail tile 11000001 TRE missing");

    // F9 : localiser précisément la TRE overview dans le filesystem et vérifier le
    // marker `0x00040101` à l'offset 67-70 de CE fichier (pas une recherche naïve
    // dans tout l'IMG qui pourrait matcher au hasard).
    let ov_tre = entries.iter()
        .find(|e| e.name == expected_ov && e.ext == "TRE")
        .expect("overview TRE entry");
    let marker_bytes = &img[ov_tre.data_offset + 67 .. ov_tre.data_offset + 71];
    assert_eq!(
        u32::from_le_bytes([marker_bytes[0], marker_bytes[1], marker_bytes[2], marker_bytes[3]]),
        0x00040101u32,
        "overview TRE must carry map format marker 0x00040101 at offset 67"
    );
    // Signature CommonHeader : offset 2-11 = "GARMIN TRE"
    assert_eq!(&img[ov_tre.data_offset + 2 .. ov_tre.data_offset + 12], b"GARMIN TRE");
}

#[test]
fn overview_feature_whitelist_restricts_rgn() {
    let fid: u16 = 1100;
    let tiles = vec![make_tile("11000001", 2143196, 262632, 2138930, 255409)];

    // Two features with different types — whitelist keeps only 0x01.
    let mut wl = std::collections::HashSet::new();
    wl.insert(0x01u32);

    // Use the extraction pipeline directly: build an MpFile with both features then filter.
    use imgforge::img::overview_map::extract_overview_features;
    use imgforge::parser::mp_types::{MpFile, MpHeader, MpPolyline};
    use std::collections::BTreeMap;

    let mut geom_a = BTreeMap::new();
    geom_a.insert(5u8, vec![Coord::new(2140000, 258000), Coord::new(2142000, 265000)]);
    let mut geom_b = BTreeMap::new();
    geom_b.insert(5u8, vec![Coord::new(2140000, 258000), Coord::new(2142000, 265000)]);

    let mp = MpFile {
        header: MpHeader { codepage: 1252, ..Default::default() },
        points: vec![],
        polylines: vec![
            MpPolyline {
                type_code: 0x01, label: String::new(), geometries: geom_a,
                end_level: Some(6), direction: false, road_id: None, route_param: None,
            },
            MpPolyline {
                type_code: 0x50, label: String::new(), geometries: geom_b,
                end_level: Some(6), direction: false, road_id: None, route_param: None,
            },
        ],
        polygons: vec![],
    };

    let features = extract_overview_features(&mp, 5, 6, Some(&wl));
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].type_code, 0x01);

    // Verify the full pipeline produces a valid sub-map.
    let ov_data = build_overview_map(&tiles, &features, compute_overview_map_id(fid), 1252);
    let ov_tile = TileSubfiles {
        map_number: ov_data.map_number.clone(),
        description: "ov".into(),
        tre: ov_data.tre, rgn: ov_data.rgn, lbl: ov_data.lbl,
        net: None, nod: None, dem: None,
    };
    let meta = GmapsuppMeta {
        family_id: fid, product_id: 1,
        family_name: "T".to_string(), area_name: String::new(),
        codepage: 1252, typ_basename: None,
    };
    let img = build_gmapsupp_with_overview(&tiles, Some(&ov_tile), "T", &meta, None).unwrap();
    assert!(!img.is_empty());

    // F10 (AC 9) : vérifier qu'uniquement type_code=0x01 apparaît dans le RGN overview.
    // Une seule feature dans features → une seule section polyline dans le RGN, donc
    // pas de pointer en tête de subdiv ; le premier byte de la section data RGN est
    // directement le type byte de la polyline.
    let entries = parse_img_directory(&img);
    let expected_ov = format!("{:08}", compute_overview_map_id(fid));
    let ov_rgn = entries.iter()
        .find(|e| e.name == expected_ov && e.ext == "RGN")
        .expect("overview RGN entry");
    let rgn_header_len = 125usize; // cf. RgnWriter::RGN_HEADER_LEN
    let type_byte = img[ov_rgn.data_offset + rgn_header_len] & 0x7F; // masque le flag longueur 0x80
    assert_eq!(type_byte, 0x01,
        "whitelist 0x01 seul : le RGN overview doit commencer par le type byte 0x01, pas {:#x}",
        type_byte);
}
