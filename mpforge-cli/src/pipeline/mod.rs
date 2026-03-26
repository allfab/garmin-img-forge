//! Pipeline orchestration module.
//!
//! Story 11.1: Parallelized tile processing via rayon thread pool.

pub mod geometry_validator;
pub mod reader;
pub mod tile_naming;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::{Config, ErrorMode, HeaderConfig};
use crate::rules::{self, RuleStats, RulesFile};
use crate::pipeline::geometry_validator::ValidationStats;
use crate::pipeline::reader::{MultiGeometryStats, SourceReader, UnsupportedTypeStats};
use crate::pipeline::tile_naming::resolve_tile_pattern;
use crate::pipeline::tiler::{clip_feature_to_tile, TileProcessor, TileBounds};
use crate::pipeline::writer::{ExportStats, MpWriter};
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{info, warn};

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

/// Result of processing a single tile.
/// Story 11.1 — Task 1: Returned by `process_single_tile()`.
#[derive(Debug)]
enum TileOutcome {
    /// Tile exported successfully with stats.
    Success(TileResult),
    /// Tile skipped (empty features, already exists, etc.).
    Skipped { existing: bool },
    /// Tile processing failed.
    Failed(TileExportError),
}

/// Successful tile result with export statistics.
/// Story 11.1 — Task 1: Aggregated by `aggregate_outcome()`.
#[derive(Debug)]
struct TileResult {
    stats: ExportStats,
    validation_stats: ValidationStats,
    unsupported: UnsupportedTypeStats,
    multi_geom: MultiGeometryStats,
    rules_stats: RuleStats,
}

/// Immutable context shared across all tile workers.
/// Story 11.1 — Task 1: Each worker reads from this, no mutation needed.
struct TileContext<'a> {
    config: &'a Config,
    rules: Option<Arc<RulesFile>>,
    error_mode: ErrorMode,
    should_skip_existing: bool,
    dry_run: bool,
    output_directory: &'a str,
    filename_pattern: &'a str,
    field_mapping_path: Option<&'a Path>,
    header_config: Option<&'a HeaderConfig>,
}

