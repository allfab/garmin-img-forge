//! Integration tests for CLI argument parsing and behavior.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

#[test]
fn test_help_displays_build_command() {
    let mut cmd = cargo_bin_cmd!("mpforge-cli");
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("build"))
        .stdout(predicate::str::contains("Build Polish Map tiles"));
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
