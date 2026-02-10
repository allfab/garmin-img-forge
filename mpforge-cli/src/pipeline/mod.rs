//! Pipeline orchestration module.

pub mod reader;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::Config;
use crate::pipeline::reader::SourceReader;
use anyhow::{anyhow, Result};
use std::time::Instant;
use tracing::info;

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

    // Validate that we have features to process
    if features.is_empty() {
        return Err(anyhow!(
            "No features loaded from {} source(s). Check your input configuration and verify that source files exist and contain valid data. Error handling mode: {}",
            config.inputs.len(),
            config.error_handling
        ));
    }

    // TODO: Story 6.2 - Initialize TileProcessor
    // TODO: Story 5.4 - Initialize MpWriter
    // TODO: Story 6.4 - Process tiles with error handling
    // TODO: Story 7.3 - Generate execution report

    info!("Pipeline completed successfully");
    Ok(())
}
