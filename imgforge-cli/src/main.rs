//! imgforge-cli: Polish Map (.mp) to Garmin IMG compiler

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
            };

            imgforge_cli::build(input_dir, output, config)?;
        }
    }

    Ok(())
}
