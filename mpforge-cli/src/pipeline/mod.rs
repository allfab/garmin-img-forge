//! Pipeline orchestration module.

pub mod reader;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::{Config, ErrorMode};
use crate::pipeline::reader::{Feature, SourceReader};
use crate::pipeline::tiler::{clip_feature_to_tile, TileBounds, TileProcessor};
use crate::pipeline::writer::{ExportStats, MpWriter};
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::time::{Instant, SystemTime};
use tracing::{info, warn};

/// Summary of multi-tile export operation.
#[derive(Debug, Clone)]
pub struct TileExportSummary {
    pub tiles_succeeded: usize,
    pub tiles_failed: usize,
    pub tiles_skipped: usize,
    pub global_stats: ExportStats,
    pub export_errors: Vec<TileExportError>,
}

/// Error details for a failed tile export.
#[derive(Debug, Clone)]
pub struct TileExportError {
    pub tile_id: String,
    pub error_message: String,
    pub attempt_time: SystemTime,
}

impl TileExportSummary {
    /// Create summary from export results.
    pub fn new(
        tiles_succeeded: usize,
        tiles_failed: usize,
        tiles_skipped: usize,
        global_stats: ExportStats,
        export_errors: Vec<TileExportError>,
    ) -> Self {
        Self {
            tiles_succeeded,
            tiles_failed,
            tiles_skipped,
            global_stats,
            export_errors,
        }
    }

    /// Total features exported across all tiles.
    pub fn total_features(&self) -> usize {
        self.global_stats.point_count
            + self.global_stats.linestring_count
            + self.global_stats.polygon_count
    }

    /// Check if export was successful (0 failures).
    pub fn is_success(&self) -> bool {
        self.tiles_failed == 0
    }
}

