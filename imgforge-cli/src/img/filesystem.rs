//! IMG Garmin filesystem: header, FAT directory and block-aligned subfile data.
//!
//! Layout (standard Garmin IMG format):
//! ```text
//! Sector  0     : IMG Header (512 bytes)
//! Sector  1     : Reserved (512 bytes of zeros)
//! Sectors 2..N  : FAT entries (512 bytes each) — volume label + file entries
//! Padding       : Zeros to fill the last header block
//! Blocks K..    : Subfile data (block-aligned)
//! ```
//!
//! The FAT uses 512-byte entries with block allocation tables. Each entry
//! can reference up to 240 blocks; larger files span multiple entries.

use crate::error::ImgError;
use crate::img::directory::{FatEntry, BLOCKS_PER_ENTRY};
use crate::img::header::{ImgDate, ImgHeader};

/// A subfile entry stored in the IMG filesystem.
pub struct ImgEntry {
    /// Map identifier (numeric string, ≤ 8 chars — used as FAT filename).
    pub map_id: String,
    /// Subfile extension (e.g. "TRE", "RGN", "LBL", "NET", "NOD").
    pub ext: String,
    /// Raw subfile data, zero-padded to the next block boundary.
    pub data: Vec<u8>,
    /// Real (unpadded) data length in bytes.
    pub size_used: u32,
}

/// The complete IMG filesystem: header + FAT directory + subfile data.
pub struct ImgFilesystem {
    /// Block size in bytes (`1 << block_size_exponent`).
    pub block_size: u32,
    /// Block size exponent.
    block_size_exponent: u8,
    /// Subfile entries.
    pub entries: Vec<ImgEntry>,
    /// Map description (written into the header).
    pub description: String,
    /// Creation date (written into the header).
    creation_date: ImgDate,
    /// Garmin family ID — used by TDB, not stored in header.
    pub family_id: u16,
    /// Garmin product ID — used by TDB, not stored in header.
    pub product_id: u16,
}

/// Internal layout computed before serialisation.
struct FsLayout {
    /// Number of blocks reserved for header + FAT area.
    header_blocks: u32,
    /// Number of 512-byte sectors in the file.
    total_file_sectors: u32,
    /// Total number of FAT entries (volume label + all file entries).
    total_fat_entries: u32,
    /// Per-file info: (first_block_index, n_blocks, n_fat_entries).
    file_info: Vec<(u32, u32, u32)>,
}

impl ImgFilesystem {
    /// Create a new empty filesystem with the given block size exponent.
    pub fn new(block_size_exponent: u8) -> Self {
        Self {
            block_size: 1u32 << block_size_exponent,
            block_size_exponent,
            entries: Vec::new(),
            description: String::new(),
            creation_date: ImgDate::now(),
            family_id: 0,
            product_id: 0,
        }
    }

    /// Add a subfile to the filesystem.
    ///
    /// `data` is padded with zero bytes to the next block boundary.
    ///
    /// # Errors
    /// Returns [`ImgError::InvalidMapId`] if `map_id` is not valid.
    pub fn add_subfile(&mut self, map_id: &str, ext: &str, data: Vec<u8>) -> Result<(), ImgError> {
        if map_id.is_empty() || !map_id.chars().all(|c| c.is_ascii_digit()) || map_id.len() > 8 {
            return Err(ImgError::InvalidMapId {
                id: map_id.to_string(),
            });
        }

        let size_used = data.len() as u32;
        let remainder = size_used % self.block_size;
        let allocated = if remainder == 0 {
            if size_used == 0 {
                self.block_size
            } else {
                size_used
            }
        } else {
            size_used + (self.block_size - remainder)
        };

        let mut padded = data;
        padded.resize(allocated as usize, 0u8);

        self.entries.push(ImgEntry {
            map_id: map_id.to_string(),
            ext: ext.to_string(),
            data: padded,
            size_used,
        });
        Ok(())
    }

