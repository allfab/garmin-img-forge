//! Configuration file parsing and structures.

use serde::Deserialize;
use std::path::Path;

#[allow(dead_code)] // Stub - will be used in Story 5.2+
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: u32,
    pub grid: GridConfig,
    pub inputs: Vec<InputSource>,
    pub output: OutputConfig,
    #[serde(default)]
    pub filters: Option<FilterConfig>,
    #[serde(default = "default_error_handling")]
    pub error_handling: String,
}

fn default_version() -> u32 {
    1
}

fn default_error_handling() -> String {
    "continue".to_string()
}

#[allow(dead_code)] // Stub - will be used in Story 5.2+
#[derive(Debug, Deserialize)]
pub struct GridConfig {
    pub cell_size: f64,
    #[serde(default)]
    pub overlap: f64,
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
}

#[allow(dead_code)] // Stub - will be used in Story 5.2+
#[derive(Debug, Deserialize)]
pub struct InputSource {
    pub path: Option<String>,
    pub connection: Option<String>,
    pub layer: Option<String>,
    pub layers: Option<Vec<String>>,
}

#[allow(dead_code)] // Stub - will be used in Story 5.2+
#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub directory: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
}

fn default_filename_pattern() -> String {
    "{x}_{y}.mp".to_string()
}

#[allow(dead_code)] // Stub - will be used in Story 5.2+
#[derive(Debug, Deserialize)]
pub struct FilterConfig {
    /// Bounding box filter: [min_lon, min_lat, max_lon, max_lat]
    /// If FilterConfig exists, bbox is required
    pub bbox: [f64; 4],
}

/// Load and parse configuration from YAML file.
/// Stub implementation - will be fully implemented in Story 5.2.
#[allow(dead_code)] // Stub - will be implemented in Story 5.2
pub fn load_config<P: AsRef<Path>>(_path: P) -> anyhow::Result<Config> {
    // TODO: Story 5.2 - Implement actual YAML parsing and validation
    todo!("Configuration loading will be implemented in Story 5.2")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.grid.cell_size, 0.15);
        assert_eq!(config.grid.overlap, 0.0);
        assert_eq!(config.output.filename_pattern, "{x}_{y}.mp");
        assert_eq!(config.error_handling, "continue");
    }

    #[test]
    fn test_grid_config_with_origin() {
        let yaml = r#"
cell_size: 0.15
overlap: 0.01
origin: [0.0, 0.0]
"#;
        let grid: GridConfig = serde_yml::from_str(yaml).unwrap();
        assert_eq!(grid.cell_size, 0.15);
        assert_eq!(grid.overlap, 0.01);
        assert_eq!(grid.origin, Some([0.0, 0.0]));
    }

    #[test]
    fn test_input_source_path() {
        let yaml = r#"
path: "data/*.shp"
"#;
        let input: InputSource = serde_yml::from_str(yaml).unwrap();
        assert_eq!(input.path, Some("data/*.shp".to_string()));
        assert!(input.connection.is_none());
    }

    #[test]
    fn test_input_source_connection() {
        let yaml = r#"
connection: "PG:host=localhost"
layer: "roads"
"#;
        let input: InputSource = serde_yml::from_str(yaml).unwrap();
        assert_eq!(input.connection, Some("PG:host=localhost".to_string()));
        assert_eq!(input.layer, Some("roads".to_string()));
    }
}
