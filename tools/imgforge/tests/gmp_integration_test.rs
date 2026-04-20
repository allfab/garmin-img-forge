// Tests d'intégration end-to-end du packaging `--packaging gmp`.
//
// AC 3 (roundtrip) + AC 4 (flag CLI → 1 FAT `.GMP` par tuile au lieu de 6) +
// AC 5 (régression zéro mode legacy).

use imgforge::img::assembler::{
    build_gmapsupp_with_meta_and_typ, GmapsuppMeta, Packaging, TileSubfiles,
};

fn make_tile(map_number: &str, fill: u8) -> TileSubfiles {
    let mut tre = vec![fill; 200];
    // Magic + minimal header for downstream tooling that might sniff it.
    tre[0..2].copy_from_slice(&188u16.to_le_bytes());
    tre[2..12].copy_from_slice(b"GARMIN TRE");
    let mut rgn = vec![fill.wrapping_add(1); 300];
    rgn[0..2].copy_from_slice(&29u16.to_le_bytes());
    rgn[2..12].copy_from_slice(b"GARMIN RGN");
    let mut lbl = vec![fill.wrapping_add(2); 150];
    lbl[0..2].copy_from_slice(&196u16.to_le_bytes());
    lbl[2..12].copy_from_slice(b"GARMIN LBL");
    TileSubfiles {
        map_number: map_number.to_string(),
        description: "test tile".to_string(),
        tre, rgn, lbl,
        net: None, nod: None, dem: None,
    }
}

fn parse_img_fat_by_ext(img: &[u8]) -> std::collections::HashMap<String, usize> {
    // Parse IMG directory starting at block 2 (offset 0x400), 512 B per entry.
    let mut map = std::collections::HashMap::<String, usize>::new();
    let mut pos = 2 * 512;
    while pos + 512 <= img.len() {
        let e = &img[pos..pos + 512];
        if e[0] != 0x01 {
            if e.iter().all(|&b| b == 0) { break; }
            pos += 512; continue;
        }
        let ext = std::str::from_utf8(&e[9..12]).unwrap_or("").trim().to_string();
        *map.entry(ext).or_insert(0) += 1;
        pos += 512;
    }
    map
}

#[test]
fn gmp_packaging_single_file_per_tile() {
    let tiles = vec![make_tile("11000001", 0x10), make_tile("11000002", 0x20)];
    let meta = GmapsuppMeta {
        family_id: 1100, product_id: 1,
        family_name: "T".into(), area_name: String::new(),
        codepage: 1252, typ_basename: None,
        packaging: Packaging::Gmp,
    };
    let img = build_gmapsupp_with_meta_and_typ(&tiles, "Test", &meta, None).unwrap();
    let counts = parse_img_fat_by_ext(&img);

    // AC 4 : 2 tuiles → 2 entrées .GMP, 0 entrée TRE/RGN/LBL/NET/NOD/DEM.
    assert_eq!(counts.get("GMP").copied().unwrap_or(0), 2);
    assert_eq!(counts.get("TRE").copied().unwrap_or(0), 0);
    assert_eq!(counts.get("RGN").copied().unwrap_or(0), 0);
    assert_eq!(counts.get("LBL").copied().unwrap_or(0), 0);
    // MPS + SRT still present.
    assert_eq!(counts.get("MPS").copied().unwrap_or(0), 1);
    assert_eq!(counts.get("SRT").copied().unwrap_or(0), 1);
}

#[test]
fn legacy_packaging_unchanged() {
    let tiles = vec![make_tile("11000001", 0x10), make_tile("11000002", 0x20)];
    let meta = GmapsuppMeta {
        family_id: 1100, product_id: 1,
        family_name: "T".into(), area_name: String::new(),
        codepage: 1252, typ_basename: None,
        packaging: Packaging::Legacy,
    };
    let img = build_gmapsupp_with_meta_and_typ(&tiles, "Test", &meta, None).unwrap();
    let counts = parse_img_fat_by_ext(&img);

    // AC 5 : mode legacy strictement identique au comportement historique.
    assert_eq!(counts.get("GMP").copied().unwrap_or(0), 0, "pas de .GMP en legacy");
    assert_eq!(counts.get("TRE").copied().unwrap_or(0), 2);
    assert_eq!(counts.get("RGN").copied().unwrap_or(0), 2);
    assert_eq!(counts.get("LBL").copied().unwrap_or(0), 2);
    assert_eq!(counts.get("MPS").copied().unwrap_or(0), 1);
    assert_eq!(counts.get("SRT").copied().unwrap_or(0), 1);
}