    /// Number of subfile entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Iterate over subfile entries as `(map_id, ext, data)` triples.
    ///
    /// `data` is the **unpadded** content slice (length = `size_used`).
    pub fn subfiles(&self) -> impl Iterator<Item = (&str, &str, &[u8])> {
        self.entries.iter().map(|e| {
            (
                e.map_id.as_str(),
                e.ext.as_str(),
                &e.data[..e.size_used as usize],
            )
        })
    }

    /// Compute the filesystem layout.
    ///
    /// Iteratively determines header_blocks because the volume label entry
    /// itself occupies FAT space (which may require additional blocks).
    fn compute_layout(&self) -> FsLayout {
        let sectors_per_block = (self.block_size / 512).max(1);

        // Per-file: (n_data_blocks, n_fat_entries)
        let per_file: Vec<(u32, u32)> = self
            .entries
            .iter()
            .map(|e| {
                let n_blocks = (e.data.len() as u32).div_ceil(self.block_size);
                let n_entries = FatEntry::entries_needed(n_blocks);
                (n_blocks, n_entries)
            })
            .collect();

        let file_fat_entries: u32 = per_file.iter().map(|(_, f)| *f).sum();

        // Iteratively compute header_blocks (volume label may need multiple entries)
        let mut volume_entries = 1u32;
        let header_blocks;
        loop {
            let total_fat_entries = volume_entries + file_fat_entries;
            let header_sectors = 2 + total_fat_entries; // header sector + reserved + FAT
            let hb = header_sectors.div_ceil(sectors_per_block);
            let new_volume_entries = FatEntry::entries_needed(hb);
            if new_volume_entries == volume_entries {
                header_blocks = hb;
                break;
            }
            volume_entries = new_volume_entries;
        }

        let total_fat_entries = volume_entries + file_fat_entries;

        // Assign block indices to files
        let mut next_block = header_blocks;
        let file_info: Vec<(u32, u32, u32)> = per_file
            .iter()
            .map(|&(n_blocks, n_entries)| {
                let first = next_block;
                next_block += n_blocks;
                (first, n_blocks, n_entries)
            })
            .collect();

        let total_blocks = next_block;
        // F7: Verify block indices fit in u16 (FAT allocation table uses LE16).
        // 0xFFFF is reserved as "unused" marker, so max valid block = 0xFFFE.
        debug_assert!(
            total_blocks <= 0xFFFE,
            "total_blocks {} exceeds u16 FAT limit (max 65534). \
             Increase block_size_exponent to reduce block count.",
            total_blocks
        );
        let total_file_sectors = total_blocks * sectors_per_block;

        FsLayout {
            header_blocks,
            total_file_sectors,
            total_fat_entries,
            file_info,
        }
    }

    /// Compute `(byte_offset, size_allocated)` for each subfile entry.
    pub fn compute_entry_offsets(&self) -> Vec<(u64, u32)> {
        let layout = self.compute_layout();
        self.entries
            .iter()
            .zip(layout.file_info.iter())
            .map(|(entry, &(first_block, _, _))| {
                let offset = first_block as u64 * self.block_size as u64;
                let size = entry.data.len() as u32;
                (offset, size)
            })
            .collect()
    }

