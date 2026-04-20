use std::fs;
use imgforge::img::assembler::{TileSubfiles, GmapsuppMeta, build_gmapsupp_with_meta_and_typ};

fn main() {
    let tre = fs::read("/tmp/mkgmap-extracted/63240001.TRE").expect("read TRE");
    let rgn = fs::read("/tmp/mkgmap-extracted/63240001.RGN").expect("read RGN");
    let lbl = fs::read("/tmp/mkgmap-extracted/63240001.LBL").expect("read LBL");
    let typ = fs::read("/home/allfab/code/forgejo/garmin-img-forge/pipeline/resources/typfiles/IGNBBTOP.typ").expect("read TYP");
    
    println!("TRE: {} RGN: {} LBL: {} TYP: {}", tre.len(), rgn.len(), lbl.len(), typ.len());
    
    let tile = TileSubfiles {
        map_number: "63240001".to_string(),
        description: "BDTOPO-002-006".to_string(),
        tre, rgn, lbl,
        net: None, nod: None, dem: None,
    };
    
    let meta = GmapsuppMeta {
        family_id: 26038,
        product_id: 1,
        family_name: "IGN BDTOPO".to_string(),
        area_name: "D038".to_string(),
        codepage: 1252,
        typ_basename: None,
        packaging: Default::default(),
    };
    
    let gmapsupp = build_gmapsupp_with_meta_and_typ(
        &[tile], "IGN BDTOPO D038", &meta, Some(&typ),
    ).expect("build failed");
    
    fs::write("/tmp/gmapsupp-mkgmap-hybrid-typ.img", &gmapsupp).unwrap();
    println!("Written: {} bytes", gmapsupp.len());
}
