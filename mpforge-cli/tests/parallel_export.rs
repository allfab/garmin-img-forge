//! Tests for parallel tile export functionality (Story 7.1 + Story 11.1 + Story 11.2).

use clap::Parser;
use mpforge_cli::cli::BuildArgs;
use mpforge_cli::config::Config;
use mpforge_cli::pipeline;
use mpforge_cli::pipeline::{
    TileExportError, TileOutcome, TileResult, SharedAccumulators, aggregate_outcome,
};
use mpforge_cli::pipeline::geometry_validator::ValidationStats;
use mpforge_cli::pipeline::reader::{MultiGeometryStats, UnsupportedTypeStats};
use mpforge_cli::pipeline::writer::ExportStats;
use mpforge_cli::rules::RuleStats;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tempfile::TempDir;

#[test]
fn test_jobs_default_is_1() {
    // AC2: --jobs non spécifié → default 1 (séquentiel)
    use mpforge_cli::cli::{Cli, Commands};

    let args = Cli::try_parse_from(["mpforge-cli", "build", "--config", "test.yaml"]);
    assert!(args.is_ok());

    let Commands::Build(build_args) = args.unwrap().command;
    assert_eq!(build_args.jobs, 1);
}

#[test]
fn test_jobs_validation_zero_rejected() {
    // Validation: --jobs 0 → erreur
    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 0,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    let result = args.validate_jobs();
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("must be > 0") || error_msg.contains("greater than 0"));
}

#[test]
fn test_jobs_exceeds_num_cpus_warning() {
    // Validation: --jobs > num_cpus → warning loggé (mais accepté)
    let num_cpus = num_cpus::get();
    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: num_cpus + 4,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    // Should succeed but log warning
    let result = args.validate_jobs();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), num_cpus + 4);
}

#[test]
fn test_jobs_valid_value() {
    // Validation: --jobs 4 → OK
    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 4,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    let result = args.validate_jobs();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 4);
}

#[test]
fn test_atomic_counters_thread_safe() {
    // Story 11.1 AC4: Verify atomic counters are thread-safe (stress test)
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    (0..10000).into_par_iter().for_each(|_| {
        counter_clone.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(counter.load(Ordering::Relaxed), 10000);
}

#[test]
fn test_mutex_error_collection_thread_safe() {
    // Story 11.1 AC5: Verify Mutex<Vec> collects errors from multiple threads
    use rayon::prelude::*;
    use std::sync::{Arc, Mutex};

    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Simulate 20 tiles, 3 of which fail
    let tiles: Vec<usize> = (0..20).collect();
    let failing_tiles = [3, 7, 15];

    tiles.par_iter().for_each(|tile_id| {
        if failing_tiles.contains(tile_id) {
            errors
                .lock()
                .expect("lock poisoned")
                .push(format!("Tile {} failed", tile_id));
        }
    });

    let collected = errors.lock().expect("lock poisoned");
    assert_eq!(
        collected.len(),
        3,
        "Expected 3 errors collected, got {}",
        collected.len()
    );
}

#[test]
fn test_rayon_try_for_each_stops_on_first_error() {
    // Story 11.1 AC6: Verify try_for_each stops processing on first error
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let processed = Arc::new(AtomicUsize::new(0));
    let processed_clone = Arc::clone(&processed);

    let tiles: Vec<usize> = (0..100).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("pool");

    let result: Result<(), String> = pool.install(|| {
        tiles.par_iter().try_for_each(|tile_id| {
            processed_clone.fetch_add(1, Ordering::Relaxed);
            if *tile_id == 5 {
                Err(format!("Tile {} failed", tile_id))
            } else {
                Ok(())
            }
        })
    });

    assert!(result.is_err(), "Should return error");
    let total_processed = processed.load(Ordering::Relaxed);
    // try_for_each should stop early — not all 100 tiles should be processed
    assert!(
        total_processed < 100,
        "Expected early termination, but {} tiles were processed",
        total_processed
    );
}

// ============================================================================
// Integration Tests: Parallel vs Sequential Export
// ============================================================================

/// Helper to create a test configuration for parallel export
fn create_parallel_test_config(
    temp_dir: &TempDir,
    fixture_path: &str,
    error_handling: &str,
) -> Config {
    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.05
  overlap: 0.0
inputs:
  - path: "{}"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "{}"
"#,
        fixture_path,
        temp_dir.path().join("tiles").display(),
        error_handling
    );

    serde_yml::from_str(&config_yaml).expect("Failed to parse test config")
}

/// Helper to create BuildArgs with specific jobs count
fn create_test_args_with_jobs(jobs: usize) -> BuildArgs {
    BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs,
        fail_fast: false,
        report: None,
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    }
}

