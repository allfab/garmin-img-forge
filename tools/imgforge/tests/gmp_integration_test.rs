// Tests d'intégration end-to-end du packaging `--packaging gmp`.
//
// AC 3 (roundtrip v2) + AC 4 (flag CLI → 1 FAT `.GMP` par tuile au lieu de 6) +
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
    rgn[0..2].copy_from_slice(&125u16.to_le_bytes());
    rgn[2..12].copy_from_slice(b"GARMIN RGN");
    // LBL_HEADER_LEN = 196 ; dernier offset @184 → blob doit être ≥ 188 bytes.
    let mut lbl = vec![fill.wrapping_add(2); 200];
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
        family_name: "T".into(), series_name: "T".into(), area_name: String::new(),
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
        family_name: "T".into(), series_name: "T".into(), area_name: String::new(),
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

/// AC 3 (v2) : le blob .GMP extrait de l'IMG contient les sous-sections avec
/// des offsets internes correctement relocalisés (GMP-absolus).
///
/// On vérifie :
/// - le magic "GARMIN GMP" dans le blob GMP
/// - la présence des magics TRE/RGN/LBL aux positions pointées par le header GMP
/// - que les offsets internes TRE/RGN/LBL ont été augmentés par leur position dans le GMP
///   (et non laissés à leur valeur standalone)
/// - pour TRE : l'extension NT (188→309) est appliquée avant la relocalisation GMP
#[test]
fn gmp_v2_roundtrip_offsets_are_gmp_absolute() {
    use imgforge::img::tre::{TreWriter, TRE_HEADER_LEN};
    use imgforge::img::rgn::{RgnWriter, RGN_HEADER_LEN};
    use imgforge::img::lbl::{LblWriter, LBL_HEADER_LEN};
    use imgforge::img::labelenc::LabelEncoding;
    use imgforge::img::gmp::NT_TRE_HLEN;

    // Construire des blobs réels avec des offsets connus
    let mut tre_writer = TreWriter::new();
    tre_writer.set_bounds(0, 0, 100, 100);
    let tre_blob = tre_writer.build();
    let rgn_blob = RgnWriter::new().build();
    let lbl_blob = LblWriter::new(LabelEncoding::Format6).build();

    // Offsets standalone attendus
    let standalone_tre_ml = u32::from_le_bytes(tre_blob[33..37].try_into().unwrap());
    let standalone_rgn_data = u32::from_le_bytes(rgn_blob[21..25].try_into().unwrap());
    let standalone_lbl_data = u32::from_le_bytes(lbl_blob[21..25].try_into().unwrap());
    assert_eq!(standalone_tre_ml, TRE_HEADER_LEN as u32);
    assert_eq!(standalone_rgn_data, RGN_HEADER_LEN as u32);
    assert_eq!(standalone_lbl_data, LBL_HEADER_LEN as u32);

    let tile = TileSubfiles {
        map_number: "11000001".to_string(),
        description: "test".to_string(),
        tre: tre_blob.clone(),
        rgn: rgn_blob.clone(),
        lbl: lbl_blob.clone(),
        net: None, nod: None, dem: None,
    };
    let meta = GmapsuppMeta {
        family_id: 1100, product_id: 1,
        family_name: "T".into(), series_name: "T".into(), area_name: String::new(),
        codepage: 1252, typ_basename: None,
        packaging: Packaging::Gmp,
    };
    let img = build_gmapsupp_with_meta_and_typ(&[tile], "Test", &meta, None).unwrap();

    // Extraire le blob .GMP depuis le FAT IMG
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

    // === Vérifications header GMP ===
    assert_eq!(&gmp[0x02..0x0C], b"GARMIN GMP");
    assert_eq!(u16::from_le_bytes([gmp[0], gmp[1]]), 0x3D);

    let gmp_tre_start = u32::from_le_bytes([gmp[0x19], gmp[0x1A], gmp[0x1B], gmp[0x1C]]);
    let gmp_rgn_start = u32::from_le_bytes([gmp[0x1D], gmp[0x1E], gmp[0x1F], gmp[0x20]]);
    let gmp_lbl_start = u32::from_le_bytes([gmp[0x21], gmp[0x22], gmp[0x23], gmp[0x24]]);

    // tre_offset doit pointer juste après header (0x3D) + copyright (0xB3) = 0xF0
    assert_eq!(gmp_tre_start, 0xF0, "TRE doit commencer à 0xF0 dans le GMP");

    // === Vérification relocalisation TRE ===
    // L'extension NT (hlen 188→309) est appliquée AVANT la relocalisation GMP.
    // map_levels_offset = standalone (188) + NT ext (121) + gmp_tre_start (240) = 549.
    let nt_ext = (NT_TRE_HLEN - TRE_HEADER_LEN as usize) as u32;
    let tre_in_gmp = &gmp[gmp_tre_start as usize..];
    assert_eq!(&tre_in_gmp[2..12], b"GARMIN TRE", "magic TRE intact après relocalisation");
    let relocated_ml = u32::from_le_bytes(tre_in_gmp[33..37].try_into().unwrap());
    assert_eq!(relocated_ml, standalone_tre_ml + nt_ext + gmp_tre_start,
        "TRE map_levels_offset doit être GMP-absolu (standalone + NT ext + GMP base)");

    // === Vérification relocalisation RGN ===
    let rgn_in_gmp = &gmp[gmp_rgn_start as usize..];
    assert_eq!(&rgn_in_gmp[2..12], b"GARMIN RGN", "magic RGN intact");
    let relocated_rgn_data = u32::from_le_bytes(rgn_in_gmp[21..25].try_into().unwrap());
    assert_eq!(relocated_rgn_data, standalone_rgn_data + gmp_rgn_start,
        "RGN data_offset doit être GMP-absolu");

    // === Vérification relocalisation LBL ===
    let lbl_in_gmp = &gmp[gmp_lbl_start as usize..];
    assert_eq!(&lbl_in_gmp[2..12], b"GARMIN LBL", "magic LBL intact");
    let relocated_lbl_data = u32::from_le_bytes(lbl_in_gmp[21..25].try_into().unwrap());
    assert_eq!(relocated_lbl_data, standalone_lbl_data + gmp_lbl_start,
        "LBL label_data_offset doit être GMP-absolu");

    // === Intégrité du payload : corps des blobs intact après relocalisation ===
    // La relocalisation ne touche que les champs offset dans les headers (< hlen) ;
    // les octets au-delà du header étendu (NT_TRE_HLEN) doivent être byte-identiques.
    // Note : les 121 bytes d'extension (TRE_HEADER_LEN..NT_TRE_HLEN) sont des zéros.
    assert_eq!(
        &gmp[gmp_tre_start as usize + NT_TRE_HLEN..gmp_rgn_start as usize],
        &tre_blob[TRE_HEADER_LEN as usize..],
        "corps TRE intact après relocalisation"
    );
    assert_eq!(
        &gmp[gmp_rgn_start as usize + RGN_HEADER_LEN as usize..gmp_lbl_start as usize],
        &rgn_blob[RGN_HEADER_LEN as usize..],
        "corps RGN intact après relocalisation"
    );
    assert_eq!(
        &gmp[gmp_lbl_start as usize + LBL_HEADER_LEN as usize..],
        &lbl_blob[LBL_HEADER_LEN as usize..],
        "corps LBL intact après relocalisation"
    );
}
