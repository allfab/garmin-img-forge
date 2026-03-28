//! Configuration file parsing and structures.

use crate::pipeline::tile_naming;
use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
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
    /// Optional header configuration for Polish Map files (Story 8.1)
    #[serde(default)]
    pub header: Option<HeaderConfig>,
    /// Optional path to YAML rules file for attribute transformation (Story 9.1)
    #[serde(default)]
    pub rules: Option<PathBuf>,
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
    /// Override source SRS (e.g., "EPSG:2154"). Story 9.4: explicit reprojection.
    #[serde(default)]
    pub source_srs: Option<String>,
    /// Target SRS for reprojection (e.g., "EPSG:4326"). Defaults to WGS84 if source_srs is set.
    #[serde(default)]
    pub target_srs: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub directory: String,
    #[serde(default = "default_filename_pattern")]
    pub filename_pattern: String,
    /// Optional path to YAML field mapping config for ogr-polishmap driver.
    /// Story 7.4: Maps source field names (e.g., MP_TYPE, NAME) to Polish Map canonical names (Type, Label).
    #[serde(default)]
    pub field_mapping_path: Option<PathBuf>,
    /// Optional: overwrite existing tile files (default: true).
    /// Set to false for skip-existing behavior via config.
    /// Story 8.3: None or Some(true) = overwrite, Some(false) = skip existing.
    #[serde(default)]
    pub overwrite: Option<bool>,
    /// Base ID for auto-generating unique tile IDs.
    /// Formula: tile_id = base_id * 10000 + seq (1-based).
    /// When set and header.id is absent or "0", each tile gets a unique 8-digit ID.
    /// Must be in range 1..=9999 to guarantee 8-digit IDs.
    #[serde(default)]
    pub base_id: Option<u32>,
}

fn default_filename_pattern() -> String {
    "{col}_{row}.mp".to_string()
}

/// Sentinel value for header.id meaning "auto-generate from base_id".
pub const AUTO_ID: &str = "0";