/// Process a single tile autonomously (thread-safe).
///
/// Story 11.1 — Task 1: Pure function that opens its own GDAL datasets,
/// reads features, clips, and exports. No shared mutable state.
///
/// The `seq` parameter is the 1-based sequential counter for filename patterns.
fn process_single_tile(
    tile_bounds: &TileBounds,
    ctx: &TileContext<'_>,
    seq: usize,
) -> Result<TileOutcome, TileExportError> {
    let tile_id = tile_bounds.tile_id();

    // 1. Load features filtered for this tile (each call opens its own GDAL datasets)
    let (features, unsupported, multi_geom) = match SourceReader::read_features_for_tile(ctx.config, tile_bounds) {
        Ok(result) => result,
        Err(e) => {
            if ctx.error_mode == ErrorMode::FailFast {
                return Err(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: format!("Failed to read features for tile {}: {}", tile_id, e),
                });
            }
            warn!(
                tile_id = %tile_id,
                error = %e,
                "Failed to read features for tile, skipping"
            );
            return Ok(TileOutcome::Skipped { existing: false });
        }
    };

    if features.is_empty() {
        return Ok(TileOutcome::Skipped { existing: false });
    }

    // 2. Apply rules engine (Arc<RulesFile> is read-only, thread-safe)
    let mut tile_rules_stats = RuleStats::default();
    let features = if let Some(ref rules_file) = ctx.rules {
        let mut transformed = Vec::with_capacity(features.len());
        for (fid, mut feature) in features.into_iter().enumerate() {
            let layer_name = feature.source_layer.clone().unwrap_or_default();
            match rules::find_ruleset(rules_file, &layer_name) {
                None => {
                    transformed.push(feature);
                }
                Some(ruleset) => {
                    match rules::evaluate_feature(ruleset, &feature.attributes) {
                        Ok(Some(new_attrs)) => {
                            feature.attributes = new_attrs;
                            transformed.push(feature);
                            tile_rules_stats.record_match(&layer_name);
                        }
                        Ok(None) => {
                            tracing::debug!(
                                fid = fid,
                                source_layer = %layer_name,
                                "Feature ignored (no matching rule)"
                            );
                            tile_rules_stats.record_ignored(&layer_name);
                        }
                        Err(e) => {
                            tracing::warn!(
                                fid = fid,
                                source_layer = %layer_name,
                                error = %e,
                                "Feature ignored (rule error)"
                            );
                            tile_rules_stats.record_error(&layer_name);
                        }
                    }
                }
            }
        }
        transformed
    } else {
        features
    };

    if features.is_empty() {
        return Ok(TileOutcome::Skipped { existing: false });
    }

    // 3. Clip features to tile boundary
    let tile_bbox_geom = match tile_bounds.to_gdal_polygon() {
        Ok(geom) => geom,
        Err(e) => {
            if ctx.error_mode == ErrorMode::FailFast {
                return Err(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: format!("Failed to create tile polygon for tile {}: {}", tile_id, e),
                });
            }
            warn!(
                tile_id = %tile_id,
                error = %e,
                "Failed to create tile polygon, skipping tile"
            );
            return Ok(TileOutcome::Skipped { existing: false });
        }
    };

    let mut clipped_features = Vec::new();
    let mut tile_validation_stats = ValidationStats::default();
    let mut clip_errors = Vec::new();

    for feature in &features {
        match clip_feature_to_tile(
            feature,
            &tile_bbox_geom,
            ctx.error_mode,
            &mut tile_validation_stats,
        ) {
            Ok(Some(clipped)) => clipped_features.push(clipped),
            Ok(None) => { /* outside tile or empty intersection */ }
            Err(e) => {
                clip_errors.push(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: format!("Clipping failed: {}", e),
                });
                if ctx.error_mode == ErrorMode::FailFast {
                    return Err(TileExportError {
                        tile_id: tile_id.clone(),
                        error_message: format!("Clipping failed (fail-fast): {}", e),
                    });
                }
            }
        }
    }
    drop(features);

    if clipped_features.is_empty() {
        return Ok(TileOutcome::Skipped { existing: false });
    }

    // 4. Resolve filename and check skip-existing
    let tile_filename = resolve_tile_pattern(
        ctx.filename_pattern,
        tile_bounds.col,
        tile_bounds.row,
        seq,
    )
    .map_err(|e| TileExportError {
        tile_id: tile_id.clone(),
        error_message: format!("Failed to resolve filename pattern: {}", e),
    })?;
    let tile_path = PathBuf::from(ctx.output_directory).join(&tile_filename);

    if ctx.should_skip_existing && tile_path.exists() {
        info!(tile_id = %tile_id, path = %tile_path.display(), "Existing tile skipped");
        return Ok(TileOutcome::Skipped { existing: true });
    }

    // 5. Dry-run: count features without writing
    if ctx.dry_run {
        let mut stats = ExportStats::default();
        for f in &clipped_features {
            match f.geometry_type {
                crate::pipeline::reader::GeometryType::Point => stats.point_count += 1,
                crate::pipeline::reader::GeometryType::LineString => stats.linestring_count += 1,
                crate::pipeline::reader::GeometryType::Polygon => stats.polygon_count += 1,
            }
        }
        return Ok(TileOutcome::Success(TileResult {
            stats,
            validation_stats: tile_validation_stats,
            unsupported,
            multi_geom,
            rules_stats: tile_rules_stats,
        }));
    }

    // 6. Create subdirectories if needed
    if let Some(parent) = tile_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TileExportError {
            tile_id: tile_id.clone(),
            error_message: format!("Failed to create directory: {}", e),
        })?;
    }

    // 7. Export tile (each call creates its own MpWriter → own GDAL dataset)
    match (|| -> Result<ExportStats> {
        let mut writer = MpWriter::new(tile_path, ctx.field_mapping_path, ctx.header_config)?;
        let tile_stats = writer.write_features(&clipped_features)?;
        writer.finalize()?;
        Ok(tile_stats)
    })() {
        Ok(tile_stats) => {
            info!(
                tile_id = %tile_id,
                points = tile_stats.point_count,
                linestrings = tile_stats.linestring_count,
                polygons = tile_stats.polygon_count,
                "Tile export succeeded"
            );
            Ok(TileOutcome::Success(TileResult {
                stats: tile_stats,
                validation_stats: tile_validation_stats,
                unsupported,
                multi_geom,
                rules_stats: tile_rules_stats,
            }))
        }
        Err(e) => {
            warn!(
                tile_id = %tile_id,
                error = %e,
                "Tile export failed"
            );
            if ctx.error_mode == ErrorMode::FailFast {
                Err(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: format!("Tile export failed (fail-fast mode): tile {}: {}", tile_id, e),
                })
            } else {
                Ok(TileOutcome::Failed(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: e.to_string(),
                }))
            }
        }
    }
}

