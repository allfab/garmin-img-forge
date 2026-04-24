//! mpforge: Polish Map tiling and export tool

use clap::Parser;
use mpforge::{
    cli::{Cli, Commands},
    config, pipeline,
    report::CheckStatus,
};
use std::ffi::CStr;
use std::process::ExitCode;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Handler GDAL/GEOS : route les messages C vers tracing avec target="gdal".
/// Permet de distinguer dans les logs les avertissements GDAL (ex: clipping
/// aux bords de tuile) des messages mpforge natifs.
unsafe extern "C" fn gdal_error_handler(
    err_class: gdal_sys::CPLErr::Type,
    _err_num: gdal_sys::CPLErrorNum,
    msg: *const std::ffi::c_char,
) {
    let msg = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    match err_class {
        gdal_sys::CPLErr::CE_Warning => tracing::warn!(target: "gdal", "{}", msg),
        gdal_sys::CPLErr::CE_Failure | gdal_sys::CPLErr::CE_Fatal => {
            tracing::error!(target: "gdal", "{}", msg)
        }
        _ => tracing::debug!(target: "gdal", "{}", msg),
    }
}

/// Setup tracing subscriber based on verbosity level.
fn setup_tracing(verbose: u8) {
    let level = match verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true)
        .finish();

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Warning: Failed to set tracing subscriber: {}", e);
    }

    // Route les messages GDAL/GEOS dans tracing (target="gdal") pour les
    // distinguer des logs mpforge natifs dans la sortie.
    unsafe { gdal_sys::CPLSetErrorHandler(Some(gdal_error_handler)) };
}

fn main() -> ExitCode {
    // Initialize PROJ with embedded proj.db
    if let Err(e) = mpforge::proj_init::init_proj() {
        eprintln!("Error: Failed to initialize PROJ: {:#}", e);
        return ExitCode::FAILURE;
    }

    // Parse command-line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Build(ref args) => {
            setup_tracing(args.verbose);

            let mut config = match config::load_config(&args.config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {:#}", e);
                    return ExitCode::FAILURE;
                }
            };

            // Tech-spec #2 Task 15: strict opt-out via CLI flag or env var.
            // Ignores the external profile catalog AND every inline
            // `generalize:` field — bringing mpforge back to a pre-tech-spec #2
            // baseline without mutating any YAML.
            let disable_profiles = args.disable_profiles
                || std::env::var("MPFORGE_PROFILES")
                    .map(|v| v.eq_ignore_ascii_case("off"))
                    .unwrap_or(false);
            if disable_profiles {
                // Tech-spec #2 AC1 (cf. tech-spec Task 15 §Pré-condition) :
                // `--disable-profiles` NE bypasse QUE le catalogue externe
                // (generalize_profiles_path). L'inline `generalize:` reste
                // actif — état "post-Z_D_H, pré-multi-Data" capturé par le
                // golden baseline. Délégué à `Config::reset_profile_map_to_inline_only`
                // (H4 code review) plutôt qu'à une mutation directe du champ.
                tracing::info!(
                    "generalize profiles disabled via --disable-profiles / MPFORGE_PROFILES=off \
                     — external catalog cleared, inline `generalize:` preserved"
                );
                config.reset_profile_map_to_inline_only();
            }

            if let Err(e) = pipeline::run(&config, args) {
                eprintln!("Error: {:#}", e);
                return ExitCode::FAILURE;
            }

            ExitCode::SUCCESS
        }
        Commands::Validate(ref args) => {
            setup_tracing(args.verbose);

            let report = match config::run_validate(&args.config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error: {:#}", e);
                    return ExitCode::FAILURE;
                }
            };

            // Display text summary
            for check in &report.checks {
                let icon = match check.status {
                    CheckStatus::Pass => "\u{2713}",
                    CheckStatus::Fail => "\u{2717}",
                    CheckStatus::Skipped => "-",
                };
                println!(
                    "{} {:<20} \u{2014} {}",
                    icon, check.name, check.details
                );
            }

            // Print warnings
            for warning in &report.warnings {
                println!("  \u{26a0} {}", warning);
            }

            println!();
            let failed = report.failed_count();
            if report.is_valid() {
                println!(
                    "Config valid. ({}/{} checks passed)",
                    report.passed_count(),
                    report.checks.len()
                );
            } else {
                println!(
                    "Config invalid. ({}/{} checks passed, {} error{})",
                    report.passed_count(),
                    report.checks.len(),
                    failed,
                    if failed > 1 { "s" } else { "" }
                );
            }

            // Write JSON report if --report specified
            if let Some(ref report_path) = args.report {
                if let Err(e) = mpforge::report::write_validation_report(&report, report_path) {
                    eprintln!("Error writing report: {:#}", e);
                    return ExitCode::FAILURE;
                }
                println!("Validation report written to: {}", report_path);
            }

            if report.is_valid() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
