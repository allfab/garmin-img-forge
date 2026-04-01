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
//
// Block order (mkgmap convention): Map(0x4C) first, then Product(0x46), then Version(0x56)

use super::labelenc::format9;

/// MPS file writer
pub struct MpsWriter {
    pub entries: Vec<MpsMapEntry>,
    pub products: Vec<MpsProductEntry>,
    pub codepage: u16,
}

/// Per-tile map entry (block type 0x4C)
///
/// mkgmap format:
///   PID(u16) + FID(u16) + MapNum(u32)
///   + MapName\0 + MapDescription\0 + AreaName\0
///   + MapNum(u32, repeated) + padding(4 bytes)
pub struct MpsMapEntry {
    pub product_id: u16,
    pub family_id: u16,
    pub map_number: u32,
    pub map_name: String,
    pub map_description: String,
    pub area_name: String,
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
            codepage: 0,
        }
    }

    pub fn add_map(&mut self, entry: MpsMapEntry) {
        self.entries.push(entry);
    }

    pub fn add_product(&mut self, entry: MpsProductEntry) {
        self.products.push(entry);
    }

    /// Encode string in codepage (for metadata strings with accented chars)
    fn encode_str(&self, s: &str) -> Vec<u8> {
        if self.codepage > 0 && self.codepage != 65001 {
            let encoded = format9::encode(s, self.codepage);
            // Remove trailing null — caller adds it
            encoded[..encoded.len() - 1].to_vec()
        } else {
            s.as_bytes().to_vec()
        }
    }

    /// Build the complete MPS file bytes
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // mkgmap order: Map entries (0x4C) first
        for entry in &self.entries {
            let mut data = Vec::new();
            data.extend_from_slice(&entry.product_id.to_le_bytes());
            data.extend_from_slice(&entry.family_id.to_le_bytes());
            data.extend_from_slice(&entry.map_number.to_le_bytes());
            // Map name (null-terminated)
            data.extend_from_slice(&self.encode_str(&entry.map_name));
            data.push(0x00);
            // Map description (null-terminated)
            data.extend_from_slice(&self.encode_str(&entry.map_description));
            data.push(0x00);
            // Area name (null-terminated)
            data.extend_from_slice(&self.encode_str(&entry.area_name));
            data.push(0x00);
            // Map number repeated (u32 LE)
            data.extend_from_slice(&entry.map_number.to_le_bytes());
            // Padding (4 zero bytes)
            data.extend_from_slice(&[0x00; 4]);
            write_block(&mut buf, 0x4C, &data);
        }

        // Then product/family blocks (0x46)
        for product in &self.products {
            let mut data = Vec::new();
            data.extend_from_slice(&product.product_id.to_le_bytes());
            data.extend_from_slice(&product.family_id.to_le_bytes());
            // Family name (null-terminated)
            data.extend_from_slice(&self.encode_str(&product.family_name));
            data.push(0x00);
            write_block(&mut buf, 0x46, &data);
        }

        // Version block (0x56)
        {
            let mut data = Vec::new();
            data.extend_from_slice(b"imgforge map set");
            data.push(0x00);
            data.push(0x00); // version number
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
            map_name: "Test Tile 1".to_string(),
            map_description: "Test Tile 1".to_string(),
            area_name: String::new(),
        });
        let data = mps.build();
        assert!(!data.is_empty());
        // First block should be map entry (0x4C) — mkgmap order
        assert_eq!(data[0], 0x4C);
    }

    #[test]
    fn test_mps_map_entry_format() {
        let mut mps = MpsWriter::new();
        mps.add_map(MpsMapEntry {
            product_id: 1,
            family_id: 1038,
            map_number: 380001,
            map_name: "Tile 1".to_string(),
            map_description: "Tile 1".to_string(),
            area_name: "France".to_string(),
        });
        let data = mps.build();
        // Block type 0x4C
        assert_eq!(data[0], 0x4C);
        let len = u16::from_le_bytes([data[1], data[2]]) as usize;
        let block = &data[3..3 + len];
        // PID + FID + MapNum = 8 bytes
        let map_num = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
        assert_eq!(map_num, 380001);
        // Repeated map number should be near the end
        let repeated = u32::from_le_bytes([
            block[len - 8],
            block[len - 7],
            block[len - 6],
            block[len - 5],
        ]);
        assert_eq!(repeated, 380001);
        // Last 4 bytes = padding zeros
        assert_eq!(&block[len - 4..], &[0, 0, 0, 0]);
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
                map_name: format!("Tile {}", i),
                map_description: format!("Tile {}", i),
                area_name: String::new(),
            });
        }
        let data = mps.build();
        // Should contain 3 map blocks + 1 product block + 1 version block = 5 blocks
        let mut block_count = 0;
        let mut pos = 0;
        while pos < data.len() {
            let _block_type = data[pos];
            let block_len = u16::from_le_bytes([data[pos + 1], data[pos + 2]]) as usize;
            pos += 3 + block_len;
            block_count += 1;
        }
        assert_eq!(block_count, 5);
        // First 3 blocks should be map entries (0x4C)
        let mut pos = 0;
        for _ in 0..3 {
            assert_eq!(data[pos], 0x4C);
            let block_len = u16::from_le_bytes([data[pos + 1], data[pos + 2]]) as usize;
            pos += 3 + block_len;
        }
        // 4th block should be product (0x46)
        assert_eq!(data[pos], 0x46);
    }

    #[test]
    fn test_mps_codepage_encoding() {
        let mut mps = MpsWriter::new();
        mps.codepage = 1252;
        mps.add_map(MpsMapEntry {
            product_id: 1,
            family_id: 1,
            map_number: 1,
            map_name: "Région".to_string(),
            map_description: "Région".to_string(),
            area_name: String::new(),
        });
        let data = mps.build();
        // "Région" in CP1252: R(52) é(e9) g(67) i(69) o(6f) n(6e)
        // Should NOT contain UTF-8 sequence c3 a9
        let block_data = &data[3..]; // skip block header
        assert!(!block_data.windows(2).any(|w| w == [0xc3, 0xa9]),
            "MPS should not contain UTF-8 é (c3 a9) when codepage is 1252");
    }
}
