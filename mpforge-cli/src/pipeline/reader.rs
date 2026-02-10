//! Source data reading from GDAL-compatible formats.

use crate::config::{Config, InputSource};
use anyhow::{anyhow, Context, Result};
use gdal::spatial_ref::SpatialRef;
use gdal::vector::{LayerAccess, OGRwkbGeometryType};
use gdal::Dataset;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Statistics for source reading.
#[derive(Debug, Default)]
struct ReaderStats {
    point_count: usize,
    linestring_count: usize,
    polygon_count: usize,
}

/// Reads features from GDAL sources.
///
/// This is a stateless utility struct - all methods are static/associated functions.
pub struct SourceReader;

impl SourceReader {
    /// Read features from a file-based GDAL source (Shapefile, GeoPackage, etc.).
    ///
    /// # Arguments
    /// * `input` - InputSource configuration with path and optional layer
    ///
    /// # Returns
    /// * `Result<Vec<Feature>>` - Vector of features read from the source
    ///
    /// # Errors
    /// * File not found or not readable
    /// * GDAL driver not available
    /// * Invalid layer name
    pub fn read_file_source(input: &InputSource) -> Result<Vec<Feature>> {
        let path = input
            .path
            .as_ref()
            .ok_or_else(|| anyhow!("No path specified for file source"))?;

        info!("Loading source: {}", path);

        // Open GDAL dataset
        let dataset =
            Dataset::open(path).with_context(|| format!("Failed to open dataset: {}", path))?;

        // Get layer (either specified by name or default to first layer)
        let mut layer = if let Some(layers) = &input.layers {
            if layers.is_empty() {
                dataset.layer(0)?
            } else {
                dataset.layer_by_name(&layers[0])?
            }
        } else {
            dataset.layer(0)?
        };

        // Check spatial reference and transform to WGS84 if needed
        let needs_transform = if let Some(spatial_ref) = layer.spatial_ref() {
            if let Ok(auth_code) = spatial_ref.auth_code() {
                if auth_code != 4326 {
                    warn!(
                        path = %path,
                        srs = auth_code,
                        "Layer SRS is not WGS84 (EPSG:4326), transforming coordinates to WGS84"
                    );
                    true
                } else {
                    false
                }
            } else {
                warn!(path = %path, "Layer has SRS but no authority code, assuming transformation needed");
                true
            }
        } else {
            warn!(path = %path, "Layer has no SRS, assuming WGS84");
            false
        };

        let wgs84 = SpatialRef::from_epsg(4326)?;

        // Read all features from the layer
        let mut features = Vec::new();
        let mut stats = ReaderStats::default();

        for gdal_feature in layer.features() {
            // Transform geometry to WGS84 if needed
            // Note: GDAL's transform_to modifies geometry in-place via &mut self
            if needs_transform {
                if let Some(geometry) = gdal_feature.geometry() {
                    // geometry() returns a mutable reference, transform_to() uses &mut self
                    if let Err(e) = geometry.transform_to(&wgs84) {
                        warn!(error = %e, "Failed to transform feature geometry to WGS84, skipping");
                        continue;
                    }
                    // Geometry is already transformed in-place, continue processing
                }
            }

            match Feature::from_gdal_feature(&gdal_feature) {
                Ok(feature) => {
                    // Update statistics
                    match feature.geometry_type {
                        GeometryType::Point => stats.point_count += 1,
                        GeometryType::LineString => stats.linestring_count += 1,
                        GeometryType::Polygon => stats.polygon_count += 1,
                    }

                    debug!(
                        geometry_type = ?feature.geometry_type,
                        coords_count = feature.geometry.len(),
                        "Feature loaded"
                    );

                    features.push(feature);
                }
                Err(e) => {
                    warn!(error = %e, "Skipping invalid feature");
                }
            }
        }

        info!(
            path = %path,
            count = features.len(),
            points = stats.point_count,
            linestrings = stats.linestring_count,
            polygons = stats.polygon_count,
            "Source loaded"
        );

        Ok(features)
    }

