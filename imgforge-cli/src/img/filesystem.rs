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

use crate::error::ImgError;
use crate::img::directory::Dirent;
use crate::img::header::{ImgDate, ImgHeader};

/// The complete IMG filesystem: header + directory + subfile data.
pub struct ImgFilesystem {
    /// Block size in bytes (`1 << block_size_exponent`).
    pub block_size: u32,
    /// Block size exponent.
    block_size_exponent: u8,
    /// Subfile entries: (directory entry, raw data).
    pub entries: Vec<(Dirent, Vec<u8>)>,
    /// Map description (written into the header).
    pub description: String,
    /// Creation date (written into the header).
    creation_date: ImgDate,
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
        }
    }

    /// Add a subfile to the filesystem.
    ///
    /// `data` is padded with zero bytes to the next block boundary.
    /// The `Dirent` block_start is computed automatically.
    ///
    /// # Errors
    /// Propagates [`ImgError::InvalidMapId`] from `Dirent::new`.
    pub fn add_subfile(&mut self, map_id: &str, ext: &str, data: Vec<u8>) -> Result<(), ImgError> {
        // Compute next free block: 1 (header) + 1 (directory) + sum of allocated blocks
        // so far. The directory always fits in one block for ≤ block_size/32 entries.
        let mut block_start: u32 = 2; // header block + directory block
        for (dirent, _) in &self.entries {
            let blocks = dirent.size_allocated.div_ceil(self.block_size);
            block_start += blocks;
        }

        if block_start > u16::MAX as u32 {
            return Err(ImgError::BlockAlignmentError {
                message: format!(
                    "block_start {} exceeds u16::MAX — too many subfiles",
                    block_start
                ),
            });
        }

        // Pad data to a whole number of blocks.
        let data_len = data.len() as u32;
        let remainder = data_len % self.block_size;
        let allocated = if remainder == 0 {
            // Empty file still gets one block so it has a physical location.
            if data_len == 0 {
                self.block_size
            } else {
                data_len
            }
        } else {
            data_len + (self.block_size - remainder)
        };

        let dirent = Dirent::new(map_id, ext, block_start as u16, allocated)?;
        // Pad to allocated size.
        let mut padded = data;
        padded.resize(allocated as usize, 0u8);

        self.entries.push((dirent, padded));
        Ok(())
    }

    /// Serialise the entire filesystem into a contiguous byte vector.
    ///
    /// Layout: header block | directory block | subfile data blocks…
    pub fn to_bytes(&self) -> Vec<u8> {
        // Compute total block count.
        let dir_blocks: u32 = 1; // directory always fits in one block for ≤ block_size/32 entries
        let data_blocks: u32 = self
            .entries
            .iter()
            .map(|(d, _)| d.size_allocated.div_ceil(self.block_size))
            .sum();
        let total_blocks: u32 = 1 + dir_blocks + data_blocks; // 1 header block

        // Build header.
        let header = ImgHeader {
            description: self.description.clone(),
            block_size_exponent: self.block_size_exponent,
            creation_date: self.creation_date.clone(),
            total_blocks,
        };

        let capacity = (total_blocks * self.block_size) as usize;
        let mut out = Vec::with_capacity(capacity);

        // Block 0: header.
        out.extend_from_slice(&header.to_bytes());

        // Block 1: directory (Dirents concatenated, zero-padded to block_size).
        let mut dir_bytes: Vec<u8> = self
            .entries
            .iter()
            .flat_map(|(d, _)| d.to_bytes())
            .collect();
        // Pad directory to one full block.
        dir_bytes.resize(self.block_size as usize, 0u8);
        out.extend_from_slice(&dir_bytes);

        // Subfile data blocks.
        for (_, data) in &self.entries {
            out.extend_from_slice(data);
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
        // Check block_start values are strictly increasing without gaps.
        let mut prev_end: u32 = 2; // header + directory = 2 blocks
        for (d, _) in &fs.entries {
            assert_eq!(
                d.block_start as u32, prev_end,
                "subfile must start immediately after previous one"
            );
            prev_end += d.size_allocated.div_ceil(fs.block_size);
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
}
