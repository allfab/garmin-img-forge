//! mpforge-cli: Polish Map tiling and export tool

mod cli;
mod config;
mod error;
mod pipeline;
mod report;

use clap::Parser;
use cli::{Cli, Commands};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Setup tracing subscriber based on verbosity level
    match cli.command {
        Commands::Build(ref args) => {
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

            // Set global tracing subscriber (can only be called once)
            // If this fails, another subscriber was already set - log to stderr and continue
            if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
                eprintln!("Warning: Failed to set tracing subscriber: {}", e);
            }

            // Load configuration (stub - will fail in this story)
            // TODO: Story 5.2 - Enable actual config loading
            // let config = config::load_config(&args.config)?;

            // For now, create a placeholder config to test compilation
            // This will be replaced in Story 5.2
            let config = config::Config {
                version: 1,
                grid: config::GridConfig {
                    cell_size: 0.15,
                    overlap: 0.0,
                    origin: None,
                },
                inputs: vec![],
                output: config::OutputConfig {
                    directory: "tiles/".to_string(),
                    filename_pattern: "{x}_{y}.mp".to_string(),
                },
                filters: None,
                error_handling: "continue".to_string(),
            };

            // Run the pipeline
            pipeline::run(&config, args)?;

            // TODO: Story 7.3 - Export execution report if requested
            if let Some(_report_path) = &args.report {
                // report::save_report(report_path)?;
            }

            Ok(())
        }
    }
}
