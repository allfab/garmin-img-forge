//! Multi-tile gmapsupp.img assembler.
//!
//! [`GmapsuppAssembler`] scans a directory for `.mp` files, compiles each tile
//! into an [`ImgFilesystem`] in memory, and concatenates all subfiles into a
//! single outer `gmapsupp.img`.
//!
//! # Layout of gmapsupp.img
//! ```text
//! Block 0       : Header (family_id, product_id, description)
//! Blocks 1..K   : FAT directory — all subfiles from all tiles
//!                 e.g. 01001001.TRE, 01001001.RGN, 01001001.LBL,
//!                      01001002.TRE, 01001002.RGN, 01001002.LBL, ...
//! Blocks K+1..  : Subfile data (sequential, one per FAT entry)
//! ```

use std::path::{Path, PathBuf};

use crate::error::ImgError;
use crate::img::filesystem::ImgFilesystem;
use crate::img::srt::SrtWriter;
use crate::img::tdb::{TdbConfig, TdbWriter, TileInfo};
use crate::img::tre::TreWriter;
use crate::img::writer::ImgWriter;
use crate::parser::MpParser;

// ── BuildConfig ───────────────────────────────────────────────────────────────

/// Configuration for [`GmapsuppAssembler::build`].
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Garmin family ID (LE16 at header 0x054).
    pub family_id: u16,
    /// Garmin product ID (LE16 at header 0x056).
    pub product_id: u16,
    /// Map description (shown in BaseCamp, max 49 bytes).
    pub description: String,
    /// Block size exponent: `block_size = 1 << block_size_exponent`.
    /// Use 14 (16 384 bytes) for production, 9 (512 bytes) for tests.
    pub block_size_exponent: u8,
    /// Optional TYP file to embed in the gmapsupp.img.
    /// When `Some`, the file is read and added as `{family_id:08}.TYP` subfile.
    pub typ_file: Option<PathBuf>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            family_id: 6324,
            product_id: 1,
            description: "mpforge map".into(),
            block_size_exponent: 14,
            typ_file: None,
        }
    }
}

// ── AssemblyStats ─────────────────────────────────────────────────────────────

/// Statistics from a successful assembly.
#[derive(Debug, Clone)]
pub struct AssemblyStats {
    /// Number of .mp tiles assembled.
    pub tile_count: usize,
    /// Total number of subfiles in the FAT directory.
    pub subfile_count: usize,
    /// Total size of the gmapsupp.img in bytes.
    pub total_bytes: u64,
    /// Path of the generated TDB companion file.
    pub tdb_path: PathBuf,
    /// Whether a TYP file was embedded in the gmapsupp.img.
    pub typ_embedded: bool,
    /// Whether the French CP1252 SRT (Sort Routines) subfile was embedded.
    pub srt_embedded: bool,
}

// ── GmapsuppAssembler ─────────────────────────────────────────────────────────

/// Assembles a directory of `.mp` tiles into a single `gmapsupp.img`.
pub struct GmapsuppAssembler;

