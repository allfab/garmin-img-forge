//! Pipeline orchestration module.

pub mod geometry_validator;
pub mod reader;
pub mod tile_naming;
pub mod tiler;
pub mod writer;

use crate::cli::BuildArgs;
use crate::config::{Config, ErrorMode};
use crate::pipeline::geometry_validator::ValidationStats;
use crate::pipeline::reader::{MultiGeometryStats, SourceReader, UnsupportedTypeStats};
use crate::pipeline::tile_naming::resolve_tile_pattern;
use crate::pipeline::tiler::{clip_feature_to_tile, TileProcessor};
use crate::pipeline::writer::{ExportStats, MpWriter};
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
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

/// Run the complete tiling pipeline (tile-centric mode).
///
/// Architecture: scan extents → generate grid → for each tile: load filtered → clip → export.
/// Memory usage is proportional to a single tile's features instead of the full dataset.
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

    // Warn if parallel jobs requested (not supported in tile-centric mode)
    if args.jobs > 1 {
        warn!(
            jobs = args.jobs,
            "Parallel tile processing not yet supported in tile-centric mode, using sequential"
        );
    }

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

    // Progress bar (no Arc needed in sequential mode)
    let progress = if args.verbose < 2 {
        let pb = ProgressBar::new(tiles.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{bar:40.cyan/blue}] {pos}/{len} tuiles ({percent}%) - ETA: {eta}")
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        Some(pb)
    } else {
        info!("Progress bar disabled in debug mode (verbose >= 2)");
        None
    };

    let mut tiles_succeeded: usize = 0;
    let mut tiles_failed: usize = 0;
    let mut tiles_skipped: usize = 0;
    let mut global_stats = ExportStats::default();
    let mut export_errors: Vec<TileExportError> = Vec::new();
    let mut global_validation_stats = ValidationStats::default();
    let mut all_unsupported = UnsupportedTypeStats::default();
    let mut all_multi_geom = MultiGeometryStats::default();
    let mut seq: usize = 0; // Story 8.2: sequential counter, incremented on successful export
    let mut tiles_skipped_existing: usize = 0; // Story 8.3: track existing tiles skipped separately

    // Story 8.3: Pre-calculate skip-existing flag (CLI --skip-existing OR config overwrite: false)
    let should_skip_existing =
        args.skip_existing || config.output.overwrite == Some(false);

    for tile_bounds in &tiles {
        // 2a. Load features filtered for this tile
        let (features, unsupported, multi_geom) = match SourceReader::read_features_for_tile(config, tile_bounds) {
            Ok(result) => result,
            Err(e) => {
                if error_mode == ErrorMode::FailFast {
                    return Err(e).context(format!(
                        "Failed to read features for tile {}",
                        tile_bounds.tile_id()
                    ));
                }
                warn!(
                    tile_id = %tile_bounds.tile_id(),
                    error = %e,
                    "Failed to read features for tile, skipping"
                );
                tiles_skipped += 1;
                if let Some(pb) = &progress {
                    pb.inc(1);
                }
                continue;
            }
        };
        all_unsupported.merge(&unsupported);
        all_multi_geom.merge(&multi_geom);

        if features.is_empty() {
            tiles_skipped += 1;
            if let Some(pb) = &progress {
                pb.inc(1);
            }
            continue;
        }

        // 2b. Clip features to tile
        let tile_bbox_geom = match tile_bounds.to_gdal_polygon() {
            Ok(geom) => geom,
            Err(e) => {
                warn!(
                    tile_id = %tile_bounds.tile_id(),
                    error = %e,
                    "Failed to create tile polygon, skipping tile"
                );
                if error_mode == ErrorMode::FailFast {
                    return Err(e).context(format!(
                        "Failed to create tile polygon for tile {}",
                        tile_bounds.tile_id()
                    ));
                }
                tiles_skipped += 1;
                if let Some(pb) = &progress {
                    pb.inc(1);
                }
                continue;
            }
        };
        let mut clipped_features = Vec::new();

        for feature in &features {
            match clip_feature_to_tile(
                feature,
                &tile_bbox_geom,
                error_mode,
                &mut global_validation_stats,
            ) {
                Ok(Some(clipped)) => clipped_features.push(clipped),
                Ok(None) => { /* outside tile or empty intersection */ }
                Err(e) => {
                    warn!(
                        tile_id = %tile_bounds.tile_id(),
                        error = %e,
                        "Failed to clip feature"
                    );
                    export_errors.push(TileExportError {
                        tile_id: tile_bounds.tile_id(),
                        error_message: format!("Clipping failed: {}", e),
                    });
                    if error_mode == ErrorMode::FailFast {
                        return Err(e);
                    }
                }
            }
        }
        drop(features); // Explicitly free source features before export

        // 2c. Export tile
        if clipped_features.is_empty() {
            tiles_skipped += 1;
            if let Some(pb) = &progress {
                pb.inc(1);
            }
            continue;
        }

        let tile_id = tile_bounds.tile_id();
        seq += 1; // Story 8.2: 1-based, incremented only for non-empty tiles
        let tile_filename = resolve_tile_pattern(
            &config.output.filename_pattern,
            tile_bounds.col,
            tile_bounds.row,
            seq,
        )
        .with_context(|| format!("Failed to resolve filename pattern for tile {}", tile_id))?;
        let tile_path = PathBuf::from(&config.output.directory).join(&tile_filename);

        // Story 8.3: Skip existing tile files
        if should_skip_existing && tile_path.exists() {
            info!(tile_id = %tile_id, path = %tile_path.display(), "Existing tile skipped");
            tiles_skipped += 1;
            tiles_skipped_existing += 1;
            if let Some(pb) = &progress {
                pb.inc(1);
            }
            continue;
        }

        // Story 8.3: Dry-run mode — count features without writing
        if args.dry_run {
            for f in &clipped_features {
                match f.geometry_type {
                    crate::pipeline::reader::GeometryType::Point => {
                        global_stats.point_count += 1;
                    }
                    crate::pipeline::reader::GeometryType::LineString => {
                        global_stats.linestring_count += 1;
                    }
                    crate::pipeline::reader::GeometryType::Polygon => {
                        global_stats.polygon_count += 1;
                    }
                }
            }
            tiles_succeeded += 1;
            if let Some(pb) = &progress {
                pb.inc(1);
            }
            continue;
        }

        // Story 8.2: Create subdirectories if pattern contains path separators
        if let Some(parent) = tile_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for tile {}", tile_id))?;
        }

        let field_mapping = config.output.field_mapping_path.as_deref();
        let header_config = config.header.as_ref();

        match (|| -> Result<ExportStats> {
            let mut writer = MpWriter::new(tile_path, field_mapping, header_config)?;
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
                global_stats.point_count += tile_stats.point_count;
                global_stats.linestring_count += tile_stats.linestring_count;
                global_stats.polygon_count += tile_stats.polygon_count;
                tiles_succeeded += 1;
            }
            Err(e) => {
                warn!(
                    tile_id = %tile_id,
                    error = %e,
                    "Tile export failed"
                );
                tiles_failed += 1;
                export_errors.push(TileExportError {
                    tile_id: tile_id.clone(),
                    error_message: e.to_string(),
                });
                if error_mode == ErrorMode::FailFast {
                    return Err(e).context(format!(
                        "Tile export failed (fail-fast mode): tile {}",
                        tile_id
                    ));
                }
            }
        }
        // `clipped_features` dropped here — memory freed

        if let Some(pb) = &progress {
            pb.inc(1);
        }
    }

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
    print_console_summary(&report, &config.output.directory, args, tiles_skipped_existing);

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
