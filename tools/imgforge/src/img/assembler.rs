// GmapsuppBuilder — multi-tile assembler, faithful to mkgmap GmapsuppBuilder.java

use crate::error::ImgError;
use super::filesystem::ImgFilesystem;
use super::mps::{MpsWriter, MpsMapEntry, MpsProductEntry};

/// Subfiles for a single tile
pub struct TileSubfiles {
    pub map_number: String,
    pub description: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
    pub net: Option<Vec<u8>>,
    pub nod: Option<Vec<u8>>,
}

/// Metadata for the gmapsupp assembly
pub struct GmapsuppMeta {
    pub family_id: u16,
    pub product_id: u16,
    pub family_name: String,
    pub area_name: String,
    pub codepage: u16,
}

impl Default for GmapsuppMeta {
    fn default() -> Self {
        Self {
            family_id: 1,
            product_id: 1,
            family_name: "Map".to_string(),
            area_name: String::new(),
            codepage: 0,
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
    if tiles.is_empty() {
        return Err(ImgError::InvalidFormat("No tiles to assemble".into()));
    }

    let mut fs = ImgFilesystem::new(description);

    for tile in tiles {
        let name = format!("{:>08}", tile.map_number);
        fs.add_file(&name, "TRE", tile.tre.clone());
        fs.add_file(&name, "RGN", tile.rgn.clone());
        fs.add_file(&name, "LBL", tile.lbl.clone());
        if let Some(ref net) = tile.net {
            fs.add_file(&name, "NET", net.clone());
        }
        if let Some(ref nod) = tile.nod {
            fs.add_file(&name, "NOD", nod.clone());
        }
    }

    // Add TYP file if provided (mkgmap convention: family_id as filename)
    if let Some(typ) = typ_data {
        let typ_name = format!("{:08}", meta.family_id);
        fs.add_file(&typ_name, "TYP", typ.to_vec());
    }

    // Build and add MPS subfile
    let mps_data = build_mps(tiles, meta);
    // MPS file uses "MAKEGMAP" as filename (mkgmap convention)
    fs.add_file("MAKEGMAP", "MPS", mps_data);

    fs.sync()
}

/// Build MPS subfile data from tiles and metadata
fn build_mps(tiles: &[TileSubfiles], meta: &GmapsuppMeta) -> Vec<u8> {
    let mut mps = MpsWriter::new();
    mps.codepage = meta.codepage;

    // One product entry for the whole map set
    mps.add_product(MpsProductEntry {
        product_id: meta.product_id,
        family_id: meta.family_id,
        family_name: meta.family_name.clone(),
    });

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
        };
        let result = build_gmapsupp(&[tile], "Test Map");
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
    }
}
