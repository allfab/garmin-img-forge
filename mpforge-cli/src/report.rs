//! Execution report generation and JSON output.

use serde::Serialize;

#[allow(dead_code)] // Stub - will be used in Story 7.3
#[derive(Debug, Serialize)]
pub struct ExecutionReport {
    pub success: bool,
    pub total_tiles: usize,
    pub failed_tiles: Vec<String>,
    pub duration_seconds: f64,
}

/// Builder for execution reports.
/// Stub implementation - will be fully implemented in Story 7.3.
#[allow(dead_code)] // Stub - will be implemented in Story 7.3
pub struct ReportBuilder {
    success: bool,
    total_tiles: usize,
    failed_tiles: Vec<String>,
}

#[allow(dead_code)] // Stub - will be implemented in Story 7.3
impl ReportBuilder {
    pub fn new() -> Self {
        Self {
            success: true,
            total_tiles: 0,
            failed_tiles: Vec::new(),
        }
    }

    pub fn add_success(&mut self) {
        self.total_tiles += 1;
    }

    pub fn add_failure(&mut self, tile_id: String) {
        self.success = false;
        self.total_tiles += 1;
        self.failed_tiles.push(tile_id);
    }

    pub fn build(self, duration_seconds: f64) -> ExecutionReport {
        ExecutionReport {
            success: self.success,
            total_tiles: self.total_tiles,
            failed_tiles: self.failed_tiles,
            duration_seconds,
        }
    }
}

impl Default for ReportBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_builder_success() {
        let mut builder = ReportBuilder::new();
        builder.add_success();
        builder.add_success();
        let report = builder.build(1.5);

        assert!(report.success);
        assert_eq!(report.total_tiles, 2);
        assert!(report.failed_tiles.is_empty());
        assert_eq!(report.duration_seconds, 1.5);
    }

    #[test]
    fn test_report_builder_with_failures() {
        let mut builder = ReportBuilder::new();
        builder.add_success();
        builder.add_failure("tile_0_0".to_string());
        let report = builder.build(2.0);

        assert!(!report.success);
        assert_eq!(report.total_tiles, 2);
        assert_eq!(report.failed_tiles.len(), 1);
        assert_eq!(report.failed_tiles[0], "tile_0_0");
    }

    #[test]
    fn test_report_builder_default() {
        let builder = ReportBuilder::default();
        let report = builder.build(0.0);

        assert!(report.success);
        assert_eq!(report.total_tiles, 0);
        assert!(report.failed_tiles.is_empty());
    }
}
