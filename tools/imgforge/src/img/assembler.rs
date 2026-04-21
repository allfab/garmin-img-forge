// GmapsuppBuilder — multi-tile assembler, faithful to mkgmap GmapsuppBuilder.java

use crate::error::ImgError;
use super::filesystem::ImgFilesystem;
use super::gmp::GmpWriter;
use super::mps::{MpsWriter, MpsMapEntry, MpsProductEntry};
use super::overview_map::OverviewMapData;

/// Mode d'emballage des sous-sections de tuile dans l'IMG.
///
/// - `Legacy` : 6 fichiers FAT par tuile (TRE/RGN/LBL/NET/NOD/DEM) — comportement historique.
/// - `Gmp` : 1 fichier FAT `.GMP` par tuile — format Garmin "NT" consolidé, aligné sur
///   les IMG Garmin modernes (cf. `docs/implementation-artifacts/imgforge-gmp-format.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Packaging {
    #[default]
    Legacy,
    Gmp,
}

/// Subfiles for a single tile
pub struct TileSubfiles {
    pub map_number: String,
    pub description: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
    pub net: Option<Vec<u8>>,
    pub nod: Option<Vec<u8>>,
    pub dem: Option<Vec<u8>>,
}

/// Metadata for the gmapsupp assembly
pub struct GmapsuppMeta {
    pub family_id: u16,
    pub product_id: u16,
    pub family_name: String,
    pub area_name: String,
    pub codepage: u16,
    /// TYP filename stem (without extension), e.g. "I2023100" from "I2023100.typ".
    /// If None, uses family_id formatted as 8-digit string.
    pub typ_basename: Option<String>,
    /// Packaging mode pour les sous-sections de tuile (Legacy = 6 FAT files / Gmp = 1 `.GMP`).
    pub packaging: Packaging,
}

impl Default for GmapsuppMeta {
    fn default() -> Self {
        Self {
            family_id: 1,
            product_id: 1,
            family_name: "Map".to_string(),
            area_name: String::new(),
            codepage: 0,
            typ_basename: None,
            packaging: Packaging::Legacy,
        }
    }
}

/// Assemble multiple tiles into a single gmapsupp.img
/// Each tile's subfiles (TRE/RGN/LBL/NET/NOD) are added as separate files
/// in a flat filesystem directory, as Garmin devices expect.
/// Includes an MPS subfile with per-tile metadata (PID, FID, description).
pub fn build_gmapsupp(
    tiles: &[TileSubfiles],
    description: &str,
) -> Result<Vec<u8>, ImgError> {
    let meta = GmapsuppMeta {
        family_name: description.to_string(),
        ..Default::default()
    };
    build_gmapsupp_with_meta(tiles, description, &meta)
}

/// Assemble with explicit metadata (family_id, product_id, etc.)
pub fn build_gmapsupp_with_meta(
    tiles: &[TileSubfiles],
    description: &str,
    meta: &GmapsuppMeta,
) -> Result<Vec<u8>, ImgError> {
    build_gmapsupp_with_meta_and_typ(tiles, description, meta, None)
}

/// Assemble with explicit metadata and optional TYP styling data
pub fn build_gmapsupp_with_meta_and_typ(
    tiles: &[TileSubfiles],
    description: &str,
    meta: &GmapsuppMeta,
    typ_data: Option<&[u8]>,
) -> Result<Vec<u8>, ImgError> {
    build_gmapsupp_with_overview(tiles, description, meta, typ_data, None)
}

