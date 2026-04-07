// TDB — Table of Database companion file, faithful to mkgmap TdbFile.java

use super::labelenc::format9;

/// TDB block types
const BLOCK_PRODUCT: u8 = 0x50;
const BLOCK_COPYRIGHT: u8 = 0x44;
const BLOCK_OVERVIEW: u8 = 0x42;
const BLOCK_DETAIL: u8 = 0x4C;

/// TDB version 4.07 (mkgmap default)
const TDB_V407: u16 = 407;

/// TDB file writer — faithful to mkgmap HeaderBlock / OverviewMapBlock / DetailMapBlock
pub struct TdbWriter {
    pub family_id: u16,
    pub product_id: u16,
    pub product_version: u16,
    pub series_name: String,
    pub family_name: String,
    pub area_name: String,
    pub copyright: String,
    pub country_name: String,
    pub country_abbr: String,
    pub region_name: String,
    pub region_abbr: String,
    pub overview_map_number: u32,
    pub tiles: Vec<TdbTile>,
    pub codepage: u16,
    /// Enable profile/elevation display (set to true when DEM is present)
    pub enable_profile: bool,
}

/// A tile entry in the TDB
pub struct TdbTile {
    pub map_number: u32,
    pub parent_map_number: u32,
    pub description: String,
    /// Bounds in 24-bit map units (as stored in TRE header).
    /// Will be shifted << 8 when written to TDB (32-bit Garmin units).
    pub north: i32,
    pub south: i32,
    pub east: i32,
    pub west: i32,
    /// Subfile names and sizes for the detail block
    pub subfiles: Vec<(String, u32)>,
}

impl TdbWriter {
    pub fn new(family_id: u16, product_id: u16) -> Self {
        Self {
            family_id,
            product_id,
            product_version: 100,
            series_name: String::new(),
            family_name: String::new(),
            area_name: String::new(),
            copyright: String::new(),
            country_name: String::new(),
            country_abbr: String::new(),
            region_name: String::new(),
            region_abbr: String::new(),
            overview_map_number: 0,
            tiles: Vec::new(),
            codepage: 0,
            enable_profile: false,
        }
    }

    /// Encode string in the map's codepage (for accented metadata)
    fn encode_str(&self, s: &str) -> Vec<u8> {
        if self.codepage > 0 && self.codepage != 65001 {
            let encoded = format9::encode(s, self.codepage);
            // Remove trailing null — caller adds it
            encoded[..encoded.len() - 1].to_vec()
        } else {
            s.as_bytes().to_vec()
        }
    }

    pub fn add_tile(&mut self, tile: TdbTile) {
        self.tiles.push(tile);
    }

    /// Build the complete TDB file
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Product info block (0x50) — mkgmap HeaderBlock
        self.write_product_block(&mut buf);

        // Copyright block (0x44)
        if !self.copyright.is_empty() {
            self.write_copyright_block(&mut buf);
        }

        // Overview map block (0x42) — mkgmap OverviewMapBlock
        self.write_overview_block(&mut buf);

        // Detail map blocks (0x4C) — mkgmap DetailMapBlock
        for tile in &self.tiles {
            self.write_detail_block(&mut buf, tile);
        }

