// Build report JSON output

use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct BuildReport {
    pub tiles_compiled: usize,
    pub total_points: usize,
    pub total_polylines: usize,
    pub total_polygons: usize,
    pub errors: Vec<String>,
    pub duration_ms: u64,
    pub output_file: String,
    pub output_size_bytes: u64,
}

impl BuildReport {
    pub fn new() -> Self {
        Self {
            tiles_compiled: 0,
            total_points: 0,
            total_polylines: 0,
            total_polygons: 0,
            errors: Vec::new(),
            duration_ms: 0,
            output_file: String::new(),
            output_size_bytes: 0,
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        self.duration_ms = duration.as_millis() as u64;
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_json() {
        let mut report = BuildReport::new();
        report.tiles_compiled = 1;
        report.total_points = 10;
        report.output_file = "test.img".to_string();
        let json = report.to_json();
        assert!(json.contains("\"tiles_compiled\": 1"));
        assert!(json.contains("\"total_points\": 10"));
    }
}
