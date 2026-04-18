//! Polish Map (.mp) file writing using GDAL PolishMap driver.

use crate::config::HeaderConfig;
use crate::pipeline::reader::{Feature, GeometryType};
use anyhow::{anyhow, Context, Result};
use gdal::cpl::CslStringList;
use gdal::vector::{Geometry as GdalGeometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager, Metadata};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, instrument, warn};

/// Field mapping configuration structure for YAML deserialization.
/// Story 7.4: Maps source field names to Polish Map canonical names.
#[derive(Debug, Deserialize)]
struct FieldMappingConfig {
    field_mapping: HashMap<String, String>,
}

/// Validate a field mapping YAML file without loading it into GDAL.
///
/// Reads the file, parses it as `FieldMappingConfig`, and returns the number of mappings.
/// The struct `FieldMappingConfig` remains private — only this validation function is exposed.
pub fn validate_field_mapping(path: &Path) -> Result<usize> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read field mapping file: {}", path.display()))?;

    let mapping_config: FieldMappingConfig = serde_yml::from_str(&content)
        .with_context(|| format!("Failed to parse field mapping YAML: {}", path.display()))?;

    Ok(mapping_config.field_mapping.len())
}

/// Statistics for export operations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExportStats {
    pub point_count: usize,
    pub linestring_count: usize,
    pub polygon_count: usize,
}

/// Writes Polish Map (.mp) files using GDAL PolishMap driver.
pub struct MpWriter {
    output_path: PathBuf,
    dataset: Option<Dataset>,
    stats: ExportStats,
    /// Optional field mapping (source_field -> polishmap_field).
    /// Story 7.4: Used to transform attribute names before writing.
    field_mapping: Option<HashMap<String, String>>,
}

impl MpWriter {
    /// Set header metadata on dataset using SetMetadataItem.
    /// Story 8.1: Converts YAML field names (snake_case) to Polish Map format (PascalCase).
    fn set_header_metadata(dataset: &mut Dataset, header: &HeaderConfig) -> Result<()> {
        // Helper macro to set metadata item if value is Some
        macro_rules! set_if_some {
            ($field:expr, $key:expr) => {
                if let Some(ref value) = $field {
                    info!(key = $key, value = value, "Setting header metadata");
                    dataset.set_metadata_item($key, value, "")?;
                }
            };
        }

        // Standard header fields (YAML snake_case -> Polish Map PascalCase)
        set_if_some!(header.name, "Name");
        set_if_some!(header.id, "ID");
        set_if_some!(header.copyright, "Copyright");
        set_if_some!(header.levels, "Levels");
        set_if_some!(header.level0, "Level0");
        set_if_some!(header.level1, "Level1");
        set_if_some!(header.level2, "Level2");
        set_if_some!(header.level3, "Level3");
        set_if_some!(header.level4, "Level4");
        set_if_some!(header.level5, "Level5");
        set_if_some!(header.level6, "Level6");
        set_if_some!(header.level7, "Level7");
        set_if_some!(header.level8, "Level8");
        set_if_some!(header.level9, "Level9");
        set_if_some!(header.tree_size, "TreeSize");
        set_if_some!(header.rgn_limit, "RgnLimit");
        set_if_some!(header.transparent, "Transparent");
        set_if_some!(header.marine, "Marine");
        set_if_some!(header.preprocess, "Preprocess");
        set_if_some!(header.lbl_coding, "LBLcoding");
        set_if_some!(header.simplify_level, "SimplifyLevel");
        set_if_some!(header.left_side_traffic, "LeftSideTraffic");
        set_if_some!(header.routing, "Routing");

        // Custom header fields (arbitrary key-value pairs)
        if let Some(ref custom) = header.custom {
            for (key, value) in custom {
                info!(key = key, value = value, "Setting custom header metadata");
                dataset.set_metadata_item(key, value, "")?;
            }
        }

        Ok(())
    }

