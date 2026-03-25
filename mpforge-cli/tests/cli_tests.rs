//! Integration tests for CLI argument parsing and behavior.
//!
//! Tests cover:
//! - Version display (AC1: Story 7.5)
//! - Help flags (AC2, AC3, AC4: Story 7.5)
//! - Argument parsing and validation

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

// ============================================================================
// Story 7.5: CLI Standards Compliance - Version Display (AC1)
// ============================================================================

#[test]
fn test_version_flag_short() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("-V");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("mpforge-cli "));
}

#[test]
fn test_version_flag_long() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with("mpforge-cli "));
}

#[test]
fn test_version_format() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--version");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Format attendu: "mpforge-cli <version>" (git describe ou CARGO_PKG_VERSION)
    assert!(
        stdout.starts_with("mpforge-cli "),
        "Version output should start with 'mpforge-cli ', got: {}",
        stdout
    );

    // Vérifier que la version contient au moins un composant SemVer (X.Y.Z)
    let version_part = stdout.trim().split_whitespace().nth(1).unwrap();
    let base_version = version_part.trim_start_matches('v');
    let semver_part: &str = base_version.split('-').next().unwrap_or(base_version);
    let parts: Vec<&str> = semver_part.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "Version should contain X.Y.Z semver base, got: {}",
        version_part
    );
}

// ============================================================================
// Story 7.5: CLI Standards Compliance - Global Help (AC2)
// ============================================================================

#[test]
fn test_help_flag_short() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("-h");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Polish Map tiling"))
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn test_help_flag_long() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Polish Map (.mp) files"));
}

#[test]
fn test_help_shows_commands() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn test_help_displays_build_command() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains(
            "Execute the complete tiling pipeline",
        ));
}

// ============================================================================
// Story 7.5: CLI Standards Compliance - Command-Specific Help (AC3)
// ============================================================================

#[test]
fn test_build_help() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Build tiled .mp files"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--jobs"))
        .stdout(predicate::str::contains("--report"));
}

#[test]
fn test_build_help_shows_all_options() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--help"]);

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--input"))
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--jobs"))
        .stdout(predicate::str::contains("--fail-fast"))
        .stdout(predicate::str::contains("--report"))
        .stdout(predicate::str::contains("-v"));
}

// ============================================================================
// Story 7.5: CLI Standards Compliance - Help Exhaustivité (AC4)
// ============================================================================

#[test]
fn test_help_documents_all_features() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--help"]);

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Vérifier documentation features clés (Epics 5-7) - options CLI
    assert!(
        stdout.contains("--config"),
        "Missing --config documentation"
    );
    assert!(stdout.contains("--input"), "Missing --input documentation");
    assert!(
        stdout.contains("--output"),
        "Missing --output documentation"
    );
    assert!(
        stdout.contains("--jobs"),
        "Missing --jobs documentation (Story 7.1)"
    );
    assert!(
        stdout.contains("--fail-fast"),
        "Missing --fail-fast documentation"
    );
    assert!(
        stdout.contains("--report"),
        "Missing --report documentation (Story 7.3)"
    );
    assert!(
        stdout.contains("-v"),
        "Missing verbose flag documentation (Story 7.2)"
    );
}

#[test]
fn test_global_help_documents_all_features() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--help");

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // AC4: Vérifier exhaustivité features dans la description globale
    assert!(
        stdout.contains("Multi-source fusion"),
        "Missing multi-source fusion feature (Epic 5)"
    );
    assert!(
        stdout.contains("grid tiling"),
        "Missing tiling pipeline feature (Epic 6)"
    );
    assert!(
        stdout.contains("Parallel processing"),
        "Missing parallel processing feature (Story 7.1)"
    );
    assert!(
        stdout.contains("Progress tracking"),
        "Missing progress tracking feature (Story 7.2)"
    );
    assert!(
        stdout.contains("JSON reports"),
        "Missing JSON reports feature (Story 7.3)"
    );
    assert!(
        stdout.contains("Field mapping"),
        "Missing field mapping feature (Story 7.4)"
    );
    assert!(
        stdout.contains("Examples:"),
        "Missing usage examples in global help (AC2)"
    );
}

// ============================================================================
// Existing CLI Tests - Argument Parsing
// ============================================================================

#[test]
fn test_build_without_config_fails() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("build");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--config"));
}

#[test]
fn test_build_with_config_succeeds() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--config", "test.yaml"]);

    // In Story 5.1, the pipeline stub succeeds with placeholder config
    // Story 5.2 will implement actual config validation
    cmd.assert().success();
}

#[test]
fn test_verbosity_flag_parsing() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--config", "test.yaml", "-vv"]);

    // Verify the command accepts verbosity flags and runs successfully
    // The stub pipeline should complete successfully in this story
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Pipeline"));
}

#[test]
fn test_jobs_option_parsing() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--config", "test.yaml", "--jobs", "4"]);

    // Verify the jobs argument is accepted and pipeline runs
    cmd.assert().success();
}

#[test]
fn test_fail_fast_flag_parsing() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.args(["build", "--config", "test.yaml", "--fail-fast"]);

    // Verify the fail-fast flag is accepted and pipeline runs
    cmd.assert().success();
}