        buf
    }

    /// Product block — faithful to mkgmap HeaderBlock.writeBody
    /// Format: productId(2) familyId(2) tdbVersion(2) seriesName\0 productVersion(2) familyName\0 [v407 extra fields]
    fn write_product_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.product_id.to_le_bytes());
        data.extend_from_slice(&self.family_id.to_le_bytes());
        data.extend_from_slice(&TDB_V407.to_le_bytes());

        // Series name (null-terminated)
        data.extend_from_slice(&self.encode_str(&self.series_name));
        data.push(0);

        data.extend_from_slice(&self.product_version.to_le_bytes());

        // Family name (null-terminated)
        data.extend_from_slice(&self.encode_str(&self.family_name));
        data.push(0);

        // v407 extra fields — faithful to mkgmap HeaderBlock.writeBody
        data.push(0x00);       // reserved
        data.push(0x12);       // lowest map level (18)
        data.push(0x01);       // reserved
        data.push(0x01);       // reserved
        data.push(0x01);       // reserved
        data.extend_from_slice(&0u32.to_le_bytes()); // reserved
        data.push(0x00);       // reserved
        data.push(0x18);       // highest routable level (24)
        data.extend_from_slice(&0u32.to_le_bytes()); // reserved
        data.extend_from_slice(&0u32.to_le_bytes()); // reserved
        data.extend_from_slice(&0u32.to_le_bytes()); // reserved
        data.extend_from_slice(&0u32.to_le_bytes()); // reserved
        // 3 bytes reserved
        data.push(0); data.push(0); data.push(0);
        data.extend_from_slice(&(self.codepage as u32).to_le_bytes()); // codePage
        data.extend_from_slice(&10000u32.to_le_bytes()); // constant
        data.push(0x01);       // map is routable
        data.push(if self.enable_profile { 0x01 } else { 0x00 }); // profile/elevation
        data.push(0x00);       // reserved

        write_block(buf, BLOCK_PRODUCT, &data);
    }

    fn write_copyright_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.encode_str(&self.copyright));
        data.push(0);
        write_block(buf, BLOCK_COPYRIGHT, &data);
    }

    /// Overview block — faithful to mkgmap OverviewMapBlock.writeBody
    /// Format: mapNumber(4) parentMapNumber(4) maxLat(4) maxLong(4) minLat(4) minLong(4) description\0
    fn write_overview_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.overview_map_number.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // parentMapNumber = 0 for overview

        // Compute overall bounds from all tiles (shifted to 32-bit)
        let (north, east, south, west) = self.compute_overall_bounds();
        data.extend_from_slice(&north.to_le_bytes());
        data.extend_from_slice(&east.to_le_bytes());
        data.extend_from_slice(&south.to_le_bytes());
        data.extend_from_slice(&west.to_le_bytes());

        // Description
        data.extend_from_slice(&self.encode_str(&self.series_name));
        data.push(0);

        write_block(buf, BLOCK_OVERVIEW, &data);
    }

    /// Detail block — faithful to mkgmap DetailMapBlock.writeBody
    /// Format: [OverviewMapBlock fields] n+1(2) n(2) sizes[n](4*n) magic(4) pad(3) names[n]\0
    fn write_detail_block(&self, buf: &mut Vec<u8>, tile: &TdbTile) {
        let mut data = Vec::new();

        // OverviewMapBlock base fields
        data.extend_from_slice(&tile.map_number.to_le_bytes());
        data.extend_from_slice(&tile.parent_map_number.to_le_bytes());

        // Bounds shifted to 32-bit Garmin units (<< 8)
        let north = (tile.north) << 8;
        let east = (tile.east) << 8;
        let south = (tile.south) << 8;
        let west = (tile.west) << 8;
        data.extend_from_slice(&north.to_le_bytes());
        data.extend_from_slice(&east.to_le_bytes());
        data.extend_from_slice(&south.to_le_bytes());
        data.extend_from_slice(&west.to_le_bytes());

        // Description (null-terminated)
        data.extend_from_slice(&self.encode_str(&tile.description));
        data.push(0);

        // Subfile info — mkgmap DetailMapBlock
        let n = tile.subfiles.len();
        data.extend_from_slice(&((n + 1) as u16).to_le_bytes());
        data.extend_from_slice(&(n as u16).to_le_bytes());

        // Sizes
        for (_name, size) in &tile.subfiles {
            data.extend_from_slice(&size.to_le_bytes());
        }

        // Magic constant (0xff00c301) + 3 padding bytes
        data.extend_from_slice(&0xff00c301u32.to_le_bytes());
        data.push(0);
        data.push(0);
        data.push(0);

        // Subfile names (null-terminated)
        for (name, _size) in &tile.subfiles {
            data.extend_from_slice(name.as_bytes());
            data.push(0);
        }

        write_block(buf, BLOCK_DETAIL, &data);
    }

    /// Compute overall bounds across all tiles (already shifted to 32-bit)
    fn compute_overall_bounds(&self) -> (i32, i32, i32, i32) {
        if self.tiles.is_empty() {
            return (0, 0, 0, 0);
        }
        let mut max_lat = i32::MIN;
        let mut max_lon = i32::MIN;
        let mut min_lat = i32::MAX;
        let mut min_lon = i32::MAX;

        for tile in &self.tiles {
            let n = tile.north << 8;
            let e = tile.east << 8;
            let s = tile.south << 8;
            let w = tile.west << 8;
            if n > max_lat { max_lat = n; }
            if e > max_lon { max_lon = e; }
            if s < min_lat { min_lat = s; }
            if w < min_lon { min_lon = w; }
        }
        (max_lat, max_lon, min_lat, min_lon)
    }
}