    /// Create a new MpWriter for a specific output file.
    ///
    /// # Arguments
    /// * `output_path` - Complete path to output .mp file (e.g., "tiles/45_12.mp")
    /// * `field_mapping_path` - Optional path to YAML field mapping config for ogr-polishmap driver (Story 7.4)
    /// * `header_config` - Optional header configuration for Polish Map files (Story 8.1)
    ///
    /// # Returns
    /// * `Result<Self>` - Initialized writer ready to accept features
    ///
    /// # Errors
    /// * GDAL driver "PolishMap" not found
    /// * Failed to create output directory
    /// * Failed to create dataset or layers
    /// * Field mapping path encoding is invalid (non-UTF8)
    /// * Header template path encoding is invalid (non-UTF8)
    ///
    /// # Breaking Change (Story 6.4)
    /// Previous signature was `new(config: &OutputConfig)`.
    /// Now accepts PathBuf directly for multi-tile support.
    ///
    /// # Story 7.4
    /// Added optional `field_mapping_path` parameter to support YAML-based field mapping.
    ///
    /// # Story 8.1
    /// Added optional `header_config` parameter to support header template and individual fields.
    #[instrument(skip_all, fields(output_path = %output_path.display(), field_mapping = ?field_mapping_path, header = ?header_config.is_some()))]
    pub fn new(
        output_path: PathBuf,
        field_mapping_path: Option<&Path>,
        header_config: Option<&HeaderConfig>,
    ) -> Result<Self> {
        info!(path = %output_path.display(), "Initializing MpWriter");

        // Note: Output directory creation is handled by caller (pipeline/mod.rs)
        // to avoid repeated filesystem calls when creating multiple tiles.

        // Get GDAL PolishMap driver
        let driver = DriverManager::get_driver_by_name("PolishMap")
            .context("PolishMap driver not available. Ensure ogr-polishmap is installed.")?;

        info!(path = %output_path.display(), "Creating MP dataset");

        // Story 7.4 + 8.1: Prepare dataset creation options (FIELD_MAPPING, HEADER_TEMPLATE)
        let mut options = CslStringList::new();
        let mut has_options = false;

        // Story 7.4: Add FIELD_MAPPING option if provided
        let field_mapping = if let Some(mapping_path) = field_mapping_path {
            // Validate file exists before processing (H3 fix - validation moved here)
            if !mapping_path.exists() {
                anyhow::bail!(
                    "Field mapping file does not exist: {}. Please provide a valid YAML mapping file.",
                    mapping_path.display()
                );
            }

            // Load and parse YAML file
            let mapping_content = std::fs::read_to_string(mapping_path).with_context(|| {
                format!(
                    "Failed to read field mapping file: {}",
                    mapping_path.display()
                )
            })?;

            let mapping_config: FieldMappingConfig = serde_yml::from_str(&mapping_content)
                .with_context(|| {
                    format!(
                        "Failed to parse field mapping YAML: {}",
                        mapping_path.display()
                    )
                })?;

            // Convert path to absolute path string for GDAL
            // Use canonicalize with fallback for symlinks/permissions edge cases (M2 fix)
            let mapping_path_abs = std::fs::canonicalize(mapping_path)
                .or_else(|_| {
                    // Fallback: use path as-is if canonicalize fails (symlinks, permissions)
                    std::env::current_dir().map(|cwd| cwd.join(mapping_path))
                        .with_context(|| format!(
                            "Failed to resolve field mapping path: {}. Ensure the file is readable.",
                            mapping_path.display()
                        ))
                })?;

            let mapping_path_str = mapping_path_abs
                .to_str()
                .context("Invalid field mapping path encoding (non-UTF8)")?;

            info!(
                field_mapping = %mapping_path_str,
                mapping_count = mapping_config.field_mapping.len(),
                "Adding FIELD_MAPPING dataset creation option"
            );

            options.set_name_value("FIELD_MAPPING", mapping_path_str)?;
            has_options = true;

            Some(mapping_config.field_mapping)
        } else {
            None
        };

        // Story 8.1: Add HEADER_TEMPLATE option if provided
        if let Some(header) = header_config {
            if let Some(template_path) = &header.template {
                // Story 8.1 Code Review Fix H2: Validate template exists at usage time (not config load)
                // This avoids TOCTOU race condition in parallel mode (same pattern as field_mapping)
                if !template_path.exists() {
                    anyhow::bail!(
                        "header.template file does not exist: {}. Please provide a valid .mp template file.",
                        template_path.display()
                    );
                }

                // Convert path to absolute path string for GDAL (same pattern as field_mapping)
                let template_path_abs = std::fs::canonicalize(template_path)
                    .or_else(|_| {
                        std::env::current_dir().map(|cwd| cwd.join(template_path))
                            .with_context(|| format!(
                                "Failed to resolve header template path: {}. Ensure the file is readable.",
                                template_path.display()
                            ))
                    })?;

                let template_path_str = template_path_abs
                    .to_str()
                    .context("Invalid header template path encoding (non-UTF8)")?;

                info!(
                    header_template = %template_path_str,
                    "Adding HEADER_TEMPLATE dataset creation option"
                );

                options.set_name_value("HEADER_TEMPLATE", template_path_str)?;
                has_options = true;
            }
        }

        // Create dataset with or without options
        let mut dataset = if has_options {
            info!("Creating dataset with creation options (FIELD_MAPPING and/or HEADER_TEMPLATE)");
            driver
                .create_with_band_type_with_options::<u8, _>(
                    &output_path,
                    0,
                    0,
                    0, // Vector-only dataset (0 dimensions, 0 bands)
                    &options,
                )
                .with_context(|| {
                    // Code Review Fix L1: Explicit error context for better DX
                    let mut msg = format!(
                        "Failed to create dataset with options: {}",
                        output_path.display()
                    );
                    if let Some(header) = header_config {
                        if let Some(template) = &header.template {
                            msg.push_str(&format!(
                                "\n  HEADER_TEMPLATE used: {} (check GDAL stderr for details)",
                                template.display()
                            ));
                        }
                    }
                    msg
                })?
        } else {
            info!("Creating dataset without creation options (backward compatible)");
            driver
                .create_vector_only(&output_path)
                .with_context(|| format!("Failed to create dataset: {}", output_path.display()))?
        };

        // Story 8.1: Set individual header fields via SetMetadataItem (if no template)
        if let Some(header) = header_config {
            // Code Review Fix M2: Mutually exclusive design - mpforge doesn't send individual
            // fields when template is present. This is NOT driver precedence, it's CLI logic.
            // Rationale: Template is meant to be a complete header replacement.
            if header.template.is_none() {
                Self::set_header_metadata(&mut dataset, header)?;
            }
        }

        // Create POI layer
        let _poi_layer = dataset
            .create_layer(LayerOptions {
                name: "POI",
                srs: None, // WGS84 is driver default
                ty: OGRwkbGeometryType::wkbPoint,
                options: None,
            })
            .context("Failed to create POI layer")?;

        // Create POLYLINE layer
        let _polyline_layer = dataset
            .create_layer(LayerOptions {
                name: "POLYLINE",
                srs: None,
                ty: OGRwkbGeometryType::wkbLineString,
                options: None,
            })
            .context("Failed to create POLYLINE layer")?;

        // Create POLYGON layer
        let _polygon_layer = dataset
            .create_layer(LayerOptions {
                name: "POLYGON",
                srs: None,
                ty: OGRwkbGeometryType::wkbPolygon,
                options: None,
            })
            .context("Failed to create POLYGON layer")?;

        info!("MpWriter initialized with 3 layers (POI, POLYLINE, POLYGON)");

        Ok(Self {
            output_path,
            dataset: Some(dataset),
            stats: ExportStats::default(),
            field_mapping,
        })
    }