/// Assemble with overview map (mirroir SUD Alpha 100 — cf. tech-spec overview-multilevel-wide-zoom)
pub fn build_gmapsupp_with_overview(
    tiles: &[TileSubfiles],
    description: &str,
    meta: &GmapsuppMeta,
    typ_data: Option<&[u8]>,
    overview: Option<&OverviewMapData>,
) -> Result<Vec<u8>, ImgError> {
    if tiles.is_empty() {
        return Err(ImgError::InvalidFormat("No tiles to assemble".into()));
    }

    let mut fs = ImgFilesystem::new(description);

    // --- File ordering: MPS first, overview TRE/RGN/LBL, tiles, TYP, SRT ---
    // Some Garmin firmware (Alpha 100) expects MPS as the first subfile.

    // 1. MPS first (mkgmap convention). L-record overview opt-in via IMGFORGE_MPS_OVERVIEW=1
    // pour diagnostic (AC 7 inversée : parité mkgmap plutôt que SUD stricte).
    let mps_with_overview = std::env::var("IMGFORGE_MPS_OVERVIEW")
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    let mps_ov = if mps_with_overview { overview.map(|o| o.map_number.as_str()) } else { None };
    let mps_data = build_mps(tiles, meta, mps_ov);
    fs.add_file("MAKEGMAP", "MPS", mps_data);

    // 1b. Overview map subfiles (parité SUD : TRE/RGN/LBL juste après le MPS).
    // Bisection diagnostique Alpha 100 via IMGFORGE_OVERVIEW_PARTS (liste CSV : tre,rgn,lbl).
    if let Some(ov) = overview {
        let parts = std::env::var("IMGFORGE_OVERVIEW_PARTS")
            .unwrap_or_else(|_| "tre,rgn,lbl".to_string());
        let parts_lc = parts.to_ascii_lowercase();
        let want = |p: &str| parts_lc.split(',').any(|x| x.trim() == p);
        if want("tre") { fs.add_file(&ov.map_number, "TRE", ov.tre.clone()); }
        if want("rgn") { fs.add_file(&ov.map_number, "RGN", ov.rgn.clone()); }
        if want("lbl") { fs.add_file(&ov.map_number, "LBL", ov.lbl.clone()); }
    }

    // 2. Tile subfiles — packaging legacy (6 FAT) ou GMP consolidé (1 FAT).
    for tile in tiles {
        let name = format!("{:>08}", tile.map_number);
        match meta.packaging {
            Packaging::Legacy => {
                fs.add_file(&name, "TRE", tile.tre.clone());
                fs.add_file(&name, "RGN", tile.rgn.clone());
                fs.add_file(&name, "LBL", tile.lbl.clone());
                if let Some(ref net) = tile.net {
                    fs.add_file(&name, "NET", net.clone());
                }
                if let Some(ref nod) = tile.nod {
                    fs.add_file(&name, "NOD", nod.clone());
                }
                if let Some(ref dem) = tile.dem {
                    fs.add_file(&name, "DEM", dem.clone());
                }
            }
            Packaging::Gmp => {
                let gmp_bytes = GmpWriter::new(
                    tile.tre.clone(),
                    tile.rgn.clone(),
                    tile.lbl.clone(),
                    tile.net.clone(),
                    tile.nod.clone(),
                    tile.dem.clone(),
                ).write();
                fs.add_file(&name, "GMP", gmp_bytes);
            }
        }
    }

    // 3. TYP file (mkgmap: after tiles, before SRT)
    if let Some(typ) = typ_data {
        let typ_name = meta.typ_basename.clone().unwrap_or_else(|| format!("{:08}", meta.family_id));
        let mut patched = typ.to_vec();
        if patched.len() >= 0x33 {
            let fid_bytes = meta.family_id.to_le_bytes();
            let pid_bytes = meta.product_id.to_le_bytes();
            patched[0x2F] = fid_bytes[0];
            patched[0x30] = fid_bytes[1];
            patched[0x31] = pid_bytes[0];
            patched[0x32] = pid_bytes[1];
        }
        fs.add_file(&typ_name, "TYP", patched);
    }

    // 4. SRT (sort descriptor) — required by some Garmin firmware (Alpha 100)
    // Only CP1252 SRT is available; other codepages fall back to it.
    {
        let srt_name = format!("{:08}", meta.family_id);
        let srt_data = super::srt::SRT_CP1252.to_vec();
        fs.add_file(&srt_name, "SRT", srt_data);
    }

    fs.sync()
}

/// Compute overview map ID from family_id
/// Convention: family_id * 10000 + 0xFFFF-like high number
/// Must fit in 8 decimal digits for FAT filename
pub fn compute_overview_map_id(family_id: u16) -> u32 {
    // Use family_id * 10000 + 1855 (matches MapSetToolkit pattern)
    let base = (family_id as u32) * 10000;
    let id = base + 1855;
    // Ensure it fits in 8 decimal digits
    if id > 99999999 { 99999999 } else { id }
}

/// Build MPS entries. Si `overview_map_number` est fourni, insère un L-record additionnel
/// pour l'overview (parité mkgmap `osmmap.img` — cf. AC 7 tech-spec).
fn build_mps(tiles: &[TileSubfiles], meta: &GmapsuppMeta, overview_map_number: Option<&str>) -> Vec<u8> {
    let mut mps = MpsWriter::new();
    mps.codepage = meta.codepage;

    // One product entry for the whole map set
    mps.add_product(MpsProductEntry {
        product_id: meta.product_id,
        family_id: meta.family_id,
        family_name: meta.family_name.clone(),
    });

    // Overview map entry (optionnel, parité mkgmap)
    if let Some(ov_num) = overview_map_number {
        let ov_map_num: u32 = ov_num.parse().unwrap_or(0);
        let ov_desc = format!("{} overview", meta.family_name);
        mps.add_map(MpsMapEntry {
            product_id: meta.product_id,
            family_id: meta.family_id,
            map_number: ov_map_num,
            map_name: ov_desc.clone(),
            map_description: ov_desc,
            area_name: String::new(),
        });
    }

    // One map entry per tile
    for tile in tiles {
        let map_num: u32 = tile.map_number.parse().unwrap_or(0);
        let desc = if tile.description.is_empty() {
            meta.family_name.clone()
        } else {
            tile.description.clone()
        };
        mps.add_map(MpsMapEntry {
            product_id: meta.product_id,
            family_id: meta.family_id,
            map_number: map_num,
            map_name: desc.clone(),
            map_description: desc,
            area_name: meta.area_name.clone(),
        });
    }

    mps.build()
}