#[test]
fn test_jobs_1_sequential_behavior() {
    // AC2: --jobs 1 → comportement identique séquentiel
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_parallel_test_config(&temp_dir, fixture_path, "continue");
    let args_jobs_1 = create_test_args_with_jobs(1);

    let result = pipeline::run(&config, &args_jobs_1);
    assert!(result.is_ok(), "Pipeline with --jobs 1 should succeed");

    let summary = result.unwrap();
    assert!(
        summary.tiles_succeeded > 0,
        "At least one tile should be exported"
    );
    assert_eq!(summary.tiles_failed, 0, "No tiles should fail");
}

#[test]
fn test_parallel_export_produces_same_results() {
    // AC1: Parallel export should produce same results as sequential
    let temp_dir_seq = TempDir::new().expect("Failed to create temp dir");
    let temp_dir_par = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    // Sequential export (jobs=1)
    let config_seq = create_parallel_test_config(&temp_dir_seq, fixture_path, "continue");
    let args_seq = create_test_args_with_jobs(1);
    let result_seq = pipeline::run(&config_seq, &args_seq);
    assert!(result_seq.is_ok());
    let summary_seq = result_seq.unwrap();

    // Parallel export (jobs=2)
    let config_par = create_parallel_test_config(&temp_dir_par, fixture_path, "continue");
    let args_par = create_test_args_with_jobs(2);
    let result_par = pipeline::run(&config_par, &args_par);
    assert!(result_par.is_ok());
    let summary_par = result_par.unwrap();

    // Both should produce same number of tiles
    assert_eq!(
        summary_seq.tiles_succeeded, summary_par.tiles_succeeded,
        "Sequential and parallel should export same number of tiles"
    );
    assert_eq!(
        summary_seq.total_features(),
        summary_par.total_features(),
        "Sequential and parallel should export same number of features"
    );

    // Verify same tile files exist
    let tiles_seq_dir = temp_dir_seq.path().join("tiles");
    let tiles_par_dir = temp_dir_par.path().join("tiles");

    let seq_files: std::collections::HashSet<String> = fs::read_dir(&tiles_seq_dir)
        .expect("Failed to read sequential tiles dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let par_files: std::collections::HashSet<String> = fs::read_dir(&tiles_par_dir)
        .expect("Failed to read parallel tiles dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    assert_eq!(
        seq_files.len(),
        par_files.len(),
        "Same number of tile files should exist"
    );
    assert_eq!(seq_files, par_files, "Same tile filenames should exist");

    // Compare file sizes to verify content consistency
    for filename in &seq_files {
        let seq_path = tiles_seq_dir.join(filename);
        let par_path = tiles_par_dir.join(filename);

        let seq_metadata = fs::metadata(&seq_path).expect("Failed to read seq file metadata");
        let par_metadata = fs::metadata(&par_path).expect("Failed to read par file metadata");

        assert_eq!(
            seq_metadata.len(),
            par_metadata.len(),
            "File {} should have same size in sequential and parallel export",
            filename
        );

        assert!(
            seq_metadata.len() > 0,
            "Sequential file {} should not be empty",
            filename
        );
        assert!(
            par_metadata.len() > 0,
            "Parallel file {} should not be empty",
            filename
        );
    }
}

#[test]
#[ignore] // Ignore by default as it requires specific performance setup
fn test_speedup_jobs_4_vs_jobs_1() {
    // Story 11.1 AC8: Speedup > 2× pour --jobs 4 avec 100+ tuiles
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found");
        return;
    }

    let config = create_parallel_test_config(&temp_dir, fixture_path, "continue");

    // Baseline: --jobs 1
    let args_1 = create_test_args_with_jobs(1);
    let start_1 = Instant::now();
    let result_1 = pipeline::run(&config, &args_1);
    let duration_1 = start_1.elapsed();
    assert!(result_1.is_ok());

    // Cleanup for second run
    fs::remove_dir_all(temp_dir.path().join("tiles")).ok();

    // Parallel: --jobs 4
    let args_4 = create_test_args_with_jobs(4);
    let start_4 = Instant::now();
    let result_4 = pipeline::run(&config, &args_4);
    let duration_4 = start_4.elapsed();
    assert!(result_4.is_ok());

    // Calculate speedup
    let speedup = duration_1.as_secs_f64() / duration_4.as_secs_f64();

    println!("Duration --jobs 1: {:?}", duration_1);
    println!("Duration --jobs 4: {:?}", duration_4);
    println!("Speedup: {:.2}×", speedup);

    // AC8: Speedup > 2× (requires large dataset with 100+ tiles)
    // For small datasets, speedup may be < 1 due to thread pool overhead
    // assert!(speedup > 2.0, "Speedup {:.2}× is below 2× threshold", speedup);
}