    /// Write features to the appropriate layers based on geometry type.
    ///
    /// # Arguments
    /// * `features` - Vector of features to write
    ///
    /// # Returns
    /// * `Result<ExportStats>` - Statistics about features written
    ///
    /// # Errors
    /// * Failed to access layer
    /// * Failed to create or write feature
    #[instrument(skip(self, features))]
    pub fn write_features(&mut self, features: &[Feature]) -> Result<ExportStats> {
        info!(
            feature_count = features.len(),
            output_path = %self.output_path.display(),
            "Starting MP export"
        );

        if features.is_empty() {
            warn!("No features to export, dataset will be empty");
            return Ok(ExportStats::default());
        }

        // Story 7.4: Extract field_mapping reference before borrowing dataset (borrow checker)
        let field_mapping = self.field_mapping.as_ref();

        let dataset = self
            .dataset
            .as_mut()
            .ok_or_else(|| anyhow!("Dataset not initialized"))?;

        // Get layers by name
        let mut poi_layer = dataset
            .layer_by_name("POI")
            .context("Failed to access POI layer")?;
        let mut polyline_layer = dataset
            .layer_by_name("POLYLINE")
            .context("Failed to access POLYLINE layer")?;
        let mut polygon_layer = dataset
            .layer_by_name("POLYGON")
            .context("Failed to access POLYGON layer")?;

        let mut stats = ExportStats::default();

        // Write each feature to appropriate layer
        for feature in features {
            match feature.geometry_type {
                GeometryType::Point => {
                    Self::write_point_feature(&mut poi_layer, feature, field_mapping)
                        .context("Failed to write POI feature")?;
                    stats.point_count += 1;
                }
                GeometryType::LineString => {
                    Self::write_linestring_feature(&mut polyline_layer, feature, field_mapping)
                        .context("Failed to write POLYLINE feature")?;
                    stats.linestring_count += 1;
                }
                GeometryType::Polygon => {
                    Self::write_polygon_feature(&mut polygon_layer, feature, field_mapping)
                        .context("Failed to write POLYGON feature")?;
                    stats.polygon_count += 1;
                }
            }
        }

        info!(
            points = stats.point_count,
            linestrings = stats.linestring_count,
            polygons = stats.polygon_count,
            "Export completed"
        );

        self.stats = stats.clone();
        Ok(stats)
    }

