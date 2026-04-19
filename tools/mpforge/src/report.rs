//! Execution report generation and JSON output.
//! Story 7.3: JSON report schema and file writer.

use anyhow::Context;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

/// Report status enum for CI/CD integration.
/// Serializes to lowercase strings: "success" or "failure".
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ReportStatus {
    Success,
    Failure,
}

/// Execution report schema for JSON output.
/// Story 7.3 - Matches Epic 7 specification for CI/CD integration.
/// Story 6.6 - Added quality section for unsupported geometry types.
#[derive(Debug, Serialize)]
pub struct ExecutionReport {
    /// Overall execution status: "success" if no failures, "failure" otherwise
    pub status: ReportStatus,
    /// Number of tiles successfully exported
    pub tiles_generated: usize,
    /// Number of tiles that failed to export
    pub tiles_failed: usize,
    /// Number of tiles skipped (empty tiles)
    pub tiles_skipped: usize,
    /// Total features processed across all tiles
    pub features_processed: usize,
    /// Tech-spec #2 AC17 : nombre de features skipées parce qu'au moins un
    /// bucket additionnel (`Data<n>=`) a échoué à l'écriture (erreur FFI
    /// `OGR_F_SetGeomField` ≠ NONE ou WKT invalide). `0` en mode mono-Data.
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped_additional_geom: usize,
    /// Total execution duration in seconds (float for precision)
    pub duration_seconds: f64,
    /// Detailed error information for failed tiles
    pub errors: Vec<TileError>,
    /// Story 8.3: Whether this was a dry-run execution (no files written)
    #[serde(skip_serializing_if = "is_false")]
    pub dry_run: bool,
    /// Quality information including unsupported geometry types (Story 6.6)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<QualitySection>,
    /// Story 9.3 AC6: Rules engine statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_stats: Option<crate::rules::RuleStats>,
}

/// Helper for conditional serialization of bool fields.
/// Story 8.3: Skip serialization when value is false (default).
fn is_false(b: &bool) -> bool {
    !*b
}

/// Helper for conditional serialization of numeric fields.
/// Tech-spec #2 : omet `skipped_additional_geom` de la sortie JSON quand zéro.
fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Quality section of the execution report.
/// Story 6.6 - Reports unsupported geometry types filtered during reading.
/// Story 6.7 - Reports multi-geometries decomposed into simple geometries.
/// Code Review M3 Fix: Use BTreeMap for deterministic JSON key ordering.
#[derive(Debug, Serialize, Clone)]
pub struct QualitySection {
    /// Code Review M3 Fix: Skip empty unsupported_types to avoid spurious `"unsupported_types": {}` in JSON.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub unsupported_types: BTreeMap<String, UnsupportedTypeReport>,

    /// Story 6.7 - Subtask 5.1: Multi-geometries decomposed into simple geometries (type -> count).
    /// Subtask 5.3: Skip serialization if None (no multi-geometries decomposed).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multi_geometries_decomposed: Option<BTreeMap<String, usize>>,
}

/// Report entry for a single unsupported geometry type.
/// Code Review M1 Fix: Added total_sources to track all sources even when Vec is truncated.
#[derive(Debug, Serialize, Clone)]
pub struct UnsupportedTypeReport {
    pub count: usize,
    pub sources: Vec<String>,
    /// Total number of distinct sources (may exceed sources.len() if truncated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_sources: Option<usize>,
}

/// Error details for a single failed tile.
#[derive(Debug, Serialize, Clone)]
pub struct TileError {
    /// Tile identifier (e.g., "12_45")
    pub tile: String,
    /// Complete error message
    pub error: String,
}

// ---------------------------------------------------------------------------
// Validation report (mpforge validate)
// ---------------------------------------------------------------------------

/// Validation status: valid or invalid.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    Valid,
    Invalid,
}

/// Status of a single validation check.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Skipped,
}

/// A single validation check result.
#[derive(Debug, Serialize, Clone)]
pub struct ValidationCheck {
    pub name: String,
    pub status: CheckStatus,
    pub details: String,
}

