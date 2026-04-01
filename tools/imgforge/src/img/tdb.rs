// TDB — Table of Database companion file, faithful to mkgmap TdbFile.java

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
        data.extend_from_slice(self.series_name.as_bytes());
        data.push(0);

        // Family name (null-terminated)
        data.extend_from_slice(self.family_name.as_bytes());
        data.push(0);

        // Area name (null-terminated)
        data.extend_from_slice(self.area_name.as_bytes());
        data.push(0);

        write_block(buf, BLOCK_PRODUCT, &data);
    }

    fn write_copyright_block(&self, buf: &mut Vec<u8>) {
        let mut data = Vec::new();
        data.extend_from_slice(self.copyright.as_bytes());
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
        data.extend_from_slice(tile.description.as_bytes());
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
}