impl GmapsuppAssembler {
    /// Compile a directory of `.mp` tiles into a single `gmapsupp.img`.
    ///
    /// # Arguments
    /// - `input_dir`: Directory containing `.mp` files (scanned non-recursively, sorted).
    /// - `output`: Path of the resulting `gmapsupp.img`.
    /// - `config`: Assembly configuration (family_id, product_id, description, block_size).
    ///
    /// # Errors
    /// - [`ImgError::IoError`] if `input_dir` cannot be read or `output` cannot be written.
    /// - [`ImgError::EmptyInputDir`] if no `.mp` files are found in `input_dir`.
    pub fn build(
        input_dir: &Path,
        output: &Path,
        config: &BuildConfig,
    ) -> Result<AssemblyStats, ImgError> {
        // Validate TYP file early — fail before any tile compilation.
        // Carry (path, bytes) together to avoid a second config.typ_file lookup for logging.
        let typ_info: Option<(PathBuf, Vec<u8>)> = if let Some(ref typ_path) = config.typ_file {
            let bytes = std::fs::read(typ_path).map_err(|e| {
                ImgError::IoError(std::io::Error::new(
                    e.kind(),
                    format!("TYP file '{}': {}", typ_path.display(), e),
                ))
            })?;
            Some((typ_path.clone(), bytes))
        } else {
            None
        };

        // Scan and sort .mp files deterministically.
        let mp_files = Self::collect_mp_files(input_dir)?;

        // Create outer filesystem.
        let mut outer_fs = ImgFilesystem::new(config.block_size_exponent);
        outer_fs.description = config.description.clone();
        outer_fs.family_id = config.family_id;
        outer_fs.product_id = config.product_id;

        let mut compiled_tiles: usize = 0;
        let mut tile_infos: Vec<TileInfo> = Vec::new();

        for mp_path in &mp_files {
            match Self::compile_tile(mp_path, config.block_size_exponent, &mut outer_fs) {
                Ok(tile_info) => {
                    compiled_tiles += 1;
                    tile_infos.push(tile_info);
                }
                Err(e) => {
                    tracing::warn!(
                        path = %mp_path.display(),
                        error = %e,
                        "Skipping tile (compilation failed)"
                    );
                }
            }
        }

        if compiled_tiles == 0 {
            return Err(ImgError::AllTilesFailedToCompile {
                path: input_dir.display().to_string(),
                count: mp_files.len(),
            });
        }

        // Embed TYP file into the outer filesystem before serialisation.
        let mut typ_embedded = false;
        if let Some((typ_path, typ_bytes)) = typ_info {
            let typ_map_id = format!("{:08}", config.family_id);
            // Collision guard: tile map_ids come from mp.header.id; a collision would require
            // a tile with an ID numerically equal to family_id (u16, max "00065535").
            debug_assert!(
                !outer_fs.entries.iter().any(|e| e.map_id == typ_map_id && e.ext == "TYP"),
                "TYP subfile collision: a tile already uses map_id={typ_map_id} with ext=TYP"
            );
            outer_fs.add_subfile(&typ_map_id, "TYP", typ_bytes)?;
            typ_embedded = true;
            tracing::info!(typ = %typ_path.display(), "TYP file embedded");
        }

        // Embed SRT (French CP1252 sort routines) — always generated for French maps.
        let srt_map_id = format!("{:08}", config.family_id);
        debug_assert!(
            !outer_fs.entries.iter().any(|e| e.map_id == srt_map_id && e.ext == "SRT"),
            "SRT subfile collision: map_id={srt_map_id} with ext=SRT already exists"
        );
        let srt_bytes = SrtWriter::build_french_cp1252();
        outer_fs.add_subfile(&srt_map_id, "SRT", srt_bytes)?;
        tracing::info!("SRT file embedded (French CP1252 collation)");

        let subfile_count = outer_fs.entry_count();

        // Serialise and write to disk.
        let bytes = outer_fs.to_bytes();
        let total_bytes = bytes.len() as u64;

        std::fs::write(output, &bytes)?;

        // Generate companion TDB file.
        let tdb_path = output.with_extension("tdb");
        let tdb_config = TdbConfig {
            family_id: config.family_id,
            product_id: config.product_id,
            series_name: config.description.clone(),
        };
        TdbWriter::write(&tdb_path, &tdb_config, &tile_infos)?;

        tracing::info!(
            tiles = compiled_tiles,
            subfiles = subfile_count,
            total_bytes = total_bytes,
            tdb = %tdb_path.display(),
            "gmapsupp.img assembled"
        );

        Ok(AssemblyStats {
            tile_count: compiled_tiles,
            subfile_count,
            total_bytes,
            tdb_path,
            typ_embedded,
            srt_embedded: true,
        })
    }

    /// Scan `input_dir` for `.mp` files, sort them alphabetically.
    fn collect_mp_files(input_dir: &Path) -> Result<Vec<PathBuf>, ImgError> {
        let mut mp_files: Vec<PathBuf> = std::fs::read_dir(input_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("mp")))
            .collect();

        mp_files.sort(); // deterministic ordering

        if mp_files.is_empty() {
            return Err(ImgError::EmptyInputDir {
                path: input_dir.display().to_string(),
            });
        }

