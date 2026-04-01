// MPS — Map Product Set subfile, faithful to mkgmap MpsFile.java
//
// The MPS file stores per-tile metadata (family_id, product_id, codepage,
// description) inside a gmapsupp.img. This is what tools like gmt read
// to display the PID, FID, CP, and name columns.
//
// Format: sequence of typed blocks, each with:
//   - type (1 byte): 0x4C = map entry, 0x46 = product entry, 0x56 = version
//   - length (2 bytes, LE): data length
//   - data (variable)

/// MPS file writer
pub struct MpsWriter {
    pub entries: Vec<MpsMapEntry>,
    pub products: Vec<MpsProductEntry>,
}

/// Per-tile map entry (block type 0x4C)
pub struct MpsMapEntry {
    pub product_id: u16,
    pub family_id: u16,
    pub map_number: u32,
    pub hex_number: String,
    pub map_description: String,
}

/// Product/family entry (block type 0x46)
pub struct MpsProductEntry {
    pub product_id: u16,
    pub family_id: u16,
    pub family_name: String,
}

impl MpsWriter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            products: Vec::new(),
        }
    }

    pub fn add_map(&mut self, entry: MpsMapEntry) {
        self.entries.push(entry);
    }

    pub fn add_product(&mut self, entry: MpsProductEntry) {
        self.products.push(entry);
    }

    /// Build the complete MPS file bytes
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Write product/family blocks (0x46) first
        for product in &self.products {
            let mut data = Vec::new();
            data.extend_from_slice(&product.product_id.to_le_bytes());
            data.extend_from_slice(&product.family_id.to_le_bytes());
            // Family name (null-terminated)
            data.extend_from_slice(product.family_name.as_bytes());
            data.push(0x00);
            write_block(&mut buf, 0x46, &data);
        }

        // Write per-map blocks (0x4C)
        for entry in &self.entries {
            let mut data = Vec::new();
            data.extend_from_slice(&entry.product_id.to_le_bytes());
            data.extend_from_slice(&entry.family_id.to_le_bytes());
            data.extend_from_slice(&entry.map_number.to_le_bytes());
            // Hex number string (null-terminated)
            data.extend_from_slice(entry.hex_number.as_bytes());
            data.push(0x00);
            // Map description (null-terminated)
            data.extend_from_slice(entry.map_description.as_bytes());
            data.push(0x00);
            // Area names: 2 empty null-terminated strings (region, country)
            data.push(0x00); // region
            data.push(0x00); // country
            write_block(&mut buf, 0x4C, &data);
        }

        // Write version block (0x56) — "OSM map set" equivalent
        {
            let mut data = Vec::new();
            // Version string (null-terminated)
            data.extend_from_slice(b"imgforge map set");
            data.push(0x00);
            // Version number
            data.push(0x00);
            write_block(&mut buf, 0x56, &data);
        }

        buf
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
    fn test_mps_basic() {
        let mut mps = MpsWriter::new();
        mps.add_product(MpsProductEntry {
            product_id: 1,
            family_id: 53403,
            family_name: "Test Map".to_string(),
        });
        mps.add_map(MpsMapEntry {
            product_id: 1,
            family_id: 53403,
            map_number: 63240001,
            hex_number: "63240001".to_string(),
            map_description: "Test Tile 1".to_string(),
        });
        let data = mps.build();
        assert!(!data.is_empty());
        // First block should be product (0x46)
        assert_eq!(data[0], 0x46);
    }

    #[test]
    fn test_mps_multiple_tiles() {
        let mut mps = MpsWriter::new();
        mps.add_product(MpsProductEntry {
            product_id: 1,
            family_id: 1,
            family_name: "Map".to_string(),
        });
        for i in 1..=3 {
            mps.add_map(MpsMapEntry {
                product_id: 1,
                family_id: 1,
                map_number: 63240000 + i,
                hex_number: format!("{:08}", 63240000 + i),
                map_description: format!("Tile {}", i),
            });
        }
        let data = mps.build();
        // Should contain 1 product block + 3 map blocks + 1 version block = 5 blocks
        let mut block_count = 0;
        let mut pos = 0;
        while pos < data.len() {
            let _block_type = data[pos];
            let block_len = u16::from_le_bytes([data[pos + 1], data[pos + 2]]) as usize;
            pos += 3 + block_len;
            block_count += 1;
        }
        assert_eq!(block_count, 5);
    }
}
