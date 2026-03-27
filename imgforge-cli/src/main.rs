//! imgforge-cli: Polish Map (.mp) to Garmin IMG compiler

use std::path::PathBuf;

use clap::Parser;
use imgforge_cli::cli::{Cli, Commands};
use imgforge_cli::BuildConfig;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Compile(args) => {
            let level = match args.verbose {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            };

            let subscriber = FmtSubscriber::builder()
                .with_max_level(level)
                .with_target(false)
                .finish();

            if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                eprintln!("Warning: Failed to set tracing subscriber: {}", e);
            }

            let input = std::path::Path::new(&args.input);
            let output = std::path::Path::new(&args.output);

            imgforge_cli::compile(input, output)?;
        }

        Commands::Build(args) => {
            let level = match args.verbose {
                0 => Level::WARN,
                1 => Level::INFO,
                2 => Level::DEBUG,
                _ => Level::TRACE,
            };

            let subscriber = FmtSubscriber::builder()
                .with_max_level(level)
                .with_target(false)
                .finish();

            if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                eprintln!("Warning: Failed to set tracing subscriber: {}", e);
            }

            let input_dir = std::path::Path::new(&args.input_dir);
            let output = std::path::Path::new(&args.output);

            let config = BuildConfig {
                family_id: args.family_id,
                product_id: args.product_id,
                description: args.description.clone(),
                block_size_exponent: 14, // production default (16 384 bytes)
                typ_file: args.typ.as_deref().map(PathBuf::from),
                jobs: args.jobs,
                // Hide progress bar in verbose mode to avoid interleaving with tracing logs.
                show_progress: args.verbose == 0,
            };

            // Use the public library API (L3: avoid bypassing lib.rs).
            let stats = imgforge_cli::build(input_dir, output, config)?;

            // Always-visible summary regardless of verbose level (AC5).
            println!(
                "Built {} tile(s) → {} subfiles, {} bytes in {:.1}s",
                stats.tile_count,
                stats.subfile_count,
                stats.total_bytes,
                stats.compilation.duration_seconds,
            );

            // Optional JSON report.
            if let Some(ref report_path) = args.report {
                use imgforge_cli::report::{BuildReport, FeaturesByType, ReportStatus};
                let report = BuildReport {
                    status: if stats.compilation.tiles_failed == 0 {
                        ReportStatus::Success
                    } else {
                        ReportStatus::Failure
                    },
                    tiles_compiled: stats.tile_count,
                    tiles_failed: stats.compilation.tiles_failed,
                    features_by_type: FeaturesByType {
                        poi: stats.compilation.poi_count,
                        polyline: stats.compilation.polyline_count,
                        polygon: stats.compilation.polygon_count,
                    },
                    routing_nodes: stats.compilation.routing_nodes,
                    routing_arcs: stats.compilation.routing_arcs,
                    img_size_bytes: stats.total_bytes,
                    duration_seconds: stats.compilation.duration_seconds,
                    errors: stats.compilation.tile_errors.clone(),
                };
                imgforge_cli::report::write_json_report(
                    &report,
                    std::path::Path::new(report_path),
                )?;
                tracing::info!(path = %report_path, "JSON report written");
            }
        }
    }

    Ok(())
}