fn write_block(buf: &mut Vec<u8>, block_type: u8, data: &[u8]) {
    buf.push(block_type);
    let len = data.len() as u16;
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tdb_basic() {
        let mut tdb = TdbWriter::new(1, 1);
        tdb.series_name = "Test".to_string();
        tdb.family_name = "Test Family".to_string();
        tdb.add_tile(TdbTile {
            map_number: 63240001,
            parent_map_number: 0,
            description: "Tile 1".to_string(),
            north: 100,
            south: 0,
            east: 100,
            west: 0,
            subfiles: vec![],
        });
        let data = tdb.build();
        assert!(!data.is_empty());
        // First block should be product (0x50)
        assert_eq!(data[0], BLOCK_PRODUCT);
    }

    #[test]
    fn test_tdb_with_copyright() {
        let mut tdb = TdbWriter::new(1, 1);
        tdb.copyright = "Copyright Test".to_string();
        let data = tdb.build();
        assert!(data.len() > 10);
    }

    #[test]
    fn test_tdb_codepage_encoding() {
        let mut tdb = TdbWriter::new(1, 1);
        tdb.codepage = 1252;
        tdb.area_name = "Région ARA".to_string();
        tdb.series_name = "Région ARA".to_string();
        let data = tdb.build();
        // "Région" in CP1252: R(52) é(e9) g(67) i(69) o(6f) n(6e)
        // Should NOT contain UTF-8 sequence c3 a9
        assert!(!data.windows(2).any(|w| w == [0xc3, 0xa9]),
            "TDB should not contain UTF-8 é (c3 a9) when codepage is 1252");
        // Should contain CP1252 é = 0xe9
        assert!(data.contains(&0xe9),
            "TDB should contain CP1252 é (0xe9)");
    }

    #[test]
    fn test_tdb_detail_block_has_bounds() {
        let mut tdb = TdbWriter::new(1, 1);
        tdb.series_name = "Test".to_string();
        tdb.family_name = "Test".to_string();
        tdb.add_tile(TdbTile {
            map_number: 1,
            parent_map_number: 0,
            description: "T".to_string(),
            north: 0x1FF558,
            south: 0x1F83AA,
            east: 0x023E36,
            west: 0x01E1CA,
            subfiles: vec![
                ("I00000001.TRE".to_string(), 1000),
                ("I00000001.RGN".to_string(), 2000),
            ],
        });
        let data = tdb.build();
        // Find the detail block (0x4C)
        let pos = data.iter().position(|&b| b == BLOCK_DETAIL).unwrap();
        let block_data = &data[pos + 3..]; // skip type + length
        // map_number
        assert_eq!(u32::from_le_bytes([block_data[0], block_data[1], block_data[2], block_data[3]]), 1);
        // parentMapNumber
        assert_eq!(u32::from_le_bytes([block_data[4], block_data[5], block_data[6], block_data[7]]), 0);
        // north = 0x1FF558 << 8 = 0x1FF55800
        let north = i32::from_le_bytes([block_data[8], block_data[9], block_data[10], block_data[11]]);
        assert_eq!(north, 0x1FF558i32 << 8);
    }
}