#[test]
fn gmp_roundtrip_subsections_recovered_from_img() {
    // Construit un gmapsupp --packaging gmp avec 1 tuile, extrait le .GMP, parse ses offsets,
    // vérifie que les 3 sous-sections (TRE/RGN/LBL) sont intactes byte-pour-byte.
    let tile = make_tile("11000001", 0xAA);
    let tre_ref = tile.tre.clone();
    let rgn_ref = tile.rgn.clone();
    let lbl_ref = tile.lbl.clone();
    let meta = GmapsuppMeta {
        family_id: 1100, product_id: 1,
        family_name: "T".into(), area_name: String::new(),
        codepage: 1252, typ_basename: None,
        packaging: Packaging::Gmp,
    };
    let img = build_gmapsupp_with_meta_and_typ(&[tile], "Test", &meta, None).unwrap();

    // Walk FAT → extract the .GMP blob bytes.
    let block_exp1 = img[0x61] as u32;
    let block_exp2 = img[0x62] as u32;
    let block_size = 1u32 << (block_exp1 + block_exp2);
    let mut gmp_blob: Option<Vec<u8>> = None;
    let mut pos = 2 * 512;
    while pos + 512 <= img.len() {
        let e = &img[pos..pos + 512];
        if e[0] != 0x01 { pos += 512; continue; }
        let ext = std::str::from_utf8(&e[9..12]).unwrap_or("").trim();
        if ext == "GMP" && e[0x11] == 0 {
            let size = u32::from_le_bytes([e[0x0C], e[0x0D], e[0x0E], e[0x0F]]) as usize;
            let mut buf = Vec::with_capacity(size);
            for i in (0x20..512).step_by(2) {
                let blk = u16::from_le_bytes([e[i], e[i + 1]]);
                if blk == 0xFFFF { break; }
                let off = blk as usize * block_size as usize;
                let end = (off + block_size as usize).min(img.len());
                buf.extend_from_slice(&img[off..end]);
            }
            buf.truncate(size);
            gmp_blob = Some(buf);
            break;
        }
        pos += 512;
    }
    let gmp = gmp_blob.expect("pas d'entrée .GMP trouvée dans l'IMG");

    // Vérifie le magic header.
    assert_eq!(&gmp[0x02..0x0C], b"GARMIN GMP");
    assert_eq!(u16::from_le_bytes([gmp[0], gmp[1]]), 0x3D);

    // Parse les offsets (packed u32 à partir de 0x19 — cf. imgforge-gmp-format.md).
    let tre_off = u32::from_le_bytes([gmp[0x19], gmp[0x1A], gmp[0x1B], gmp[0x1C]]) as usize;
    let rgn_off = u32::from_le_bytes([gmp[0x1D], gmp[0x1E], gmp[0x1F], gmp[0x20]]) as usize;
    let lbl_off = u32::from_le_bytes([gmp[0x21], gmp[0x22], gmp[0x23], gmp[0x24]]) as usize;

    // tre_offset doit pointer juste après header (0x3D) + copyright (0xB3) = 0xF0.
    assert_eq!(tre_off, 0xF0);

    // AC 3 : les sous-sections récupérées = celles fournies.
    assert_eq!(&gmp[tre_off..tre_off + tre_ref.len()], &tre_ref[..]);
    assert_eq!(&gmp[rgn_off..rgn_off + rgn_ref.len()], &rgn_ref[..]);
    assert_eq!(&gmp[lbl_off..lbl_off + lbl_ref.len()], &lbl_ref[..]);
}