/// Aggregate a `TileOutcome` into the thread-safe accumulators.
#[allow(clippy::too_many_arguments)]
fn aggregate_outcome(
    outcome: TileOutcome,
    tiles_succeeded: &AtomicUsize,
    tiles_failed: &AtomicUsize,
    tiles_skipped: &AtomicUsize,
    tiles_skipped_existing: &AtomicUsize,
    global_stats: &Mutex<ExportStats>,
    export_errors: &Mutex<Vec<TileExportError>>,
    global_validation_stats: &Mutex<ValidationStats>,
    all_unsupported: &Mutex<UnsupportedTypeStats>,
    all_multi_geom: &Mutex<MultiGeometryStats>,
    rules_stats: &Mutex<RuleStats>,
) {
    match outcome {
        TileOutcome::Success(result) => {
            tiles_succeeded.fetch_add(1, Ordering::Relaxed);
            {
                let mut stats = global_stats.lock().unwrap_or_else(|e| e.into_inner());
                stats.point_count += result.stats.point_count;
                stats.linestring_count += result.stats.linestring_count;
                stats.polygon_count += result.stats.polygon_count;
            }
            {
                let mut vs = global_validation_stats.lock().unwrap_or_else(|e| e.into_inner());
                vs.valid_count += result.validation_stats.valid_count;
                vs.repaired_make_valid += result.validation_stats.repaired_make_valid;
                vs.repaired_buffer_zero += result.validation_stats.repaired_buffer_zero;
                vs.rejected_invalid_coords += result.validation_stats.rejected_invalid_coords;
                vs.rejected_irrecoverable += result.validation_stats.rejected_irrecoverable;
            }
            {
                let mut us = all_unsupported.lock().unwrap_or_else(|e| e.into_inner());
                us.merge(&result.unsupported);
            }
            {
                let mut mg = all_multi_geom.lock().unwrap_or_else(|e| e.into_inner());
                mg.merge(&result.multi_geom);
            }
            {
                let mut rs = rules_stats.lock().unwrap_or_else(|e| e.into_inner());
                rs.matched += result.rules_stats.matched;
                rs.ignored += result.rules_stats.ignored;
                rs.errors += result.rules_stats.errors;
                for (layer, layer_stats) in &result.rules_stats.by_ruleset {
                    let entry = rs.by_ruleset.entry(layer.clone()).or_default();
                    entry.matched += layer_stats.matched;
                    entry.ignored += layer_stats.ignored;
                    entry.errors += layer_stats.errors;
                }
            }
        }
        TileOutcome::Skipped { existing } => {
            tiles_skipped.fetch_add(1, Ordering::Relaxed);
            if existing {
                tiles_skipped_existing.fetch_add(1, Ordering::Relaxed);
            }
        }
        TileOutcome::Failed(err) => {
            tiles_failed.fetch_add(1, Ordering::Relaxed);
            export_errors.lock().unwrap_or_else(|e| e.into_inner()).push(err);
        }
    }
}

