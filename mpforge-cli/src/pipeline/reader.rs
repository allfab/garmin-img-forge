//! Source data reading from GDAL-compatible formats.

use crate::config::{Config, InputSource};
use anyhow::{anyhow, Context, Result};
use gdal::spatial_ref::SpatialRef;
use gdal::vector::{LayerAccess, OGRwkbGeometryType};
use gdal::Dataset;
use rstar::{RTree, RTreeObject, AABB};
use std::collections::{BTreeMap, HashMap};
use tracing::{debug, info, instrument, trace, warn};

/// Statistics for source reading.
#[derive(Debug, Default)]
struct ReaderStats {
    point_count: usize,
    linestring_count: usize,
    polygon_count: usize,
}

/// Maximum number of source names to track per unsupported type.
/// Story 6.6 - Code Review M1 Fix: Prevent unbounded Vec growth.
const MAX_SOURCES_TRACKED: usize = 10;

/// Entry for tracking unsupported geometry type occurrences.
/// Story 6.6 - Code Review M1 Fix: Added total_sources to track all sources even when Vec is truncated.
#[derive(Debug, Default, Clone)]
pub struct UnsupportedTypeEntry {
    pub count: usize,
    pub sources: Vec<String>,
    /// Total number of distinct source files (may exceed sources.len() if truncated)
    pub total_sources: usize,
}

/// Statistics for unsupported geometry types filtered during reading.
/// Story 6.6 - Tracks count and source files for each unsupported geometry type.
/// Code Review M3 Fix: Use BTreeMap for deterministic iteration order.
#[derive(Debug, Default, Clone)]
pub struct UnsupportedTypeStats {
    pub by_type: BTreeMap<String, UnsupportedTypeEntry>,
}

impl UnsupportedTypeStats {
    /// Record an unsupported geometry type occurrence.
    /// Story 6.6 - Code Review M1 Fix: Limit sources Vec to MAX_SOURCES_TRACKED.
    /// Code Review M5 Fix: Added tracing instrumentation.
    #[instrument(skip(self))]
    pub fn record(&mut self, type_name: String, source_name: String) {
        let entry = self.by_type.entry(type_name).or_default();
        entry.count += 1;
        if !entry.sources.contains(&source_name) {
            entry.total_sources += 1;
            if entry.sources.len() < MAX_SOURCES_TRACKED {
                entry.sources.push(source_name);
            }
        }
    }

    pub fn total(&self) -> usize {
        self.by_type.values().map(|e| e.count).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_type.is_empty()
    }

    /// Merge another UnsupportedTypeStats into this one.
    /// Story 6.6 - Code Review H3 Fix: Warn on duplicate sources to detect double-counting.
    /// Code Review M5 Fix: Added tracing instrumentation.
    #[instrument(skip(self, other))]
    pub fn merge(&mut self, other: &UnsupportedTypeStats) {
        for (type_name, entry) in &other.by_type {
            let target = self.by_type.entry(type_name.clone()).or_default();
            target.count += entry.count;

            // H3 Fix: Warn if merging duplicate sources (possible double-counting)
            let mut new_sources_count = 0;
            for source in &entry.sources {
                if target.sources.contains(source) {
                    warn!(
                        type_name = %type_name,
                        source = %source,
                        "Merging unsupported type stats with duplicate source - possible double-counting"
                    );
                } else {
                    new_sources_count += 1;
                    if target.sources.len() < MAX_SOURCES_TRACKED {
                        target.sources.push(source.clone());
                    }
                }
            }

            // Update total_sources count
            target.total_sources += new_sources_count;
        }
    }
}

/// Feature envelope for R-tree indexing.
///
/// Wraps a feature with its spatial bounding box for efficient spatial queries.
#[derive(Debug, Clone)]
pub struct FeatureEnvelope {
    /// Feature unique index in the global feature vector.
    pub feature_id: usize,
    /// Bounding box of the feature geometry.
    pub bbox: AABB<[f64; 2]>,
}