    /// Serialise the entire filesystem into a contiguous byte vector.
    ///
    /// Layout: header sector | reserved sector | FAT entries | padding | data blocks…
    pub fn to_bytes(&self) -> Vec<u8> {
        let layout = self.compute_layout();

        // Build header
        let header = ImgHeader {
            description: self.description.clone(),
            block_size_exponent: self.block_size_exponent,
            creation_date: self.creation_date.clone(),
            total_file_sectors: layout.total_file_sectors,
        };

        let total_bytes = (layout.total_file_sectors as u64 * 512) as usize;
        let mut out = Vec::with_capacity(total_bytes);

        // ── Sector 0: header ────────────────────────────────────────────
        out.extend_from_slice(&header.to_bytes());

        // ── Sector 1: reserved (zeros) ──────────────────────────────────
        out.extend_from_slice(&[0u8; 512]);

        // ── FAT entries ─────────────────────────────────────────────────

        // Volume label entry (allocates header blocks)
        let header_block_list: Vec<u16> =
            (0..layout.header_blocks).map(|i| i as u16).collect();
        let volume_label = FatEntry::new_volume_label(
            layout.total_fat_entries as u16,
            header_block_list.clone(),
        );

        // Write volume label entries (may need multiple for many header blocks)
        let vol_entries_needed = FatEntry::entries_needed(layout.header_blocks);
        for part in 0..vol_entries_needed {
            let start = (part * BLOCKS_PER_ENTRY as u32) as usize;
            let end = ((part + 1) * BLOCKS_PER_ENTRY as u32) as usize;
            let chunk: Vec<u16> = header_block_list
                .iter()
                .skip(start)
                .take(end - start)
                .copied()
                .collect();

            if part == 0 {
                let mut entry = volume_label.clone();
                entry.blocks = chunk;
                out.extend_from_slice(&entry.to_bytes());
            } else {
                let mut entry = FatEntry::new_volume_label(0, chunk);
                entry.part = 0x03; // keep volume flag
                entry.file_size = 0;
                out.extend_from_slice(&entry.to_bytes());
            }
        }

        // File FAT entries
        for (i, entry) in self.entries.iter().enumerate() {
            let (first_block, n_blocks, n_fat_entries) = layout.file_info[i];

            for part in 0..n_fat_entries {
                let block_start = part * BLOCKS_PER_ENTRY as u32;
                let block_end = ((part + 1) * BLOCKS_PER_ENTRY as u32).min(n_blocks);
                let blocks: Vec<u16> = (first_block + block_start..first_block + block_end)
                    .map(|b| b as u16)
                    .collect();

                let file_size = if part == 0 {
                    entry.size_used
                } else {
                    0
                };

                let fat = FatEntry::new_file(
                    &entry.map_id,
                    &entry.ext,
                    file_size,
                    part as u8,
                    blocks,
                )
                .expect("map_id already validated in add_subfile");

                out.extend_from_slice(&fat.to_bytes());
            }
        }

        // ── Pad header area to block boundary ───────────────────────────
        let header_area_bytes = layout.header_blocks as usize * self.block_size as usize;
        if out.len() < header_area_bytes {
            out.resize(header_area_bytes, 0u8);
        }

        // ── Subfile data blocks ─────────────────────────────────────────
        for entry in &self.entries {
            out.extend_from_slice(&entry.data);
        }

        debug_assert_eq!(
            out.len(),
            total_bytes,
            "output size mismatch: expected {} bytes, got {}",
            total_bytes,
            out.len()
        );

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fs() -> ImgFilesystem {
        let mut fs = ImgFilesystem::new(9); // block_size = 512
        fs.description = "Test".to_string();
        fs.add_subfile("63240001", "TRE", vec![]).unwrap();
        fs.add_subfile("63240001", "RGN", vec![]).unwrap();
        fs.add_subfile("63240001", "LBL", vec![]).unwrap();
        fs
    }

    #[test]
    fn test_filesystem_aligned() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        assert_eq!(
            bytes.len() % fs.block_size as usize,
            0,
            "total size must be a multiple of block_size"
        );
    }

