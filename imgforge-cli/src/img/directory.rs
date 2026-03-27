//! Garmin IMG FAT directory entry — 512 bytes (one sector) per entry.
//!
//! Each entry contains a 32-byte header followed by a 480-byte block
//! allocation table (240 × LE16 block numbers, 0xFFFF = unused).
//!
//! Large files that need more than 240 blocks use multiple FAT entries
//! (continuation parts) with incrementing part numbers.
//!
//! ```text
//! Offset  Len  Content
//! 0x00    1    Flag: 0x01 = file entry
//! 0x01    8    Filename, space-padded (ASCII)
//! 0x09    3    Extension (e.g. "TRE", "RGN", "LBL")
//! 0x0C    4    File size (LE32) — only in first entry (part 0)
//! 0x10    1    Part number (0, 1, 2, …)
//! 0x11    15   Reserved (zeros)
//! 0x20    480  Block allocation table (240 × LE16, 0xFFFF = unused)
//! ```

use crate::error::ImgError;

/// Size of a single FAT entry in bytes (one 512-byte sector).
pub const FAT_ENTRY_SIZE: usize = 512;

/// Offset where the block allocation table starts within the entry.
const BLOCK_TABLE_OFFSET: usize = 0x20;

/// Maximum number of block indices per FAT entry (480 / 2).
pub const BLOCKS_PER_ENTRY: usize = 240;

/// A single 512-byte Garmin FAT directory entry.
#[derive(Debug, Clone)]
pub struct FatEntry {
    /// 8-byte filename, space-padded (0x20).
    pub name: [u8; 8],
    /// 3-byte extension (e.g. *b"TRE"*).
    pub ext: [u8; 3],
    /// File size in bytes (only meaningful for part 0).
    pub file_size: u32,
    /// Subtype flag at offset 0x10: 0x00 = regular file, 0x03 = volume label.
    pub subtype: u8,
    /// Part number at offset 0x11: 0 = first entry, 1+ = continuation.
    pub part: u8,
    /// Allocated block indices for this entry.
    pub blocks: Vec<u16>,
}

impl FatEntry {
    /// Create a file FAT entry.
    ///
    /// - `map_id`: numeric filename (≤ 8 chars, ASCII digits).
    /// - `ext`: subfile extension ("TRE", "RGN", etc.).
    /// - `file_size`: real file size (only stored in part 0).
    /// - `part`: part number (0 for first entry).
    /// - `blocks`: block indices allocated to this part (max 240).
    ///
    /// # Errors
    /// Returns [`ImgError::InvalidMapId`] if `map_id` is invalid.
    pub fn new_file(
        map_id: &str,
        ext: &str,
        file_size: u32,
        part: u8,
        blocks: Vec<u16>,
    ) -> Result<Self, ImgError> {
        if map_id.is_empty() || !map_id.chars().all(|c| c.is_ascii_digit()) || map_id.len() > 8 {
            return Err(ImgError::InvalidMapId {
                id: map_id.to_string(),
            });
        }

        let mut name = [0x20u8; 8];
        let id_bytes = map_id.as_bytes();
        name[..id_bytes.len()].copy_from_slice(id_bytes);

        let mut ext_buf = [0x20u8; 3];
        let ext_bytes = ext.as_bytes();
        let len = ext_bytes.len().min(3);
        ext_buf[..len].copy_from_slice(&ext_bytes[..len]);

        Ok(Self {
            name,
            ext: ext_buf,
            file_size,
            subtype: 0x00, // regular file
            part,
            blocks,
        })
    }

    /// Create a volume label FAT entry (header area descriptor).
    ///
    /// The volume label uses space-padded name and extension. Its block
    /// allocation table lists all blocks occupied by the header area.
    pub fn new_volume_label(total_fat_entries: u16, header_blocks: Vec<u16>) -> Self {
        // file_size = total header-area bytes used (header + reserved + FAT sectors).
        // Must be >= cluster_size (2^(E1+E2) = 2048) for gmt compatibility.
        let used_bytes = (2 + total_fat_entries as u32) * 512;
        let cluster_size = 2048u32; // 2^(E1+E2) = 2^(9+2)
        Self {
            name: [0x20; 8],
            ext: [0x20; 3],
            file_size: used_bytes.max(cluster_size),
            subtype: 0x03, // volume label flag
            part: 0x00,
            blocks: header_blocks,
        }
    }

    /// Serialise this entry into a fixed 512-byte buffer.
    pub fn to_bytes(&self) -> [u8; FAT_ENTRY_SIZE] {
        let mut buf = [0u8; FAT_ENTRY_SIZE];

        // Fill block allocation area with 0xFFFF (unused marker).
        buf[BLOCK_TABLE_OFFSET..].fill(0xFF);

        // ── Header (0x00–0x1F) ──────────────────────────────────────────
        buf[0x00] = 0x01; // flag: file entry
        buf[0x01..0x09].copy_from_slice(&self.name);
        buf[0x09..0x0C].copy_from_slice(&self.ext);
        buf[0x0C..0x10].copy_from_slice(&self.file_size.to_le_bytes());
        buf[0x10] = self.subtype; // 0x00 = file, 0x03 = volume label
        buf[0x11] = self.part;    // part number (0 = first, 1+ = continuation)
        // 0x12–0x1F: reserved (already zeros)

        // ── Block allocation table (0x20–0x1FF) ────────────────────────
        for (i, &block) in self.blocks.iter().enumerate() {
            if i >= BLOCKS_PER_ENTRY {
                break;
            }
            let off = BLOCK_TABLE_OFFSET + i * 2;
            buf[off..off + 2].copy_from_slice(&block.to_le_bytes());
        }

        buf
    }