/// Run the complete tiling pipeline (tile-centric mode).
///
/// Architecture: scan extents → generate grid → for each tile: load filtered → clip → export.
/// Memory usage is proportional to a single tile's features instead of the full dataset.
/// Story 11.1: Supports parallel processing via `--jobs N` with rayon thread pool.
///
/// Returns `TileExportSummary` with export statistics and errors.
#[tracing::instrument(skip(config, args))]
pub fn run(config: &Config, args: &BuildArgs) -> Result<TileExportSummary> {
    info!(
        "Pipeline started (tile-centric) with config version {} and {} jobs",
        config.version, args.jobs
    );
    info!("Grid cell size: {}", config.grid.cell_size);
    info!("Output directory: {}", config.output.directory);
    info!("Error handling mode: {}", config.error_handling);

    // Story 11.1: Log parallelism mode
    if args.jobs > 1 {
        info!(jobs = args.jobs, "Pipeline parallèle : {} workers rayon", args.jobs);
    } else {
        info!("Pipeline séquentiel : 1 thread");
    }

    // Load rules file if configured (Story 9.1: fail-fast before expensive processing)
    // Story 9.3: rules used for attribute transformation in the per-feature loop
    let rules: Option<RulesFile> = if let Some(rules_path) = &config.rules {
        Some(rules::load_rules(rules_path)
            .with_context(|| format!("Failed to load rules file: {}", rules_path.display()))?)
    } else {
        None
    };

    let start_time = Instant::now();

    // ========================================================================
    // Phase 1: Scan extents (no feature loading) and generate grid
    // ========================================================================
    info!("Phase 1: Scanning source extents");
    let scan_start = Instant::now();

    let global_extent = match SourceReader::scan_extents(config) {
        Ok(extent) => extent,
        Err(_) if config.inputs.is_empty() => {
            // No inputs at all — return empty success (Story 5.4 AC5)
            warn!("No input sources configured, nothing to process");
            return Ok(TileExportSummary::new(
                0,
                0,
                0,
                ExportStats::default(),
                Vec::new(),
            ));
        }
        Err(e) => return Err(e),
    };

    let scan_elapsed = scan_start.elapsed();
    info!(
        duration_ms = scan_elapsed.as_millis(),
        source_count = global_extent.layer_count,
        "Extent scan completed"
    );

    // Generate tile grid from bbox
    let tile_processor = TileProcessor::new(config.grid.clone());
    let bbox = global_extent.to_bbox();
    let tiles = tile_processor.generate_tiles_from_bbox(&bbox, &config.filters);

    if tiles.is_empty() {
        warn!("No tiles generated from extents, pipeline has nothing to process");
        return Ok(TileExportSummary::new(
            0,
            0,
            0,
            ExportStats::default(),
            Vec::new(),
        ));
    }

    info!(tiles_count = tiles.len(), "Grid generated");

    // ========================================================================
    // Phase 2: Tile-centric processing (load → clip → export per tile)
    // ========================================================================
    info!("Phase 2: Processing {} tiles (tile-centric)", tiles.len());

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

    // Ensure output directory exists (skip in dry-run: no side effects)
    if !args.dry_run {
        std::fs::create_dir_all(&config.output.directory)
            .context("Failed to create output directory")?;
    }

    // Story 11.1 — Task 5: Progress bar wrapped in Arc for thread-safe sharing
    let progress: Option<Arc<ProgressBar>> = if args.verbose < 2 {
        let pb = ProgressBar::new(tiles.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} tuiles ({percent}%) - ETA: {eta}")
                .expect("valid progress bar template")
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        Some(Arc::new(pb))
    } else {
        info!("Progress bar disabled in debug mode (verbose >= 2)");
        None
    };

    // Story 11.1 — Task 2: Thread-safe counters and accumulators
    let tiles_succeeded = Arc::new(AtomicUsize::new(0));
    let tiles_failed = Arc::new(AtomicUsize::new(0));
    let tiles_skipped = Arc::new(AtomicUsize::new(0));
    let tiles_skipped_existing = Arc::new(AtomicUsize::new(0));
    let global_stats = Arc::new(Mutex::new(ExportStats::default()));
    let export_errors: Arc<Mutex<Vec<TileExportError>>> = Arc::new(Mutex::new(Vec::new()));
    let global_validation_stats = Arc::new(Mutex::new(ValidationStats::default()));
    let all_unsupported = Arc::new(Mutex::new(UnsupportedTypeStats::default()));
    let all_multi_geom = Arc::new(Mutex::new(MultiGeometryStats::default()));
    let rules_stats = Arc::new(Mutex::new(RuleStats::default()));

    // Story 8.3: Pre-calculate skip-existing flag
    let should_skip_existing =
        args.skip_existing || config.output.overwrite == Some(false);

    // Story 11.1 — Task 1: Build immutable context shared across workers
    let ctx = TileContext {
        config,
        rules: rules.map(Arc::new),
        error_mode,
        should_skip_existing,
        dry_run: args.dry_run,
        output_directory: &config.output.directory,
        filename_pattern: &config.output.filename_pattern,
        field_mapping_path: config.output.field_mapping_path.as_deref(),
        header_config: config.header.as_ref(),
    };

    // Story 11.1 — Task 3: Sequential counter for filename patterns (atomic for thread-safety).
    let seq_counter = Arc::new(AtomicUsize::new(0));

    // Story 11.1 — Code review M1: Warn about non-deterministic {seq} in parallel mode
    if args.jobs > 1 && config.output.filename_pattern.contains("{seq}") {
        warn!(
            jobs = args.jobs,
            "Le pattern {{seq}} produit des noms non-déterministes en mode parallèle. \
             Utilisez {{col}}_{{row}} pour des résultats reproductibles."
        );
    }

    // Story 11.1 — Task 3 & 4: Conditional sequential/parallel execution
    let fail_fast = error_mode == ErrorMode::FailFast || args.fail_fast;

    // Inner function to process one tile and aggregate results
    #[allow(clippy::too_many_arguments)]
    fn process_and_aggregate(
        tile_bounds: &TileBounds,
        ctx: &TileContext<'_>,
        seq_counter: &AtomicUsize,
        progress: &Option<Arc<ProgressBar>>,
        tiles_succeeded: &AtomicUsize,
        tiles_failed: &AtomicUsize,
        tiles_skipped: &AtomicUsize,
        tiles_skipped_existing: &AtomicUsize,
        global_stats: &Mutex<ExportStats>,
        export_errors: &Mutex<Vec<TileExportError>>,
        global_validation_stats: &Mutex<ValidationStats>,
        all_unsupported: &Mutex<UnsupportedTypeStats>,
        all_multi_geom: &Mutex<MultiGeometryStats>,
        rules_stats: &Mutex<RuleStats>,
    ) -> Result<(), TileExportError> {
        let seq = seq_counter.fetch_add(1, Ordering::Relaxed) + 1; // 1-based

        let outcome = match process_single_tile(tile_bounds, ctx, seq) {
            Ok(outcome) => outcome,
            Err(e) => {
                if let Some(ref pb) = progress {
                    pb.inc(1);
                }
                return Err(e);
            }
        };

        aggregate_outcome(
            outcome,
            tiles_succeeded,
            tiles_failed,
            tiles_skipped,
            tiles_skipped_existing,
            global_stats,
            export_errors,
            global_validation_stats,
            all_unsupported,
            all_multi_geom,
            rules_stats,
        );

        if let Some(ref pb) = progress {
            pb.inc(1);
        }

        Ok(())
    }

    if args.jobs == 1 {
        // Sequential mode: no rayon overhead (AC2)
        if fail_fast {
            for tile_bounds in &tiles {
                process_and_aggregate(
                    tile_bounds, &ctx, &seq_counter, &progress,
                    &tiles_succeeded, &tiles_failed, &tiles_skipped, &tiles_skipped_existing,
                    &global_stats, &export_errors, &global_validation_stats,
                    &all_unsupported, &all_multi_geom, &rules_stats,
                ).map_err(|e| anyhow::anyhow!("{}", e.error_message))?;
            }
        } else {
            for tile_bounds in &tiles {
                let _ = process_and_aggregate(
                    tile_bounds, &ctx, &seq_counter, &progress,
                    &tiles_succeeded, &tiles_failed, &tiles_skipped, &tiles_skipped_existing,
                    &global_stats, &export_errors, &global_validation_stats,
                    &all_unsupported, &all_multi_geom, &rules_stats,
                );
            }
        }
    } else {
        // Parallel mode: rayon thread pool (AC1)
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(args.jobs)
            .build()
            .context("Failed to create rayon thread pool")?;

        let parallel_result = pool.install(|| {
            if fail_fast {
                // Story 11.1 — Task 4: try_for_each stops on first error (AC6)
                tiles.par_iter().try_for_each(|tile_bounds| {
                    process_and_aggregate(
                        tile_bounds, &ctx, &seq_counter, &progress,
                        &tiles_succeeded, &tiles_failed, &tiles_skipped, &tiles_skipped_existing,
                        &global_stats, &export_errors, &global_validation_stats,
                        &all_unsupported, &all_multi_geom, &rules_stats,
                    )
                })
            } else {
                // Continue mode: process all tiles, collect errors (AC5)
                tiles.par_iter().for_each(|tile_bounds| {
                    let _ = process_and_aggregate(
                        tile_bounds, &ctx, &seq_counter, &progress,
                        &tiles_succeeded, &tiles_failed, &tiles_skipped, &tiles_skipped_existing,
                        &global_stats, &export_errors, &global_validation_stats,
                        &all_unsupported, &all_multi_geom, &rules_stats,
                    );
                });
                Ok(())
            }
        });

        if let Err(e) = parallel_result {
            return Err(anyhow::anyhow!(
                "Parallel export failed (fail-fast): {}", e.error_message
            ));
        }
    }

    // Extract final values from thread-safe accumulators
    let tiles_succeeded = tiles_succeeded.load(Ordering::Relaxed);
    let tiles_failed = tiles_failed.load(Ordering::Relaxed);
    let tiles_skipped = tiles_skipped.load(Ordering::Relaxed);
    let tiles_skipped_existing = tiles_skipped_existing.load(Ordering::Relaxed);
    let global_stats = Arc::try_unwrap(global_stats)
        .map_err(|_| anyhow::anyhow!("global_stats Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    let export_errors = Arc::try_unwrap(export_errors)
        .map_err(|_| anyhow::anyhow!("export_errors Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    let global_validation_stats = Arc::try_unwrap(global_validation_stats)
        .map_err(|_| anyhow::anyhow!("validation_stats Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    let all_unsupported = Arc::try_unwrap(all_unsupported)
        .map_err(|_| anyhow::anyhow!("all_unsupported Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    let all_multi_geom = Arc::try_unwrap(all_multi_geom)
        .map_err(|_| anyhow::anyhow!("all_multi_geom Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    let rules_stats = Arc::try_unwrap(rules_stats)
        .map_err(|_| anyhow::anyhow!("rules_stats Arc still has active references"))?
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());

    // ========================================================================
    // Phase 3: Reporting
    // ========================================================================

    let mut summary = TileExportSummary::new(
        tiles_succeeded,
        tiles_failed,
        tiles_skipped,
        global_stats,
        export_errors,
    );
    summary.validation_stats = Some(global_validation_stats);

    // Finalize progress bar
    if let Some(pb) = progress {
        let mut msg = format!(
            "✓ Export terminé: {} tuiles réussies, {} échouées",
            summary.tiles_succeeded, summary.tiles_failed
        );
        if summary.tiles_skipped > 0 {
            msg.push_str(&format!(", {} skippées", summary.tiles_skipped));
            if tiles_skipped_existing > 0 {
                msg.push_str(&format!(" ({} existing)", tiles_skipped_existing));
            }
        }
        if args.dry_run {
            msg.push_str(" (DRY-RUN)");
        }
        pb.finish_with_message(msg);
    }

    // Log validation summary
    if let Some(ref vs) = summary.validation_stats {
        if vs.rejected_count() > 0 || vs.repaired_count() > 0 {
            info!(
                valid = vs.valid_count,
                repaired_make_valid = vs.repaired_make_valid,
                repaired_buffer_zero = vs.repaired_buffer_zero,
                rejected_coords = vs.rejected_invalid_coords,
                rejected_irrecoverable = vs.rejected_irrecoverable,
                recovery_rate = %format!("{:.1}%", vs.recovery_rate() * 100.0),
                "Geometry validation summary"
            );
        }
    }

    info!(
        tiles_succeeded = summary.tiles_succeeded,
        tiles_failed = summary.tiles_failed,
        tiles_skipped = summary.tiles_skipped,
        total_features = summary.total_features(),
        "Tile-centric pipeline completed"
    );

    // Total pipeline duration
    let total_duration = start_time.elapsed().as_secs_f64();

    // Build quality section from stats
    let quality = build_quality_section(&all_unsupported, &all_multi_geom);

    // Build ExecutionReport
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
        dry_run: args.dry_run,
        errors: summary
            .export_errors
            .iter()
            .map(|e| crate::report::TileError {
                tile: e.tile_id.clone(),
                error: e.error_message.clone(),
            })
            .collect(),
        quality,
        rules_stats: if rules_stats.matched > 0 || rules_stats.ignored > 0 || rules_stats.errors > 0 {
            Some(rules_stats.clone())
        } else {
            None
        },
    };

    // Write JSON report if requested
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

    // Console summary
    print_console_summary(&report, &config.output.directory, args, tiles_skipped_existing, &rules_stats);

    // Exit with appropriate code for CI/CD
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

/// Build quality section from unsupported type and multi-geometry stats.
fn build_quality_section(
    unsupported_type_stats: &UnsupportedTypeStats,
    multi_geom_stats: &MultiGeometryStats,
) -> Option<crate::report::QualitySection> {
    if unsupported_type_stats.is_empty() && multi_geom_stats.is_empty() {
        return None;
    }

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

    let multi_geometries_decomposed = if multi_geom_stats.is_empty() {
        None
    } else {
        Some(
            multi_geom_stats
                .by_type
                .iter()
                .map(|(type_name, entry)| (type_name.clone(), entry.count))
                .collect(),
        )
    };

    Some(crate::report::QualitySection {
        unsupported_types,
        multi_geometries_decomposed,
    })
}

/// Story 7.3 - Task 6: Print structured console summary with French i18n.
/// AC1: Display status, counts, duration, and top errors.
fn print_console_summary(
    report: &crate::report::ExecutionReport,
    output_directory: &str,
    args: &BuildArgs,
    tiles_skipped_existing: usize,
    rules_stats: &RuleStats,
) {
    use crate::report::ReportStatus;

    // Status header
    let (status_symbol, status_text) = match report.status {
        ReportStatus::Success => ("✅", "SUCCÈS"),
        ReportStatus::Failure => ("❌", "ÉCHEC"),
    };

    // Story 8.3: Dry-run banner
    if report.dry_run {
        println!("\n⚠️  MODE DRY-RUN : Aucun fichier écrit");
    }

    println!(
        "\n{} Exécution terminée - Statut: {}",
        status_symbol, status_text
    );
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║ RÉSUMÉ D'EXÉCUTION                                     ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!(
        "║ Tuiles générées  : {:>10}                      ║",
        report.tiles_generated
    );
    println!(
        "║ Tuiles échouées  : {:>10}                      ║",
        report.tiles_failed
    );
    println!(
        "║ Tuiles skippées  : {:>10}                      ║",
        report.tiles_skipped
    );
    println!(
        "║ Features traitées: {:>10}                      ║",
        report.features_processed
    );
    println!(
        "║ Durée totale     : {:>7.1} sec                   ║",
        report.duration_seconds
    );
    println!("╚════════════════════════════════════════════════════════╝");
    println!("   Répertoire de sortie : {}", output_directory);
    if tiles_skipped_existing > 0 {
        println!(
            "   Dont {} tuile(s) skippée(s) (existing)",
            tiles_skipped_existing
        );
    }

    // Story 9.3 AC6: Display rules statistics
    if rules_stats.matched > 0 || rules_stats.ignored > 0 || rules_stats.errors > 0 {
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║ Règles appliquées                                      ║");
        println!("╠════════════════════════════════════════════════════════╣");
        let mut sorted_layers: Vec<_> = rules_stats.by_ruleset.keys().collect();
        sorted_layers.sort();
        for layer in &sorted_layers {
            let stats = &rules_stats.by_ruleset[*layer];
            // Truncate layer name to 22 chars to preserve box alignment
            let display_name = if layer.len() > 22 {
                format!("{}…", &layer[..21])
            } else {
                layer.to_string()
            };
            println!(
                "║   {:<22}: {:>5} matchées / {:>5} ignorées  ║",
                display_name, stats.matched, stats.ignored
            );
        }
        println!(
            "║   {:<22}: {:>5} matchées / {:>5} ignorées  ║",
            "Total", rules_stats.matched, rules_stats.ignored
        );
        if rules_stats.errors > 0 {
            println!(
                "║   Erreurs             : {:>5}                        ║",
                rules_stats.errors
            );
        }
        println!("╚════════════════════════════════════════════════════════╝");
    }

    // Show top errors (not all, to avoid console pollution)
    // M4 Fix: Use named constant instead of magic number
    if !report.errors.is_empty() {
        println!(
            "\n⚠️  Top {} erreurs:",
            report.errors.len().min(MAX_CONSOLE_ERRORS)
        );
        for (i, error) in report.errors.iter().take(MAX_CONSOLE_ERRORS).enumerate() {
            println!("  {}. Tuile {} : {}", i + 1, error.tile, error.error);
        }
        if report.errors.len() > MAX_CONSOLE_ERRORS {
            println!(
                "  ... et {} autres erreurs (voir rapport JSON)",
                report.errors.len() - MAX_CONSOLE_ERRORS
            );
        }
    }

    // JSON report written message
    if let Some(report_path) = &args.report {
        println!("\n📄 Rapport JSON écrit: {}", report_path);
    }

    println!("\n💡 Astuce : Utilisez -vv pour des logs de débogage détaillés");
}
