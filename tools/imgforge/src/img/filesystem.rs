// ImgFS + BlockManager — IMG filesystem container
// Faithful to mkgmap ImgFS.java + BlockManager.java

use super::directory::{Dirent, Directory, ENTRY_SIZE, TABLE_SIZE};
use super::header::ImgHeader;
use crate::error::ImgError;

/// Sequential block allocator — mkgmap BlockManager.java
pub struct BlockManager {
    block_size: u32,
    current_block: u32,
    max_block: u32,
}

impl BlockManager {
    pub fn new(block_size: u32, initial_block: u32) -> Self {
        Self {
            block_size,
            current_block: initial_block,
            max_block: 0xFFFE,
        }
    }

    pub fn allocate(&mut self) -> Result<u16, ImgError> {
        let n = self.current_block;
        if n > self.max_block {
            return Err(ImgError::BlockOverflow(format!(
                "Block overflow: {} > max {}. Use larger block size.",
                n, self.max_block
            )));
        }
        self.current_block += 1;
        Ok(n as u16)
    }

    pub fn block_size(&self) -> u32 {
        self.block_size
    }

    pub fn current_block(&self) -> u32 {
        self.current_block
    }
}

/// A file to be written into the IMG filesystem
struct ImgFile {
    name: String,
    ext: String,
    data: Vec<u8>,
}

/// IMG Filesystem — mkgmap ImgFS.java
pub struct ImgFilesystem {
    files: Vec<ImgFile>,
    description: String,
}

impl ImgFilesystem {
    pub fn new(description: &str) -> Self {
        Self {
            files: Vec::new(),
            description: description.to_string(),
        }
    }

    pub fn add_file(&mut self, name: &str, ext: &str, data: Vec<u8>) {
        self.files.push(ImgFile {
            name: name.to_string(),
            ext: ext.to_string(),
            data,
        });
    }

    /// Assemble the complete IMG file — mkgmap ImgFS.sync()
    pub fn sync(&self) -> Result<Vec<u8>, ImgError> {
        // Step 1: Calculate optimal block size
        let total_file_size: usize = self.files.iter().map(|f| f.data.len()).sum();
        let block_size = calc_block_size(total_file_size, self.files.len());

        // Step 2: Pre-compute blocks per file to get accurate directory part counts
        let file_blocks: Vec<u32> = self.files.iter()
            .map(|f| (f.data.len() as u32 + block_size - 1) / block_size)
            .collect();

        let directory_start_entry: u8 = 2;
        let header_blocks_512 = directory_start_entry as usize;

        // Build directory with pre-allocated block counts for correct num_parts
        let mut directory = Directory::new();

        let mut header_entry = Dirent::new("        ", "   ");
        header_entry.special = true;
        directory.add_entry(header_entry);

        for (idx, file) in self.files.iter().enumerate() {
            let mut entry = Dirent::new(&file.name, &file.ext);
            entry.size = file.data.len() as u32;
            // Add placeholder blocks so num_parts() is correct for directory sizing
            for b in 0..file_blocks[idx] {
                entry.add_block(b as u16);
            }
            directory.add_entry(entry);
        }

        // Now directory.total_directory_blocks() accounts for multi-part entries
        let dir_blocks_512 = directory.total_directory_blocks();
        let total_header_512 = header_blocks_512 + dir_blocks_512;
        let header_fs_blocks = (total_header_512 as u32 * 512 + block_size - 1) / block_size;

        // Step 3: Allocate real blocks
        let mut block_manager = BlockManager::new(block_size, header_fs_blocks);

        // Header entry covers header + directory blocks
        directory.entries[0].blocks.clear();
        directory.entries[0].size = (total_header_512 * 512) as u32;
        for i in 0..header_fs_blocks {
            directory.entries[0].add_block(i as u16);
        }

        // Replace placeholder blocks with real sequential allocations
        for (i, _file) in self.files.iter().enumerate() {
            let entry = &mut directory.entries[i + 1];
            entry.blocks.clear();
            for _ in 0..file_blocks[i] {
                let blk = block_manager.allocate()?;
                entry.add_block(blk);
            }
        }

        let total_blocks = block_manager.current_block();

        // Step 4: Build the IMG header
        let mut img_header = ImgHeader::new(block_size, &self.description);
        img_header.num_blocks = total_blocks;
        let header_bytes = img_header.write();

        // Step 5: Assemble final output
        let total_size = total_blocks as usize * block_size as usize;
        let mut output = vec![0u8; total_size];

        // Write header (512 bytes at position 0)
        output[..512].copy_from_slice(&header_bytes);

        // Write directory after header
        let dir_bytes = directory.write();
        let dir_start = header_blocks_512 * 512;
        let dir_end = dir_start + dir_bytes.len();
        if dir_end <= output.len() {
            output[dir_start..dir_end].copy_from_slice(&dir_bytes);
        }

        // Write file data at their allocated blocks
        for (i, file) in self.files.iter().enumerate() {
            let entry = &directory.entries[i + 1];
            for (blk_idx, &blk_num) in entry.blocks.iter().enumerate() {
                let file_offset = blk_idx * block_size as usize;
                let img_offset = blk_num as usize * block_size as usize;
                let chunk_size = (file.data.len() - file_offset).min(block_size as usize);
                if file_offset < file.data.len() && img_offset + chunk_size <= output.len() {
                    output[img_offset..img_offset + chunk_size]
                        .copy_from_slice(&file.data[file_offset..file_offset + chunk_size]);
                }
            }
        }

        Ok(output)
    }
}

