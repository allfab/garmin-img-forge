//! imgforge-cli library: public API for Polish Map (.mp) to IMG compilation.

pub mod cli;
pub mod error;
pub mod img;
pub mod parser;
pub mod report;
pub mod routing;

pub use error::ImgError;
pub use img::assembler::{AssemblyStats, BuildConfig, CompilationStats, GmapsuppAssembler};
pub use img::tdb::{TdbConfig, TdbWriter, TileInfo};
pub use img::writer::ImgWriter;
pub use report::{BuildReport, FeaturesByType, ReportStatus, TileError};

use std::path::Path;

/// Compile a Polish Map (.mp) file to Garmin IMG format.
///
/// Parses the input `.mp` file via [`parser::MpParser`], then writes the
/// Garmin IMG filesystem (header + FAT-like directory + subfile stubs) via
/// [`ImgWriter`].
pub fn compile(input: &Path, output: &Path) -> anyhow::Result<()> {
    use parser::MpParser;

    let mp = MpParser::parse_file(input)?;
    ImgWriter::write(&mp, output)?;
    Ok(())
}

/// Assemble a directory of Polish Map (.mp) tiles into a single `gmapsupp.img`.
///
/// Scans `input_dir` for `.mp` files, compiles each tile in memory, and writes
/// the resulting multi-tile `gmapsupp.img` to `output`.
pub fn build(input_dir: &Path, output: &Path, config: BuildConfig) -> anyhow::Result<AssemblyStats> {
    let stats = GmapsuppAssembler::build(input_dir, output, &config)?;
    tracing::info!(
        tiles = stats.tile_count,
        subfiles = stats.subfile_count,
        bytes = stats.total_bytes,
        jobs = config.jobs,
        "gmapsupp.img assemblé"
    );
    Ok(stats)
}
