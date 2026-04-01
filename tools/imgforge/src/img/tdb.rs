// TDB — Table of Database companion file, faithful to mkgmap TdbFile.java

use super::labelenc::format9;

/// TDB block types
const BLOCK_PRODUCT: u8 = 0x50;
const BLOCK_COPYRIGHT: u8 = 0x44;
const BLOCK_OVERVIEW: u8 = 0x42;
const BLOCK_DETAIL: u8 = 0x4C;

/// TDB file writer
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
}

/// A tile entry in the TDB
pub struct TdbTile {
    pub map_number: u32,
    pub description: String,
    pub north: i32,
    pub south: i32,
    pub east: i32,
    pub west: i32,
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

        // Product info block (0x50)
        self.write_product_block(&mut buf);

        // Copyright block (0x44)
        if !self.copyright.is_empty() {
            self.write_copyright_block(&mut buf);
        }

        // Overview map block (0x42)
        self.write_overview_block(&mut buf);

        // Detail map blocks (0x4C)
        for tile in &self.tiles {
            self.write_detail_block(&mut buf, tile);
        }

        buf
    }

    fn write_product_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.family_id.to_le_bytes());
        data.extend_from_slice(&self.product_id.to_le_bytes());
        data.extend_from_slice(&self.product_version.to_le_bytes());

        // Series name (null-terminated)
        data.extend_from_slice(&self.encode_str(&self.series_name));
        data.push(0);

        // Family name (null-terminated)
        data.extend_from_slice(&self.encode_str(&self.family_name));
        data.push(0);

        // Area name (null-terminated)
        data.extend_from_slice(&self.encode_str(&self.area_name));
        data.push(0);

        write_block(buf, BLOCK_PRODUCT, &data);
    }

    fn write_copyright_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.encode_str(&self.copyright));
        data.push(0);
        write_block(buf, BLOCK_COPYRIGHT, &data);
    }

    fn write_overview_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(&self.overview_map_number.to_le_bytes());
        write_block(buf, BLOCK_OVERVIEW, &data);
    }

    fn write_detail_block(&self, buf: &mut Vec<u8>, tile: &TdbTile) {
        let mut data = Vec::new();
        data.extend_from_slice(&tile.map_number.to_le_bytes());

        // Bounds (4 × i32 LE)
        data.extend_from_slice(&tile.north.to_le_bytes());
        data.extend_from_slice(&tile.east.to_le_bytes());
        data.extend_from_slice(&tile.south.to_le_bytes());
        data.extend_from_slice(&tile.west.to_le_bytes());

        // Description (null-terminated)
        data.extend_from_slice(&self.encode_str(&tile.description));
        data.push(0);

        write_block(buf, BLOCK_DETAIL, &data);
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
            description: "Tile 1".to_string(),
            north: 100,
            south: 0,
            east: 100,
            west: 0,
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
        let data = tdb.build();
        // "Région" in CP1252: R(52) é(e9) g(67) i(69) o(6f) n(6e)
        // Should NOT contain UTF-8 sequence c3 a9
        assert!(!data.windows(2).any(|w| w == [0xc3, 0xa9]),
            "TDB should not contain UTF-8 é (c3 a9) when codepage is 1252");
        // Should contain CP1252 é = 0xe9
        assert!(data.contains(&0xe9),
            "TDB should contain CP1252 é (0xe9)");
    }
}