    /// Write a POI feature to the POI layer.
    fn write_point_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        if feature.geometry.is_empty() {
            warn!("Skipping POI feature with empty geometry");
            return Ok(());
        }

        let (lon, lat) = feature.geometry[0];

        // Create GDAL Point geometry
        let geometry = GdalGeometry::from_wkt(&format!("POINT ({} {})", lon, lat))
            .context("Failed to create Point geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Write a POLYLINE feature to the POLYLINE layer.
    fn write_linestring_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        if feature.geometry.len() < 2 {
            warn!("Skipping POLYLINE feature with less than 2 points");
            return Ok(());
        }

        // Build WKT for LineString
        let coords = feature
            .geometry
            .iter()
            .map(|(lon, lat)| format!("{} {}", lon, lat))
            .collect::<Vec<_>>()
            .join(", ");

        let wkt = format!("LINESTRING ({})", coords);

        let geometry =
            GdalGeometry::from_wkt(&wkt).context("Failed to create LineString geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Write a POLYGON feature to the POLYGON layer.
    fn write_polygon_feature(
        layer: &mut gdal::vector::Layer,
        feature: &Feature,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        // Note: Minimum 4 points for closed polygon ring (start point == end point)
        // This assumes the polygon is not auto-closed by GDAL.
        // If GDAL auto-closes, 3 points would suffice for a triangle.
        if feature.geometry.len() < 4 {
            warn!("Skipping POLYGON feature with less than 4 points (need closed ring)");
            return Ok(());
        }

        // Build WKT for Polygon (outer ring)
        let coords = feature
            .geometry
            .iter()
            .map(|(lon, lat)| format!("{} {}", lon, lat))
            .collect::<Vec<_>>()
            .join(", ");

        let wkt = format!("POLYGON (({}))", coords);

        let geometry = GdalGeometry::from_wkt(&wkt).context("Failed to create Polygon geometry")?;

        // Create feature
        let layer_defn = layer.defn();
        let mut ogr_feature =
            gdal::vector::Feature::new(layer_defn).context("Failed to create OGR feature")?;

        ogr_feature
            .set_geometry(geometry)
            .context("Failed to set geometry")?;

        // Set attributes
        Self::set_feature_attributes(
            layer_defn,
            &mut ogr_feature,
            &feature.attributes,
            field_mapping,
        )?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Set feature attributes from HashMap to OGR feature.
    /// Story 7.4: Uses field_mapping to transform source field names to Polish Map canonical names.
    fn set_feature_attributes(
        layer_defn: &gdal::vector::Defn,
        ogr_feature: &mut gdal::vector::Feature,
        attributes: &HashMap<String, String>,
        field_mapping: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        // Tri des clés pour itération déterministe — pré-requis Tech-spec #2 AC0
        // (cf. tech-spec-mpforge-multi-data-bdtopo-profiles.md R2).
        let mut sorted_keys: Vec<&String> = attributes.keys().collect();
        sorted_keys.sort();
        for source_key in sorted_keys {
            let value = &attributes[source_key];
            // Story 7.4: Transform field name using mapping if provided
            let target_key = if let Some(mapping) = field_mapping {
                // Use mapped name if it exists, otherwise use source name as-is
                mapping
                    .get(source_key)
                    .map(|s| s.as_str())
                    .unwrap_or(source_key)
            } else {
                // No mapping - use source name (backward compatible)
                source_key
            };

            // DEBUG: Log mapping transformation
            if source_key != target_key {
                info!(
                    source_field = source_key,
                    target_field = target_key,
                    value = value,
                    "Field mapping applied"
                );
            }

            // Find field index by name (using target_key)
            if let Ok(field_idx) = layer_defn.field_index(target_key) {
                // Set field using index
                if let Err(e) = ogr_feature.set_field_string(field_idx, value) {
                    // Field set failed - log warning and continue (graceful degradation)
                    warn!(
                        source_field = source_key,
                        target_field = target_key,
                        value = value,
                        error = %e,
                        "Failed to set field attribute, skipping"
                    );
                    continue;
                }
                info!(
                    source_field = source_key,
                    target_field = target_key,
                    value = value,
                    "Field set successfully"
                );
            } else {
                // Field not in schema - log warning for debugging
                warn!(
                    source_field = source_key,
                    target_field = target_key,
                    "Field not found in layer schema"
                );
            }
        }
        Ok(())
    }

    /// Finalize writing and close the dataset.
    ///
    /// # Returns
    /// * `Result<ExportStats>` - Final statistics
    ///
    /// # Errors
    /// * Failed to flush dataset
    #[instrument(skip(self))]
    pub fn finalize(mut self) -> Result<ExportStats> {
        info!(
            path = %self.output_path.display(),
            points = self.stats.point_count,
            linestrings = self.stats.linestring_count,
            polygons = self.stats.polygon_count,
            "Finalizing MP export"
        );

        // Drop dataset to flush and close
        if let Some(dataset) = self.dataset.take() {
            drop(dataset);
        }

        info!("MP export finalized successfully");

        Ok(self.stats.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_stats_default() {
        let stats = ExportStats::default();
        assert_eq!(stats.point_count, 0);
        assert_eq!(stats.linestring_count, 0);
        assert_eq!(stats.polygon_count, 0);
    }

    #[test]
    fn test_export_stats_clone() {
        let stats = ExportStats {
            point_count: 10,
            linestring_count: 5,
            polygon_count: 3,
        };
        let cloned = stats.clone();
        assert_eq!(stats, cloned);
    }
}
