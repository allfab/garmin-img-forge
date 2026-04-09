
use imgforge::img::assembler::{TileSubfiles, GmapsuppMeta, build_gmapsupp_with_meta_and_typ};

fn main() {
    let tiles: Vec<TileSubfiles> = vec![
        TileSubfiles {
            map_number: "63240001".to_string(),
            description: "63240001".to_string(),
            tre: include_bytes!("/tmp/mkgmap-tiles-extracted/63240001.TRE").to_vec(),
            rgn: include_bytes!("/tmp/mkgmap-tiles-extracted/63240001.RGN").to_vec(),
            lbl: include_bytes!("/tmp/mkgmap-tiles-extracted/63240001.LBL").to_vec(),
            net: None, nod: None, dem: None,
        },
        TileSubfiles {
            map_number: "63240002".to_string(),
            description: "63240002".to_string(),
            tre: include_bytes!("/tmp/mkgmap-tiles-extracted/63240002.TRE").to_vec(),
            rgn: include_bytes!("/tmp/mkgmap-tiles-extracted/63240002.RGN").to_vec(),
            lbl: include_bytes!("/tmp/mkgmap-tiles-extracted/63240002.LBL").to_vec(),
            net: None, nod: None, dem: None,
        },
        TileSubfiles {
            map_number: "63240003".to_string(),
            description: "63240003".to_string(),
            tre: include_bytes!("/tmp/mkgmap-tiles-extracted/63240003.TRE").to_vec(),
            rgn: include_bytes!("/tmp/mkgmap-tiles-extracted/63240003.RGN").to_vec(),
            lbl: include_bytes!("/tmp/mkgmap-tiles-extracted/63240003.LBL").to_vec(),
            net: None, nod: None, dem: None,
        },
        TileSubfiles {
            map_number: "63240004".to_string(),
            description: "63240004".to_string(),
            tre: include_bytes!("/tmp/mkgmap-tiles-extracted/63240004.TRE").to_vec(),
            rgn: include_bytes!("/tmp/mkgmap-tiles-extracted/63240004.RGN").to_vec(),
            lbl: include_bytes!("/tmp/mkgmap-tiles-extracted/63240004.LBL").to_vec(),
            net: None, nod: None, dem: None,
        },
        TileSubfiles {
            map_number: "63240005".to_string(),
            description: "63240005".to_string(),
            tre: include_bytes!("/tmp/mkgmap-tiles-extracted/63240005.TRE").to_vec(),
            rgn: include_bytes!("/tmp/mkgmap-tiles-extracted/63240005.RGN").to_vec(),
            lbl: include_bytes!("/tmp/mkgmap-tiles-extracted/63240005.LBL").to_vec(),
            net: None, nod: None, dem: None,
        },
    ];
    let meta = GmapsuppMeta {
        family_id: 26038,
        product_id: 1,
        family_name: "TEST-HYBRID".to_string(),
        area_name: String::new(),
        codepage: 1252,
    };
    let typ = include_bytes!("/tmp/mkgmap-tiles-extracted/typ.bin");
    let gmapsupp = build_gmapsupp_with_meta_and_typ(&tiles, "TEST-HYBRID", &meta, Some(typ), None).unwrap();
    std::fs::write("/tmp/test-3tiles/gmapsupp-hybrid.img", &gmapsupp).unwrap();
    println!("Written {} bytes", gmapsupp.len());
}