    /// Number of FAT entries required for a file occupying `n_blocks` blocks.
    pub fn entries_needed(n_blocks: u32) -> u32 {
        n_blocks.div_ceil(BLOCKS_PER_ENTRY as u32).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fat_entry_size() {
        let e = FatEntry::new_file("63240001", "TRE", 1024, 0, vec![10]).unwrap();
        assert_eq!(e.to_bytes().len(), 512);
    }

    #[test]
    fn test_fat_entry_name_padding() {
        let e = FatEntry::new_file("12345", "TRE", 512, 0, vec![5]).unwrap();
        assert_eq!(e.name, [b'1', b'2', b'3', b'4', b'5', 0x20, 0x20, 0x20]);
    }

    #[test]
    fn test_fat_entry_name_no_padding_8chars() {
        let e = FatEntry::new_file("63240001", "TRE", 512, 0, vec![5]).unwrap();
        assert_eq!(e.name, *b"63240001");
    }

    #[test]
    fn test_fat_entry_flag() {
        let e = FatEntry::new_file("63240001", "TRE", 512, 0, vec![5]).unwrap();
        assert_eq!(e.to_bytes()[0x00], 0x01);
    }

    #[test]
    fn test_fat_entry_extension() {
        let e = FatEntry::new_file("63240001", "LBL", 512, 0, vec![5]).unwrap();
        assert_eq!(&e.to_bytes()[0x09..0x0C], b"LBL");
    }

    #[test]
    fn test_fat_entry_file_size() {
        let e = FatEntry::new_file("63240001", "RGN", 0x1234, 0, vec![5]).unwrap();
        let bytes = e.to_bytes();
        let sz = u32::from_le_bytes([bytes[0x0C], bytes[0x0D], bytes[0x0E], bytes[0x0F]]);
        assert_eq!(sz, 0x1234);
    }

    #[test]
    fn test_fat_entry_part_number() {
        let e = FatEntry::new_file("63240001", "RGN", 0, 2, vec![100, 101]).unwrap();
        assert_eq!(e.to_bytes()[0x10], 0x00, "subtype = 0x00 for regular file");
        assert_eq!(e.to_bytes()[0x11], 2, "part number at offset 0x11");
    }

    #[test]
    fn test_fat_entry_block_allocation() {
        let blocks = vec![10u16, 11, 12];
        let e = FatEntry::new_file("63240001", "TRE", 1024, 0, blocks).unwrap();
        let bytes = e.to_bytes();
        assert_eq!(u16::from_le_bytes([bytes[0x20], bytes[0x21]]), 10);
        assert_eq!(u16::from_le_bytes([bytes[0x22], bytes[0x23]]), 11);
        assert_eq!(u16::from_le_bytes([bytes[0x24], bytes[0x25]]), 12);
        // Unused slots filled with 0xFFFF
        assert_eq!(u16::from_le_bytes([bytes[0x26], bytes[0x27]]), 0xFFFF);
    }

    #[test]
    fn test_fat_entry_volume_label() {
        let e = FatEntry::new_volume_label(5, vec![0, 1, 2]);
        let bytes = e.to_bytes();
        assert_eq!(bytes[0x00], 0x01);
        assert_eq!(&bytes[0x01..0x09], &[0x20; 8]); // spaces
        assert_eq!(&bytes[0x09..0x0C], &[0x20; 3]); // spaces
        // file_size = max((2 + total_fat_entries) * 512, 2048)
        // For 5 entries: (2+5)*512 = 3584 > 2048 → 3584
        let fs = u32::from_le_bytes([bytes[0x0C], bytes[0x0D], bytes[0x0E], bytes[0x0F]]);
        assert_eq!(fs, 3584, "volume label file_size = (2 + 5) * 512 = 3584");
        assert_eq!(bytes[0x10], 0x03, "subtype = 0x03 for volume label");
        assert_eq!(bytes[0x11], 0x00, "part = 0");
        assert_eq!(u16::from_le_bytes([bytes[0x20], bytes[0x21]]), 0);
        assert_eq!(u16::from_le_bytes([bytes[0x22], bytes[0x23]]), 1);
        assert_eq!(u16::from_le_bytes([bytes[0x24], bytes[0x25]]), 2);
    }

    #[test]
    fn test_fat_entry_invalid_map_id_empty() {
        let err = FatEntry::new_file("", "TRE", 0, 0, vec![]).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_fat_entry_invalid_map_id_non_digit() {
        let err = FatEntry::new_file("NOTDIGIT", "TRE", 0, 0, vec![]).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_fat_entry_invalid_map_id_too_long() {
        let err = FatEntry::new_file("123456789", "TRE", 0, 0, vec![]).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_entries_needed() {
        assert_eq!(FatEntry::entries_needed(0), 1);
        assert_eq!(FatEntry::entries_needed(1), 1);
        assert_eq!(FatEntry::entries_needed(240), 1);
        assert_eq!(FatEntry::entries_needed(241), 2);
        assert_eq!(FatEntry::entries_needed(480), 2);
        assert_eq!(FatEntry::entries_needed(481), 3);
    }
}