    #[test]
    fn test_filesystem_dskimg_signature() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
    }

    #[test]
    fn test_filesystem_garmin_signature() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
    }

    #[test]
    fn test_filesystem_boot_signature() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
    }

    #[test]
    fn test_filesystem_fat_entries_at_sector_2() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        // First FAT entry (volume label) starts at offset 0x400 (sector 2)
        assert_eq!(bytes[0x400], 0x01, "volume label flag must be 0x01");
        // Name = spaces
        assert_eq!(&bytes[0x401..0x409], &[0x20; 8]);
    }

    #[test]
    fn test_filesystem_file_fat_entries() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        // With block_size=512, 3 files + volume label = 4 FAT entries
        // header_sectors = 2 + 4 = 6 → 6 header blocks
        // Volume label allocates blocks 0-5
        // File entries start at sector 3 (offset 0x600)
        let tre_offset = 0x600;
        assert_eq!(bytes[tre_offset], 0x01); // flag
        assert_eq!(&bytes[tre_offset + 1..tre_offset + 9], b"63240001");
        assert_eq!(&bytes[tre_offset + 9..tre_offset + 12], b"TRE");
    }

    #[test]
    fn test_filesystem_total_size() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        // With block_size=512:
        // 4 FAT entries (1 volume + 3 files)
        // header_sectors = 2 + 4 = 6 → 6 header blocks
        // 3 data blocks (one per empty stub)
        // Total = 9 blocks × 512 = 4608 bytes
        assert_eq!(bytes.len(), 9 * 512);
    }

    #[test]
    fn test_filesystem_empty_entries() {
        let fs = ImgFilesystem::new(9);
        let bytes = fs.to_bytes();
        // No subfiles: 1 volume label entry
        // header_sectors = 2 + 1 = 3 → 3 header blocks
        // Total = 3 blocks × 512 = 1536 bytes
        assert_eq!(bytes.len(), 3 * 512);
    }

    #[test]
    fn test_filesystem_data_at_correct_offset() {
        let mut fs = ImgFilesystem::new(9);
        fs.add_subfile("63240001", "TRE", vec![0xAB; 100]).unwrap();
        let bytes = fs.to_bytes();

        // 1 volume + 1 file FAT entry → header_sectors = 2 + 2 = 4 → 4 blocks
        // Data starts at block 4 = offset 2048
        let data_offset = 4 * 512;
        assert_eq!(bytes[data_offset], 0xAB);
        assert_eq!(bytes[data_offset + 99], 0xAB);
        assert_eq!(bytes[data_offset + 100], 0x00); // padding
    }

    #[test]
    fn test_filesystem_block_allocation_in_fat() {
        let mut fs = ImgFilesystem::new(9);
        fs.add_subfile("63240001", "TRE", vec![0xAB; 100]).unwrap();
        let bytes = fs.to_bytes();

        // File FAT entry at sector 3 (volume label at sector 2)
        let fat_offset = 3 * 512;
        // Block allocation starts at offset 0x20 within the entry
        let block_alloc = fat_offset + 0x20;
        let first_block = u16::from_le_bytes([bytes[block_alloc], bytes[block_alloc + 1]]);
        // Data starts at block 4
        assert_eq!(first_block, 4);
    }

    #[test]
    fn test_filesystem_entry_count() {
        let fs = make_fs();
        assert_eq!(fs.entry_count(), 3);
    }

    #[test]
    fn test_filesystem_subfiles_iterator() {
        let fs = make_fs();
        let subs: Vec<_> = fs.subfiles().collect();
        assert_eq!(subs.len(), 3);
        assert_eq!(subs[0].0, "63240001");
        assert_eq!(subs[0].1, "TRE");
        assert_eq!(subs[1].1, "RGN");
        assert_eq!(subs[2].1, "LBL");
    }

    #[test]
    fn test_filesystem_compute_entry_offsets() {
        let fs = make_fs();
        let offsets = fs.compute_entry_offsets();
        assert_eq!(offsets.len(), 3);
        // header_blocks = 6 (with block_size=512)
        // TRE at block 6 = offset 3072
        assert_eq!(offsets[0].0, 6 * 512, "TRE offset = block 6");
        assert_eq!(offsets[1].0, 7 * 512, "RGN offset = block 7");
        assert_eq!(offsets[2].0, 8 * 512, "LBL offset = block 8");
    }

    #[test]
    fn test_filesystem_large_block_size() {
        // With block_size=16384 (exp=14), header area fits in 1 block
        let mut fs = ImgFilesystem::new(14);
        fs.add_subfile("63240001", "TRE", vec![0x01; 100]).unwrap();
        fs.add_subfile("63240001", "RGN", vec![0x02; 200]).unwrap();
        let bytes = fs.to_bytes();

        // 1 volume + 2 file entries → 3 FAT sectors (1536 bytes)
        // header_sectors = 5, sectors_per_block = 32 → 1 header block
        // 2 data blocks → total 3 blocks × 16384 = 49152 bytes
        assert_eq!(bytes.len(), 3 * 16384);

        // DSKIMG and GARMIN signatures
        assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
        assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");

        // Data at block 1 (offset 16384)
        assert_eq!(bytes[16384], 0x01); // TRE data
    }
}