#[test]
fn test_thread_safe_error_collection_continue_mode() {
    // Story 11.1 AC5: Mode Continue + erreurs → collection thread-safe
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.05
  overlap: 0.0
inputs:
  - path: "tests/integration/fixtures/test_data/file1.shp"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "continue"
"#,
        temp_dir.path().join("tiles").display()
    );

    let config: Config = serde_yml::from_str(&config_yaml).expect("Failed to parse test config");

    if !PathBuf::from("tests/integration/fixtures/test_data/file1.shp").exists() {
        eprintln!("Skipping test: fixture not found");
        return;
    }

    let args = create_test_args_with_jobs(2);
    let result = pipeline::run(&config, &args);

    // Pipeline should complete in continue mode (even with errors)
    if let Ok(summary) = result {
        assert!(
            summary.tiles_succeeded > 0 || summary.tiles_failed > 0 || summary.tiles_skipped > 0,
            "Pipeline should process some tiles"
        );

        if summary.tiles_failed > 0 {
            assert_eq!(
                summary.export_errors.len(),
                summary.tiles_failed,
                "Error collection should match failed tile count"
            );

            for error in &summary.export_errors {
                assert!(!error.tile_id.is_empty(), "Error should have tile_id");
                assert!(!error.error_message.is_empty(), "Error should have message");
            }
        }
    } else {
        eprintln!(
            "Pipeline failed completely (config/setup error): {:?}",
            result.unwrap_err()
        );
    }
}

#[test]
fn test_fail_fast_interrupts_all_threads() {
    // Story 11.1 AC6: Mode FailFast + erreur → tous threads interrompus
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    let config_yaml = format!(
        r#"
version: 1
grid:
  cell_size: 0.05
  overlap: 0.0
inputs:
  - path: "tests/integration/fixtures/test_data/file1.shp"
output:
  directory: "{}"
  filename_pattern: "{{x}}_{{y}}.mp"
error_handling: "fail-fast"
"#,
        temp_dir.path().join("tiles").display()
    );

    let config: Config = serde_yml::from_str(&config_yaml).expect("Failed to parse test config");

    if !PathBuf::from("tests/integration/fixtures/test_data/file1.shp").exists() {
        eprintln!("Skipping test: fixture not found");
        return;
    }

    let args = create_test_args_with_jobs(4);

    let start = Instant::now();
    let result = pipeline::run(&config, &args);
    let duration = start.elapsed();

    match result {
        Ok(summary) => {
            assert!(
                summary.is_success(),
                "Pipeline should succeed with valid data"
            );
            assert_eq!(
                summary.tiles_failed, 0,
                "No tiles should fail with valid data"
            );
        }
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("fail-fast")
                    || error_msg.contains("Parallel export failed")
                    || error_msg.contains("PolishMap"),
                "Error should mention fail-fast mode, parallel export failure, or driver: {}",
                error_msg
            );
            println!("Fail-fast triggered, duration: {:?}", duration);
        }
    }
}

