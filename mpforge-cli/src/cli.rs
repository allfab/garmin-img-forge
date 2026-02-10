//! CLI argument parsing using clap.

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mpforge-cli")]
#[command(about = "CLI tool for tiling and exporting Polish Map (.mp) files")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Build Polish Map tiles from input sources
    Build(BuildArgs),
}

#[derive(Parser, Debug)]
pub struct BuildArgs {
    /// Path to configuration YAML file
    #[arg(short, long)]
    pub config: String,

    /// Override input directory/path from config
    #[arg(short, long)]
    pub input: Option<String>,

    /// Override output directory from config
    #[arg(short, long)]
    pub output: Option<String>,

    /// Number of parallel jobs (default: 1)
    #[arg(short, long, default_value = "1")]
    pub jobs: usize,

    /// Stop on first error instead of continuing
    #[arg(long)]
    pub fail_fast: bool,

    /// Path to export JSON execution report
    #[arg(short, long)]
    pub report: Option<String>,

    /// Verbosity level (-v: INFO, -vv: DEBUG, -vvv: TRACE)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_help() {
        // This test verifies that --help works (will be tested in integration tests)
        // Unit test just validates struct definition compiles
        let _args = BuildArgs {
            config: "test.yaml".to_string(),
            input: None,
            output: None,
            jobs: 1,
            fail_fast: false,
            report: None,
            verbose: 0,
        };
    }

    #[test]
    fn test_build_args_defaults() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge-cli", "build", "--config", "test.yaml"]);
        assert!(args.is_ok());

        let Commands::Build(build_args) = args.unwrap().command;
        assert_eq!(build_args.config, "test.yaml");
        assert_eq!(build_args.jobs, 1);
        assert!(!build_args.fail_fast);
        assert_eq!(build_args.verbose, 0);
    }

    #[test]
    fn test_build_args_missing_config() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge-cli", "build"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_build_args_all_options() {
        use clap::Parser;
        let args = Cli::try_parse_from([
            "mpforge-cli",
            "build",
            "--config",
            "test.yaml",
            "--input",
            "input/",
            "--output",
            "output/",
            "--jobs",
            "4",
            "--fail-fast",
            "--report",
            "report.json",
            "-vv",
        ]);
        assert!(args.is_ok());

        let Commands::Build(build_args) = args.unwrap().command;
        assert_eq!(build_args.config, "test.yaml");
        assert_eq!(build_args.input, Some("input/".to_string()));
        assert_eq!(build_args.output, Some("output/".to_string()));
        assert_eq!(build_args.jobs, 4);
        assert!(build_args.fail_fast);
        assert_eq!(build_args.report, Some("report.json".to_string()));
        assert_eq!(build_args.verbose, 2);
    }
}
