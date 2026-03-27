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
use std::time::Instant;

use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

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
    /// Number of parallel compilation threads.
    /// `0` means auto-detect via `std::thread::available_parallelism()`.
    pub jobs: usize,
    /// Show indicatif progress bar during compilation.
    /// Set to `false` in verbose mode to avoid interleaving with tracing logs.
    pub show_progress: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            family_id: 6324,
            product_id: 1,
            description: "mpforge map".into(),
            block_size_exponent: 14,
            typ_file: None,
            jobs: 0, // 0 = auto
            show_progress: true,
        }
    }
}

// ── CompilationStats ──────────────────────────────────────────────────────────

/// Compilation statistics aggregated across all tiles.
#[derive(Debug, Clone)]
pub struct CompilationStats {
    pub poi_count: usize,
    pub polyline_count: usize,
    pub polygon_count: usize,
    pub routing_nodes: usize,
    pub routing_arcs: usize,
    pub tiles_failed: usize,
    pub tile_errors: Vec<crate::report::TileError>,
    pub duration_seconds: f64,
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
    /// Compilation-phase statistics (features, routing, errors, duration).
    pub compilation: CompilationStats,
}

// ── TileCompileResult ─────────────────────────────────────────────────────────

/// Result of compiling a single tile (parallel phase).
struct TileCompileResult {
    /// Owned subfile data ready to merge into outer_fs.
    subfiles: Vec<(String, String, Vec<u8>)>,
    /// TileInfo for TDB generation.
    tile_info: TileInfo,
    /// Feature statistics for the report.
    poi_count: usize,
    polyline_count: usize,
    polygon_count: usize,
    /// Routing statistics.
    routing_node_count: usize,
    routing_arc_count: usize,
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
    /// - `config`: Assembly configuration (family_id, product_id, description, block_size, jobs).
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

