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
    /// - Real LBL binary content — CP1252-encoded label strings with deduplication
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

        // Story 14.2: Build road network graph (before subfile generation).
        let road_network = crate::routing::graph_builder::build_road_network(&mp_file.polylines);
        let routable_count = mp_file.polylines.iter().filter(|p| p.routing.is_some()).count();
        tracing::info!(
            nodes = road_network.nodes.len(),
            arcs = road_network.arcs.len(),
            road_defs = road_network.road_defs.len(),
            routable_polylines = routable_count,
            ratio = if !road_network.nodes.is_empty() {
                road_network.arcs.len() as f64 / road_network.nodes.len() as f64
            } else {
                0.0
            },
            "Road network graph built"
        );

        use crate::img::lbl::LblWriter;
        use crate::img::net::{NetWriter, SubdivRoadRef};
        use crate::img::nod::{patch_nod2_offsets, NodWriter};
        use crate::img::rgn::RgnWriter;
        use crate::img::tre::{levels_from_mp, TreWriter};
        let levels = levels_from_mp(&mp_file.header);
        let has_routing = !road_network.road_defs.is_empty();

        // Pass 0: build LBL → label_offsets used by NET and RGN passes.
        let lbl = LblWriter::build(mp_file);

        // Pre-compute subdiv_road_refs for NET level divisions.
        // Maps each routable polyline to its road_def index and tracks its
        // sequential position within each subdivision (matching RGN ordering).
        let subdiv_road_refs: Vec<SubdivRoadRef> = if has_routing {
            let mut polyline_road_def: Vec<Option<usize>> = vec![None; mp_file.polylines.len()];
            let mut rd_idx = 0usize;
            for (pi, pl) in mp_file.polylines.iter().enumerate() {
                if pl.routing.is_some() {
                    polyline_road_def[pi] = Some(rd_idx);
                    rd_idx += 1;
                }
            }
            let mut refs = Vec::new();
            for (i, _) in levels.iter().enumerate() {
                let subdiv_number = (i + 1) as u16;
                let mut polyline_index: u8 = 0;
                for (pi, pl) in mp_file.polylines.iter().enumerate() {
                    if pl.end_level.unwrap_or(u8::MAX) >= i as u8 {
                        if let Some(rd_idx) = polyline_road_def[pi] {
                            refs.push(SubdivRoadRef {
                                road_def_idx: rd_idx,
                                subdiv_number,
                                polyline_index,
                            });
                        }
                        polyline_index = polyline_index.saturating_add(1);
                    }
                }
            }
            refs
        } else {
            Vec::new()
        };

        // Pass 1: build NET → road_offsets used by RGN for cross-references.
        // NET is built before RGN so that NET1 offsets are available for embedding in RGN.
        let net = if has_routing {
            let net_result =
                NetWriter::build(&road_network, &lbl.label_offsets, &subdiv_road_refs, &mp_file.polylines);
            tracing::info!(
                road_defs = road_network.road_defs.len(),
                net1_size = net_result.data.len() - 55,
                net3_records = road_network.road_defs.iter().filter(|r| r.label.is_some()).count(),
                "NET subfile encoded"
            );
            Some(net_result)
        } else {
            None
        };

        // Pass 2: build RGN with LBL offsets and NET1 cross-references.
        let net_offsets: Vec<u32> = net.as_ref().map_or_else(Vec::new, |n| n.road_offsets.clone());
        let rgn = RgnWriter::build_with_net_offsets(
            mp_file,
            &levels,
            &lbl.label_offsets,
            &net_offsets,
        );

        // Pass 3: build TRE with RGN subdivision offsets and routing flag.
        let tre_data = if has_routing {
            TreWriter::build_with_rgn_offsets_and_routing(mp_file, &rgn.subdivision_offsets)
        } else {
            TreWriter::build_with_rgn_offsets(mp_file, &rgn.subdivision_offsets)
        };

        // Pass 4: build NOD (after TRE; depends on road_network + net offsets).
        let nod = if has_routing {
            let net_result = net.as_ref().unwrap();
            let nod_result = NodWriter::build(&road_network, &net_result.road_offsets, &mp_file.polylines);
            tracing::info!(
                road_defs = road_network.road_defs.len(),
                nod2_offsets = nod_result.nod2_road_offsets.len(),
                nod_size = nod_result.data.len(),
                "NOD subfile encoded"
            );
            Some(nod_result)
        } else {
            None
        };

        // Patch NOD2 offsets into NET data before adding to the filesystem.
        let net_data_patched: Option<Vec<u8>> = if has_routing {
            let net_result = net.as_ref().unwrap();
            let nod_result = nod.as_ref().unwrap();
            let mut data = net_result.data.clone();
            patch_nod2_offsets(
                &mut data,
                &net_result.nod2_patch_positions,
                &nod_result.nod2_road_offsets,
            );
            Some(data)
        } else {
            None
        };

        fs.add_subfile(map_id, "TRE", tre_data)?;
        fs.add_subfile(map_id, "RGN", rgn.data)?;
        fs.add_subfile(map_id, "LBL", lbl.data)?;
        if has_routing {
            fs.add_subfile(map_id, "NET", net_data_patched.unwrap())?;
            fs.add_subfile(map_id, "NOD", nod.unwrap().data)?;
        }

        // Verify subfile count invariant before serialisation.
        let entry_count = fs.entries.len();
        let expected = if has_routing { 5 } else { 3 };
        debug_assert_eq!(
            entry_count,
            expected,
            "expected {} subfile entries, got {}",
            expected,
            entry_count
        );

        // Capture all per-subfile stats before serialisation.
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
        let (net_offset_b, net_size) = if has_routing {
            (
                fs.entries[3].0.block_start as u64 * block_size as u64,
                fs.entries[3].0.size_allocated,
            )
        } else {
            (0, 0)
        };
        let (nod_offset_b, nod_size) = if has_routing {
            (
                fs.entries[4].0.block_start as u64 * block_size as u64,
                fs.entries[4].0.size_allocated,
            )
        } else {
            (0, 0)
        };

        // Serialise.
        let bytes = fs.to_bytes();

        if has_routing {
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
                net_offset = net_offset_b,
                net_size = net_size,
                nod_offset = nod_offset_b,
                nod_size = nod_size,
                "IMG written (with routing)"
            );
        } else {
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
        }

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
    fn test_writer_lbl_non_empty() {
        // After Story 13.5, LBL subfile must contain real content when features have labels.
        let mp = MpParser::parse_file(&fixture("minimal_for_img.mp")).unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        ImgWriter::write(&mp, tmp.path()).unwrap();
        let bytes = std::fs::read(tmp.path()).unwrap();
        // Directory at block 1 (offset 512 for block_size=512). LBL is Dirent index 2.
        let dir_start = 512usize;
        let lbl_dirent = dir_start + 2 * 32;
        let size_used = u32::from_le_bytes([
            bytes[lbl_dirent + 0x12],
            bytes[lbl_dirent + 0x13],
            bytes[lbl_dirent + 0x14],
            bytes[lbl_dirent + 0x15],
        ]);
        // A minimal LBL with no labels is 29 bytes (28-byte header + 1-byte null sentinel).
        // Any fixture with at least one labeled feature must produce size_used > 29.
        assert!(
            size_used > 29,
            "LBL subfile size_used must be > 29 (header=28 + sentinel=1 is minimum; \
             real labels add more). Got {}",
            size_used
        );
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
