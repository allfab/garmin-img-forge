//! IMG FAT-like filesystem: block allocation and binary serialisation.
//!
//! Layout (example with block_size = 512, 3 subfiles):
//! ```text
//! Block 0 : IMG Header (512 bytes)
//! Block 1 : Directory — Dirents × 3 = 96 bytes, zero-padded to 512 bytes
//! Block 2 : 63240001.TRE  (stub, 1 block of zeros)
//! Block 3 : 63240001.RGN  (stub, 1 block of zeros)
//! Block 4 : 63240001.LBL  (stub, 1 block of zeros)
//! Total   : 5 × 512 = 2560 bytes
//! ```
//!
//! Multi-block directory (example with 20 tiles × 3 subfiles = 60 entries):
//! ```text
//! Block 0 : Header (512 bytes)
//! Block 1–N : Directory — 60 entries × 32 bytes = 1920 bytes → 4 dir blocks for block_size=512
//! Block 5.. : Subfile data blocks
//! ```

use crate::error::ImgError;
use crate::img::directory::Dirent;
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

/// The complete IMG filesystem: header + directory + subfile data.
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
    /// Garmin family ID — written to header at offset 0x054.
    pub family_id: u16,
    /// Garmin product ID — written to header at offset 0x056.
    pub product_id: u16,
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
    /// Block start positions are computed lazily in [`to_bytes`].
    ///
    /// # Errors
    /// Returns [`ImgError::InvalidMapId`] if `map_id` is not a non-empty numeric string ≤ 8 chars.
    pub fn add_subfile(&mut self, map_id: &str, ext: &str, data: Vec<u8>) -> Result<(), ImgError> {
        // Validate map_id: non-empty, all ASCII digits, max 8 chars.
        if map_id.is_empty() || !map_id.chars().all(|c| c.is_ascii_digit()) || map_id.len() > 8 {
            return Err(ImgError::InvalidMapId {
                id: map_id.to_string(),
            });
        }

        let size_used = data.len() as u32;
        let remainder = size_used % self.block_size;
        let allocated = if remainder == 0 {
            // Empty file still gets one block so it has a physical location.
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
    /// `data` is the **unpadded** content slice (length = `size_used`), without the
    /// block-alignment padding appended by [`add_subfile`].
    /// Used by [`GmapsuppAssembler`] to copy tile subfiles into the outer filesystem.
    pub fn subfiles(&self) -> impl Iterator<Item = (&str, &str, &[u8])> {
        self.entries.iter().map(|e| {
            (
                e.map_id.as_str(),
                e.ext.as_str(),
                &e.data[..e.size_used as usize],
            )
        })
    }

    /// Number of directory blocks needed to hold all Dirents.
    ///
    /// `dir_blocks = ceil(n_entries × 32 / block_size)`, minimum 1.
    fn dir_blocks(&self) -> u32 {
        (self.entries.len() as u32 * 32)
            .div_ceil(self.block_size)
            .max(1)
    }

    /// Compute `(byte_offset, size_allocated)` for each subfile entry.
    ///
    /// `byte_offset = block_start × block_size`.
    /// Used for logging in [`ImgWriter`].
    pub fn compute_entry_offsets(&self) -> Vec<(u64, u32)> {
        let mut block_start = 1u32 + self.dir_blocks();
        let mut result = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let size_allocated = entry.data.len() as u32;
            result.push((block_start as u64 * self.block_size as u64, size_allocated));
            block_start += size_allocated.div_ceil(self.block_size);
        }
        result
    }

    /// Serialise the entire filesystem into a contiguous byte vector.
    ///
    /// Layout: header block | directory blocks (N) | subfile data blocks…
    ///
    /// The number of directory blocks is computed dynamically via [`dir_blocks`]:
    /// `dir_blocks = ceil(n_entries × 32 / block_size)`, minimum 1.
    pub fn to_bytes(&self) -> Vec<u8> {
        let dir_blocks = self.dir_blocks();
        let data_blocks: u32 = self
            .entries
            .iter()
            .map(|e| (e.data.len() as u32).div_ceil(self.block_size))
            .sum();
        let total_blocks: u32 = 1 + dir_blocks + data_blocks; // 1 header block

        // Build header.
        let header = ImgHeader {
            description: self.description.clone(),
            block_size_exponent: self.block_size_exponent,
            creation_date: self.creation_date.clone(),
            total_blocks,
            family_id: self.family_id,
            product_id: self.product_id,
        };

        let capacity = (total_blocks * self.block_size) as usize;
        let mut out = Vec::with_capacity(capacity);

        // Block 0: header.
        out.extend_from_slice(&header.to_bytes());

        // Directory blocks: compute block_start for each entry dynamically.
        let mut dir_bytes: Vec<u8> = Vec::new();
        let mut block_start = 1u32 + dir_blocks;
        for entry in &self.entries {
            let size_allocated = entry.data.len() as u32;
            debug_assert!(
                block_start <= u16::MAX as u32,
                "block_start {} exceeds u16::MAX — too many subfiles",
                block_start
            );
            let dirent = Dirent::new(
                &entry.map_id,
                &entry.ext,
                block_start as u16,
                size_allocated,
                entry.size_used,
            )
            .expect("map_id already validated in add_subfile");
            dir_bytes.extend_from_slice(&dirent.to_bytes());
            block_start += size_allocated.div_ceil(self.block_size);
        }
        // Pad directory to dir_blocks × block_size bytes.
        dir_bytes.resize((dir_blocks * self.block_size) as usize, 0u8);
        out.extend_from_slice(&dir_bytes);

        // Subfile data blocks.
        for entry in &self.entries {
            out.extend_from_slice(&entry.data);
        }

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
    fn test_filesystem_no_overlap() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        let block_size = fs.block_size as usize;
        let dir_start = block_size; // Block 1 = directory
        let n_entries = fs.entry_count();

        // With 3 entries and block_size=512: dir_blocks = ceil(3*32/512) = 1
        // So first subfile starts at block 2.
        let mut prev_end: u32 = 2;
        for i in 0..n_entries {
            let dirent_off = dir_start + i * 32;
            let block_start =
                u16::from_le_bytes([bytes[dirent_off + 0x0C], bytes[dirent_off + 0x0D]]) as u32;
            assert_eq!(
                block_start, prev_end,
                "subfile {} must start immediately after previous one",
                i
            );
            let size_allocated = u32::from_le_bytes([
                bytes[dirent_off + 0x0E],
                bytes[dirent_off + 0x0F],
                bytes[dirent_off + 0x10],
                bytes[dirent_off + 0x11],
            ]);
            prev_end += size_allocated.div_ceil(fs.block_size);
        }
    }

    #[test]
    fn test_filesystem_directory_offsets() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        let block_size = fs.block_size as usize;

        // Directory is at block 1.
        let dir_start = block_size;

        // Read first Dirent (TRE) from directory.
        let dirent_bytes: [u8; 32] = bytes[dir_start..dir_start + 32].try_into().unwrap();
        let block_start_tre = u16::from_le_bytes([dirent_bytes[0x0C], dirent_bytes[0x0D]]);
        assert_eq!(block_start_tre, 2, "TRE must start at block 2");

        // Verify the byte offset matches the actual position in the output.
        let expected_byte_offset = block_start_tre as usize * block_size;
        // The TRE block should contain zeros (stub).
        assert!(
            bytes[expected_byte_offset..expected_byte_offset + block_size]
                .iter()
                .all(|&b| b == 0),
            "stub subfile must be all zeros"
        );
    }

    #[test]
    fn test_filesystem_total_size() {
        let fs = make_fs();
        let bytes = fs.to_bytes();
        // 1 header + 1 directory + 3 stubs (1 block each) = 5 blocks × 512 = 2560 bytes
        assert_eq!(bytes.len(), 5 * 512);
    }

    #[test]
    fn test_filesystem_empty_entries() {
        let fs = ImgFilesystem::new(9);
        let bytes = fs.to_bytes();
        // Without any subfiles: 1 header + 1 directory = 2 blocks
        assert_eq!(bytes.len(), 2 * 512);
    }

    #[test]
    fn test_filesystem_multi_block_directory() {
        // 6 entries × 32 bytes = 192 bytes with block_size=512 → still 1 dir block
        // For dir_blocks > 1, need > 512/32 = 16 entries with block_size=512
        // Use 17 entries to force 2 dir blocks
        let mut fs = ImgFilesystem::new(9); // block_size = 512
        for i in 0..17u32 {
            let map_id = format!("{:08}", i + 1);
            fs.add_subfile(&map_id, "TRE", vec![]).unwrap();
        }
        // 17 entries × 32 = 544 bytes > 512 → dir_blocks = 2
        let bytes = fs.to_bytes();
        let dir_start = 512usize; // block 1
        // First entry should start at block 3 (1 header + 2 dir blocks)
        let block_start_first =
            u16::from_le_bytes([bytes[dir_start + 0x0C], bytes[dir_start + 0x0D]]);
        assert_eq!(
            block_start_first, 3,
            "with 17 entries and block_size=512, first subfile starts at block 3"
        );
        // Total size: 1 header + 2 dir + 17 data = 20 blocks × 512 = 10240 bytes
        assert_eq!(bytes.len(), 20 * 512);
    }

    #[test]
    fn test_filesystem_family_id_product_id() {
        let mut fs = ImgFilesystem::new(9);
        fs.family_id = 6324;
        fs.product_id = 1;
        let bytes = fs.to_bytes();
        let fid = u16::from_le_bytes([bytes[0x054], bytes[0x055]]);
        let pid = u16::from_le_bytes([bytes[0x056], bytes[0x057]]);
        assert_eq!(fid, 6324, "family_id must be at header offset 0x054");
        assert_eq!(pid, 1, "product_id must be at header offset 0x056");
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
        // block_size=512, dir_blocks=1, first entry at block 2 = offset 1024
        assert_eq!(offsets[0].0, 2 * 512, "TRE offset = block 2");
        assert_eq!(offsets[1].0, 3 * 512, "RGN offset = block 3");
        assert_eq!(offsets[2].0, 4 * 512, "LBL offset = block 4");
    }
}