/// Calculate optimal block size — mkgmap ImgFS.calcBlockParam
/// Tries block sizes from 512 upward, picks the one that minimizes total blocks
/// while keeping header blocks <= 240 and total <= 0xFFFE
fn calc_block_size(total_file_size: usize, num_files: usize) -> u32 {
    // Estimate directory entries, accounting for multi-part entries
    // Files > 240 blocks need extra directory entries
    // Rough estimate: assume average file uses ~1.2 entries to be safe
    let dir_entries = (num_files as f64 * 1.5) as usize + 1; // +1 for header entry
    let dir_size = dir_entries * ENTRY_SIZE;
    let header_size = 2 * 512; // 2 blocks of 512 for the header proper
    let overhead = header_size + dir_size;
    let total = total_file_size + overhead;

    // Start at 1024 (not 512) — some Garmin firmware (Alpha 100) may not
    // handle 512-byte blocks correctly in gmapsupp files.
    let mut block_size: u32 = 1024;
    loop {
        let header_blocks = (overhead as u32 + block_size - 1) / block_size;
        let total_blocks = (total as u32 + block_size - 1) / block_size;

        if header_blocks <= TABLE_SIZE as u32 && total_blocks <= 0xFFFE {
            return block_size;
        }

        block_size *= 2;
        if block_size > 0x1000000 {
            // 16MB max
            return block_size / 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_manager_sequential() {
        let mut bm = BlockManager::new(512, 5);
        assert_eq!(bm.allocate().unwrap(), 5);
        assert_eq!(bm.allocate().unwrap(), 6);
        assert_eq!(bm.allocate().unwrap(), 7);
    }

    #[test]
    fn test_calc_block_size_small() {
        let bs = calc_block_size(1000, 3);
        assert_eq!(bs, 1024);
    }

    #[test]
    fn test_calc_block_size_large() {
        // 50 MB of data should need a larger block size
        let bs = calc_block_size(50_000_000, 50);
        assert!(bs > 512);
    }

    #[test]
    fn test_filesystem_single_file() {
        let mut fs = ImgFilesystem::new("Test Map");
        let data = vec![0xAB; 1024];
        fs.add_file("63240001", "TRE", data);
        let img = fs.sync().unwrap();

        // Should be at least 512 bytes (header)
        assert!(img.len() >= 512);

        // Check header signatures
        assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
        assert_eq!(&img[0x41..0x48], b"GARMIN\0");

        // Check partition sig
        assert_eq!(img[0x1FE], 0x55);
        assert_eq!(img[0x1FF], 0xAA);
    }

    #[test]
    fn test_filesystem_multiple_files() {
        let mut fs = ImgFilesystem::new("Multi File Test");
        fs.add_file("63240001", "TRE", vec![0x01; 500]);
        fs.add_file("63240001", "RGN", vec![0x02; 2000]);
        fs.add_file("63240001", "LBL", vec![0x03; 800]);

        let img = fs.sync().unwrap();
        assert!(img.len() >= 512);

        // Verify directory has entries
        // Directory starts at block 2 (offset 1024 for block_size=512)
        let dir_start = 2 * 512;
        // First entry should be special header entry
        assert_eq!(img[dir_start], 0x01); // used flag
    }

    #[test]
    fn test_filesystem_data_integrity() {
        let mut fs = ImgFilesystem::new("Data Test");
        let data = vec![0xDE; 256];
        fs.add_file("00000001", "TRE", data.clone());

        let img = fs.sync().unwrap();

        // Find the data in the output — it should be somewhere after the directory
        let found = img.windows(256).any(|w| w == &data[..]);
        assert!(found, "File data not found in IMG output");
    }
}
