// Directory + Dirent — 512B directory entries with 240 block slots
// Faithful to mkgmap Dirent.java + Directory.java

/// Size of each directory entry in bytes
pub const ENTRY_SIZE: usize = 512;
/// Maximum number of block pointers per directory entry
pub const TABLE_SIZE: usize = 240;
/// Block table starts at offset 0x20 within each entry
const BLOCKS_TABLE_START: usize = 0x20;
const MAX_FILE_LEN: usize = 8;
const MAX_EXT_LEN: usize = 3;

// Field offsets within a directory entry
const OFF_FILE_USED: usize = 0x00;
const OFF_NAME: usize = 0x01;
const OFF_EXT: usize = 0x09;
const OFF_SIZE: usize = 0x0C;
const OFF_FLAG: usize = 0x10;
const OFF_FILE_PART: usize = 0x11;

/// A single file entry in the IMG directory
pub struct Dirent {
    pub name: String,
    pub ext: String,
    pub size: u32,
    pub blocks: Vec<u16>,
    pub special: bool,
}

impl Dirent {
    pub fn new(name: &str, ext: &str) -> Self {
        Self {
            name: pad_or_truncate(name, MAX_FILE_LEN, b'0', true),
            ext: ext[..MAX_EXT_LEN.min(ext.len())].to_string(),
            size: 0,
            blocks: Vec::new(),
            special: false,
        }
    }

    pub fn add_block(&mut self, block: u16) {
        self.blocks.push(block);
    }

    /// Number of 512-byte directory entries needed for this file
    pub fn num_parts(&self) -> usize {
        let n = self.blocks.len();
        if n == 0 {
            1
        } else {
            (n + TABLE_SIZE - 1) / TABLE_SIZE
        }
    }

    /// Write all directory entry parts (each 512 bytes) — mkgmap Dirent.sync
    pub fn write(&self) -> Vec<u8> {
        let n_parts = self.num_parts();
        let mut buf = vec![0u8; ENTRY_SIZE * n_parts];

        for part in 0..n_parts {
            let base = part * ENTRY_SIZE;

            // File used flag
            buf[base + OFF_FILE_USED] = 0x01;

            // Name (8 bytes, space-padded)
            let name_bytes = self.name.as_bytes();
            for i in 0..MAX_FILE_LEN {
                buf[base + OFF_NAME + i] = if i < name_bytes.len() {
                    name_bytes[i]
                } else {
                    b' '
                };
            }

            // Extension (3 bytes)
            let ext_bytes = self.ext.as_bytes();
            for i in 0..MAX_EXT_LEN {
                buf[base + OFF_EXT + i] = if i < ext_bytes.len() {
                    ext_bytes[i]
                } else {
                    b' '
                };
            }

            // Size only in first part
            if part == 0 {
                let sb = self.size.to_le_bytes();
                buf[base + OFF_SIZE..base + OFF_SIZE + 4].copy_from_slice(&sb);
            }

            // Flag
            buf[base + OFF_FLAG] = if self.special { 0x03 } else { 0x00 };

            // Part number (u16 LE, mkgmap writes as putChar = unsigned 16-bit)
            let part_bytes = (part as u16).to_le_bytes();
            buf[base + OFF_FILE_PART] = part_bytes[0];
            buf[base + OFF_FILE_PART + 1] = part_bytes[1];

            // Block table for this part
            let block_start = part * TABLE_SIZE;
            let block_end = (block_start + TABLE_SIZE).min(self.blocks.len());
            for (i, &blk) in self.blocks[block_start..block_end].iter().enumerate() {
                let off = base + BLOCKS_TABLE_START + i * 2;
                let b = blk.to_le_bytes();
                buf[off] = b[0];
                buf[off + 1] = b[1];
            }
            // Fill remaining slots with 0xFFFF
            for i in (block_end - block_start)..TABLE_SIZE {
                let off = base + BLOCKS_TABLE_START + i * 2;
                buf[off] = 0xFF;
                buf[off + 1] = 0xFF;
            }
        }

        buf
    }
}

/// The directory containing all file entries
pub struct Directory {
    pub entries: Vec<Dirent>,
}