/// Summary of the validated configuration.
#[derive(Debug, Serialize, Clone)]
pub struct ValidationSummary {
    pub grid_cell_size: f64,
    pub grid_overlap: f64,
    pub input_sources: usize,
    pub output_directory: String,
    pub filename_pattern: String,
}

/// Validation report schema for JSON output (`mpforge validate --report`).
#[derive(Debug, Serialize, Clone)]
pub struct ValidationReport {
    pub status: ValidationStatus,
    pub config_file: String,
    pub checks: Vec<ValidationCheck>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ValidationSummary>,
}

impl ValidationReport {
    /// Returns true if the overall status is valid.
    pub fn is_valid(&self) -> bool {
        self.status == ValidationStatus::Valid
    }

    /// Count passed checks.
    pub fn passed_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count()
    }

    /// Count failed checks.
    pub fn failed_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Fail)
            .count()
    }
}

/// Write validation report to JSON file with pretty formatting.
pub fn write_validation_report(report: &ValidationReport, path: &str) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(report)
        .context("Failed to serialize validation report to JSON")?;

    let mut file = File::create(path)
        .with_context(|| format!("Failed to create validation report file: {}", path))?;

    file.write_all(json.as_bytes())
        .with_context(|| format!("Failed to write validation report to file: {}", path))?;
    file.write_all(b"\n")
        .with_context(|| format!("Failed to write trailing newline to file: {}", path))?;

    Ok(())
}

