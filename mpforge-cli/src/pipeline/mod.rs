//! Pipeline orchestration module.

pub mod reader;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::Config;
use crate::pipeline::reader::SourceReader;
use crate::pipeline::writer::MpWriter;
use anyhow::{Context, Result};
use std::time::Instant;
use tracing::{info, warn};

/// Run the complete tiling pipeline.
/// Orchestrates reader, tiler, and writer components.
#[tracing::instrument(skip(config, args))]
pub fn run(config: &Config, args: &BuildArgs) -> Result<()> {
    info!(
        "Pipeline started with config version {} and {} jobs",
        config.version, args.jobs
    );
    info!("Grid cell size: {}", config.grid.cell_size);
    info!("Output directory: {}", config.output.directory);
    info!("Error handling mode: {}", config.error_handling);

    // Story 5.3 - Read all sources
    info!("Phase 1: Reading sources");
    let start_time = Instant::now();

    let features = SourceReader::read_all_sources(config)?;

    let elapsed = start_time.elapsed();
    info!(
        duration_ms = elapsed.as_millis(),
        feature_count = features.len(),
        "Source reading completed"
    );

    // Story 5.4 - Export to Polish Map format
    info!("Phase 2: Writing MP file");
    let export_start = Instant::now();

    // Warn if no features (AC5)
    if features.is_empty() {
        warn!(
            "No features loaded from {} source(s). Creating empty dataset.",
            config.inputs.len()
        );
    }

    // Initialize MP writer
    let mut writer = MpWriter::new(&config.output).context("Failed to initialize MP writer")?;

    // Write features to .mp file
    let stats = writer
        .write_features(&features)
        .context("Failed to write features to MP file")?;

    // Finalize and close dataset
    writer.finalize().context("Failed to finalize MP file")?;

    let export_elapsed = export_start.elapsed();

    // Display summary (AC4)
    info!(
        duration_ms = export_elapsed.as_millis(),
        points = stats.point_count,
        linestrings = stats.linestring_count,
        polygons = stats.polygon_count,
        total_features = stats.point_count + stats.linestring_count + stats.polygon_count,
        "MP export completed"
    );

    // Console summary output (AC4)
    let output_path =
        std::path::PathBuf::from(&config.output.directory).join(&config.output.filename_pattern);

    println!("\n✅ Export completed successfully!");
    println!("   Output file: {}", output_path.display());
    println!("   Features exported:");
    println!("     - POI (points):     {}", stats.point_count);
    println!("     - POLYLINE (lines): {}", stats.linestring_count);
    println!("     - POLYGON (areas):  {}", stats.polygon_count);
    println!(
        "   Total: {} features",
        stats.point_count + stats.linestring_count + stats.polygon_count
    );
    println!("   Duration: {:.2}s", export_elapsed.as_secs_f64());

    // TODO: Story 6.2 - Initialize TileProcessor
    // TODO: Story 6.4 - Process tiles with error handling
    // TODO: Story 7.3 - Generate execution report

    info!("Pipeline completed successfully");
    Ok(())
}
