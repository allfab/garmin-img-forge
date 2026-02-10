//! Polish Map (.mp) file writing using GDAL PolishMap driver.

use crate::pipeline::reader::{Feature, GeometryType};
use anyhow::{anyhow, Context, Result};
use gdal::vector::{Geometry as GdalGeometry, LayerAccess, LayerOptions, OGRwkbGeometryType};
use gdal::{Dataset, DriverManager};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, instrument, warn};

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
}

impl MpWriter {
    /// Create a new MpWriter for a specific output file.
    ///
    /// # Arguments
    /// * `output_path` - Complete path to output .mp file (e.g., "tiles/45_12.mp")
    ///
    /// # Returns
    /// * `Result<Self>` - Initialized writer ready to accept features
    ///
    /// # Errors
    /// * GDAL driver "PolishMap" not found
    /// * Failed to create output directory
    /// * Failed to create dataset or layers
    ///
    /// # Breaking Change (Story 6.4)
    /// Previous signature was `new(config: &OutputConfig)`.
    /// Now accepts PathBuf directly for multi-tile support.
    #[instrument(skip_all, fields(output_path = %output_path.display()))]
    pub fn new(output_path: PathBuf) -> Result<Self> {
        info!(path = %output_path.display(), "Initializing MpWriter");

        // Note: Output directory creation is handled by caller (pipeline/mod.rs)
        // to avoid repeated filesystem calls when creating multiple tiles.

        // Get GDAL PolishMap driver
        let driver = DriverManager::get_driver_by_name("PolishMap")
            .context("PolishMap driver not available. Ensure ogr-polishmap is installed.")?;

        info!(path = %output_path.display(), "Creating MP dataset");

        // Create dataset (for vector drivers, create_vector_only() is used)
        let mut dataset = driver
            .create_vector_only(&output_path)
            .with_context(|| format!("Failed to create dataset: {}", output_path.display()))?;

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
                    Self::write_point_feature(&mut poi_layer, feature)
                        .context("Failed to write POI feature")?;
                    stats.point_count += 1;
                }
                GeometryType::LineString => {
                    Self::write_linestring_feature(&mut polyline_layer, feature)
                        .context("Failed to write POLYLINE feature")?;
                    stats.linestring_count += 1;
                }
                GeometryType::Polygon => {
                    Self::write_polygon_feature(&mut polygon_layer, feature)
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
    fn write_point_feature(layer: &mut gdal::vector::Layer, feature: &Feature) -> Result<()> {
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
        Self::set_feature_attributes(layer_defn, &mut ogr_feature, &feature.attributes)?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Write a POLYLINE feature to the POLYLINE layer.
    fn write_linestring_feature(layer: &mut gdal::vector::Layer, feature: &Feature) -> Result<()> {
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
        Self::set_feature_attributes(layer_defn, &mut ogr_feature, &feature.attributes)?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Write a POLYGON feature to the POLYGON layer.
    fn write_polygon_feature(layer: &mut gdal::vector::Layer, feature: &Feature) -> Result<()> {
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
        Self::set_feature_attributes(layer_defn, &mut ogr_feature, &feature.attributes)?;

        // Write to layer
        ogr_feature
            .create(layer)
            .context("Failed to create feature in layer")?;

        Ok(())
    }

    /// Set feature attributes from HashMap to OGR feature.
    fn set_feature_attributes(
        layer_defn: &gdal::vector::Defn,
        ogr_feature: &mut gdal::vector::Feature,
        attributes: &HashMap<String, String>,
    ) -> Result<()> {
        for (key, value) in attributes {
            // Find field index by name
            if let Ok(field_idx) = layer_defn.field_index(key) {
                // Set field using index
                if let Err(e) = ogr_feature.set_field_string(field_idx, value) {
                    // Field set failed - log warning and continue (graceful degradation)
                    warn!(
                        field = key,
                        value = value,
                        error = %e,
                        "Failed to set field attribute, skipping"
                    );
                    continue;
                }
            }
            // Field not in schema - skip silently (expected for non-standard attributes)
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
