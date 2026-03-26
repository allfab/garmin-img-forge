//! CLI argument parsing using clap derive macros.

use clap::{Parser, Subcommand};

/// imgforge-cli: Polish Map (.mp) to Garmin IMG compiler
///
/// Compiles Polish Map (.mp) files produced by mpforge-cli into Garmin
/// binary IMG format (.img) suitable for upload to GPS devices.
#[derive(Parser, Debug)]
#[command(
    name = "imgforge-cli",
    version = env!("GIT_VERSION"),
    about = "Polish Map (.mp) to Garmin IMG compiler",
    long_about = "imgforge-cli: Compiles Polish Map (.mp) files into Garmin binary IMG format.\n\n\
                  Features:\n\
                  - Pure Rust implementation (no Java/mkgmap dependency)\n\
                  - Direct .mp parsing with routing attribute support\n\
                  - Standalone binary with zero external runtime dependencies\n\n\
                  Examples:\n  \
                  imgforge-cli compile map.mp -o map.img\n  \
                  imgforge-cli compile map.mp -o map.img -v"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compile a Polish Map (.mp) file to Garmin IMG format
    ///
    /// Reads the input .mp file, parses all features (POI, POLYLINE, POLYGON),
    /// and produces a binary .img file compatible with Garmin GPS devices.
    Compile(CompileArgs),
}

#[derive(Parser, Debug)]
pub struct CompileArgs {
    /// Input Polish Map (.mp) file to compile
    pub input: String,

    /// Output Garmin IMG file path
    #[arg(short, long)]
    pub output: String,

    /// Verbosity level (-v: INFO, -vv: DEBUG, -vvv: TRACE)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_compile_required_args() {
        let cli = Cli::try_parse_from(["imgforge-cli", "compile", "map.mp", "-o", "map.img"]);
        assert!(cli.is_ok());
        let Commands::Compile(args) = cli.unwrap().command;
        assert_eq!(args.input, "map.mp");
        assert_eq!(args.output, "map.img");
        assert_eq!(args.verbose, 0);
    }

    #[test]
    fn test_cli_compile_missing_output_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli", "compile", "map.mp"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_compile_missing_input_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli", "compile"]);
        assert!(cli.is_err());
    }

    #[test]
    fn test_cli_compile_verbose() {
        let cli =
            Cli::try_parse_from(["imgforge-cli", "compile", "map.mp", "-o", "map.img", "-vv"]);
        assert!(cli.is_ok());
        let Commands::Compile(args) = cli.unwrap().command;
        assert_eq!(args.verbose, 2);
    }

    #[test]
    fn test_cli_no_subcommand_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli"]);
        assert!(cli.is_err());
    }
}