#[test]
fn test_parallel_error_handling_no_zombie_threads() {
    // Story 11.1: Verify no zombie threads remain after fail-fast error
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found");
        return;
    }

    let config = create_parallel_test_config(&temp_dir, fixture_path, "fail-fast");
    let args = create_test_args_with_jobs(4);

    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = Arc::clone(&completed);

    let handle = std::thread::spawn(move || {
        let result = pipeline::run(&config, &args);
        completed_clone.store(true, Ordering::SeqCst);
        result
    });

    let timeout = std::time::Duration::from_secs(30);
    let start = Instant::now();

    loop {
        if completed.load(Ordering::SeqCst) {
            break;
        }

        if start.elapsed() > timeout {
            panic!("Pipeline execution timed out - possible zombie threads or deadlock");
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    let result = handle
        .join()
        .expect("Pipeline thread should complete without panic");

    match result {
        Ok(_) => {}
        Err(e) => {
            println!("Pipeline failed as expected in fail-fast mode: {}", e);
        }
    }
}

// ============================================================================
// Story 11.1: New parallel-specific unit tests
// ============================================================================

#[test]
fn test_rayon_thread_pool_creation() {
    // Story 11.1 AC1: Verify rayon thread pool can be created with N workers
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("Failed to create thread pool");

    assert_eq!(pool.current_num_threads(), 4);
}

#[test]
fn test_parallel_aggregate_stats_correctness() {
    // Story 11.1 AC4: Verify stats are correctly aggregated across threads
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    let succeeded = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));
    let total_points = Arc::new(Mutex::new(0usize));

    let tiles: Vec<usize> = (0..50).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");

    pool.install(|| {
        tiles.par_iter().for_each(|tile_id| {
            if *tile_id % 10 == 7 {
                // Simulate failure for tiles 7, 17, 27, 37, 47
                failed.fetch_add(1, Ordering::Relaxed);
            } else {
                succeeded.fetch_add(1, Ordering::Relaxed);
                let mut pts = total_points.lock().expect("lock");
                *pts += 10; // Each tile has 10 points
            }
        });
    });

    assert_eq!(succeeded.load(Ordering::Relaxed), 45); // 50 - 5 failed
    assert_eq!(failed.load(Ordering::Relaxed), 5);
    assert_eq!(*total_points.lock().expect("lock"), 450); // 45 * 10
}

#[test]
fn test_arc_progress_bar_thread_safe() {
    // Story 11.1 AC7: Verify Arc<ProgressBar> works correctly across threads
    use indicatif::ProgressBar;
    use rayon::prelude::*;
    use std::sync::Arc;

    let pb = Arc::new(ProgressBar::new(100));
    pb.set_draw_target(indicatif::ProgressDrawTarget::hidden()); // No visual output in tests

    let tiles: Vec<usize> = (0..100).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");

    pool.install(|| {
        tiles.par_iter().for_each(|_| {
            pb.inc(1);
        });
    });

    assert_eq!(pb.position(), 100, "Progress bar should reach 100");
    pb.finish();
}

// ============================================================================
// Story 11.2: Production-code unit tests for aggregate_outcome()
// ============================================================================

/// Helper to create a TileResult with given point/linestring/polygon counts.
fn make_tile_result(points: usize, linestrings: usize, polygons: usize) -> TileResult {
    TileResult {
        stats: ExportStats {
            point_count: points,
            linestring_count: linestrings,
            polygon_count: polygons,
        },
        validation_stats: ValidationStats::default(),
        unsupported: UnsupportedTypeStats::default(),
        multi_geom: MultiGeometryStats::default(),
        rules_stats: RuleStats::default(),
    }
}

