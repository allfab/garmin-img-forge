//! Pipeline orchestration module.

pub mod reader;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::{Config, ErrorMode};
use crate::pipeline::reader::{Feature, SourceReader};
use crate::pipeline::tiler::{clip_feature_to_tile, TileBounds, TileProcessor};
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
    // Story 6.1 - Build R-tree spatial index
    info!("Phase 1: Reading sources and building spatial index");
    let start_time = Instant::now();

    let (features, rtree) = SourceReader::read_all_sources(config)?;

    let elapsed = start_time.elapsed();
    info!(
        duration_ms = elapsed.as_millis(),
        feature_count = features.len(),
        "Source reading completed"
    );

    // Log R-tree index statistics
    let global_bbox = rtree.global_bbox();
    info!(
        rtree_size = rtree.tree_size(),
        bbox_min = ?global_bbox.lower(),
        bbox_max = ?global_bbox.upper(),
        "R-tree index ready for tiling"
    );

    // Story 6.2 - Initialize TileProcessor and generate grid
    let tile_processor = TileProcessor::new(config.grid.clone());

    // Generate tile grid with optional spatial filtering
    let tiles = tile_processor.generate_tiles(&rtree, &config.filters);

    // Assign features to tiles via R-tree queries
    let tile_assignments = tile_processor.assign_features_to_tiles(&rtree, tiles);

    // Story 6.3 - Clip features to tile boundaries
    info!("Phase 1.5: Clipping features to tile boundaries");
    let clipping_start = Instant::now();

    let error_mode = ErrorMode::from_str(&config.error_handling).unwrap_or_default();
    let mut tile_features: Vec<(TileBounds, Vec<Feature>)> = Vec::new();
    let mut total_clipped = 0;
    let mut total_skipped = 0;

    for (tile_bounds, feature_ids) in tile_assignments {
        let tile_bbox_geom = tile_bounds.to_gdal_polygon()?;
        let mut clipped_features = Vec::new();

        for &feature_id in &feature_ids {
            let feature = &features[feature_id];

            match clip_feature_to_tile(feature, &tile_bbox_geom, error_mode) {
                Ok(Some(clipped_feature)) => {
                    clipped_features.push(clipped_feature);
                    total_clipped += 1;
                }
                Ok(None) => {
                    total_skipped += 1;
                }
                Err(e) => {
                    // In fail-fast mode, this would have already bailed
                    warn!(error = %e, "Failed to clip feature");
                }
            }
        }

        if !clipped_features.is_empty() {
            info!(
                tile_id = %tile_bounds.tile_id(),
                candidates = feature_ids.len(),
                clipped = clipped_features.len(),
                "Tile clipping completed"
            );
            tile_features.push((tile_bounds, clipped_features));
        }
    }

    let clipping_elapsed = clipping_start.elapsed();
    info!(
        duration_ms = clipping_elapsed.as_millis(),
        tiles_processed = tile_features.len(),
        features_clipped = total_clipped,
        features_skipped = total_skipped,
        "Geometry clipping completed"
    );

    info!(
        tiles_to_process = tile_features.len(),
        "Tiling pipeline ready"
    );

    // TODO: Story 6.4 - Export multi-tiles .mp (one .mp file per tile)
    // For now, we continue with the original single-file export for backward compatibility

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

    // TODO: Story 6.3 - Clip geometries at tile boundaries
    // TODO: Story 6.4 - Process tiles with error handling and export multi-tiles .mp
    // TODO: Story 7.3 - Generate execution report

    info!("Pipeline completed successfully");
    Ok(())
}