/// Legacy wrapper: assemble from pre-built single-tile IMG bytes
/// Extracts subfiles from each tile IMG by parsing its directory
pub fn build_gmapsupp_from_imgs(
    tiles: &[(String, Vec<u8>)],
    description: &str,
) -> Result<Vec<u8>, ImgError> {
    if tiles.is_empty() {
        return Err(ImgError::InvalidFormat("No tiles to assemble".into()));
    }

    let mut subfiles = Vec::new();
    for (map_number, img_data) in tiles {
        // Extract subfiles from the tile IMG
        let extracted = extract_subfiles(img_data, map_number)?;
        subfiles.push(extracted);
    }

    build_gmapsupp(&subfiles, description)
}

/// Extract TRE/RGN/LBL/NET/NOD subfiles from a single-tile IMG
fn extract_subfiles(img_data: &[u8], map_number: &str) -> Result<TileSubfiles, ImgError> {
    // Parse the IMG directory to find subfile locations
    // Directory starts at block 2 (offset 1024 for 512-byte blocks)
    if img_data.len() < 2048 {
        return Err(ImgError::InvalidFormat("IMG too small to contain directory".into()));
    }

    let block_exp1 = img_data[0x61] as u32;
    let block_exp2 = img_data[0x62] as u32;
    if block_exp1 + block_exp2 > 24 {
        return Err(ImgError::InvalidFormat(format!(
            "Invalid block size exponents: {} + {} > 24", block_exp1, block_exp2
        )));
    }
    let block_size = 1u32 << (block_exp1 + block_exp2);
    let dir_start = 2 * 512; // directory always starts at 512-byte block 2

    let mut tre = Vec::new();
    let mut rgn = Vec::new();
    let mut lbl = Vec::new();
    let mut net = None;
    let mut nod = None;

    // Scan directory entries (each 512 bytes)
    let mut pos = dir_start;
    while pos + 512 <= img_data.len() {
        let entry = &img_data[pos..pos + 512];
        if entry[0] != 0x01 {
            pos += 512;
            continue; // unused entry
        }

        // Extract name and extension
        let name = std::str::from_utf8(&entry[1..9]).unwrap_or("").trim();
        let ext = std::str::from_utf8(&entry[9..12]).unwrap_or("").trim();

        // Size (only valid in part 0)
        let part = u16::from_le_bytes([entry[0x11], entry[0x12]]);
        if part != 0 {
            pos += 512;
            continue;
        }

        let size = u32::from_le_bytes([entry[0x0C], entry[0x0D], entry[0x0E], entry[0x0F]]) as usize;

        // Read block numbers from block table (starts at 0x20, 240 slots of u16)
        let mut file_data = Vec::with_capacity(size);
        for slot in 0..240 {
            let blk_off = 0x20 + slot * 2;
            let blk = u16::from_le_bytes([entry[blk_off], entry[blk_off + 1]]);
            if blk == 0xFFFF {
                break;
            }
            let data_start = blk as usize * block_size as usize;
            let data_end = (data_start + block_size as usize).min(img_data.len());
            if data_start < img_data.len() {
                let chunk_end = (file_data.len() + (data_end - data_start)).min(size);
                let take = chunk_end - file_data.len();
                if data_start + take <= img_data.len() {
                    file_data.extend_from_slice(&img_data[data_start..data_start + take]);
                }
            }
        }
        file_data.truncate(size);

        // Skip the special header entry (flag byte 0x03 at offset 0x10)
        let flag = entry[0x10];
        if flag == 0x03 || (name.is_empty() || (name.chars().all(|c| c == ' ' || c == '0') && ext.trim().is_empty())) {
            pos += 512;
            continue;
        }

        match ext {
            "TRE" => tre = file_data,
            "RGN" => rgn = file_data,
            "LBL" => lbl = file_data,
            "NET" => net = Some(file_data),
            "NOD" => nod = Some(file_data),
            _ => {}
        }

        pos += 512;
    }

    if tre.is_empty() || rgn.is_empty() || lbl.is_empty() {
        return Err(ImgError::InvalidFormat(format!(
            "Missing required subfiles (TRE/RGN/LBL) in tile {}", map_number
        )));
    }

    Ok(TileSubfiles {
        map_number: map_number.to_string(),
        description: String::new(),
        tre,
        rgn,
        lbl,
        net,
        nod,
        dem: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_gmapsupp_empty() {
        let result = build_gmapsupp(&[], "Test");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_gmapsupp_with_subfiles() {
        let tile = TileSubfiles {
            map_number: "63240001".to_string(),
            description: "Test".to_string(),
            tre: vec![0x01; 200],
            rgn: vec![0x02; 300],
            lbl: vec![0x03; 150],
            net: None,
            nod: None,
            dem: None,
        };
        let result = build_gmapsupp(&[tile], "Test Map");
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
    }

    #[test]
    fn test_typ_fid_pid_patched() {
        // Build a fake TYP with FID=1100 PID=1 at offsets 0x2F-0x32
        let mut fake_typ = vec![0u8; 256];
        fake_typ[0..12].copy_from_slice(b"\xae\x00GARMIN TYP");
        // Original FID=1100 (0x044C), PID=1
        fake_typ[0x2F] = 0x4C;
        fake_typ[0x30] = 0x04;
        fake_typ[0x31] = 0x01;
        fake_typ[0x32] = 0x00;

        let tile = TileSubfiles {
            map_number: "63240001".to_string(),
            description: "Test".to_string(),
            tre: vec![0x01; 200],
            rgn: vec![0x02; 300],
            lbl: vec![0x03; 150],
            net: None,
            nod: None,
            dem: None,
        };
        let meta = GmapsuppMeta {
            family_id: 6324,  // 0x18B4
            product_id: 2,
            family_name: "Test".to_string(),
            area_name: String::new(),
            codepage: 0,
            typ_basename: None,
            packaging: Packaging::Legacy,
        };
        let result = build_gmapsupp_with_meta_and_typ(&[tile], "Test", &meta, Some(&fake_typ));
        assert!(result.is_ok());
        let img = result.unwrap();

        // Find the TYP data in the output — search for GARMIN TYP signature
        let typ_pos = img.windows(10).position(|w| w == b"GARMIN TYP").unwrap();
        let typ_start = typ_pos - 2; // header starts 2 bytes before signature
        // FID should be patched to 6324 (0x18B4)
        let fid = u16::from_le_bytes([img[typ_start + 0x2F], img[typ_start + 0x30]]);
        let pid = u16::from_le_bytes([img[typ_start + 0x31], img[typ_start + 0x32]]);
        assert_eq!(fid, 6324, "TYP FID should be patched to match family_id");
        assert_eq!(pid, 2, "TYP PID should be patched to match product_id");
    }

    #[test]
    fn test_overview_map_id_computation() {
        assert_eq!(compute_overview_map_id(1100), 11001855);
        assert_eq!(compute_overview_map_id(26038), 99999999); // capped
        assert_eq!(compute_overview_map_id(1), 11855);
    }

    #[test]
    fn test_gmapsupp_no_overview() {
        let mut tre = vec![0u8; 200];
        // Set up minimal TRE bounds at offsets 21-32
        tre[0] = 188; tre[1] = 0;
        tre[2..12].copy_from_slice(b"GARMIN TRE");
        // Some non-zero bounds
        let n = 2143196i32; let e = 262632i32; let s = 2138930i32; let w = 255409i32;
        tre[21..24].copy_from_slice(&n.to_le_bytes()[..3]);
        tre[24..27].copy_from_slice(&e.to_le_bytes()[..3]);
        tre[27..30].copy_from_slice(&s.to_le_bytes()[..3]);
        tre[30..33].copy_from_slice(&w.to_le_bytes()[..3]);

        let tile = TileSubfiles {
            map_number: "11000001".to_string(),
            description: "Test".to_string(),
            tre,
            rgn: vec![0x02; 300],
            lbl: vec![0x03; 150],
            net: None,
            nod: None,
            dem: None,
        };
        let meta = GmapsuppMeta {
            family_id: 1100,
            product_id: 1,
            family_name: "Test".to_string(),
            area_name: String::new(),
            codepage: 1252,
            typ_basename: None,
            packaging: Packaging::Legacy,
        };
        let result = build_gmapsupp_with_meta_and_typ(&[tile], "Test", &meta, None);
        assert!(result.is_ok());
        let img = result.unwrap();
        // Overview map marker 0x00040101 should NOT be present (causes Alpha 100 issues)
        let overview_marker = 0x00040101u32.to_le_bytes();
        let has_overview = img.windows(4).any(|w| w == overview_marker);
        assert!(!has_overview, "gmapsupp should NOT contain overview map");
    }
}
