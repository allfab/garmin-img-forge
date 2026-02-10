//! Integration test for complete pipeline
//! Story 5.3 - Task 7.5: End-to-end pipeline test

use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;

#[test]
fn test_pipeline_with_valid_sources() {
    // Create a temporary config file
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs:
  - path: tests/integration/fixtures/test_data/file1.shp
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: continue
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        verbose: 0,
    };

    // Run the pipeline
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should succeed with valid sources: {:?}",
        result.err()
    );
}

#[test]
fn test_pipeline_with_empty_sources() {
    // Config with no inputs
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs: []
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: continue
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        verbose: 0,
    };

    // Run the pipeline - Story 5.4 AC5: empty datasets are now supported
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_ok(),
        "Pipeline should succeed with empty dataset (Story 5.4 AC5): {:?}",
        result.err()
    );

    // Story 5.4 AC5: Empty datasets are now valid - file is created with warning logged
}

#[test]
fn test_pipeline_with_invalid_source() {
    // Config with invalid source
    let config_content = r#"
version: 1
grid:
  cell_size: 0.1
  overlap: 0.01
inputs:
  - path: /nonexistent/file.shp
output:
  directory: /tmp/mpforge-test
  filename_pattern: "tile_{x}_{y}.mp"
error_handling: fail-fast
"#;

    let config: Config = serde_yml::from_str(config_content).expect("Failed to parse config");

    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 1,
        fail_fast: false,
        report: None,
        verbose: 0,
    };

    // Run the pipeline - should fail in fail-fast mode
    let result = pipeline::run(&config, &args);

    assert!(
        result.is_err(),
        "Pipeline should fail with invalid source in fail-fast mode"
    );
}
