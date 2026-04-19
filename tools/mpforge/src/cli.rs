//! CLI argument parsing using clap.

use anyhow;
use clap::{Parser, Subcommand};
use num_cpus;
use tracing;

/// Polish Map tiling and processing pipeline
///
/// mpforge processes vector data sources into tiled Polish Map (.mp) files
/// optimized for Garmin GPS devices. Supports multi-source fusion, configurable
/// grid tiling, parallel processing, and comprehensive error handling.
#[derive(Parser, Debug)]
#[command(
    name = "mpforge",
    version = env!("GIT_VERSION"),
    about = "Polish Map tiling and processing pipeline",
    long_about = "mpforge: Production-ready CLI for processing vector data into tiled Polish Map (.mp) files for Garmin GPS devices.\n\n\
                  Features:\n\
                  - Multi-source fusion (Shapefile, GeoPackage, PostGIS)\n\
                  - Configurable grid tiling with geometry clipping\n\
                  - Parallel processing for large datasets\n\
                  - Progress tracking and JSON reports\n\
                  - Field mapping configuration (YAML-based source-to-target mapping)\n\
                  - Robust error handling with configurable modes\n\n\
                  Examples:\n  \
                  mpforge build --config config.yaml\n  \
                  mpforge build --config config.yaml --jobs 8\n  \
                  mpforge build --config config.yaml --jobs 4 --report report.json"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Execute the complete tiling pipeline
    ///
    /// Processes vector sources through fusion, tiling, clipping, and export phases.
    /// Supports parallel processing, progress tracking, and comprehensive error reporting.
    #[command(
        long_about = "Build tiled .mp files from vector sources using configured grid and processing options.\n\n\
                            Options:\n  \
                            --skip-existing  Resume interrupted export by skipping already exported tiles\n  \
                            --dry-run        Preview mode: show what would be exported without writing files\n\n\
                            Example:\n  \
                            mpforge build --config config.yaml --jobs 8\n  \
                            mpforge build --config config.yaml --skip-existing\n  \
                            mpforge build --config config.yaml --dry-run"
    )]
    Build(BuildArgs),

    /// Validate configuration files without running the pipeline
    ///
    /// Checks YAML syntax, semantic rules, referenced files (rules, field mapping,
    /// header template), and input file existence after wildcard resolution.
    #[command(
        long_about = "Validate configuration files without running the pipeline.\n\n\
                            Checks performed:\n  \
                            1. YAML syntax validation\n  \
                            2. Semantic rules (grid, inputs, bbox, SRS, base_id, filename pattern, header)\n  \
                            3. Input files existence (after wildcard resolution)\n  \
                            4. Rules file parsing and validation\n  \
                            5. Field mapping file parsing\n  \
                            6. Header template file existence\n  \
                            7. Spatial filter source file existence\n  \
                            8. Generalize configuration\n  \
                            9. Label case configuration (coherence with Label fields)\n\n\
                            Exit code: 0 if valid, 1 if invalid.\n\n\
                            Example:\n  \
                            mpforge validate --config config.yaml\n  \
                            mpforge validate --config config.yaml --report report.json\n  \
                            mpforge validate --config config.yaml -v"
    )]
    Validate(ValidateArgs),
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
    ///
    /// Use 1 for sequential processing (debug mode).
    /// Use 2-8 for parallel processing (production mode).
    /// Warning: Values > num_cpus may degrade performance.
    #[arg(short, long, default_value = "1")]
    pub jobs: usize,

    /// Stop on first error instead of continuing
    #[arg(long)]
    pub fail_fast: bool,

    /// Path to export JSON execution report
    #[arg(short, long)]
    pub report: Option<String>,

    /// Skip tiles whose output file already exists (resume interrupted export)
    #[arg(long)]
    pub skip_existing: bool,

    /// Simulate export without writing files (preview mode)
    #[arg(long)]
    pub dry_run: bool,

    /// Tech-spec #2 Task 15: ignore the external catalog referenced by
    /// `generalize_profiles_path`. Inline `generalize:` fields in
    /// `sources.yaml` remain active — this matches the state captured by the
    /// golden baseline `mpforge-multi-data-profiles-disabled.sha256`
    /// (pre-multi-Data but post-`ZONE_D_HABITATION`). Used to regenerate the
    /// baseline without mutating any YAML. Also accepts the env var
    /// `MPFORGE_PROFILES=off`.
    #[arg(long)]
    pub disable_profiles: bool,

    /// Verbosity level (-v: INFO, -vv: DEBUG, -vvv: TRACE)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to configuration YAML file
    #[arg(short, long)]
    pub config: String,

    /// Path to export JSON validation report
    #[arg(short, long)]
    pub report: Option<String>,

    /// Verbosity level (-v: INFO, -vv: DEBUG, -vvv: TRACE)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