impl RTreeObject for FeatureEnvelope {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.bbox
    }
}

/// Spatial index for efficient tile-based queries.
///
/// Uses R-tree data structure to enable O(log n + k) spatial queries
/// instead of O(n) naive iteration over all features.
#[derive(Debug)]
pub struct RTreeIndex {
    tree: RTree<FeatureEnvelope>,
    /// Total bounding box of all indexed features (for grid calculation).
    global_bbox: AABB<[f64; 2]>,
}

impl RTreeIndex {
    /// Build R-tree index from feature vector.
    ///
    /// # Arguments
    /// * `features` - All features from all sources
    ///
    /// # Returns
    /// * `Result<RTreeIndex>` - R-tree index with all features indexed by bounding box
    ///
    /// # Errors
    /// * Currently infallible - Result signature maintained for API consistency and future extensibility
    ///   (e.g., validation of bbox validity, memory allocation failures in extreme cases)
    pub fn build(features: &[Feature]) -> Result<Self> {
        // Handle empty feature vector
        if features.is_empty() {
            info!("Building R-tree index from 0 features");
            let tree = RTree::new();
            // Note: global_bbox is set to [0,0]->[0,0] for empty index (invalid but consistent)
            // Callers MUST check tree.is_empty() before using global_bbox for grid calculation
            let global_bbox = AABB::from_corners([0.0, 0.0], [0.0, 0.0]);
            return Ok(RTreeIndex { tree, global_bbox });
        }

        let mut envelopes = Vec::with_capacity(features.len());
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for (id, feature) in features.iter().enumerate() {
            // Calculate feature bounding box
            let (bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y) =
                Self::calculate_feature_bbox(feature);

            // Update global bbox
            min_x = min_x.min(bbox_min_x);
            min_y = min_y.min(bbox_min_y);
            max_x = max_x.max(bbox_max_x);
            max_y = max_y.max(bbox_max_y);

            // Create envelope (with sanity check for valid bbox)
            debug_assert!(
                bbox_min_x <= bbox_max_x && bbox_min_y <= bbox_max_y,
                "Invalid bbox for feature {}: min ({}, {}) > max ({}, {})",
                id,
                bbox_min_x,
                bbox_min_y,
                bbox_max_x,
                bbox_max_y
            );
            let aabb = AABB::from_corners([bbox_min_x, bbox_min_y], [bbox_max_x, bbox_max_y]);
            envelopes.push(FeatureEnvelope {
                feature_id: id,
                bbox: aabb,
            });
        }

        let tree = RTree::bulk_load(envelopes);
        let global_bbox = AABB::from_corners([min_x, min_y], [max_x, max_y]);

        info!(
            features = features.len(),
            bbox_min_x = min_x,
            bbox_min_y = min_y,
            bbox_max_x = max_x,
            bbox_max_y = max_y,
            "R-tree index built"
        );

        Ok(RTreeIndex { tree, global_bbox })
    }

    /// Calculate bounding box of a feature.
    ///
    /// Note: Manual iteration used instead of geo::BoundingRect because Feature stores
    /// coordinates as Vec<(f64, f64)> rather than geo::Geometry types. This is optimal
    /// for the current data structure (single-pass O(n) over coordinates).
    ///
    /// # Arguments
    /// * `feature` - Feature to calculate bbox for
    ///
    /// # Returns
    /// * `(min_x, min_y, max_x, max_y)` - Bounding box coordinates
    fn calculate_feature_bbox(feature: &Feature) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for &(x, y) in &feature.geometry {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        (min_x, min_y, max_x, max_y)
    }

    /// Query features intersecting a bounding box.
    ///
    /// # Arguments
    /// * `query_bbox` - Bounding box to query (typically a tile bbox)
    ///
    /// # Returns
    /// * `Vec<usize>` - Feature IDs whose bboxes intersect the query bbox
    pub fn query_intersecting(&self, query_bbox: &AABB<[f64; 2]>) -> Vec<usize> {
        let candidates: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(query_bbox)
            .map(|envelope| envelope.feature_id)
            .collect();

        trace!(candidates = candidates.len(), "R-tree query completed");

        candidates
    }

