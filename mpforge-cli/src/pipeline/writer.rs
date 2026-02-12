//! Polish Map (.mp) file writing using GDAL PolishMap driver.

use crate::pipeline::reader::{Feature, GeometryType};
use anyhow::{anyhow, Context, Result};
use gdal::cpl::CslStringList;
use gdal::vector::{Geometry as GdalGeometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager};
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
    /// Create a new MpWriter for a specific output file.
    ///
    /// # Arguments
    /// * `output_path` - Complete path to output .mp file (e.g., "tiles/45_12.mp")
    /// * `field_mapping_path` - Optional path to YAML field mapping config for ogr-polishmap driver (Story 7.4)
    ///
    /// # Returns
    /// * `Result<Self>` - Initialized writer ready to accept features
    ///
    /// # Errors
    /// * GDAL driver "PolishMap" not found
    /// * Failed to create output directory
    /// * Failed to create dataset or layers
    /// * Field mapping path encoding is invalid (non-UTF8)
    ///
    /// # Breaking Change (Story 6.4)
    /// Previous signature was `new(config: &OutputConfig)`.
    /// Now accepts PathBuf directly for multi-tile support.
    ///
    /// # Story 7.4
    /// Added optional `field_mapping_path` parameter to support YAML-based field mapping.
    #[instrument(skip_all, fields(output_path = %output_path.display(), field_mapping = ?field_mapping_path))]
    pub fn new(output_path: PathBuf, field_mapping_path: Option<&Path>) -> Result<Self> {
        info!(path = %output_path.display(), "Initializing MpWriter");

        // Note: Output directory creation is handled by caller (pipeline/mod.rs)
        // to avoid repeated filesystem calls when creating multiple tiles.

        // Get GDAL PolishMap driver
        let driver = DriverManager::get_driver_by_name("PolishMap")
            .context("PolishMap driver not available. Ensure ogr-polishmap is installed.")?;

        info!(path = %output_path.display(), "Creating MP dataset");

        // Story 7.4: Load field mapping if provided
        let (field_mapping, mut dataset) = if let Some(mapping_path) = field_mapping_path {
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
                "Creating dataset with field mapping configuration"
            );

            // Create dataset with FIELD_MAPPING creation option
            let mut options = CslStringList::new();
            options.set_name_value("FIELD_MAPPING", mapping_path_str)?;

            let dataset = driver
                .create_with_band_type_with_options::<u8, _>(
                    &output_path,
                    0,
                    0,
                    0, // Vector-only dataset (0 dimensions, 0 bands)
                    &options,
                )
                .with_context(|| {
                    format!(
                        "Failed to create dataset with field mapping: {}",
                        output_path.display()
                    )
                })?;

            (Some(mapping_config.field_mapping), dataset)
        } else {
            // No field mapping - use hardcoded aliases (backward compatible)
            info!("Field mapping not configured, using driver hardcoded aliases (backward compatible)");
            let dataset = driver
                .create_vector_only(&output_path)
                .with_context(|| format!("Failed to create dataset: {}", output_path.display()))?;

            (None, dataset)
        };

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
        for (source_key, value) in attributes {
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
