//! Execution report generation and JSON output.
//! Story 7.3: JSON report schema and file writer.

use anyhow::Context;
use serde::Serialize;
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
    /// Total execution duration in seconds (float for precision)
    pub duration_seconds: f64,
    /// Detailed error information for failed tiles
    pub errors: Vec<TileError>,
}

/// Error details for a single failed tile.
#[derive(Debug, Serialize, Clone)]
pub struct TileError {
    /// Tile identifier (e.g., "12_45")
    pub tile: String,
    /// Complete error message
    pub error: String,
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
    let json = serde_json::to_string_pretty(report)
        .context("Failed to serialize report to JSON")?;

    // Create and write file
    let mut file = File::create(path)
        .with_context(|| format!("Failed to create report file: {}", path))?;

    file.write_all(json.as_bytes())
        .with_context(|| format!("Failed to write report to file: {}", path))?;

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
            duration_seconds: 5.5,
            errors: vec![],
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
            duration_seconds: 3.0,
            errors: vec![],
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
}
