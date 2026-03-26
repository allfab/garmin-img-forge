//! High-level IMG file writer: orchestrates header, directory and subfile stubs.

use std::path::Path;

use crate::error::ImgError;
use crate::img::filesystem::ImgFilesystem;
use crate::parser::mp_types::MpFile;

/// Writes a Garmin IMG file from a parsed Polish Map.
pub struct ImgWriter {
    /// Block size exponent (9 = 512 bytes for tests, 14 = 16 384 bytes for production).
    block_size_exponent: u8,
}

impl ImgWriter {
    /// Create a new writer with the given block size exponent.
    pub fn new(block_size_exponent: u8) -> Self {
        Self {
            block_size_exponent,
        }
    }

    /// Write a Garmin IMG file from a parsed `.mp` file.
    ///
    /// The resulting `.img` contains:
    /// - A valid 512-byte header (magic, date, XOR, DOS signature)
    /// - A FAT-like directory with entries for TRE, RGN and LBL
    /// - Real TRE binary content (geographic index) — levels, subdivisions, RGN offsets
    /// - Real RGN binary content — POI, polyline and polygon records with delta-encoded coordinates
    /// - Empty LBL stub (label strings populated in Story 13.5)
    ///
    /// Uses `block_size_exponent = 9` (512-byte blocks) which is appropriate for
    /// stub/test output. For production maps with real TRE/RGN/LBL content
    /// (Stories 13.3–13.5), use [`ImgWriter::write_with_block_size`] with
    /// exponent 14 (16 384-byte blocks, mkgmap default).
    ///
    /// # Errors
    /// - [`ImgError::InvalidMapId`] if `mp_file.header.id` is not a valid numeric string ≤ 8 chars
    /// - [`ImgError::IoError`] on I/O failure
    pub fn write(mp_file: &MpFile, output: &Path) -> Result<(), ImgError> {
        let writer = Self::new(9); // 512-byte blocks — suitable for test/stub output
        writer.write_with_block_size(mp_file, output)
    }

    /// Write an IMG file using this writer's configured block size.
    pub fn write_with_block_size(&self, mp_file: &MpFile, output: &Path) -> Result<(), ImgError> {
        let map_id = &mp_file.header.id;

        // Validate map ID early.
        if map_id.is_empty() || !map_id.chars().all(|c| c.is_ascii_digit()) || map_id.len() > 8 {
            return Err(ImgError::InvalidMapId { id: map_id.clone() });
        }

        let block_size = 1u32 << self.block_size_exponent;

        // Build filesystem.
        let mut fs = ImgFilesystem::new(self.block_size_exponent);
        fs.description = mp_file.header.name.clone();

        // Add subfiles: real TRE + real RGN geographic content; LBL still a stub (Story 13.5).
        use crate::img::tre::{TreWriter, levels_from_mp};
        use crate::img::rgn::RgnWriter;
        let levels = levels_from_mp(&mp_file.header);
        // Pass 1: build RGN to get per-subdivision offsets.
        let rgn = RgnWriter::build(mp_file, &levels);
        // Pass 2: build TRE with real RGN offsets patched into subdivisions.
        let tre_data = TreWriter::build_with_rgn_offsets(mp_file, &rgn.subdivision_offsets);
        fs.add_subfile(map_id, "TRE", tre_data)?;
        fs.add_subfile(map_id, "RGN", rgn.data)?;
        fs.add_subfile(map_id, "LBL", vec![])?;

        // Capture per-subfile stats before consuming fs into bytes.
        // Order must match add_subfile calls above: TRE=0, RGN=1, LBL=2.
        debug_assert!(
            fs.entries.len() == 3,
            "expected exactly 3 subfile entries (TRE, RGN, LBL), got {}",
            fs.entries.len()
        );
        let (tre_offset_b, tre_size) = (
            fs.entries[0].0.block_start as u64 * block_size as u64,
            fs.entries[0].0.size_allocated,
        );
        let (rgn_offset_b, rgn_size) = (
            fs.entries[1].0.block_start as u64 * block_size as u64,
            fs.entries[1].0.size_allocated,
        );
        let (lbl_offset_b, lbl_size) = (
            fs.entries[2].0.block_start as u64 * block_size as u64,
            fs.entries[2].0.size_allocated,
        );

        // Serialise.
        let bytes = fs.to_bytes();

        tracing::info!(
            map_id = %map_id,
            block_size = block_size,
            total_bytes = bytes.len(),
            tre_offset = tre_offset_b,
            tre_size = tre_size,
            rgn_offset = rgn_offset_b,
            rgn_size = rgn_size,
            lbl_offset = lbl_offset_b,
            lbl_size = lbl_size,
            "IMG written"
        );

        std::fs::write(output, &bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::MpParser;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn test_writer_creates_file() {
        let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        ImgWriter::write(&mp, tmp.path()).unwrap();
        let metadata = std::fs::metadata(tmp.path()).unwrap();
        assert!(metadata.len() > 0, "output file must be non-empty");
    }

    #[test]
    fn test_writer_round_trip() {
        let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        ImgWriter::write(&mp, tmp.path()).unwrap();
        let bytes = std::fs::read(tmp.path()).unwrap();
        // Magic "GARMIN" at 0x002
        assert_eq!(&bytes[0x002..0x008], b"GARMIN");
        // DOS signature at 0x1FE
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
        // XOR of all 512 header bytes must be 0x00.
        let xor = bytes[..512].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(xor, 0x00, "header XOR must be 0x00");
    }

    #[test]
    fn test_writer_invalid_id_empty() {
        use crate::parser::mp_types::{MpFile, MpHeader};
        let mp = MpFile {
            header: MpHeader {
                id: String::new(),
                ..Default::default()
            },
            points: vec![],
            polylines: vec![],
            polygons: vec![],
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = ImgWriter::write(&mp, tmp.path()).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_writer_invalid_id_non_numeric() {
        use crate::parser::mp_types::{MpFile, MpHeader};
        let mp = MpFile {
            header: MpHeader {
                id: "NOTNUM".to_string(),
                ..Default::default()
            },
            points: vec![],
            polylines: vec![],
            polygons: vec![],
        };
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let err = ImgWriter::write(&mp, tmp.path()).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }
}