    /// Read features from all sources configured in the configuration.
    ///
    /// # Arguments
    /// * `config` - Configuration with list of input sources
    ///
    /// # Returns
    /// * `Result<Vec<Feature>>` - All features from all sources combined
    ///
    /// # Errors
    /// * File not found or not readable (depending on error_handling mode)
    /// * GDAL errors
    pub fn read_all_sources(config: &Config) -> Result<Vec<Feature>> {
        let mut all_features = Vec::new();
        let mut total_stats = ReaderStats::default();

        info!(
            source_count = config.inputs.len(),
            error_handling = %config.error_handling,
            "Starting multi-source reading"
        );

        for (idx, input) in config.inputs.iter().enumerate() {
            info!(
                source_index = idx + 1,
                source_count = config.inputs.len(),
                "Loading source"
            );

            match Self::read_file_source(input) {
                Ok(features) => {
                    let count = features.len();

                    // Update statistics
                    for feature in &features {
                        match feature.geometry_type {
                            GeometryType::Point => total_stats.point_count += 1,
                            GeometryType::LineString => total_stats.linestring_count += 1,
                            GeometryType::Polygon => total_stats.polygon_count += 1,
                        }
                    }

                    info!(
                        source_index = idx + 1,
                        feature_count = count,
                        points = features
                            .iter()
                            .filter(|f| f.geometry_type == GeometryType::Point)
                            .count(),
                        linestrings = features
                            .iter()
                            .filter(|f| f.geometry_type == GeometryType::LineString)
                            .count(),
                        polygons = features
                            .iter()
                            .filter(|f| f.geometry_type == GeometryType::Polygon)
                            .count(),
                        "Source loaded successfully"
                    );

                    all_features.extend(features);
                }
                Err(e) => {
                    warn!(
                        source_index = idx + 1,
                        error = %e,
                        "Failed to load source"
                    );

                    // In fail-fast mode, propagate the error immediately
                    if config.error_handling == "fail-fast" {
                        return Err(e);
                    }
                    // In continue mode, log and continue with next source
                }
            }
        }

        info!(
            total_features = all_features.len(),
            points = total_stats.point_count,
            linestrings = total_stats.linestring_count,
            polygons = total_stats.polygon_count,
            "All sources loaded"
        );

        if all_features.is_empty() {
            warn!("No features loaded from any source");
        }

        Ok(all_features)
    }
}

/// Geometry type enumeration for features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryType {
    Point,
    LineString,
    Polygon,
}

/// Feature data structure with geometry and attributes.
/// Coordinates are stored in WGS84 (EPSG:4326) as (longitude, latitude) pairs.
#[derive(Debug, Clone)]
pub struct Feature {
    /// Type of geometry (Point, LineString, or Polygon)
    pub geometry_type: GeometryType,
    /// Coordinates in WGS84 (longitude, latitude)
    pub geometry: Vec<(f64, f64)>,
    /// Feature attributes (key-value pairs)
    pub attributes: HashMap<String, String>,
}

