//! JSON build report for imgforge-cli.
//!
//! Provides [`BuildReport`] (serialisable to JSON) and [`write_json_report`]
//! to persist it to disk after a successful `build` command.

use std::path::Path;

use serde::Serialize;

/// Overall build outcome.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ReportStatus {
    Success,
    Failure,
}

/// Feature counts per geometry type across all compiled tiles.
#[derive(Debug, Serialize, Clone, Default)]
pub struct FeaturesByType {
    pub poi: usize,
    pub polyline: usize,
    pub polygon: usize,
}

/// A per-tile compilation error captured for the report.
#[derive(Debug, Serialize, Clone)]
pub struct TileError {
    pub tile: String,
    pub error: String,
}

/// Full build report written by `--report`.
#[derive(Debug, Serialize, Clone)]
pub struct BuildReport {
    pub status: ReportStatus,
    pub tiles_compiled: usize,
    pub tiles_failed: usize,
    pub features_by_type: FeaturesByType,
    pub routing_nodes: usize,
    pub routing_arcs: usize,
    pub img_size_bytes: u64,
    pub duration_seconds: f64,
    pub errors: Vec<TileError>,
}

/// Serialise `report` as pretty-printed JSON and write it to `path`.
pub fn write_json_report(report: &BuildReport, path: &Path) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_status_serialization() {
        let success = serde_json::to_string(&ReportStatus::Success).unwrap();
        let failure = serde_json::to_string(&ReportStatus::Failure).unwrap();
        assert_eq!(success, r#""success""#);
        assert_eq!(failure, r#""failure""#);
    }

    #[test]
    fn test_report_success_json_schema() {
        let report = BuildReport {
            status: ReportStatus::Success,
            tiles_compiled: 100,
            tiles_failed: 0,
            features_by_type: FeaturesByType {
                poi: 1000,
                polyline: 5000,
                polygon: 500,
            },
            routing_nodes: 12345,
            routing_arcs: 23456,
            img_size_bytes: 9_876_543,
            duration_seconds: 12.5,
            errors: vec![],
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["status"], "success");
        assert_eq!(value["tiles_compiled"], 100);
        assert_eq!(value["tiles_failed"], 0);
        assert_eq!(value["features_by_type"]["poi"], 1000);
        assert_eq!(value["features_by_type"]["polyline"], 5000);
        assert_eq!(value["features_by_type"]["polygon"], 500);
        assert_eq!(value["routing_nodes"], 12345);
        assert_eq!(value["routing_arcs"], 23456);
        assert_eq!(value["img_size_bytes"], 9_876_543);
        assert_eq!(value["duration_seconds"], 12.5);
        assert!(value["errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_write_json_report_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("report.json");

        let report = BuildReport {
            status: ReportStatus::Failure,
            tiles_compiled: 5,
            tiles_failed: 2,
            features_by_type: FeaturesByType::default(),
            routing_nodes: 0,
            routing_arcs: 0,
            img_size_bytes: 1024,
            duration_seconds: 1.0,
            errors: vec![
                TileError {
                    tile: "01001234".to_string(),
                    error: "Invalid map ID ''".to_string(),
                },
            ],
        };

        write_json_report(&report, &path).unwrap();
        assert!(path.exists(), "report file must be created on disk");

        let content = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value["status"], "failure");
        assert_eq!(value["tiles_failed"], 2);
        assert_eq!(value["errors"][0]["tile"], "01001234");
    }

    #[test]
    fn test_write_json_report_fails_if_parent_dir_missing() {
        let path = std::path::Path::new("/nonexistent/directory/report.json");
        let report = BuildReport {
            status: ReportStatus::Success,
            tiles_compiled: 1,
            tiles_failed: 0,
            features_by_type: FeaturesByType::default(),
            routing_nodes: 0,
            routing_arcs: 0,
            img_size_bytes: 0,
            duration_seconds: 0.0,
            errors: vec![],
        };
        let result = write_json_report(&report, path);
        assert!(result.is_err(), "write to non-existent parent dir must fail");
    }
}