        Ok(mp_files)
    }

    /// Parse and compile a single `.mp` tile, adding its subfiles to `outer_fs`.
    ///
    /// Returns a [`TileInfo`] with the tile's bounding box and map ID for TDB generation.
    fn compile_tile(
        mp_path: &Path,
        block_size_exponent: u8,
        outer_fs: &mut ImgFilesystem,
    ) -> Result<TileInfo, ImgError> {
        let mp = MpParser::parse_file(mp_path)
            .map_err(|e| ImgError::IoError(std::io::Error::other(e.to_string())))?;

        // map_id is validated inside build_filesystem and add_subfile.
        let map_id = &mp.header.id;
        let tile_fs = ImgWriter::build_filesystem(&mp, block_size_exponent)?;

        for (tile_map_id, ext, data) in tile_fs.subfiles() {
            outer_fs.add_subfile(tile_map_id, ext, data.to_vec())?;
        }

        // Collect bounding box for TDB generation.
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(&mp);
        let map_id_u32 = mp.header.id.parse::<u32>().map_err(|_| {
            ImgError::IoError(std::io::Error::other(format!(
                "Invalid map ID '{}': expected decimal u32",
                mp.header.id
            )))
        })?;
        let tile_info = TileInfo {
            map_id: map_id_u32,
            north: max_lat,
            east: max_lon,
            south: min_lat,
            west: min_lon,
            description: mp.header.name.clone(),
        };

        tracing::debug!(
            map_id = %map_id,
            subfiles = tile_fs.entry_count(),
            "Tile compiled and added to outer filesystem"
        );

        Ok(tile_info)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }

    fn tile_a() -> PathBuf {
        fixture_dir().join("tile_a.mp")
    }

    fn tile_b() -> PathBuf {
        fixture_dir().join("tile_b.mp")
    }

    fn test_config_512() -> BuildConfig {
        BuildConfig {
            family_id: 6324,
            product_id: 1,
            description: "Test Assembly".into(),
            block_size_exponent: 9, // 512 bytes — fast for tests
            typ_file: None,
        }
    }

    #[test]
    fn test_assembler_empty_dir_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("out.img");
        let result = GmapsuppAssembler::build(tmp.path(), &output, &test_config_512());
        assert!(matches!(result, Err(ImgError::EmptyInputDir { .. })));
    }

    #[test]
    fn test_assembler_nonexistent_dir_returns_error() {
        let output = std::env::temp_dir().join("out_nonexistent.img");
        let result = GmapsuppAssembler::build(
            Path::new("/nonexistent/directory/path"),
            &output,
            &test_config_512(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_assembler_single_tile() {
        let tiles_dir = tempfile::tempdir().unwrap();
        std::fs::copy(tile_a(), tiles_dir.path().join("tile_a.mp")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("gmapsupp.img");
        let stats =
            GmapsuppAssembler::build(tiles_dir.path(), &output, &test_config_512())
                .unwrap();
        assert_eq!(stats.tile_count, 1);
        assert!(output.exists());
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.len() > 0);
        // Valid GARMIN header
        assert_eq!(&bytes[0x002..0x008], b"GARMIN");
        // DOS signature
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
        // XOR invariant
        let xor = bytes[..512].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(xor, 0x00, "header XOR must be 0x00");
    }

    #[test]
    fn test_assembler_two_tiles_subfile_count() {
        let tmp_tiles = tempfile::tempdir().unwrap();
        // Copy tile_a and tile_b into a fresh directory
        std::fs::copy(tile_a(), tmp_tiles.path().join("tile_a.mp")).unwrap();
        std::fs::copy(tile_b(), tmp_tiles.path().join("tile_b.mp")).unwrap();

        let output = tempfile::NamedTempFile::new().unwrap();
        let stats =
            GmapsuppAssembler::build(tmp_tiles.path(), output.path(), &test_config_512()).unwrap();
        assert_eq!(stats.tile_count, 2, "must compile exactly 2 tiles");
        // Each non-routing tile has 3 subfiles, routing has 5.
        // tile_a has routing → 5, tile_b has routing → 5 → total ≥ 6
        assert!(stats.subfile_count >= 6, "must have at least 6 subfiles for 2 tiles");
    }

    #[test]
    fn test_assembler_family_id_in_header() {
        let tmp_tiles = tempfile::tempdir().unwrap();
        std::fs::copy(tile_a(), tmp_tiles.path().join("tile_a.mp")).unwrap();

        let config = BuildConfig {
            family_id: 6324,
            product_id: 1,
            description: "Test".into(),
            block_size_exponent: 9,
            typ_file: None,
        };
        let output = tempfile::NamedTempFile::new().unwrap();
        GmapsuppAssembler::build(tmp_tiles.path(), output.path(), &config).unwrap();
        let bytes = std::fs::read(output.path()).unwrap();
        let fid = u16::from_le_bytes([bytes[0x054], bytes[0x055]]);
        let pid = u16::from_le_bytes([bytes[0x056], bytes[0x057]]);
        assert_eq!(fid, 6324, "family_id must be at header offset 0x054");
        assert_eq!(pid, 1, "product_id must be at header offset 0x056");
    }
}
