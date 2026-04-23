use std::fs;
use imgforge::img::assembler::{TileSubfiles, GmapsuppMeta, build_gmapsupp_with_meta_and_typ};

fn read_tile(base: &str, number: &str) -> TileSubfiles {
    let p = format!("{base}/{number}");
    TileSubfiles {
        map_number: number.to_string(),
        description: number.to_string(),
        tre: fs::read(format!("{p}.TRE")).expect("read TRE"),
        rgn: fs::read(format!("{p}.RGN")).expect("read RGN"),
        lbl: fs::read(format!("{p}.LBL")).expect("read LBL"),
        net: None, nod: None, dem: None,
    }
}

fn main() {
    let base = "/tmp/mkgmap-tiles-extracted";
    let tiles = vec![
        read_tile(base, "63240001"),
        read_tile(base, "63240002"),
        read_tile(base, "63240003"),
        read_tile(base, "63240004"),
        read_tile(base, "63240005"),
    ];
    let meta = GmapsuppMeta {
        family_id: 26038,
        product_id: 1,
        family_name: "TEST-HYBRID".to_string(),
        series_name: "TEST".to_string(),
        area_name: String::new(),
        codepage: 1252,
        typ_basename: None,
        packaging: Default::default(),
    };
    let typ = fs::read(format!("{base}/typ.bin")).expect("read typ.bin");
    let gmapsupp = build_gmapsupp_with_meta_and_typ(&tiles, "TEST-HYBRID", &meta, Some(&typ)).unwrap();
    fs::write("/tmp/test-3tiles/gmapsupp-hybrid.img", &gmapsupp).unwrap();
    println!("Written {} bytes", gmapsupp.len());
}
