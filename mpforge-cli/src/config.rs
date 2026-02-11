//! Configuration file parsing and structures.

use anyhow::Context;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

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

/// Grid configuration for spatial tiling.
/// Story 6.2: Clone trait required for TileProcessor ownership in pipeline orchestration.
#[derive(Debug, Clone, Deserialize)]
pub struct GridConfig {
    pub cell_size: f64,
    #[serde(default)]
    pub overlap: f64,
    #[serde(default)]
    pub origin: Option<[f64; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct InputSource {
    pub path: Option<String>,
    pub connection: Option<String>,
    pub layer: Option<String>,
    pub layers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub directory: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
}

fn default_filename_pattern() -> String {
    "{x}_{y}.mp".to_string()
}

#[derive(Debug, Deserialize)]
pub struct FilterConfig {
    /// Bounding box filter: [min_lon, min_lat, max_lon, max_lat]
    /// If FilterConfig exists, bbox is required
    pub bbox: [f64; 4],
}

/// Source type enumeration for InputSource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    File,
    PostGIS,
}

/// Error handling mode for geometry clipping operations.
/// Story 6.3: Controls behavior when encountering invalid geometries during clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum ErrorMode {
    /// Continue processing, skip invalid features (production-friendly default)
    #[default]
    Continue,
    /// Stop pipeline on first error (useful for debugging)
    FailFast,
}

impl std::str::FromStr for ErrorMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "continue" => Ok(Self::Continue),
            "fail-fast" => Ok(Self::FailFast),
            _ => anyhow::bail!(
                "Invalid error_handling mode '{}', expected 'continue' or 'fail-fast'",
                s
            ),
        }
    }
}

impl InputSource {
    /// Detect source type based on connection string pattern.
    pub fn source_type(&self) -> SourceType {
        if let Some(conn) = &self.connection {
            if conn.starts_with("PG:") || conn.contains("host=") {
                return SourceType::PostGIS;
            }
        }
        SourceType::File
    }
}

impl Config {
    /// Validate configuration semantic rules.
    pub fn validate(&self) -> anyhow::Result<()> {
        // Grid validation
        if self.grid.cell_size <= 0.0 {
            anyhow::bail!(
                "grid.cell_size must be positive, got: {}",
                self.grid.cell_size
            );
        }

        if self.grid.overlap < 0.0 {
            anyhow::bail!(
                "grid.overlap cannot be negative, got: {}",
                self.grid.overlap
            );
        }

        // Inputs validation
        if self.inputs.is_empty() {
            anyhow::bail!("At least one input source is required");
        }

        for (i, input) in self.inputs.iter().enumerate() {
            let has_path = input.path.is_some();
            let has_connection = input.connection.is_some();

            if has_path == has_connection {
                anyhow::bail!(
                    "InputSource #{} must have either 'path' or 'connection', not both or none",
                    i
                );
            }
        }

        // Error handling validation
        if self.error_handling != "continue" && self.error_handling != "fail-fast" {
            anyhow::bail!(
                "error_handling must be 'continue' or 'fail-fast', got: '{}'",
                self.error_handling
            );
        }

        // Filters validation (if present)
        if let Some(filters) = &self.filters {
            let bbox = filters.bbox;
            if bbox[0] >= bbox[2] {
                anyhow::bail!(
                    "bbox min_lon must be < max_lon, got: [{}, {}]",
                    bbox[0],
                    bbox[2]
                );
            }
            if bbox[1] >= bbox[3] {
                anyhow::bail!(
                    "bbox min_lat must be < max_lat, got: [{}, {}]",
                    bbox[1],
                    bbox[3]
                );
            }
        }

        Ok(())
    }
}

/// Resolve wildcard patterns to actual file paths.
fn resolve_wildcard_paths(pattern: &str) -> anyhow::Result<Vec<PathBuf>> {
    let paths: Vec<PathBuf> = glob::glob(pattern)
        .with_context(|| format!("Invalid glob pattern: {}", pattern))?
        .filter_map(|entry| entry.ok())
        .collect();

    if paths.is_empty() {
        warn!(pattern, "No files matched wildcard pattern");
    } else {
        info!(pattern, count = paths.len(), "Resolved wildcard pattern");
    }

    Ok(paths)
}