        // Resolve effective thread count (cap at 1024 to prevent resource exhaustion).
        let effective_jobs = if config.jobs == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        } else {
            config.jobs.min(1024)
        };

        // Build rayon thread pool.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(effective_jobs)
            .build()
            .map_err(|e| ImgError::IoError(std::io::Error::other(e.to_string())))?;

        // Progress bar — hidden in verbose mode to avoid interleaving with tracing logs.
        let total = mp_files.len() as u64;
        let pb = if config.show_progress {
            ProgressBar::new(total)
        } else {
            ProgressBar::hidden()
        };
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:40}] {pos}/{len} tiles ({elapsed_precise})")
                .unwrap()
                .progress_chars("##-"),
        );

        let start = Instant::now();

        // ── Phase 1 : parallel compilation ───────────────────────────────────
        let mut compile_results: Vec<(PathBuf, Result<TileCompileResult, ImgError>)> =
            pool.install(|| {
                mp_files
                    .par_iter()
                    .map(|mp_path| {
                        let result =
                            Self::compile_tile_parallel(mp_path, config.block_size_exponent);
                        pb.inc(1);
                        (mp_path.clone(), result)
                    })
                    .collect()
            });

        pb.finish_with_message("compilation terminée");

        // Sort by path for deterministic FAT ordering regardless of par_iter completion order.
        compile_results.sort_by(|a, b| a.0.cmp(&b.0));

        // ── Phase 2 : sequential merge + stats aggregation ───────────────────
        let mut outer_fs = ImgFilesystem::new(config.block_size_exponent);
        outer_fs.description = config.description.clone();
        outer_fs.family_id = config.family_id;
        outer_fs.product_id = config.product_id;

        let mut compiled_tiles = 0usize;
        let mut failed_tiles = 0usize;
        let mut tile_infos: Vec<TileInfo> = Vec::new();
        let mut total_poi = 0usize;
        let mut total_polyline = 0usize;
        let mut total_polygon = 0usize;
        let mut total_routing_nodes = 0usize;
        let mut total_routing_arcs = 0usize;
        let mut tile_errors: Vec<crate::report::TileError> = Vec::new();

        for (mp_path, result) in compile_results {
            match result {
                Ok(tile_result) => {
                    for (mid, ext, data) in tile_result.subfiles {
                        outer_fs.add_subfile(&mid, &ext, data)?;
                    }
                    compiled_tiles += 1;
                    tile_infos.push(tile_result.tile_info);
                    total_poi += tile_result.poi_count;
                    total_polyline += tile_result.polyline_count;
                    total_polygon += tile_result.polygon_count;
                    total_routing_nodes += tile_result.routing_node_count;
                    total_routing_arcs += tile_result.routing_arc_count;
                }
                Err(e) => {
                    failed_tiles += 1;
                    tracing::warn!(
                        path = %mp_path.display(),
                        error = %e,
                        "Skipping tile (compilation failed)"
                    );
                    tile_errors.push(crate::report::TileError {
                        tile: mp_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        error: e.to_string(),
                    });
                }
            }
        }

        let duration_seconds = start.elapsed().as_secs_f64();

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

        let compilation = CompilationStats {
            poi_count: total_poi,
            polyline_count: total_polyline,
            polygon_count: total_polygon,
            routing_nodes: total_routing_nodes,
            routing_arcs: total_routing_arcs,
            tiles_failed: failed_tiles,
            tile_errors,
            duration_seconds,
        };

        tracing::info!(
            tiles = compiled_tiles,
            subfiles = subfile_count,
            total_bytes = total_bytes,
            tdb = %tdb_path.display(),
            jobs = effective_jobs,
            duration_secs = duration_seconds,
            "gmapsupp.img assembled"
        );

        Ok(AssemblyStats {
            tile_count: compiled_tiles,
            subfile_count,
            total_bytes,
            tdb_path,
            typ_embedded,
            srt_embedded: true,
            compilation,
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

    /// Parse and compile a single `.mp` tile without mutating shared state.
    ///
    /// Returns a [`TileCompileResult`] with owned subfile bytes, TileInfo, and stats.
    /// Designed for use inside `rayon::par_iter`.
    fn compile_tile_parallel(
        mp_path: &Path,
        block_size_exponent: u8,
    ) -> Result<TileCompileResult, ImgError> {
        let mp = MpParser::parse_file(mp_path)
            .map_err(|e| ImgError::IoError(std::io::Error::other(e.to_string())))?;

        // Collect feature stats from parsed mp.
        let poi_count = mp.points.len();
        let polyline_count = mp.polylines.len();
        let polygon_count = mp.polygons.len();

        // Collect routing stats — build_road_network is pure/fast.
        // Note: ImgWriter::build_filesystem calls it again internally (intentional duplication).
        let road_network =
            crate::routing::graph_builder::build_road_network(&mp.polylines);
        let routing_node_count = road_network.nodes.len();
        let routing_arc_count = road_network.arcs.len();

        // Build in-memory filesystem (owns all subfile bytes).
        let map_id = mp.header.id.clone();
        let tile_fs = ImgWriter::build_filesystem(&mp, block_size_exponent)?;

        // Collect subfiles as owned data.
        let subfiles: Vec<(String, String, Vec<u8>)> = tile_fs
            .subfiles()
            .map(|(mid, ext, data)| (mid.to_string(), ext.to_string(), data.to_vec()))
            .collect();

        // Compute bounding box for TDB.
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(&mp);
        let map_id_u32 = map_id.parse::<u32>().map_err(|_| {
            ImgError::IoError(std::io::Error::other(format!(
                "Invalid map ID '{}': expected decimal u32",
                map_id
            )))
        })?;

        tracing::debug!(
            map_id = %map_id,
            subfiles = subfiles.len(),
            "Tile compiled (parallel)"
        );

        Ok(TileCompileResult {
            subfiles,
            tile_info: TileInfo {
                map_id: map_id_u32,
                north: max_lat,
                east: max_lon,
                south: min_lat,
                west: min_lon,
                description: mp.header.name.clone(),
            },
            poi_count,
            polyline_count,
            polygon_count,
            routing_node_count,
            routing_arc_count,
        })
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
            jobs: 1, // sequential for deterministic tests
            show_progress: false,
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
            GmapsuppAssembler::build(tiles_dir.path(), &output, &test_config_512()).unwrap();
        assert_eq!(stats.tile_count, 1);
        assert!(output.exists());
        let bytes = std::fs::read(&output).unwrap();
        assert!(!bytes.is_empty());
        // Standard Garmin IMG signatures
        assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
        assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
        // Boot signature
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
    }

    #[test]
    fn test_assembler_two_tiles_subfile_count() {
        let tmp_tiles = tempfile::tempdir().unwrap();
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
    fn test_assembler_standard_header_format() {
        let tmp_tiles = tempfile::tempdir().unwrap();
        std::fs::copy(tile_a(), tmp_tiles.path().join("tile_a.mp")).unwrap();

        let config = BuildConfig {
            family_id: 6324,
            product_id: 1,
            description: "Test".into(),
            block_size_exponent: 9,
            typ_file: None,
            jobs: 1,
            show_progress: false,
        };
        let output = tempfile::NamedTempFile::new().unwrap();
        GmapsuppAssembler::build(tmp_tiles.path(), output.path(), &config).unwrap();
        let bytes = std::fs::read(output.path()).unwrap();
        // Standard Garmin IMG header signatures
        assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
        assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
        // Description at 0x049
        assert_eq!(&bytes[0x049..0x049 + 4], b"Test");
    }
}
