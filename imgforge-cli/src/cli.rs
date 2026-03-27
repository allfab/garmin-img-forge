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
                  imgforge-cli compile map.mp -o map.img -v\n  \
                  imgforge-cli build --input-dir tiles/ -o gmapsupp.img\n  \
                  imgforge-cli build --input-dir tiles/ -o gmapsupp.img --family-id 6324 --description \"France BDTOPO 2025\""
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

    /// Assemble a directory of Polish Map (.mp) tiles into a single gmapsupp.img
    ///
    /// Scans the input directory for .mp files, compiles each tile into subfiles
    /// (TRE/RGN/LBL and NET/NOD if routing is present), and produces a single
    /// gmapsupp.img suitable for direct copy to a Garmin GPS device.
    Build(BuildArgs),
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

#[derive(Parser, Debug)]
pub struct BuildArgs {
    /// Input directory containing .mp tile files
    #[arg(long)]
    pub input_dir: String,

    /// Output gmapsupp.img file path
    #[arg(short, long)]
    pub output: String,

    /// Garmin family ID — identifies the map family in BaseCamp (LE16 at header offset 0x054)
    #[arg(long, default_value_t = 6324)]
    pub family_id: u16,

    /// Garmin product ID — identifies the product in BaseCamp (LE16 at header offset 0x056)
    #[arg(long, default_value_t = 1)]
    pub product_id: u16,

    /// Map description visible in BaseCamp
    #[arg(long, default_value = "mpforge map")]
    pub description: String,

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
        let Commands::Compile(args) = cli.unwrap().command else { panic!("expected Compile") };
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
        let Commands::Compile(args) = cli.unwrap().command else { panic!("expected Compile") };
        assert_eq!(args.verbose, 2);
    }

    #[test]
    fn test_cli_no_subcommand_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli"]);
        assert!(cli.is_err());
    }

    // ── Build subcommand ─────────────────────────────────────────────────────

    #[test]
    fn test_cli_build_missing_output_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli", "build", "--input-dir", "tiles/"]);
        assert!(cli.is_err(), "missing -o/--output must fail");
    }

    #[test]
    fn test_cli_build_required_args() {
        let cli =
            Cli::try_parse_from(["imgforge-cli", "build", "--input-dir", "tiles/", "-o", "out.img"]);
        assert!(cli.is_ok());
        let Commands::Build(args) = cli.unwrap().command else {
            panic!("expected Build command");
        };
        assert_eq!(args.input_dir, "tiles/");
        assert_eq!(args.output, "out.img");
    }

    #[test]
    fn test_cli_build_missing_input_dir_is_error() {
        let cli = Cli::try_parse_from(["imgforge-cli", "build", "-o", "out.img"]);
        assert!(cli.is_err(), "missing --input-dir must fail");
    }

    #[test]
    fn test_cli_build_default_family_id() {
        let cli =
            Cli::try_parse_from(["imgforge-cli", "build", "--input-dir", "tiles/", "-o", "out.img"]);
        assert!(cli.is_ok());
        let Commands::Build(args) = cli.unwrap().command else {
            panic!("expected Build command");
        };
        assert_eq!(args.family_id, 6324, "default family_id must be 6324");
        assert_eq!(args.product_id, 1, "default product_id must be 1");
        assert_eq!(args.description, "mpforge map");
    }

    #[test]
    fn test_cli_build_explicit_family_id() {
        let cli = Cli::try_parse_from([
            "imgforge-cli",
            "build",
            "--input-dir",
            "tiles/",
            "-o",
            "out.img",
            "--family-id",
            "1234",
            "--product-id",
            "2",
            "--description",
            "France BDTOPO 2025",
        ]);
        assert!(cli.is_ok());
        let Commands::Build(args) = cli.unwrap().command else {
            panic!("expected Build command");
        };
        assert_eq!(args.family_id, 1234);
        assert_eq!(args.product_id, 2);
        assert_eq!(args.description, "France BDTOPO 2025");
    }
}
