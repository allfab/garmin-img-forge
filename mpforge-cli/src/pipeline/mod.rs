//! Pipeline orchestration module.

pub mod geometry_validator;
pub mod reader;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::{Config, ErrorMode};
use crate::pipeline::geometry_validator::ValidationStats;
use crate::pipeline::reader::{Feature, SourceReader};
use crate::pipeline::tiler::{clip_feature_to_tile, TileBounds, TileProcessor};
use crate::pipeline::writer::{ExportStats, MpWriter};
use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Maximum number of errors to display in console summary.
/// Story 7.3 - M4 Fix: Named constant instead of magic number.
const MAX_CONSOLE_ERRORS: usize = 5;

/// Summary of multi-tile export operation.
#[derive(Debug, Clone)]
pub struct TileExportSummary {
    pub tiles_succeeded: usize,
    pub tiles_failed: usize,
    pub tiles_skipped: usize,
    pub global_stats: ExportStats,
    pub export_errors: Vec<TileExportError>,
    /// Story 6.5: Geometry validation statistics for programmatic access.
    pub validation_stats: Option<ValidationStats>,
}

/// Error details for a failed tile export.
/// Story 7.3 - M2 Fix: Removed unused attempt_time field.
#[derive(Debug, Clone)]
pub struct TileExportError {
    pub tile_id: String,
    pub error_message: String,
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
            validation_stats: None,
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
    // Story 6.6 - Collect unsupported geometry type stats
    info!("Phase 1: Reading sources and building spatial index");
    let start_time = Instant::now();

    let (features, rtree, unsupported_type_stats) = SourceReader::read_all_sources(config)?;

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
    let mut global_validation_stats = ValidationStats::default();