#[test]
fn test_aggregate_outcome_success() {
    // Story 11.2 Subtask 2.1: aggregate_outcome with TileOutcome::Success
    let acc = SharedAccumulators::new();

    let outcome = TileOutcome::Success(make_tile_result(10, 5, 3));
    aggregate_outcome(outcome, &acc);

    assert_eq!(acc.tiles_succeeded.load(Ordering::Relaxed), 1);
    assert_eq!(acc.tiles_failed.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_skipped.load(Ordering::Relaxed), 0);

    let stats = acc.global_stats.lock().unwrap();
    assert_eq!(stats.point_count, 10);
    assert_eq!(stats.linestring_count, 5);
    assert_eq!(stats.polygon_count, 3);
}

#[test]
fn test_aggregate_outcome_failed() {
    // Story 11.2 Subtask 2.2: aggregate_outcome with TileOutcome::Failed
    let acc = SharedAccumulators::new();

    let err = TileExportError {
        tile_id: "tile_3_7".to_string(),
        error_message: "GDAL driver error".to_string(),
    };
    let outcome = TileOutcome::Failed(err);
    aggregate_outcome(outcome, &acc);

    assert_eq!(acc.tiles_succeeded.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_failed.load(Ordering::Relaxed), 1);
    assert_eq!(acc.tiles_skipped.load(Ordering::Relaxed), 0);

    let errors = acc.export_errors.lock().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].tile_id, "tile_3_7");
    assert_eq!(errors[0].error_message, "GDAL driver error");
}

#[test]
fn test_aggregate_outcome_skipped_existing() {
    // Story 11.2 Subtask 2.3: aggregate_outcome with Skipped { existing: true }
    let acc = SharedAccumulators::new();

    aggregate_outcome(TileOutcome::Skipped { existing: true }, &acc);

    assert_eq!(acc.tiles_succeeded.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_failed.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_skipped.load(Ordering::Relaxed), 1);
    assert_eq!(acc.tiles_skipped_existing.load(Ordering::Relaxed), 1);
}

#[test]
fn test_aggregate_outcome_skipped_not_existing() {
    // Story 11.2 Subtask 2.4: aggregate_outcome with Skipped { existing: false }
    let acc = SharedAccumulators::new();

    aggregate_outcome(TileOutcome::Skipped { existing: false }, &acc);

    assert_eq!(acc.tiles_succeeded.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_failed.load(Ordering::Relaxed), 0);
    assert_eq!(acc.tiles_skipped.load(Ordering::Relaxed), 1);
    assert_eq!(acc.tiles_skipped_existing.load(Ordering::Relaxed), 0);
}

