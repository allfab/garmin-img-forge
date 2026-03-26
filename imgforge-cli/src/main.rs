//! imgforge-cli: Polish Map (.mp) to Garmin IMG compiler

use clap::Parser;
use imgforge_cli::cli::{Cli, Commands};
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
    }

    Ok(())
}