impl Feature {
    /// Convert a GDAL feature to internal Feature representation.
    ///
    /// # Arguments
    /// * `gdal_feature` - GDAL feature to convert
    ///
    /// # Returns
    /// * `Result<Feature>` - Converted feature or error
    ///
    /// # Errors
    /// * Unsupported geometry type (not Point, LineString, or Polygon)
    /// * Invalid geometry structure
    pub fn from_gdal_feature(gdal_feature: &gdal::vector::Feature) -> Result<Self> {
        // 1. Extract and validate geometry type
        let geometry = gdal_feature
            .geometry()
            .ok_or_else(|| anyhow!("Feature has no geometry"))?;

        let geometry_type = match geometry.geometry_type() {
            OGRwkbGeometryType::wkbPoint => GeometryType::Point,
            OGRwkbGeometryType::wkbLineString => GeometryType::LineString,
            OGRwkbGeometryType::wkbPolygon => GeometryType::Polygon,
            other => return Err(anyhow!("Unsupported geometry type: {:?}", other)),
        };

        // 2. Extract coordinates
        let geometry_coords = Self::extract_coordinates(geometry, geometry_type)?;

        // 3. Extract all attributes from feature fields
        let mut attributes = HashMap::new();

        for (field_name, field_value) in gdal_feature.fields() {
            // Convert field value to string representation
            let value_str = match field_value {
                Some(gdal::vector::FieldValue::StringValue(s)) => s.to_string(),
                Some(gdal::vector::FieldValue::IntegerValue(i)) => i.to_string(),
                Some(gdal::vector::FieldValue::Integer64Value(i)) => i.to_string(),
                Some(gdal::vector::FieldValue::RealValue(r)) => r.to_string(),
                Some(gdal::vector::FieldValue::DateValue(d)) => format!("{:?}", d),
                Some(gdal::vector::FieldValue::DateTimeValue(dt)) => format!("{:?}", dt),
                // Handle list types by converting to JSON-like string
                Some(gdal::vector::FieldValue::IntegerListValue(list)) => {
                    format!(
                        "[{}]",
                        list.iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
                Some(gdal::vector::FieldValue::Integer64ListValue(list)) => {
                    format!(
                        "[{}]",
                        list.iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
                Some(gdal::vector::FieldValue::RealListValue(list)) => {
                    format!(
                        "[{}]",
                        list.iter()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
                Some(gdal::vector::FieldValue::StringListValue(list)) => {
                    format!(
                        "[{}]",
                        list.iter()
                            .map(|s| format!("\"{}\"", s))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                }
                None => String::new(),
            };

            attributes.insert(field_name.to_string(), value_str);
        }

        // Log debug information about the feature
        debug!(
            geometry_type = ?geometry_type,
            coord_count = geometry_coords.len(),
            attr_count = attributes.len(),
            "Feature extracted from GDAL"
        );

        Ok(Feature {
            geometry_type,
            geometry: geometry_coords,
            attributes,
        })
    }

    /// Extract coordinates from a GDAL geometry based on its type.
    fn extract_coordinates(
        geometry: &gdal::vector::Geometry,
        geom_type: GeometryType,
    ) -> Result<Vec<(f64, f64)>> {
        match geom_type {
            GeometryType::Point => {
                let (x, y, _) = geometry.get_point(0);
                Ok(vec![(x, y)])
            }
            GeometryType::LineString => {
                let point_count = geometry.point_count();
                let mut coords = Vec::with_capacity(point_count);
                for i in 0..point_count {
                    let (x, y, _) = geometry.get_point(i as i32);
                    coords.push((x, y));
                }
                Ok(coords)
            }
            GeometryType::Polygon => {
                // For polygons, extract exterior ring coordinates
                // (simplification for Story 5.3 - Epic 6 will handle holes/multi-rings)
                let geometry_count = geometry.geometry_count();
                if geometry_count == 0 {
                    return Err(anyhow!("Polygon has no rings (invalid geometry)"));
                }

                let ring = geometry.get_geometry(0);
                let point_count = ring.point_count();
                if point_count == 0 {
                    return Err(anyhow!("Polygon exterior ring has no points"));
                }
                let mut coords = Vec::with_capacity(point_count);
                for i in 0..point_count {
                    let (x, y, _) = ring.get_point(i as i32);
                    coords.push((x, y));
                }
                Ok(coords)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_geometry_type_enum() {
        // Test GeometryType enum variants exist
        // This test will fail until GeometryType is implemented
        let _point = GeometryType::Point;
        let _linestring = GeometryType::LineString;
        let _polygon = GeometryType::Polygon;

        // Test PartialEq
        assert_eq!(GeometryType::Point, GeometryType::Point);
        assert_ne!(GeometryType::Point, GeometryType::LineString);
    }

    #[test]
    fn test_feature_struct_fields() {
        // Test Feature struct has required fields
        // This test will fail until Feature is properly implemented
        let mut attributes = HashMap::new();
        attributes.insert("Type".to_string(), "0x0100".to_string());
        attributes.insert("Label".to_string(), "Test Point".to_string());
        attributes.insert("EndLevel".to_string(), "3".to_string());

        let feature = Feature {
            geometry_type: GeometryType::Point,
            geometry: vec![(2.3488, 48.8534)], // Paris coordinates
            attributes,
        };

        assert_eq!(feature.geometry_type, GeometryType::Point);
        assert_eq!(feature.geometry.len(), 1);
        assert_eq!(feature.geometry[0], (2.3488, 48.8534));
        assert_eq!(feature.attributes.get("Type"), Some(&"0x0100".to_string()));
        assert_eq!(
            feature.attributes.get("Label"),
            Some(&"Test Point".to_string())
        );
        assert_eq!(feature.attributes.get("EndLevel"), Some(&"3".to_string()));
    }

    #[test]
    fn test_feature_multiple_coordinates() {
        // Test Feature can hold multiple coordinates (for LineString/Polygon)
        let coords = vec![(2.3488, 48.8534), (2.3500, 48.8550), (2.3520, 48.8570)];

        let feature = Feature {
            geometry_type: GeometryType::LineString,
            geometry: coords.clone(),
            attributes: HashMap::new(),
        };

        assert_eq!(feature.geometry_type, GeometryType::LineString);
        assert_eq!(feature.geometry.len(), 3);
        assert_eq!(feature.geometry, coords);
    }

    #[test]
    fn test_feature_empty_attributes() {
        // Test Feature can be created with empty attributes
        let feature = Feature {
            geometry_type: GeometryType::Polygon,
            geometry: vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.0, 0.0)],
            attributes: HashMap::new(),
        };

        assert_eq!(feature.geometry_type, GeometryType::Polygon);
        assert!(feature.attributes.is_empty());
    }
}