impl Directory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: Dirent) {
        self.entries.push(entry);
    }

    /// Total number of 512-byte blocks used by the directory
    pub fn total_directory_blocks(&self) -> usize {
        self.entries.iter().map(|e| e.num_parts()).sum()
    }

    /// Write all directory entries
    pub fn write(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for entry in &self.entries {
            buf.extend_from_slice(&entry.write());
        }
        buf
    }
}

/// Pad or truncate a string to the given length
fn pad_or_truncate(s: &str, len: usize, pad_char: u8, pad_left: bool) -> String {
    if s.len() > len {
        s[..len].to_string()
    } else if s.len() < len {
        let padding = std::iter::repeat(pad_char as char)
            .take(len - s.len())
            .collect::<String>();
        if pad_left {
            format!("{}{}", padding, s)
        } else {
            format!("{}{}", s, padding)
        }
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirent_size_512() {
        let d = Dirent::new("63240001", "TRE");
        let buf = d.write();
        assert_eq!(buf.len(), ENTRY_SIZE);
    }

    #[test]
    fn test_dirent_used_flag() {
        let d = Dirent::new("63240001", "TRE");
        let buf = d.write();
        assert_eq!(buf[0], 0x01);
    }

    #[test]
    fn test_dirent_name_ext() {
        let d = Dirent::new("63240001", "TRE");
        let buf = d.write();
        assert_eq!(&buf[OFF_NAME..OFF_NAME + 8], b"63240001");
        assert_eq!(&buf[OFF_EXT..OFF_EXT + 3], b"TRE");
    }

    #[test]
    fn test_dirent_name_padding() {
        let d = Dirent::new("1234", "RGN");
        let buf = d.write();
        // Left-padded with '0' to 8 chars
        assert_eq!(&buf[OFF_NAME..OFF_NAME + 8], b"00001234");
    }

    #[test]
    fn test_dirent_blocks() {
        let mut d = Dirent::new("63240001", "TRE");
        d.add_block(5);
        d.add_block(6);
        d.add_block(7);
        d.size = 1024;
        let buf = d.write();

        // Size in first part
        let size = u32::from_le_bytes([buf[OFF_SIZE], buf[OFF_SIZE+1], buf[OFF_SIZE+2], buf[OFF_SIZE+3]]);
        assert_eq!(size, 1024);

        // Block 0
        let blk0 = u16::from_le_bytes([buf[BLOCKS_TABLE_START], buf[BLOCKS_TABLE_START+1]]);
        assert_eq!(blk0, 5);
    }

    #[test]
    fn test_dirent_multi_part() {
        let mut d = Dirent::new("63240001", "TRE");
        // Add 241 blocks → needs 2 parts
        for i in 0..241 {
            d.add_block(i);
        }
        d.size = 241 * 512;
        assert_eq!(d.num_parts(), 2);
        let buf = d.write();
        assert_eq!(buf.len(), ENTRY_SIZE * 2);

        // Second part: part number = 1
        let part_num = u16::from_le_bytes([buf[ENTRY_SIZE + OFF_FILE_PART], buf[ENTRY_SIZE + OFF_FILE_PART + 1]]);
        assert_eq!(part_num, 1);

        // Second part: size should be 0
        let size2 = u32::from_le_bytes([
            buf[ENTRY_SIZE + OFF_SIZE],
            buf[ENTRY_SIZE + OFF_SIZE + 1],
            buf[ENTRY_SIZE + OFF_SIZE + 2],
            buf[ENTRY_SIZE + OFF_SIZE + 3],
        ]);
        assert_eq!(size2, 0);
    }

    #[test]
    fn test_unused_slots_ffff() {
        let mut d = Dirent::new("63240001", "TRE");
        d.add_block(10);
        let buf = d.write();
        // Slot 1 should be 0xFFFF
        let slot1 = u16::from_le_bytes([buf[BLOCKS_TABLE_START + 2], buf[BLOCKS_TABLE_START + 3]]);
        assert_eq!(slot1, 0xFFFF);
    }

    #[test]
    fn test_directory_total_blocks() {
        let mut dir = Directory::new();
        dir.add_entry(Dirent::new("FILE1", "TRE"));
        dir.add_entry(Dirent::new("FILE2", "RGN"));
        assert_eq!(dir.total_directory_blocks(), 2);
    }
}