/// Write execution report to JSON file with pretty formatting.
/// Story 7.3 - Task 2: JSON report writer.
///
/// # Arguments
/// * `report` - The execution report to serialize
/// * `path` - Output file path for JSON report
///
/// # Errors
/// Returns error if file cannot be created or JSON serialization fails.
pub fn write_json_report(report: &ExecutionReport, path: &str) -> anyhow::Result<()> {
    // Serialize with pretty print for human readability
    let json =
        serde_json::to_string_pretty(report).context("Failed to serialize report to JSON")?;

    // Create and write file
    let mut file =
        File::create(path).with_context(|| format!("Failed to create report file: {}", path))?;

    file.write_all(json.as_bytes())
        .with_context(|| format!("Failed to write report to file: {}", path))?;
    file.write_all(b"\n")
        .with_context(|| format!("Failed to write trailing newline to file: {}", path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Story 7.3 - Subtask 1.5: Tests for new ExecutionReport schema

    #[test]
    fn test_report_serialization_success() {
        let report = ExecutionReport {
            status: ReportStatus::Success,
            tiles_generated: 10,
            tiles_failed: 0,
            tiles_skipped: 2,
            features_processed: 1000,
            skipped_additional_geom: 0,
            duration_seconds: 5.5,
            errors: vec![],
            dry_run: false,
            quality: None,
            rules_stats: None,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"tiles_generated\":10"));
        assert!(json.contains("\"tiles_failed\":0"));
        assert!(json.contains("\"tiles_skipped\":2"));
        assert!(json.contains("\"features_processed\":1000"));
        assert!(json.contains("\"duration_seconds\":5.5"));
    }

    #[test]
    fn test_report_serialization_failure_with_errors() {
        let report = ExecutionReport {
            status: ReportStatus::Failure,
            tiles_generated: 8,
            tiles_failed: 2,
            tiles_skipped: 0,
            features_processed: 800,
            skipped_additional_geom: 0,
            duration_seconds: 10.2,
            errors: vec![
                TileError {
                    tile: "12_45".to_string(),
                    error: "GDAL error: Invalid geometry".to_string(),
                },
                TileError {
                    tile: "15_60".to_string(),
                    error: "I/O error: Permission denied".to_string(),
                },
            ],
            dry_run: false,
            quality: None,
            rules_stats: None,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"status\":\"failure\""));
        assert!(json.contains("\"tiles_generated\":8"));
        assert!(json.contains("\"tiles_failed\":2"));
        assert!(json.contains("\"12_45\""));
        assert!(json.contains("\"GDAL error: Invalid geometry\""));
        assert!(json.contains("\"15_60\""));
    }

    #[test]
    fn test_report_status_enum_serialization() {
        // Verify enum serializes to lowercase strings
        let success_json = serde_json::to_string(&ReportStatus::Success).unwrap();
        assert_eq!(success_json, "\"success\"");

        let failure_json = serde_json::to_string(&ReportStatus::Failure).unwrap();
        assert_eq!(failure_json, "\"failure\"");
    }

    #[test]
    fn test_tile_error_serialization() {
        let tile_error = TileError {
            tile: "0_0".to_string(),
            error: "Test error".to_string(),
        };

        let json = serde_json::to_string(&tile_error).unwrap();
        assert!(json.contains("\"tile\":\"0_0\""));
        assert!(json.contains("\"error\":\"Test error\""));
    }

    #[test]
    fn test_write_json_report() {
        use tempfile::NamedTempFile;

        let report = ExecutionReport {
            status: ReportStatus::Success,
            tiles_generated: 5,
            tiles_failed: 0,
            tiles_skipped: 1,
            features_processed: 500,
            skipped_additional_geom: 0,
            duration_seconds: 3.0,
            errors: vec![],
            dry_run: false,
            quality: None,
            rules_stats: None,
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        // Write JSON report
        write_json_report(&report, path).unwrap();

        // Read back and verify
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("\"status\": \"success\""));
        assert!(content.contains("\"tiles_generated\": 5"));
        assert!(content.contains("\"features_processed\": 500"));

        // Verify pretty print (has indentation)
        assert!(content.contains("  "));
    }

    // -- ValidationReport tests --

    #[test]
    fn test_validation_report_valid() {
        let report = ValidationReport {
            status: ValidationStatus::Valid,
            config_file: "config.yaml".to_string(),
            checks: vec![
                ValidationCheck {
                    name: "yaml_syntax".to_string(),
                    status: CheckStatus::Pass,
                    details: "Parsed successfully".to_string(),
                },
                ValidationCheck {
                    name: "semantic_validation".to_string(),
                    status: CheckStatus::Pass,
                    details: "All rules passed".to_string(),
                },
            ],
            errors: vec![],
            warnings: vec![],
            summary: Some(ValidationSummary {
                grid_cell_size: 0.15,
                grid_overlap: 0.01,
                input_sources: 12,
                output_directory: "tiles/".to_string(),
                filename_pattern: "{col}_{row}.mp".to_string(),
            }),
        };

        assert!(report.is_valid());
        assert_eq!(report.passed_count(), 2);
        assert_eq!(report.failed_count(), 0);

        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"status\": \"valid\""));
        assert!(json.contains("\"yaml_syntax\""));
        assert!(json.contains("\"grid_cell_size\": 0.15"));
    }

    #[test]
    fn test_validation_report_invalid() {
        let report = ValidationReport {
            status: ValidationStatus::Invalid,
            config_file: "broken.yaml".to_string(),
            checks: vec![
                ValidationCheck {
                    name: "yaml_syntax".to_string(),
                    status: CheckStatus::Pass,
                    details: "Parsed successfully".to_string(),
                },
                ValidationCheck {
                    name: "semantic_validation".to_string(),
                    status: CheckStatus::Fail,
                    details: "grid.cell_size must be positive".to_string(),
                },
            ],
            errors: vec!["grid.cell_size must be positive, got: -1".to_string()],
            warnings: vec![],
            summary: None,
        };

        assert!(!report.is_valid());
        assert_eq!(report.passed_count(), 1);
        assert_eq!(report.failed_count(), 1);

        let json = serde_json::to_string_pretty(&report).unwrap();
        assert!(json.contains("\"status\": \"invalid\""));
        assert!(json.contains("\"fail\""));
    }

    #[test]
    fn test_write_validation_report() {
        use tempfile::NamedTempFile;

        let report = ValidationReport {
            status: ValidationStatus::Valid,
            config_file: "test.yaml".to_string(),
            checks: vec![ValidationCheck {
                name: "yaml_syntax".to_string(),
                status: CheckStatus::Pass,
                details: "OK".to_string(),
            }],
            errors: vec![],
            warnings: vec![],
            summary: None,
        };

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        write_validation_report(&report, path).unwrap();

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("\"status\": \"valid\""));
        assert!(content.contains("\"yaml_syntax\""));
    }
}