/// Run the complete tiling pipeline.
/// Orchestrates reader, tiler, and writer components.
///
/// Returns `TileExportSummary` with export statistics and errors.
#[tracing::instrument(skip(config, args))]
pub fn run(config: &Config, args: &BuildArgs) -> Result<TileExportSummary> {
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

    let error_mode = config
        .error_handling
        .parse::<ErrorMode>()
        .unwrap_or_else(|e| {
            warn!(
                error_handling = %config.error_handling,
                error = %e,
                "Invalid error_handling mode in config, defaulting to 'continue'"
            );
            ErrorMode::default()
        });
    let mut tile_features: Vec<(TileBounds, Vec<Feature>)> = Vec::new();
    let mut total_clipped = 0;
    let mut total_skipped = 0;
    let mut clipping_errors: Vec<(String, usize, String)> = Vec::new(); // (tile_id, feature_id, error)

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
                    // In continue mode, collect error for final report
                    warn!(
                        tile_id = %tile_bounds.tile_id(),
                        feature_id,
                        error = %e,
                        "Failed to clip feature"
                    );
                    clipping_errors.push((tile_bounds.tile_id(), feature_id, e.to_string()));
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

    // Report clipping errors if any (mode Continue)
    if !clipping_errors.is_empty() {
        warn!(
            error_count = clipping_errors.len(),
            "Geometry clipping completed with errors"
        );
        // TODO Story 7.3: Include clipping_errors in execution report JSON
    }

    info!(
        duration_ms = clipping_elapsed.as_millis(),
        tiles_processed = tile_features.len(),
        features_clipped = total_clipped,
        features_skipped = total_skipped,
        features_failed = clipping_errors.len(),
        "Geometry clipping completed"
    );

    info!(
        tiles_to_process = tile_features.len(),
        "Tiling pipeline ready"
    );

    // Story 6.4 - Export multi-tiles .mp (one .mp file per tile)
    info!(
        "Phase 2: Exporting {} tiles as .mp files",
        tile_features.len()
    );
    let export_start = Instant::now();

    // Ensure output directory exists
    std::fs::create_dir_all(&config.output.directory)
        .context("Failed to create output directory")?;

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut global_stats = ExportStats::default();
    let mut export_errors: Vec<TileExportError> = Vec::new();

    for (tile_bounds, features) in tile_features {
        let tile_id = tile_bounds.tile_id();

        // Skip empty tiles
        if features.is_empty() {
            tracing::debug!(tile_id = %tile_id, "Tile has no features, skipping export");
            skipped += 1;
            continue;
        }

        // Resolve tile filename
        let tile_filename = format!("{}.mp", tile_id);
        let tile_path = PathBuf::from(&config.output.directory).join(tile_filename);

        // Create writer for this tile
        let mut writer = match MpWriter::new(tile_path) {
            Ok(w) => w,
            Err(e) => {
                handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
                continue;
            }
        };

        // Write features
        let tile_stats = match writer.write_features(&features) {
            Ok(stats) => stats,
            Err(e) => {
                handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
                continue;
            }
        };

        // Finalize tile dataset
        if let Err(e) = writer.finalize() {
            handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
            continue;
        }

        info!(
            tile_id = %tile_id,
            points = tile_stats.point_count,
            linestrings = tile_stats.linestring_count,
            polygons = tile_stats.polygon_count,
            "Tile export succeeded"
        );

        // Aggregate global stats
        global_stats.point_count += tile_stats.point_count;
        global_stats.linestring_count += tile_stats.linestring_count;
        global_stats.polygon_count += tile_stats.polygon_count;
        succeeded += 1;
    }

    let export_elapsed = export_start.elapsed();

    // Report export errors if any (mode Continue)
    if !export_errors.is_empty() {
        warn!(
            error_count = export_errors.len(),
            "Multi-tile export completed with errors"
        );
        // TODO Story 7.3: Include export_errors in execution report JSON
    }

    let summary = TileExportSummary::new(succeeded, failed, skipped, global_stats, export_errors);

    // Display final summary (AC4)
    info!(
        duration_ms = export_elapsed.as_millis(),
        tiles_succeeded = summary.tiles_succeeded,
        tiles_failed = summary.tiles_failed,
        tiles_skipped = summary.tiles_skipped,
        total_features = summary.total_features(),
        "Multi-tile export completed"
    );

    // Console summary output (AC4)
    println!("\n✅ Export completed successfully!");
    println!("   Output directory: {}", config.output.directory);
    println!("   Tiles generated:");
    println!("     - Succeeded: {}", summary.tiles_succeeded);
    println!("     - Failed:    {}", summary.tiles_failed);
    println!("     - Skipped:   {} (empty tiles)", summary.tiles_skipped);
    println!("   Features exported:");
    println!("     - POI (points):     {}", summary.global_stats.point_count);
    println!("     - POLYLINE (lines): {}", summary.global_stats.linestring_count);
    println!("     - POLYGON (areas):  {}", summary.global_stats.polygon_count);
    println!("   Total: {} features", summary.total_features());
    println!("   Duration: {:.2}s", export_elapsed.as_secs_f64());

    // Fail if any tiles failed (exit code non-zero)
    if !summary.is_success() {
        return Err(anyhow!(
            "Multi-tile export failed: {} tile(s) failed to export",
            summary.tiles_failed
        ));
    }

    // TODO: Story 7.3 - Generate execution report JSON

    info!("Pipeline completed successfully");
    Ok(summary)
}

/// Handle tile export error based on ErrorMode.
fn handle_export_error(
    tile_id: &str,
    error: anyhow::Error,
    error_mode: ErrorMode,
    failed_count: &mut usize,
    error_list: &mut Vec<TileExportError>,
) -> Result<()> {
    *failed_count += 1;

    match error_mode {
        ErrorMode::Continue => {
            warn!(
                tile_id = %tile_id,
                error = %error,
                "Tile export failed, continuing with next tile"
            );
            error_list.push(TileExportError {
                tile_id: tile_id.to_string(),
                error_message: error.to_string(),
                attempt_time: SystemTime::now(),
            });
            Ok(()) // Continue pipeline
        }
        ErrorMode::FailFast => Err(anyhow!(
            "Tile export failed (fail-fast mode): tile {} - {}",
            tile_id,
            error
        )),
    }
}