    /// Get the global bounding box of all indexed features.
    ///
    /// # Returns
    /// * `AABB<[f64; 2]>` - Global bounding box
    pub fn global_bbox(&self) -> AABB<[f64; 2]> {
        self.global_bbox
    }

    /// Get the number of features in the R-tree index.
    ///
    /// # Returns
    /// * `usize` - Number of indexed features
    pub fn tree_size(&self) -> usize {
        self.tree.size()
    }
}


/// Extract a human-readable source name from a path/connection string.
/// Story 6.6 - Code Review H2 Fix: Handle PostGIS, URLs, and file paths properly.
fn extract_source_name(path: &str) -> String {
    if path.starts_with("PG:") || path.starts_with("PostgreSQL:") {
        // PostGIS connection string - extract database name
        path.split("dbname=")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("PostGIS")
            .to_string()
    } else if path.starts_with("http://") || path.starts_with("https://") {
        // URL - extract filename from path
        path.rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or("remote_source")
            .to_string()
    } else {
        // File path - extract filename
        std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown_source".to_string())
    }
}

/// Reads features from GDAL sources.
///
/// This is a stateless utility struct - all methods are static/associated functions.
pub struct SourceReader;

impl SourceReader {
    /// Read features from a file-based GDAL source (Shapefile, GeoPackage, etc.).
    ///
    /// Uses default error handling mode (fail-fast) for layer loading failures.
    ///
    /// # Arguments
    /// * `input` - InputSource configuration with path and optional layer/layers
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, UnsupportedTypeStats)>` - Features and unsupported type statistics
    ///
    /// # Errors
    /// * File not found or not readable
    /// * GDAL driver not available
    /// * Invalid layer name
    pub fn read_file_source(input: &InputSource) -> Result<(Vec<Feature>, UnsupportedTypeStats)> {
        Self::read_file_source_with_error_handling(input, "fail-fast")
    }

    /// Read features from a file-based GDAL source with configurable error handling.
    ///
    /// # Arguments
    /// * `input` - InputSource configuration with path and optional layer/layers
    /// * `error_handling` - Error handling mode: "continue" or "fail-fast"
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, UnsupportedTypeStats)>` - Features and unsupported type statistics
    ///
    /// # Errors
    /// * File not found or not readable
    /// * GDAL driver not available
    /// * Invalid layer name (in fail-fast mode)
    fn read_file_source_with_error_handling(
        input: &InputSource,
        error_handling: &str,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats)> {
        let path = input
            .path
            .as_ref()
            .ok_or_else(|| anyhow!("No path specified for file source"))?;

        info!("Loading source: {}", path);

        // Open GDAL dataset
        let dataset =
            Dataset::open(path).with_context(|| format!("Failed to open dataset: {}", path))?;

        let wgs84 = SpatialRef::from_epsg(4326)?;
        let mut all_features = Vec::new();
        let mut all_unsupported = UnsupportedTypeStats::default();

        // Handle multi-layer or single-layer loading
        if let Some(layers) = &input.layers {
            if layers.is_empty() {
                // Empty list: use default layer 0 with warning
                warn!(path = %path, "Empty layers list, using default layer 0");
                let (features, unsupported) = Self::load_layer_by_index(&dataset, 0, path, &wgs84)?;
                all_features.extend(features);
                all_unsupported.merge(&unsupported);
            } else {
                // Multi-layers: iterate over all configured layers
                for layer_name in layers {
                    info!(path = %path, layer = %layer_name, "Loading layer");
                    match Self::load_layer_by_name(&dataset, layer_name, path, &wgs84) {
                        Ok((features, unsupported)) => {
                            info!(
                                path = %path,
                                layer = %layer_name,
                                count = features.len(),
                                "Layer loaded"
                            );
                            all_features.extend(features);
                            all_unsupported.merge(&unsupported);
                        }
                        Err(e) => {
                            warn!(
                                path = %path,
                                layer = %layer_name,
                                error = %e,
                                "Failed to load layer"
                            );

                            // Apply error_handling mode for layer failures
                            if error_handling == "fail-fast" {
                                return Err(e);
                            }
                            // In continue mode: log and continue with next layer
                        }
                    }
                }
            }
        } else {
            // None: default behavior (load layer 0 only, no warning)
            let (features, unsupported) = Self::load_layer_by_index(&dataset, 0, path, &wgs84)?;
            all_features.extend(features);
            all_unsupported.merge(&unsupported);
        }

        // Log total statistics
        let mut total_stats = ReaderStats::default();
        for feature in &all_features {
            match feature.geometry_type {
                GeometryType::Point => total_stats.point_count += 1,
                GeometryType::LineString => total_stats.linestring_count += 1,
                GeometryType::Polygon => total_stats.polygon_count += 1,
            }
        }

        info!(
            path = %path,
            count = all_features.len(),
            points = total_stats.point_count,
            linestrings = total_stats.linestring_count,
            polygons = total_stats.polygon_count,
            "Source loaded"
        );

        Ok((all_features, all_unsupported))
    }

    /// Load features from a layer by index.
    ///
    /// Helper function to load features from a specific layer by index (e.g., layer 0).
    /// Used for default layer loading.
    fn load_layer_by_index(
        dataset: &Dataset,
        layer_index: usize,
        path: &str,
        wgs84: &SpatialRef,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats)> {
        let mut layer = dataset.layer(layer_index).with_context(|| {
            format!(
                "Failed to access layer {} in dataset: {}",
                layer_index, path
            )
        })?;

        Self::load_features_from_layer(&mut layer, path, wgs84)
    }

    /// Load features from a layer by name.
    ///
    /// Helper function to load features from a specific layer by name.
    /// Used for multi-layer GeoPackage loading.
    fn load_layer_by_name(
        dataset: &Dataset,
        layer_name: &str,
        path: &str,
        wgs84: &SpatialRef,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats)> {
        let mut layer = dataset
            .layer_by_name(layer_name)
            .with_context(|| format!("Layer '{}' not found in dataset: {}", layer_name, path))?;

        Self::load_features_from_layer(&mut layer, path, wgs84)
    }

    /// Load all features from a given layer with SRS transformation.
    ///
    /// Core feature loading logic extracted to avoid duplication.
    /// Handles SRS transformation to WGS84 if needed.
    /// Returns features and statistics about unsupported geometry types filtered.
    fn load_features_from_layer(
        layer: &mut gdal::vector::Layer,
        path: &str,
        wgs84: &SpatialRef,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats)> {
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

        // Read all features from the layer
        let mut features = Vec::new();
        let mut unsupported_stats = UnsupportedTypeStats::default();

        // Story 6.6 - Code Review H2 Fix: Extract source name properly for PostGIS, URLs, etc.
        let source_name = extract_source_name(path);

        for gdal_feature in layer.features() {
            // Transform geometry to WGS84 if needed
            if needs_transform {
                if let Some(geometry) = gdal_feature.geometry() {
                    if let Err(e) = geometry.transform_to(wgs84) {
                        warn!(error = %e, "Failed to transform feature geometry to WGS84, skipping");
                        continue;
                    }
                }
            }

            match Feature::from_gdal_feature(&gdal_feature) {
                Ok(Some(feature)) => {
                    debug!(
                        geometry_type = ?feature.geometry_type,
                        coords_count = feature.geometry.len(),
                        "Feature loaded"
                    );
                    features.push(feature);
                }
                Ok(None) => {
                    // Unsupported geometry type filtered - record for stats
                    // Story 6.6 - Code Review H1 Fix: Use explicit mapping instead of Debug format
                    if let Some(geom) = gdal_feature.geometry() {
                        let type_name = match geom.geometry_type() {
                            OGRwkbGeometryType::wkbMultiPoint => "MultiPoint".to_string(),
                            OGRwkbGeometryType::wkbMultiLineString => "MultiLineString".to_string(),
                            OGRwkbGeometryType::wkbMultiPolygon => "MultiPolygon".to_string(),
                            OGRwkbGeometryType::wkbGeometryCollection => "GeometryCollection".to_string(),
                            // Fallback for any other types using Debug format
                            other => {
                                let debug_str = format!("{:?}", other);
                                debug_str.strip_prefix("wkb").unwrap_or(&debug_str).to_string()
                            }
                        };
                        unsupported_stats.record(type_name, source_name.clone());
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Skipping invalid feature");
                }
            }
        }

        Ok((features, unsupported_stats))
    }

    /// Read features from all sources and build R-tree spatial index.
    ///
    /// # Arguments
    /// * `config` - Configuration with list of input sources
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, RTreeIndex, UnsupportedTypeStats)>` - All features, R-tree index, and unsupported type stats
    ///
    /// # Errors
    /// * File not found or not readable (depending on error_handling mode)
    /// * GDAL errors
    /// * R-tree construction errors (should never happen in practice)
    pub fn read_all_sources(config: &Config) -> Result<(Vec<Feature>, RTreeIndex, UnsupportedTypeStats)> {
        let mut all_features = Vec::new();
        let mut total_stats = ReaderStats::default();
        let mut all_unsupported = UnsupportedTypeStats::default();

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

            match Self::read_file_source_with_error_handling(input, &config.error_handling) {
                Ok((features, unsupported)) => {
                    let count = features.len();
                    all_unsupported.merge(&unsupported);

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

        // Story 6.6 - Task 4: Log unique INFO summary for unsupported geometry types
        // Code Review M2 Fix: Limit sources displayed in log to avoid pollution
        if all_unsupported.total() > 0 {
            let breakdown: Vec<String> = all_unsupported
                .by_type
                .iter()
                .map(|(type_name, entry)| {
                    let sources_display = if entry.sources.len() > 3 {
                        format!(
                            "{} et {} autres",
                            entry.sources[..3].join(", "),
                            entry.total_sources - 3
                        )
                    } else {
                        entry.sources.join(", ")
                    };
                    format!("{}: {} ({})", type_name, entry.count, sources_display)
                })
                .collect();
            info!(
                total = all_unsupported.total(),
                breakdown = %breakdown.join("; "),
                "Unsupported geometry types filtered"
            );
        }

        info!(
            total_features = all_features.len(),
            points = total_stats.point_count,
            linestrings = total_stats.linestring_count,
            polygons = total_stats.polygon_count,
            "All sources loaded, building R-tree index"
        );

        if all_features.is_empty() {
            warn!("No features loaded from any source");
        }

        // Build R-tree spatial index (currently infallible, but Result kept for API consistency)
        let rtree = RTreeIndex::build(&all_features)?;

        Ok((all_features, rtree, all_unsupported))
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
    /// * `Result<Option<Feature>>` - Some(feature) for supported types, None for unsupported types
    ///
    /// # Errors
    /// * Invalid geometry structure
    pub fn from_gdal_feature(gdal_feature: &gdal::vector::Feature) -> Result<Option<Self>> {
        // 1. Extract and validate geometry type
        let geometry = gdal_feature
            .geometry()
            .ok_or_else(|| anyhow!("Feature has no geometry"))?;

        let geometry_type = match geometry.geometry_type() {
            OGRwkbGeometryType::wkbPoint => GeometryType::Point,
            OGRwkbGeometryType::wkbLineString => GeometryType::LineString,
            OGRwkbGeometryType::wkbPolygon => GeometryType::Polygon,
            other => {
                debug!(geometry_type = ?other, "Skipping unsupported geometry type");
                return Ok(None);
            }
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

        Ok(Some(Feature {
            geometry_type,
            geometry: geometry_coords,
            attributes,
        }))
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