impl BuildArgs {
    /// Validate and normalize jobs parameter.
    ///
    /// Returns validated jobs count or error if invalid.
    /// Logs warning if jobs > num_cpus.
    pub fn validate_jobs(&self) -> anyhow::Result<usize> {
        if self.jobs == 0 {
            anyhow::bail!("--jobs must be > 0 (greater than 0)");
        }

        let num_cpus = num_cpus::get();
        if self.jobs > num_cpus {
            tracing::warn!(
                jobs = self.jobs,
                num_cpus = num_cpus,
                "WARNING: --jobs exceeds available CPUs, may degrade performance"
            );
        }

        tracing::info!(jobs = self.jobs, "Parallel jobs configured");
        Ok(self.jobs)
    }
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
            skip_existing: false,
            dry_run: false,
            disable_profiles: false,
            verbose: 0,
        };
    }

    #[test]
    fn test_build_args_defaults() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge", "build", "--config", "test.yaml"]);
        assert!(args.is_ok());

        let Commands::Build(build_args) = args.unwrap().command else {
            panic!("Expected Build command");
        };
        assert_eq!(build_args.config, "test.yaml");
        assert_eq!(build_args.jobs, 1);
        assert!(!build_args.fail_fast);
        assert!(!build_args.skip_existing);
        assert!(!build_args.dry_run);
        assert_eq!(build_args.verbose, 0);
    }

    #[test]
    fn test_build_args_missing_config() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge", "build"]);
        assert!(args.is_err());
    }

    #[test]
    fn test_build_args_all_options() {
        use clap::Parser;
        let args = Cli::try_parse_from([
            "mpforge",
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
            "--skip-existing",
            "--dry-run",
            "-vv",
        ]);
        assert!(args.is_ok());

        let Commands::Build(build_args) = args.unwrap().command else {
            panic!("Expected Build command");
        };
        assert_eq!(build_args.config, "test.yaml");
        assert_eq!(build_args.input, Some("input/".to_string()));
        assert_eq!(build_args.output, Some("output/".to_string()));
        assert_eq!(build_args.jobs, 4);
        assert!(build_args.fail_fast);
        assert_eq!(build_args.report, Some("report.json".to_string()));
        assert!(build_args.skip_existing);
        assert!(build_args.dry_run);
        assert_eq!(build_args.verbose, 2);
    }

    #[test]
    fn test_validate_args_defaults() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge", "validate", "--config", "test.yaml"]);
        assert!(args.is_ok());

        let Commands::Validate(validate_args) = args.unwrap().command else {
            panic!("Expected Validate command");
        };
        assert_eq!(validate_args.config, "test.yaml");
        assert!(validate_args.report.is_none());
        assert_eq!(validate_args.verbose, 0);
    }

    #[test]
    fn test_validate_args_all_options() {
        use clap::Parser;
        let args = Cli::try_parse_from([
            "mpforge",
            "validate",
            "--config",
            "test.yaml",
            "--report",
            "report.json",
            "-vv",
        ]);
        assert!(args.is_ok());

        let Commands::Validate(validate_args) = args.unwrap().command else {
            panic!("Expected Validate command");
        };
        assert_eq!(validate_args.config, "test.yaml");
        assert_eq!(validate_args.report, Some("report.json".to_string()));
        assert_eq!(validate_args.verbose, 2);
    }

    #[test]
    fn test_validate_args_missing_config() {
        use clap::Parser;
        let args = Cli::try_parse_from(["mpforge", "validate"]);
        assert!(args.is_err());
    }
}