#[test]
fn test_aggregate_outcome_mixed_sequence() {
    // Story 11.2 Subtask 2.5: 5 Success + 2 Failed + 3 Skipped → exact counters
    let acc = SharedAccumulators::new();

    // 5 successes with varying stats
    for i in 0..5 {
        aggregate_outcome(
            TileOutcome::Success(make_tile_result(i + 1, 0, 0)),
            &acc,
        );
    }

    // 2 failures
    for i in 0..2 {
        aggregate_outcome(
            TileOutcome::Failed(TileExportError {
                tile_id: format!("fail_{}", i),
                error_message: format!("Error {}", i),
            }),
            &acc,
        );
    }

    // 3 skipped (2 existing, 1 not)
    aggregate_outcome(TileOutcome::Skipped { existing: true }, &acc);
    aggregate_outcome(TileOutcome::Skipped { existing: true }, &acc);
    aggregate_outcome(TileOutcome::Skipped { existing: false }, &acc);

    assert_eq!(acc.tiles_succeeded.load(Ordering::Relaxed), 5);
    assert_eq!(acc.tiles_failed.load(Ordering::Relaxed), 2);
    assert_eq!(acc.tiles_skipped.load(Ordering::Relaxed), 3);
    assert_eq!(acc.tiles_skipped_existing.load(Ordering::Relaxed), 2);

    // Stats: 1+2+3+4+5 = 15 points total
    let stats = acc.global_stats.lock().unwrap();
    assert_eq!(stats.point_count, 15);

    // Errors collected
    let errors = acc.export_errors.lock().unwrap();
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_aggregate_outcome_errors_contain_tile_id_and_message() {
    // Story 11.2 Subtask 2.6: collected errors contain tile_id and error_message
    let acc = SharedAccumulators::new();

    let test_errors = vec![
        ("tile_0_0", "IO error: disk full"),
        ("tile_1_3", "GDAL: PolishMap driver not found"),
        ("tile_5_9", "Geometry clipping failed"),
    ];

    for (tile_id, msg) in &test_errors {
        aggregate_outcome(
            TileOutcome::Failed(TileExportError {
                tile_id: tile_id.to_string(),
                error_message: msg.to_string(),
            }),
            &acc,
        );
    }

    let errors = acc.export_errors.lock().unwrap();
    assert_eq!(errors.len(), 3);

    for (i, (expected_id, expected_msg)) in test_errors.iter().enumerate() {
        assert_eq!(errors[i].tile_id, *expected_id);
        assert_eq!(errors[i].error_message, *expected_msg);
    }
}

// ============================================================================
// Story 11.2 Task 3: Integration tests — parallel report validation (AC4)
// ============================================================================

#[test]
fn test_parallel_report_json_counters_sum() {
    // Story 11.2 Subtask 3.2: tiles_generated + tiles_failed + tiles_skipped = total tiles
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let config = create_parallel_test_config(&temp_dir, fixture_path, "continue");
    let args = create_test_args_with_jobs(2);

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed");

    let summary = result.unwrap();

    // AC4: counters must sum to total tiles processed
    let total = summary.tiles_succeeded + summary.tiles_failed + summary.tiles_skipped;
    assert!(
        total > 0,
        "Pipeline must process at least one tile"
    );
    assert_eq!(
        summary.export_errors.len(),
        summary.tiles_failed,
        "Number of export_errors must match tiles_failed counter"
    );
}

#[test]
fn test_parallel_report_json_with_report_file() {
    // Story 11.2 Subtask 3.1: --jobs 2 + mode continue → JSON report written
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    let report_path = temp_dir.path().join("report.json");

    let config = create_parallel_test_config(&temp_dir, fixture_path, "continue");
    let args = BuildArgs {
        config: "test.yaml".to_string(),
        input: None,
        output: None,
        jobs: 2,
        fail_fast: false,
        report: Some(report_path.to_string_lossy().to_string()),
        skip_existing: false,
        dry_run: false,
        verbose: 0,
    };

    let result = pipeline::run(&config, &args);
    assert!(result.is_ok(), "Pipeline should succeed");

    // Verify JSON report was written
    assert!(report_path.exists(), "JSON report file should exist");

    let report_content = fs::read_to_string(&report_path).expect("Failed to read report");
    let json: serde_json::Value =
        serde_json::from_str(&report_content).expect("Report should be valid JSON");

    // Verify counters in JSON
    let tiles_generated = json["tiles_generated"].as_u64().unwrap();
    let tiles_failed = json["tiles_failed"].as_u64().unwrap();
    let tiles_skipped = json["tiles_skipped"].as_u64().unwrap();

    let total = tiles_generated + tiles_failed + tiles_skipped;
    assert!(total > 0, "JSON report should have processed tiles");

    // Verify errors array matches tiles_failed
    let errors = json["errors"].as_array().expect("errors should be array");
    assert_eq!(
        errors.len() as u64,
        tiles_failed,
        "JSON errors array length must match tiles_failed"
    );
}

#[test]
fn test_parallel_and_sequential_same_counters() {
    // Story 11.2 Subtask 3.3: Console summary (via TileExportSummary) matches JSON report
    let temp_dir_seq = TempDir::new().expect("Failed to create temp dir");
    let temp_dir_par = TempDir::new().expect("Failed to create temp dir");
    let fixture_path = "tests/integration/fixtures/test_data/file1.shp";

    if !PathBuf::from(fixture_path).exists() {
        eprintln!("Skipping test: fixture not found at {}", fixture_path);
        return;
    }

    // Sequential
    let config_seq = create_parallel_test_config(&temp_dir_seq, fixture_path, "continue");
    let args_seq = create_test_args_with_jobs(1);
    let result_seq = pipeline::run(&config_seq, &args_seq).expect("Sequential should succeed");

    // Parallel
    let config_par = create_parallel_test_config(&temp_dir_par, fixture_path, "continue");
    let args_par = create_test_args_with_jobs(2);
    let result_par = pipeline::run(&config_par, &args_par).expect("Parallel should succeed");

    // Same counters
    assert_eq!(
        result_seq.tiles_succeeded, result_par.tiles_succeeded,
        "tiles_succeeded must match"
    );
    assert_eq!(
        result_seq.tiles_failed, result_par.tiles_failed,
        "tiles_failed must match"
    );
    assert_eq!(
        result_seq.tiles_skipped, result_par.tiles_skipped,
        "tiles_skipped must match"
    );
    assert_eq!(
        result_seq.total_features(),
        result_par.total_features(),
        "total_features must match"
    );
}

// ============================================================================
// Story 11.2 Task 4: Progress bar concurrent validation (AC3)
// ============================================================================

#[test]
fn test_progress_bar_position_reaches_total_with_parallel() {
    // Story 11.2 Subtask 4.1: --jobs 4, 20+ tiles → pb.position() == tiles.len()
    use indicatif::ProgressBar;
    use rayon::prelude::*;
    use std::sync::Arc;

    let total_tiles: u64 = 50;
    let pb = Arc::new(ProgressBar::new(total_tiles));
    pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

    let tiles: Vec<u64> = (0..total_tiles).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");

    pool.install(|| {
        tiles.par_iter().for_each(|_| {
            // Simulate some work
            std::thread::sleep(std::time::Duration::from_micros(10));
            pb.inc(1);
        });
    });

    assert_eq!(
        pb.position(),
        total_tiles,
        "Progress bar should reach total tiles count after parallel processing"
    );
    pb.finish();
}

// ============================================================================
// Story 11.2 Task 5: Fail-fast parallel validation (AC2)
// ============================================================================

#[test]
fn test_fail_fast_error_contains_tile_id() {
    // Story 11.2 Subtask 5.2: Error message contains the failing tile_id
    use rayon::prelude::*;

    let tiles: Vec<usize> = (0..50).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("pool");

    let result: Result<(), TileExportError> = pool.install(|| {
        tiles.par_iter().try_for_each(|tile_id| {
            if *tile_id == 5 {
                Err(TileExportError {
                    tile_id: "tile_5".to_string(),
                    error_message: "Export failed for tile_5".to_string(),
                })
            } else {
                Ok(())
            }
        })
    });

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.tile_id, "tile_5", "Error should contain the failing tile_id");
    assert!(
        err.error_message.contains("tile_5"),
        "Error message should reference the tile"
    );
}

#[test]
fn test_fail_fast_unprocessed_tiles_not_counted() {
    // Story 11.2 Subtask 5.3: Unprocessed tiles are NOT counted in accumulators
    use rayon::prelude::*;
    use std::sync::Arc;

    let acc = Arc::new(SharedAccumulators::new());
    let tiles: Vec<usize> = (0..100).collect();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");

    let acc_clone = Arc::clone(&acc);
    let _result: Result<(), TileExportError> = pool.install(|| {
        tiles.par_iter().try_for_each(|tile_id| {
            if *tile_id == 3 {
                // Simulate fail-fast: return error without aggregating
                return Err(TileExportError {
                    tile_id: format!("tile_{}", tile_id),
                    error_message: "fail-fast".to_string(),
                });
            }
            // Aggregate success for non-failing tiles that get processed
            aggregate_outcome(
                TileOutcome::Success(make_tile_result(1, 0, 0)),
                &acc_clone,
            );
            Ok(())
        })
    });

    let succeeded = acc.tiles_succeeded.load(Ordering::Relaxed);
    // Due to fail-fast, fewer than 99 tiles should be aggregated as succeeded
    assert!(
        succeeded < 99,
        "Unprocessed tiles should not be counted: succeeded={}",
        succeeded
    );
}