/// Load and parse configuration from YAML file.
pub fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let path = path.as_ref();

    // I/O error context
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    // YAML parsing error context
    let mut config: Config = serde_yml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML config: {}", path.display()))?;

    // Validation error context
    config
        .validate()
        .with_context(|| format!("Config validation failed for: {}", path.display()))?;

    // Wildcard resolution for file inputs
    for input in &mut config.inputs {
        if let Some(pattern) = &input.path {
            if pattern.contains('*') || pattern.contains('?') {
                let resolved = resolve_wildcard_paths(pattern)?;
                debug!(pattern, resolved = ?resolved, "Wildcard expanded");
            }
        }

        // Log source type for each input
        let source_type = input.source_type();
        match source_type {
            SourceType::File => {
                if let Some(path) = &input.path {
                    info!(path, "Detected File input source");
                }
            }
            SourceType::PostGIS => {
                if let Some(conn) = &input.connection {
                    info!(connection = conn, "Detected PostGIS input source");
                }
            }
        }
    }

    Ok(config)
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

    // Tests for Config::validate()
    #[test]
    fn test_config_validate_positive_cell_size() {
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
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_negative_cell_size_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: -0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cell_size must be positive"));
    }

    #[test]
    fn test_config_validate_zero_cell_size_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.0
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cell_size must be positive"));
    }

    #[test]
    fn test_config_validate_non_negative_overlap() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
  overlap: 0.005
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_negative_overlap_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
  overlap: -0.01
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("overlap cannot be negative"));
    }

    #[test]
    fn test_config_validate_error_handling_values() {
        let yaml_continue = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "continue"
"#;
        let config: Config = serde_yml::from_str(yaml_continue).unwrap();
        assert!(config.validate().is_ok());

        let yaml_fail_fast = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "fail-fast"
"#;
        let config: Config = serde_yml::from_str(yaml_fail_fast).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_error_handling() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
error_handling: "invalid"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("error_handling must be"));
    }

    #[test]
    fn test_config_validate_at_least_one_input() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs: []
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one input source"));
    }

    #[test]
    fn test_input_source_mutual_exclusion() {
        // Valid: has path only
        let yaml_path = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml_path).unwrap();
        assert!(config.validate().is_ok());

        // Valid: has connection only
        let yaml_conn = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - connection: "PG:host=localhost"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml_conn).unwrap();
        assert!(config.validate().is_ok());

        // Invalid: has both (note: serde won't allow this in YAML, but we test struct validation)
        let mut config = Config {
            version: 1,
            grid: GridConfig {
                cell_size: 0.15,
                overlap: 0.0,
                origin: None,
            },
            inputs: vec![InputSource {
                path: Some("data.shp".to_string()),
                connection: Some("PG:host=localhost".to_string()),
                layer: None,
                layers: None,
            }],
            output: OutputConfig {
                directory: "tiles/".to_string(),
                filename_pattern: "{x}_{y}.mp".to_string(),
            },
            filters: None,
            error_handling: "continue".to_string(),
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have either 'path' or 'connection'"));

        // Invalid: has neither
        config.inputs[0].path = None;
        config.inputs[0].connection = None;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must have either 'path' or 'connection'"));
    }

    #[test]
    fn test_filter_bbox_validation() {
        // Valid bbox
        let yaml_valid = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [-5.0, 41.0, 10.0, 51.5]
"#;
        let config: Config = serde_yml::from_str(yaml_valid).unwrap();
        assert!(config.validate().is_ok());

        // Invalid: min_lon >= max_lon
        let yaml_invalid_lon = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [10.0, 41.0, -5.0, 51.5]
"#;
        let config: Config = serde_yml::from_str(yaml_invalid_lon).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_lon must be < max_lon"));

        // Invalid: min_lat >= max_lat
        let yaml_invalid_lat = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
filters:
  bbox: [-5.0, 51.5, 10.0, 41.0]
"#;
        let config: Config = serde_yml::from_str(yaml_invalid_lat).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("min_lat must be < max_lat"));
    }

    #[test]
    fn test_input_source_type_detection() {
        // File type (path)
        let input_file = InputSource {
            path: Some("data.shp".to_string()),
            connection: None,
            layer: None,
            layers: None,
        };
        assert_eq!(input_file.source_type(), SourceType::File);

        // PostGIS type (PG: prefix)
        let input_pg1 = InputSource {
            path: None,
            connection: Some("PG:host=localhost dbname=gis".to_string()),
            layer: Some("roads".to_string()),
            layers: None,
        };
        assert_eq!(input_pg1.source_type(), SourceType::PostGIS);

        // PostGIS type (host= pattern)
        let input_pg2 = InputSource {
            path: None,
            connection: Some("host=localhost dbname=gis user=admin".to_string()),
            layer: None,
            layers: None,
        };
        assert_eq!(input_pg2.source_type(), SourceType::PostGIS);

        // File type (connection is not PostGIS-like)
        let input_other = InputSource {
            path: None,
            connection: Some("sqlite://db.sqlite".to_string()),
            layer: None,
            layers: None,
        };
        assert_eq!(input_other.source_type(), SourceType::File);
    }

    #[test]
    fn test_resolve_wildcard_paths() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp directory with test files
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        fs::write(temp_path.join("file1.shp"), "").unwrap();
        fs::write(temp_path.join("file2.shp"), "").unwrap();
        fs::write(temp_path.join("roads.gpkg"), "").unwrap();

        // Test wildcard expansion
        let pattern = format!("{}/*.shp", temp_path.display());
        let resolved = resolve_wildcard_paths(&pattern).unwrap();

        assert_eq!(resolved.len(), 2);
        assert!(resolved
            .iter()
            .any(|p| p.file_name().unwrap() == "file1.shp"));
        assert!(resolved
            .iter()
            .any(|p| p.file_name().unwrap() == "file2.shp"));

        // Test no matches (warning logged)
        let pattern_no_match = format!("{}/*.xyz", temp_path.display());
        let resolved_empty = resolve_wildcard_paths(&pattern_no_match).unwrap();
        assert_eq!(resolved_empty.len(), 0);
    }
}