/// Header configuration for Polish Map files.
/// Story 8.1: Allows configuring header options (template, name, levels, etc.) via YAML.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HeaderConfig {
    /// Optional path to header template file (.mp) for HEADER_TEMPLATE DSCO
    #[serde(default)]
    pub template: Option<PathBuf>,
    /// Map name (Polish Map: Name)
    #[serde(default)]
    pub name: Option<String>,
    /// Map ID (Polish Map: ID).
    /// Set to "0" (AUTO_ID) to auto-generate unique IDs via output.base_id.
    #[serde(default)]
    pub id: Option<String>,
    /// Copyright notice (Polish Map: Copyright)
    #[serde(default)]
    pub copyright: Option<String>,
    /// Number of detail levels (Polish Map: Levels)
    #[serde(default)]
    pub levels: Option<String>,
    /// Level 0 zoom (Polish Map: Level0)
    #[serde(default)]
    pub level0: Option<String>,
    /// Level 1 zoom (Polish Map: Level1)
    #[serde(default)]
    pub level1: Option<String>,
    /// Level 2 zoom (Polish Map: Level2)
    #[serde(default)]
    pub level2: Option<String>,
    /// Level 3 zoom (Polish Map: Level3)
    #[serde(default)]
    pub level3: Option<String>,
    /// Level 4 zoom (Polish Map: Level4)
    #[serde(default)]
    pub level4: Option<String>,
    /// Level 5 zoom (Polish Map: Level5)
    #[serde(default)]
    pub level5: Option<String>,
    /// Level 6 zoom (Polish Map: Level6)
    #[serde(default)]
    pub level6: Option<String>,
    /// Level 7 zoom (Polish Map: Level7)
    #[serde(default)]
    pub level7: Option<String>,
    /// Level 8 zoom (Polish Map: Level8)
    #[serde(default)]
    pub level8: Option<String>,
    /// Level 9 zoom (Polish Map: Level9)
    #[serde(default)]
    pub level9: Option<String>,
    /// Tree size parameter (Polish Map: TreeSize)
    #[serde(default)]
    pub tree_size: Option<String>,
    /// Region limit parameter (Polish Map: RgnLimit)
    #[serde(default)]
    pub rgn_limit: Option<String>,
    /// Transparency setting (Polish Map: Transparent)
    #[serde(default)]
    pub transparent: Option<String>,
    /// Marine map setting (Polish Map: Marine)
    #[serde(default)]
    pub marine: Option<String>,
    /// Preprocessing mode (Polish Map: Preprocess)
    #[serde(default)]
    pub preprocess: Option<String>,
    /// Label encoding (Polish Map: LBLcoding)
    #[serde(default)]
    pub lbl_coding: Option<String>,
    /// Simplification level (Polish Map: SimplifyLevel)
    #[serde(default)]
    pub simplify_level: Option<String>,
    /// Left-side traffic setting (Polish Map: LeftSideTraffic)
    #[serde(default)]
    pub left_side_traffic: Option<String>,
    /// Custom arbitrary header fields
    #[serde(default)]
    pub custom: Option<HashMap<String, String>>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

        // Error handling validation (use ErrorMode::from_str for consistency)
        ErrorMode::from_str(&self.error_handling)
            .with_context(|| format!("Invalid error_handling value: '{}'", self.error_handling))?;

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

        // Story 9.4: Validate SRS definitions (fail-fast before processing)
        for (i, input) in self.inputs.iter().enumerate() {
            if let Some(ref srs) = input.source_srs {
                gdal::spatial_ref::SpatialRef::from_definition(srs).with_context(|| {
                    format!(
                        "InputSource #{}: invalid source_srs '{}' — must be a valid SRS definition (e.g., EPSG:2154)",
                        i, srs
                    )
                })?;
            }
            if let Some(ref srs) = input.target_srs {
                gdal::spatial_ref::SpatialRef::from_definition(srs).with_context(|| {
                    format!(
                        "InputSource #{}: invalid target_srs '{}' — must be a valid SRS definition (e.g., EPSG:4326)",
                        i, srs
                    )
                })?;
            }
            if input.target_srs.is_some() && input.source_srs.is_none() {
                warn!(
                    source_index = i,
                    target_srs = ?input.target_srs,
                    "InputSource #{}: target_srs without source_srs has no effect (ignored)",
                    i
                );
            }
        }

        // base_id validation: must produce valid 8-digit Garmin IDs
        if let Some(base_id) = self.output.base_id {
            if base_id == 0 || base_id > 9999 {
                anyhow::bail!(
                    "output.base_id must be in range 1..=9999, got: {}. \
                     Formula: base_id * 10000 + seq must fit in 8 digits.",
                    base_id
                );
            }
        }

        // Filename pattern validation (Story 8.2)
        tile_naming::validate_tile_pattern(&self.output.filename_pattern)
            .with_context(|| format!("Invalid filename_pattern: '{}'", self.output.filename_pattern))?;

        // Output field_mapping_path validation removed (Story 7.4)
        // Validation moved to MpWriter::new() to avoid race condition in parallel mode
        // where file could be deleted between config validation and usage.
        // See writer.rs:65-69 for canonicalize() with proper error context.

        // Header template validation removed (Story 8.1 Code Review Fix H2)
        // Validation moved to MpWriter::new() to avoid TOCTOU race condition in parallel mode.
        // Same rationale as field_mapping: file could be deleted between config load and first tile export.
        // See writer.rs for validation with proper error context at actual usage time.

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
        assert_eq!(config.output.filename_pattern, "{col}_{row}.mp");
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
        // Updated for L2 fix: validation now uses ErrorMode::from_str
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Invalid error_handling")
                || error_msg.contains("expected 'continue' or 'fail-fast'")
        );
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
                source_srs: None,
                target_srs: None,
            }],
            output: OutputConfig {
                directory: "tiles/".to_string(),
                filename_pattern: "{col}_{row}.mp".to_string(),
                field_mapping_path: None,
                overwrite: None,
                base_id: None,
            },
            filters: None,
            error_handling: "continue".to_string(),
            header: None,
            rules: None,
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
            source_srs: None,
            target_srs: None,
        };
        assert_eq!(input_file.source_type(), SourceType::File);

        // PostGIS type (PG: prefix)
        let input_pg1 = InputSource {
            path: None,
            connection: Some("PG:host=localhost dbname=gis".to_string()),
            layer: Some("roads".to_string()),
            layers: None,
            source_srs: None,
            target_srs: None,
        };
        assert_eq!(input_pg1.source_type(), SourceType::PostGIS);

        // PostGIS type (host= pattern)
        let input_pg2 = InputSource {
            path: None,
            connection: Some("host=localhost dbname=gis user=admin".to_string()),
            layer: None,
            layers: None,
            source_srs: None,
            target_srs: None,
        };
        assert_eq!(input_pg2.source_type(), SourceType::PostGIS);

        // File type (connection is not PostGIS-like)
        let input_other = InputSource {
            path: None,
            connection: Some("sqlite://db.sqlite".to_string()),
            layer: None,
            layers: None,
            source_srs: None,
            target_srs: None,
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

    // Story 7.4: field_mapping_path tests
    #[test]
    fn test_config_with_field_mapping_path() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp file for mapping
        let temp_dir = TempDir::new().unwrap();
        let mapping_path = temp_dir.path().join("mapping.yaml");
        fs::write(&mapping_path, "MP_TYPE: Type\nNAME: Label").unwrap();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  field_mapping_path: "{}"
"#,
            mapping_path.display()
        );
        let config: Config = serde_yml::from_str(&yaml).unwrap();
        assert!(config.output.field_mapping_path.is_some());
        assert_eq!(
            config.output.field_mapping_path.as_ref().unwrap(),
            &mapping_path
        );
        // Validation should pass when file exists
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_without_field_mapping_path() {
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
        assert!(config.output.field_mapping_path.is_none());
        // Validation should pass even without field_mapping_path (backward compat)
        assert!(config.validate().is_ok());
    }

    // Test removed (Story 7.4 Code Review Fix H3):
    // field_mapping_path validation moved from config.validate() to MpWriter::new()
    // to avoid race condition in parallel mode. See test_field_mapping_invalid_path_error
    // in tests/integration_export.rs for validation coverage.

    // Story 8.1: HeaderConfig tests
    #[test]
    fn test_config_with_header_section() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  name: "Ma Carte"
  levels: "4"
  transparent: "Y"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert_eq!(header.name, Some("Ma Carte".to_string()));
        assert_eq!(header.levels, Some("4".to_string()));
        assert_eq!(header.transparent, Some("Y".to_string()));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_without_header_section() {
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
        assert!(config.header.is_none());
        // Validation should pass without header (backward compat)
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_with_header_template() {
        use std::fs;
        use tempfile::TempDir;

        // Create temp template file
        let temp_dir = TempDir::new().unwrap();
        let template_path = temp_dir.path().join("template.mp");
        fs::write(&template_path, "[IMG ID]\nName=Template").unwrap();

        let yaml = format!(
            r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  template: "{}"
"#,
            template_path.display()
        );
        let config: Config = serde_yml::from_str(&yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert!(header.template.is_some());
        assert_eq!(header.template.as_ref().unwrap(), &template_path);
        // Validation should pass when template exists
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_header_template_no_early_validation() {
        // Story 8.1 Code Review Fix H2: Template validation moved to MpWriter::new()
        // Config::validate() no longer checks template existence to avoid TOCTOU race
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  template: "/nonexistent/template.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        // Should pass - validation happens at usage time in MpWriter::new()
        assert!(result.is_ok(), "Config validation should not check template existence (TOCTOU fix)");
    }

    #[test]
    fn test_config_header_custom_fields() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
header:
  custom:
    DrawPriority: "25"
    MG: "N"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.header.is_some());
        let header = config.header.clone().unwrap();
        assert!(header.custom.is_some());
        let custom = header.custom.unwrap();
        assert_eq!(custom.get("DrawPriority"), Some(&"25".to_string()));
        assert_eq!(custom.get("MG"), Some(&"N".to_string()));
        assert!(config.validate().is_ok());
    }

    // Story 8.2: Filename pattern validation tests
    #[test]
    fn test_config_validate_valid_filename_pattern() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{col:03}_{row:03}.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_filename_pattern() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  filename_pattern: "{invalid_var}.mp"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid filename_pattern"));
    }

    #[test]
    fn test_config_validate_default_pattern_valid() {
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
        // Default {col}_{row}.mp should validate
        assert!(config.validate().is_ok());
    }

    // Story 8.3: OutputConfig overwrite field tests
    #[test]
    fn test_config_output_overwrite_absent() {
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
        assert!(config.output.overwrite.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_output_overwrite_true() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  overwrite: true
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.output.overwrite, Some(true));
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_output_overwrite_false() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
output:
  directory: "tiles/"
  overwrite: false
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(config.output.overwrite, Some(false));
        assert!(config.validate().is_ok());
    }

    // Story 9.4: source_srs / target_srs tests
    #[test]
    fn test_input_source_with_source_srs_and_target_srs() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:4326"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(
            config.inputs[0].source_srs,
            Some("EPSG:2154".to_string())
        );
        assert_eq!(
            config.inputs[0].target_srs,
            Some("EPSG:4326".to_string())
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_with_source_srs_only() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert_eq!(
            config.inputs[0].source_srs,
            Some("EPSG:2154".to_string())
        );
        assert!(config.inputs[0].target_srs.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_without_srs_backward_compat() {
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
        assert!(config.inputs[0].source_srs.is_none());
        assert!(config.inputs[0].target_srs.is_none());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_input_source_invalid_source_srs_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:99999"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid source_srs"));
        assert!(err.contains("EPSG:99999"));
    }

    #[test]
    fn test_input_source_invalid_target_srs_error() {
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    source_srs: "EPSG:2154"
    target_srs: "EPSG:99999"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid target_srs"));
        assert!(err.contains("EPSG:99999"));
    }

    #[test]
    fn test_input_source_target_srs_without_source_srs_warning() {
        // target_srs without source_srs should still parse and validate OK (just warns)
        let yaml = r#"
version: 1
grid:
  cell_size: 0.15
inputs:
  - path: "data.shp"
    target_srs: "EPSG:4326"
output:
  directory: "tiles/"
"#;
        let config: Config = serde_yml::from_str(yaml).unwrap();
        assert!(config.inputs[0].source_srs.is_none());
        assert_eq!(
            config.inputs[0].target_srs,
            Some("EPSG:4326".to_string())
        );
        // Should validate OK (warning only, not error)
        assert!(config.validate().is_ok());
    }
}