    for (tile_bounds, feature_ids) in tile_assignments {
        let tile_bbox_geom = tile_bounds.to_gdal_polygon()?;
        let mut clipped_features = Vec::new();

        for &feature_id in &feature_ids {
            let feature = &features[feature_id];
            let _span = tracing::info_span!("validate_feature", fid = feature_id, tile = %tile_bounds.tile_id()).entered();

            match clip_feature_to_tile(feature, &tile_bbox_geom, error_mode, &mut global_validation_stats) {
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

    // Convert clipping errors to TileExportError format (Story 7.3 - H1 Fix)
    let clipping_export_errors: Vec<TileExportError> = clipping_errors
        .iter()
        .map(|(tile_id, feature_id, error_msg)| TileExportError {
            tile_id: tile_id.clone(),
            error_message: format!("Clipping failed for feature {}: {}", feature_id, error_msg),
        })
        .collect();

    // Report clipping errors if any (mode Continue)
    if !clipping_export_errors.is_empty() {
        warn!(
            error_count = clipping_export_errors.len(),
            "Geometry clipping completed with errors"
        );
    }

    // Story 6.5: Log validation summary
    if global_validation_stats.rejected_count() > 0 || global_validation_stats.repaired_count() > 0 {
        info!(
            valid = global_validation_stats.valid_count,
            repaired_make_valid = global_validation_stats.repaired_make_valid,
            repaired_buffer_zero = global_validation_stats.repaired_buffer_zero,
            rejected_coords = global_validation_stats.rejected_invalid_coords,
            rejected_irrecoverable = global_validation_stats.rejected_irrecoverable,
            recovery_rate = %format!("{:.1}%", global_validation_stats.recovery_rate() * 100.0),
            "Geometry validation summary"
        );
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

    // Validate jobs parameter
    let jobs = args.validate_jobs()?;

    // Story 6.4 & 7.1 - Export multi-tiles .mp (sequential or parallel)
    info!(
        "Phase 2: Exporting {} tiles as .mp files (jobs: {})",
        tile_features.len(),
        jobs
    );
    let export_start = Instant::now();

    // Story 7.2 - Create progress bar (disabled if verbose >= 2 to avoid log pollution)
    let progress = if args.verbose < 2 {
        Some(create_progress_bar(tile_features.len()))
    } else {
        info!("Progress bar disabled in debug mode (verbose >= 2)");
        None
    };

    // Choose export strategy based on jobs parameter
    let mut summary = if jobs == 1 {
        // Sequential export (Epic 6 behavior, debug mode)
        export_tiles_sequential(tile_features, config, error_mode, &progress)?
    } else {
        // Parallel export (Epic 7 Story 7.1)
        export_tiles_parallel(tile_features, config, error_mode, jobs, &progress)?
    };

    // Story 7.3 - H1 Fix: Merge clipping errors into export errors
    summary.export_errors.extend(clipping_export_errors);

    // Story 6.5: Include validation stats for programmatic access (M4 fix)
    summary.validation_stats = Some(global_validation_stats);

    // Story 7.2 - Finalize progress bar
    if let Some(pb) = progress {
        pb.finish_with_message(format!(
            "✓ Export terminé: {} tuiles réussies, {} échouées",
            summary.tiles_succeeded,
            summary.tiles_failed
        ));
    }

    let export_elapsed = export_start.elapsed();

    // Display final summary (AC4)
    info!(
        duration_ms = export_elapsed.as_millis(),
        tiles_succeeded = summary.tiles_succeeded,
        tiles_failed = summary.tiles_failed,
        tiles_skipped = summary.tiles_skipped,
        total_features = summary.total_features(),
        "Multi-tile export completed"
    );

    // Story 7.3 - Task 4: Calculate total pipeline duration
    let total_duration = start_time.elapsed().as_secs_f64();

    // Story 6.6 - Build quality section from unsupported type stats
    // Code Review M1 Fix: Include total_sources in report
    let quality = if unsupported_type_stats.is_empty() {
        None
    } else {
        let unsupported_types = unsupported_type_stats
            .by_type
            .iter()
            .map(|(type_name, entry)| {
                (
                    type_name.clone(),
                    crate::report::UnsupportedTypeReport {
                        count: entry.count,
                        sources: entry.sources.clone(),
                        total_sources: if entry.total_sources > entry.sources.len() {
                            Some(entry.total_sources)
                        } else {
                            None
                        },
                    },
                )
            })
            .collect();
        Some(crate::report::QualitySection { unsupported_types })
    };

    // Story 7.3 - Task 5: Build ExecutionReport from TileExportSummary
    let report = crate::report::ExecutionReport {
        status: if summary.is_success() {
            crate::report::ReportStatus::Success
        } else {
            crate::report::ReportStatus::Failure
        },
        tiles_generated: summary.tiles_succeeded,
        tiles_failed: summary.tiles_failed,
        tiles_skipped: summary.tiles_skipped,
        features_processed: summary.total_features(),
        duration_seconds: total_duration,
        errors: summary
            .export_errors
            .iter()
            .map(|e| crate::report::TileError {
                tile: e.tile_id.clone(),
                error: e.error_message.clone(),
            })
            .collect(),
        quality,
    };

    // Story 7.3 - Task 5: Write JSON report if requested
    // M3 Fix: Use error!() for better visibility of JSON write failures
    if let Some(report_path) = &args.report {
        if let Err(e) = crate::report::write_json_report(&report, report_path) {
            tracing::error!(
                path = %report_path,
                error = %e,
                "ÉCHEC CRITIQUE: Impossible d'écrire le rapport JSON. Le pipeline continue mais le rapport est manquant."
            );
        } else {
            info!(path = %report_path, "Rapport JSON écrit avec succès");
        }
    }

    // Story 7.3 - Task 6: Improved console summary with structured output
    print_console_summary(&report, &config.output.directory, args);

    // Story 7.3 - Task 5: Exit with appropriate code for CI/CD
    if !summary.is_success() {
        warn!(
            tiles_failed = summary.tiles_failed,
            "Pipeline completed with errors, exiting with code 1"
        );
        std::process::exit(1);
    }

    info!("Pipeline completed successfully");
    Ok(summary)
}

/// Create a thread-safe progress bar for multi-tile export.
/// Returns Arc<ProgressBar> for sharing across threads.
fn create_progress_bar(total: usize) -> Arc<ProgressBar> {
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{bar:40.cyan/blue}] {pos}/{len} tuiles ({percent}%) - ETA: {eta}")
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    Arc::new(pb)
}

/// Export tiles sequentially (Epic 6 behavior, debug mode).
/// Story 7.2 enhancement: Added progress bar support.
fn export_tiles_sequential(
    tile_features: Vec<(TileBounds, Vec<Feature>)>,
    config: &Config,
    error_mode: ErrorMode,
    progress: &Option<Arc<ProgressBar>>,
) -> Result<TileExportSummary> {
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
            debug!(tile_id = %tile_id, "Tile has no features, skipping export");
            skipped += 1;
            // Story 7.2 - Increment progress even for skipped tiles
            if let Some(pb) = progress {
                pb.inc(1);
            }
            continue;
        }

        // Resolve tile filename
        let tile_filename = format!("{}.mp", tile_id);
        let tile_path = PathBuf::from(&config.output.directory).join(tile_filename);

        // Create writer for this tile (Story 7.4: with optional field mapping)
        let field_mapping = config.output.field_mapping_path.as_deref();
        let mut writer = match MpWriter::new(tile_path, field_mapping) {
            Ok(w) => w,
            Err(e) => {
                handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
                // Story 7.2 - Increment progress even on error
                if let Some(pb) = progress {
                    pb.inc(1);
                }
                continue;
            }
        };

        // Write features
        let tile_stats = match writer.write_features(&features) {
            Ok(stats) => stats,
            Err(e) => {
                handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
                // Story 7.2 - Increment progress even on error
                if let Some(pb) = progress {
                    pb.inc(1);
                }
                continue;
            }
        };

        // Finalize tile dataset
        if let Err(e) = writer.finalize() {
            handle_export_error(&tile_id, e, error_mode, &mut failed, &mut export_errors)?;
            // Story 7.2 - Increment progress even on error
            if let Some(pb) = progress {
                pb.inc(1);
            }
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

        // Story 7.2 - Increment progress on success
        if let Some(pb) = progress {
            pb.inc(1);
        }
    }

    // Report export errors if any (mode Continue)
    if !export_errors.is_empty() {
        warn!(
            error_count = export_errors.len(),
            "Multi-tile export completed with errors"
        );
    }

    Ok(TileExportSummary::new(succeeded, failed, skipped, global_stats, export_errors))
}

/// Export tiles in parallel using rayon thread pool (Epic 7 Story 7.1).
/// Story 7.2 enhancement: Added progress bar support.
#[tracing::instrument(skip(tile_features, config, progress))]
fn export_tiles_parallel(
    tile_features: Vec<(TileBounds, Vec<Feature>)>,
    config: &Config,
    error_mode: ErrorMode,
    jobs: usize,
    progress: &Option<Arc<ProgressBar>>,
) -> Result<TileExportSummary> {
    // Thread-safe shared state with lock-free atomic counters for better performance
    let succeeded = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let skipped = Arc::new(AtomicUsize::new(0));
    // Use separate AtomicUsize for stats to avoid mutex contention (performance optimization)
    let point_count = Arc::new(AtomicUsize::new(0));
    let linestring_count = Arc::new(AtomicUsize::new(0));
    let polygon_count = Arc::new(AtomicUsize::new(0));
    let export_errors = Arc::new(Mutex::new(Vec::new()));

    // Ensure output directory exists ONCE (before parallel loop)
    std::fs::create_dir_all(&config.output.directory)
        .context("Failed to create output directory")?;

    // Create rayon thread pool
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .context("Failed to create rayon thread pool")?;

    info!(jobs = jobs, tiles = tile_features.len(), "Starting parallel export");

    // Create shared export context (reduces parameter count)
    // Story 7.4: Added field_mapping_path to context
    let ctx = TileExportContext {
        succeeded: &succeeded,
        failed: &failed,
        skipped: &skipped,
        point_count: &point_count,
        linestring_count: &linestring_count,
        polygon_count: &polygon_count,
        export_errors: &export_errors,
        progress,
        error_mode,
        field_mapping_path: config.output.field_mapping_path.as_deref(),
    };

    // Execute parallel export
    let result = pool.install(|| {
        match error_mode {
            ErrorMode::Continue => {
                // Use .for_each() - collect all errors
                tile_features.par_iter().for_each(|(tile_bounds, features)| {
                    // export_single_tile handles its own errors in Continue mode
                    let _ = export_single_tile(
                        tile_bounds,
                        features,
                        &config.output.directory,
                        &ctx,
                    );
                });
                Ok(())
            }
            ErrorMode::FailFast => {
                // Use .try_for_each() - early exit on first error
                tile_features.par_iter().try_for_each(|(tile_bounds, features)| {
                    export_single_tile(
                        tile_bounds,
                        features,
                        &config.output.directory,
                        &ctx,
                    )
                })
            }
        }
    });

    // Handle fail-fast error
    if let Err(e) = result {
        let error_msg = e.to_string();
        // Extract tile_id from error context if available
        return Err(e).context(format!("Parallel export failed in fail-fast mode: {}", error_msg));
    }

    // Extract final results from Arc wrappers
    let succeeded_count = succeeded.load(Ordering::SeqCst);
    let failed_count = failed.load(Ordering::SeqCst);
    let skipped_count = skipped.load(Ordering::SeqCst);

    // Build final stats from atomic counters (lock-free)
    let final_stats = ExportStats {
        point_count: point_count.load(Ordering::SeqCst),
        linestring_count: linestring_count.load(Ordering::SeqCst),
        polygon_count: polygon_count.load(Ordering::SeqCst),
    };
    let final_errors = export_errors.lock().unwrap().clone();

    info!(
        tiles_succeeded = succeeded_count,
        tiles_failed = failed_count,
        tiles_skipped = skipped_count,
        total_features = final_stats.point_count + final_stats.linestring_count + final_stats.polygon_count,
        "Parallel export completed"
    );

    Ok(TileExportSummary::new(succeeded_count, failed_count, skipped_count, final_stats, final_errors))
}

/// Shared context for tile export operations.
/// Reduces function parameter count and improves readability.
struct TileExportContext<'a> {
    succeeded: &'a Arc<AtomicUsize>,
    failed: &'a Arc<AtomicUsize>,
    skipped: &'a Arc<AtomicUsize>,
    point_count: &'a Arc<AtomicUsize>,
    linestring_count: &'a Arc<AtomicUsize>,
    polygon_count: &'a Arc<AtomicUsize>,
    export_errors: &'a Arc<Mutex<Vec<TileExportError>>>,
    progress: &'a Option<Arc<ProgressBar>>,
    error_mode: ErrorMode,
    /// Optional YAML field mapping path for ogr-polishmap driver (Story 7.4)
    field_mapping_path: Option<&'a Path>,
}

/// Export a single tile (thread-safe, called from parallel loop).
/// Handles errors according to error_mode:
/// - Continue: Logs error, updates failed counter, collects error details, returns Ok(())
/// - FailFast: Returns Err() to propagate to caller for immediate termination
///
/// Story 7.2 enhancement: Added progress bar support.
fn export_single_tile(
    tile_bounds: &TileBounds,
    features: &[Feature],
    output_directory: &str,
    ctx: &TileExportContext,
) -> Result<()> {
    let tile_id = tile_bounds.tile_id();

    // Skip empty tiles
    if features.is_empty() {
        debug!(tile_id = %tile_id, "Skipping empty tile");
        ctx.skipped.fetch_add(1, Ordering::SeqCst);
        // Story 7.2 - Increment progress even for skipped tiles
        if let Some(pb) = ctx.progress {
            pb.inc(1);
        }
        return Ok(());
    }

    // Resolve tile filename
    let tile_filename = format!("{}.mp", tile_id);
    let tile_path = PathBuf::from(output_directory).join(tile_filename);

    // Helper to handle errors based on mode
    let handle_error = |error: anyhow::Error| -> Result<()> {
        ctx.failed.fetch_add(1, Ordering::SeqCst);

        // Story 7.2 - Increment progress even on error (before returning)
        if let Some(pb) = ctx.progress {
            pb.inc(1);
        }

        match ctx.error_mode {
            ErrorMode::Continue => {
                // Log warning and collect error details
                warn!(
                    tile_id = %tile_id,
                    error = %error,
                    "Tile export failed, continuing with next tile"
                );
                ctx.export_errors.lock().unwrap().push(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: error.to_string(),
                });
                Ok(()) // Continue processing
            }
            ErrorMode::FailFast => {
                // Propagate error for immediate termination
                Err(error).context(format!("Tile export failed (fail-fast mode): tile {}", tile_id))
            }
        }
    };

