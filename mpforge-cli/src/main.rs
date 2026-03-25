//! mpforge-cli: Polish Map tiling and export tool

use clap::Parser;
use mpforge_cli::{
    cli::{Cli, Commands},
    config, pipeline,
};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Initialize PROJ with embedded proj.db
    mpforge_cli::proj_init::init_proj()?;

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

            // Load configuration from YAML file
            let config = config::load_config(&args.config)?;

            // Run the pipeline (report JSON written inside pipeline::run if --report specified)
            pipeline::run(&config, args)?;

            Ok(())
        }
    }
}