    // Create writer, write features, finalize (with error handling)
    // Story 7.4: Pass field mapping path from context
    let mut writer = match MpWriter::new(tile_path, ctx.field_mapping_path) {
        Ok(w) => w,
        Err(e) => return handle_error(e),
    };

    let tile_stats = match writer.write_features(features) {
        Ok(stats) => stats,
        Err(e) => return handle_error(e),
    };

    if let Err(e) = writer.finalize() {
        return handle_error(e);
    }

    // Log success
    info!(
        tile_id = %tile_id,
        points = tile_stats.point_count,
        linestrings = tile_stats.linestring_count,
        polygons = tile_stats.polygon_count,
        "Tile export succeeded"
    );

    // Update global stats (lock-free atomic operations for better performance)
    ctx.point_count.fetch_add(tile_stats.point_count, Ordering::Relaxed);
    ctx.linestring_count.fetch_add(tile_stats.linestring_count, Ordering::Relaxed);
    ctx.polygon_count.fetch_add(tile_stats.polygon_count, Ordering::Relaxed);

    ctx.succeeded.fetch_add(1, Ordering::SeqCst);

    // Story 7.2 - Increment progress on success
    if let Some(pb) = ctx.progress {
        pb.inc(1);
    }

    Ok(())
}

/// Story 7.3 - Task 6: Print structured console summary with French i18n.
/// AC1: Display status, counts, duration, and top errors.
fn print_console_summary(
    report: &crate::report::ExecutionReport,
    output_directory: &str,
    args: &BuildArgs,
) {
    use crate::report::ReportStatus;

    // Status header
    let (status_symbol, status_text) = match report.status {
        ReportStatus::Success => ("✅", "SUCCÈS"),
        ReportStatus::Failure => ("❌", "ÉCHEC"),
    };

    println!("\n{} Exécution terminée - Statut: {}", status_symbol, status_text);
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║ RÉSUMÉ D'EXÉCUTION                                     ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║ Tuiles générées  : {:>10}                      ║", report.tiles_generated);
    println!("║ Tuiles échouées  : {:>10}                      ║", report.tiles_failed);
    println!("║ Tuiles skippées  : {:>10}                      ║", report.tiles_skipped);
    println!("║ Features traitées: {:>10}                      ║", report.features_processed);
    println!("║ Durée totale     : {:>7.1} sec                   ║", report.duration_seconds);
    println!("╚════════════════════════════════════════════════════════╝");
    println!("   Répertoire de sortie : {}", output_directory);

    // Show top errors (not all, to avoid console pollution)
    // M4 Fix: Use named constant instead of magic number
    if !report.errors.is_empty() {
        println!("\n⚠️  Top {} erreurs:", report.errors.len().min(MAX_CONSOLE_ERRORS));
        for (i, error) in report.errors.iter().take(MAX_CONSOLE_ERRORS).enumerate() {
            println!("  {}. Tuile {} : {}", i + 1, error.tile, error.error);
        }
        if report.errors.len() > MAX_CONSOLE_ERRORS {
            println!("  ... et {} autres erreurs (voir rapport JSON)", report.errors.len() - MAX_CONSOLE_ERRORS);
        }
    }

    // JSON report written message
    if let Some(report_path) = &args.report {
        println!("\n📄 Rapport JSON écrit: {}", report_path);
    }

    println!("\n💡 Astuce : Utilisez -vv pour des logs de débogage détaillés");
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
