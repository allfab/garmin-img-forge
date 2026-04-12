//! Source data reading from GDAL-compatible formats.

use crate::config::{Config, InputSource};
use anyhow::{anyhow, Context, Result};
use gdal::spatial_ref::{CoordTransform, SpatialRef};
use gdal::vector::{LayerAccess, OGRwkbGeometryType};
use gdal::Dataset;
use rstar::{RTree, RTreeObject, AABB};
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, info, instrument, trace, warn};

/// Pre-built spatial filter geometry with metadata for thread-safe sharing.
#[derive(Debug, Clone)]
pub struct SpatialFilterGeometry {
    /// WKB-encoded union+buffer geometry.
    pub wkb: Vec<u8>,
    /// SRS definition string (e.g., "EPSG:2154") of the source shapefile.
    pub srs: Option<String>,
    /// Envelope [min_x, min_y, max_x, max_y] in the source SRS, for fast rejection.
    pub envelope: [f64; 4],
}

/// Binary tree union of geometries: O(n log n) instead of O(n²) incremental.
fn binary_tree_union(mut geoms: Vec<gdal::vector::Geometry>) -> Result<gdal::vector::Geometry> {
    while geoms.len() > 1 {
        let mut next = Vec::with_capacity((geoms.len() + 1) / 2);
        let mut i = 0;
        while i + 1 < geoms.len() {
            let a = &geoms[i];
            let b = &geoms[i + 1];
            let merged = a.union(b)
                .ok_or_else(|| anyhow!("Geometry union failed at binary tree merge step"))?;
            next.push(merged);
            i += 2;
        }
        if i < geoms.len() {
            // Odd element: carry forward
            next.push(geoms.pop().unwrap());
        }
        geoms = next;
    }
    Ok(geoms.into_iter().next().unwrap())
}

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

/// Entry for tracking multi-geometry decomposition occurrences.
/// Story 6.7 - Subtask 1.1: Track count of decomposed multi-geometries per type.
#[derive(Debug, Default, Clone)]
pub struct MultiGeometryDecomposedEntry {
    pub count: usize,
}

/// Statistics for multi-geometries decomposed during reading.
/// Story 6.7 - Subtask 1.2: Track decomposition by geometry type (MultiPoint, MultiLineString, MultiPolygon).
/// Uses BTreeMap for deterministic iteration order (Story 6.6 Code Review M3 learning).
#[derive(Debug, Default, Clone)]
pub struct MultiGeometryStats {
    pub by_type: BTreeMap<String, MultiGeometryDecomposedEntry>,
}

impl MultiGeometryStats {
    /// Record a multi-geometry decomposition occurrence.
    /// Story 6.7 - Subtask 1.3: Increment count for a given multi-geometry type.
    /// Code Review M5 learning: Added tracing instrumentation.
    #[instrument(skip(self))]
    pub fn record(&mut self, type_name: String) {
        let entry = self.by_type.entry(type_name).or_default();
        entry.count += 1;
    }

    /// Get total count of all decomposed multi-geometries across all types.
    /// Story 6.7 - Subtask 1.4: Sum all counts for reporting.
    pub fn total(&self) -> usize {
        self.by_type.values().map(|e| e.count).sum()
    }

    /// Check if any multi-geometries were decomposed.
    /// Story 6.7 - Subtask 1.5: Used to conditionally log/report stats.
    pub fn is_empty(&self) -> bool {
        self.by_type.is_empty()
    }

    /// Merge another MultiGeometryStats into this one.
    /// Code Review M2 Fix: O(T) merge instead of O(N) loop-and-record pattern.
    /// Mirrors UnsupportedTypeStats::merge() for API symmetry.
    #[instrument(skip(self, other))]
    pub fn merge(&mut self, other: &MultiGeometryStats) {
        for (type_name, entry) in &other.by_type {
            let target = self.by_type.entry(type_name.clone()).or_default();
            target.count += entry.count;
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

/// Helper enum for layer selection (by index or by name).
enum LayerSelector {
    Index(usize),
    Name(String),
}

/// Global bounding box computed from all source extents.
///
/// Lightweight structure returned by `SourceReader::scan_extents()`.
/// Does not depend on R-tree or any feature loading.
#[derive(Debug, Clone)]
pub struct GlobalExtent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    /// Number of layers successfully scanned (may differ from file count for multi-layer sources).
    pub layer_count: usize,
}

impl GlobalExtent {
    /// Convert to [min_x, min_y, max_x, max_y] array for grid generation.
    pub fn to_bbox(&self) -> [f64; 4] {
        [self.min_x, self.min_y, self.max_x, self.max_y]
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
    /// * `Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)>` - Features, unsupported type stats, and multi-geometry stats
    ///
    /// # Errors
    /// * File not found or not readable
    /// * GDAL driver not available
    /// * Invalid layer name
    /// NOTE: this helper does NOT activate Config.default_dedup_by_field (the
    /// Config isn't available here). Used by tests and callers that work
    /// feature-by-feature without pipeline-level config. If dedup is needed,
    /// go through `read_all_features` / `read_features_for_tile`.
    pub fn read_file_source(
        input: &InputSource,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
        Self::read_file_source_with_error_handling(input, "fail-fast", None)
    }

    /// Read features from a file-based GDAL source with configurable error handling.
    ///
    /// # Arguments
    /// * `input` - InputSource configuration with path and optional layer/layers
    /// * `error_handling` - Error handling mode: "continue" or "fail-fast"
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)>` - Features, unsupported type stats, and multi-geometry stats
    ///
    /// # Errors
    /// * File not found or not readable
    /// * GDAL driver not available
    /// * Invalid layer name (in fail-fast mode)
    fn read_file_source_with_error_handling(
        input: &InputSource,
        error_handling: &str,
        dedup_by_field: Option<&str>,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
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
        let mut all_multi_geom = MultiGeometryStats::default(); // Story 6.7 - Subtask 4.1

        // Handle multi-layer or single-layer loading
        if let Some(layers) = &input.layers {
            if layers.is_empty() {
                // Empty list: use default layer 0 with warning
                warn!(path = %path, "Empty layers list, using default layer 0");
                let (features, unsupported, multi_geom) =
                    Self::load_layer_by_index(&dataset, 0, path, &wgs84, input.source_srs.as_deref(), input.target_srs.as_deref(), input.attribute_filter.as_deref(), input.layer_alias.as_deref(), dedup_by_field)?;
                all_features.extend(features);
                all_unsupported.merge(&unsupported);
                // Code Review M2 Fix: Use merge() for O(T) instead of O(N) loop
                all_multi_geom.merge(&multi_geom);
            } else {
                // Multi-layers: iterate over all configured layers
                for layer_name in layers {
                    info!(path = %path, layer = %layer_name, "Loading layer");
                    match Self::load_layer_by_name(&dataset, layer_name, path, &wgs84, input.source_srs.as_deref(), input.target_srs.as_deref(), input.attribute_filter.as_deref(), input.layer_alias.as_deref(), dedup_by_field) {
                        Ok((features, unsupported, multi_geom)) => {
                            info!(
                                path = %path,
                                layer = %layer_name,
                                count = features.len(),
                                "Layer loaded"
                            );
                            all_features.extend(features);
                            all_unsupported.merge(&unsupported);
                            // Code Review M2 Fix: Use merge() for O(T) instead of O(N) loop
                            all_multi_geom.merge(&multi_geom);
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
            let (features, unsupported, multi_geom) =
                Self::load_layer_by_index(&dataset, 0, path, &wgs84, input.source_srs.as_deref(), input.target_srs.as_deref(), input.attribute_filter.as_deref(), input.layer_alias.as_deref(), dedup_by_field)?;
            all_features.extend(features);
            all_unsupported.merge(&unsupported);
            // Code Review M2 Fix: Use merge() for O(T) instead of O(N) loop
            all_multi_geom.merge(&multi_geom);
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

        Ok((all_features, all_unsupported, all_multi_geom))
    }

    /// Load features from a layer by index.
    ///
    /// Helper function to load features from a specific layer by index (e.g., layer 0).
    /// Used for default layer loading.
    /// Story 6.7 - Task 4: Added MultiGeometryStats to return type.
    /// Code Review H2 Fix: Added source_srs/target_srs params to propagate explicit SRS.
    fn load_layer_by_index(
        dataset: &Dataset,
        layer_index: usize,
        path: &str,
        wgs84: &SpatialRef,
        source_srs: Option<&str>,
        target_srs: Option<&str>,
        attribute_filter: Option<&str>,
        layer_alias: Option<&str>,
        dedup_by_field: Option<&str>,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
        let mut layer = dataset.layer(layer_index).with_context(|| {
            format!(
                "Failed to access layer {} in dataset: {}",
                layer_index, path
            )
        })?;

        Self::load_features_from_layer(&mut layer, path, wgs84, source_srs, target_srs, attribute_filter, layer_alias, dedup_by_field)
    }

    /// Load features from a layer by name.
    ///
    /// Helper function to load features from a specific layer by name.
    /// Used for multi-layer GeoPackage loading.
    /// Story 6.7 - Task 4: Added MultiGeometryStats to return type.
    /// Code Review H2 Fix: Added source_srs/target_srs params to propagate explicit SRS.
    fn load_layer_by_name(
        dataset: &Dataset,
        layer_name: &str,
        path: &str,
        wgs84: &SpatialRef,
        source_srs: Option<&str>,
        target_srs: Option<&str>,
        attribute_filter: Option<&str>,
        layer_alias: Option<&str>,
        dedup_by_field: Option<&str>,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
        let mut layer = dataset
            .layer_by_name(layer_name)
            .with_context(|| format!("Layer '{}' not found in dataset: {}", layer_name, path))?;

        Self::load_features_from_layer(&mut layer, path, wgs84, source_srs, target_srs, attribute_filter, layer_alias, dedup_by_field)
    }

    /// Load all features from a given layer with SRS transformation.
    ///
    /// Core feature loading logic extracted to avoid duplication.
    /// Handles SRS transformation to WGS84 if needed.
    /// Returns features and statistics about unsupported geometry types filtered and multi-geometries decomposed.
    /// Story 6.7 - Task 4: Added MultiGeometryStats to return type.
    /// Story 9.4 - Task 2: Added explicit source_srs/target_srs parameters for reprojection override.
    fn load_features_from_layer(
        layer: &mut gdal::vector::Layer,
        path: &str,
        wgs84: &SpatialRef,
        source_srs: Option<&str>,
        target_srs: Option<&str>,
        attribute_filter: Option<&str>,
        layer_alias: Option<&str>,
        dedup_by_field: Option<&str>,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
        // Apply OGR SQL attribute filter if configured
        if let Some(attr_filter) = attribute_filter {
            layer.set_attribute_filter(attr_filter)
                .with_context(|| format!("Failed to set attribute filter '{}' on: {}", attr_filter, path))?;
        }
        // Story 9.4: Build coordinate transform based on explicit SRS or auto-detect
        // Priority: 1) source_srs + target_srs explicit  2) source_srs + default WGS84  3) auto-detect
        let coord_transform: Option<CoordTransform> = if let Some(src_def) = source_srs {
            // Explicit source SRS → build CoordTransform
            let mut src = SpatialRef::from_definition(src_def)
                .with_context(|| format!("Failed to create SpatialRef from source_srs: {}", src_def))?;
            src.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
            let mut dst = if let Some(dst_def) = target_srs {
                SpatialRef::from_definition(dst_def)
                    .with_context(|| format!("Failed to create SpatialRef from target_srs: {}", dst_def))?
            } else {
                // AC2: Default target is WGS84
                SpatialRef::from_epsg(4326)?
            };
            dst.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
            info!(
                path = %path,
                source_srs = src_def,
                target_srs = target_srs.unwrap_or("EPSG:4326 (default)"),
                "Using explicit SRS reprojection"
            );
            Some(CoordTransform::new(&src, &dst)?)
        } else {
            // AC3: No explicit SRS → auto-detect via legacy_transform (below).
            // Code Review M1 Fix: Removed dead needs_transform variable — only keep warning logs.
            if let Some(spatial_ref) = layer.spatial_ref() {
                if let Ok(auth_code) = spatial_ref.auth_code() {
                    if auth_code != 4326 {
                        warn!(
                            path = %path,
                            srs = auth_code,
                            "Layer SRS is not WGS84 (EPSG:4326), transforming coordinates to WGS84"
                        );
                    }
                } else {
                    warn!(path = %path, "Layer has SRS but no authority code, assuming transformation needed");
                }
            } else {
                warn!(path = %path, "Layer has no SRS, assuming WGS84");
            }
            None
        };

        // Determine if we need legacy transform (auto-detect path)
        // Story 9.4: Pre-compute legacy CoordTransform before the feature iteration loop
        // to avoid borrow conflicts with the layer's mutable iterator.
        // Code Review M2: TraditionalGisOrder is intentionally NOT set here (unlike explicit SRS path).
        // The legacy auto-detect uses GDAL's default axis mapping because layer.spatial_ref()
        // returns a SpatialRef with GDAL's default axis order, and coordinates from
        // geometry.get_point() match that same order. Setting TraditionalGisOrder on only
        // one side would cause axis inversion (see Debug Log: axis-ordering GDAL 3.x issue).
        let legacy_transform: Option<CoordTransform> = if source_srs.is_none() {
            let needs_it = if let Some(spatial_ref) = layer.spatial_ref() {
                if let Ok(auth_code) = spatial_ref.auth_code() {
                    auth_code != 4326
                } else {
                    true
                }
            } else {
                false
            };
            if needs_it {
                if let Some(layer_srs) = layer.spatial_ref() {
                    CoordTransform::new(&layer_srs, wgs84).ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Read all features from the layer
        let mut features = Vec::new();
        let mut unsupported_stats = UnsupportedTypeStats::default();
        let mut multi_geom_stats = MultiGeometryStats::default(); // Story 6.7 - Subtask 4.1

        // Dedup (scope = this layer read, not persisted across tile calls).
        // Resolve the field index ONCE via the layer definition to avoid scanning
        // all attributes on every feature. If the field doesn't exist in this
        // layer's schema, warn once and disable dedup for this layer.
        let dedup_field_idx: Option<usize> = dedup_by_field.and_then(|name| {
            match layer.defn().field_index(name) {
                Ok(idx) => Some(idx),
                Err(_) => {
                    warn!(
                        path = %path,
                        field = name,
                        "dedup_by_field not present in layer schema, dedup disabled for this layer"
                    );
                    None
                }
            }
        });
        // Pre-size the HashSet from the layer feature count (cap at 10M to guard
        // against lying drivers or pathological layers).
        let estimated_cap = layer.feature_count().min(10_000_000) as usize;
        let mut seen_dedup_ids: HashSet<String> = HashSet::with_capacity(estimated_cap);
        let mut dedup_skipped: usize = 0;
        let mut features_seen: usize = 0;

        // Story 9.3 - Subtask 1.2/1.3: Capture layer name for rules engine matching
        // Use layer_alias if configured to override GDAL native layer name
        let layer_name = layer_alias.map(|s| s.to_string()).unwrap_or_else(|| layer.name());

        // Story 6.6 - Code Review H2 Fix: Extract source name properly for PostGIS, URLs, etc.
        let source_name = extract_source_name(path);

        for gdal_feature in layer.features() {
            features_seen += 1;
            // Dedup by attribute field, O(1) per feature (index resolved once).
            // Null / empty values keep the feature (dedup is skipped).
            // field_as_string formats int/int64 natively and delegates f64 to
            // GDAL's string cast (deterministic across reads of the same file).
            if let Some(idx) = dedup_field_idx {
                if let Ok(Some(key)) = gdal_feature.field_as_string(idx) {
                    if !key.is_empty() && !seen_dedup_ids.insert(key) {
                        dedup_skipped += 1;
                        continue;
                    }
                }
            }

            // Story 6.7 - Subtask 4.2: Detect multi-geometry BEFORE decomposition to record stats
            let is_multi_geometry = if let Some(geom) = gdal_feature.geometry() {
                let geom_type = geom.geometry_type();
                matches!(
                    geom_type,
                    OGRwkbGeometryType::wkbMultiPoint
                        | OGRwkbGeometryType::wkbMultiPoint25D
                        | OGRwkbGeometryType::wkbMultiLineString
                        | OGRwkbGeometryType::wkbMultiLineString25D
                        | OGRwkbGeometryType::wkbMultiPolygon
                        | OGRwkbGeometryType::wkbMultiPolygon25D
                )
            } else {
                false
            };

            // Subtask 3.6: Handle Vec<Feature> return type (supports multi-geometry decomposition)
            match Feature::from_gdal_feature(&gdal_feature) {
                Ok(feature_vec) => {
                    if feature_vec.is_empty() {
                        // Unsupported geometry type filtered - record for stats
                        // Story 6.7: Multi-geometries are now decomposed, so only truly unsupported types reach here
                        if let Some(geom) = gdal_feature.geometry() {
                            let type_name = match geom.geometry_type() {
                                // Story 6.7: MultiPoint/MultiLineString/MultiPolygon are now SUPPORTED (decomposed)
                                // Only GeometryCollection and truly unknown types are unsupported
                                OGRwkbGeometryType::wkbGeometryCollection
                                | OGRwkbGeometryType::wkbGeometryCollection25D => {
                                    "GeometryCollection".to_string()
                                }
                                // Fallback for any other types using Debug format
                                other => {
                                    let debug_str = format!("{:?}", other);
                                    debug_str
                                        .strip_prefix("wkb")
                                        .unwrap_or(&debug_str)
                                        .to_string()
                                }
                            };
                            unsupported_stats.record(type_name, source_name.clone());
                        }
                    } else {
                        // Story 6.7 - Subtask 4.2: Record multi-geometry decomposition stats
                        if is_multi_geometry {
                            // This was a multi-geometry that was decomposed
                            if let Some(geom) = gdal_feature.geometry() {
                                let type_name = match geom.geometry_type() {
                                    OGRwkbGeometryType::wkbMultiPoint
                                    | OGRwkbGeometryType::wkbMultiPoint25D => "MultiPoint",
                                    OGRwkbGeometryType::wkbMultiLineString
                                    | OGRwkbGeometryType::wkbMultiLineString25D => {
                                        "MultiLineString"
                                    }
                                    OGRwkbGeometryType::wkbMultiPolygon
                                    | OGRwkbGeometryType::wkbMultiPolygon25D => "MultiPolygon",
                                    _ => "Unknown", // Should not reach here
                                };
                                multi_geom_stats.record(type_name.to_string());
                            }
                        }

                        // One or more features (simple geometry returns vec![feature], multi-geometry returns vec![f1, f2, ...])
                        debug!(
                            feature_count = feature_vec.len(),
                            is_multi = is_multi_geometry,
                            "Features loaded from GDAL feature (1 = simple, N = decomposed multi-geometry)"
                        );
                        // Story 9.3 - Subtask 1.2: Set source_layer on each feature for rules engine matching
                        // Story 9.4: Apply coordinate transformation after extraction
                        let active_transform = coord_transform.as_ref().or(legacy_transform.as_ref());
                        // Code Review H1 Fix: Handle transform_coords errors instead of silently ignoring
                        features.extend(feature_vec.into_iter().filter_map(|mut f| {
                            f.source_layer = Some(layer_name.clone());
                            // Transform coordinates if needed
                            if let Some(ct) = active_transform {
                                let mut xs: Vec<f64> = f.geometry.iter().map(|(x, _)| *x).collect();
                                let mut ys: Vec<f64> = f.geometry.iter().map(|(_, y)| *y).collect();
                                if let Err(e) = ct.transform_coords(&mut xs, &mut ys, &mut []) {
                                    warn!(
                                        error = %e,
                                        path = %path,
                                        "Coordinate transform failed, skipping feature"
                                    );
                                    return None;
                                }
                                f.geometry = xs.into_iter().zip(ys).collect();
                            }
                            Some(f)
                        }));
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Skipping invalid feature");
                }
            }
        }

        if dedup_skipped > 0 {
            info!(
                path = %path,
                field = dedup_by_field.unwrap_or(""),
                skipped = dedup_skipped,
                "Deduplicated features by attribute field"
            );
        } else if dedup_field_idx.is_some() && features_seen > 0 && seen_dedup_ids.is_empty() {
            // Field exists in schema but every feature had a null/empty value.
            // Likely config mistake (wrong field name cased differently, or the
            // field is only populated on a sibling layer).
            warn!(
                path = %path,
                field = dedup_by_field.unwrap_or(""),
                features_seen,
                "dedup_by_field present in schema but empty on every feature, dedup had no effect"
            );
        }

        Ok((features, unsupported_stats, multi_geom_stats))
    }

    /// Pre-scan all source extents without loading any features.
    ///
    /// Opens each GDAL dataset, reads the layer extent via `try_get_extent()`
    /// (O(1) for shapefiles via .shx) with fallback to `get_extent()`,
    /// and computes the union of all bounding boxes.
    ///
    /// # Arguments
    /// * `config` - Configuration with list of input sources
    /// * `sf_envelopes` - Spatial filter envelopes per source index:
    ///   `(envelope_in_source_srs, srs_definition)`. When present, the source
    ///   extent is clamped (intersected) with this envelope so that sources
    ///   covering a larger area than the clipping geometry do not inflate the grid.
    ///
    /// # Returns
    /// * `Result<GlobalExtent>` - Union bounding box of all sources
    ///
    /// # Errors
    /// * No valid sources found (all failed or empty inputs)
    /// * GDAL errors (depending on error_handling mode)
    pub fn scan_extents(
        config: &Config,
        sf_envelopes: &std::collections::HashMap<usize, ([f64; 4], Option<String>)>,
    ) -> Result<GlobalExtent> {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut layer_count: usize = 0;

        info!(
            source_count = config.inputs.len(),
            "Scanning source extents (no feature loading)"
        );

        for (idx, input) in config.inputs.iter().enumerate() {
            let path = match input.path.as_ref() {
                Some(p) => p,
                None => {
                    warn!(source_index = idx + 1, "No path specified, skipping");
                    continue;
                }
            };

            let dataset = match Dataset::open(path) {
                Ok(ds) => ds,
                Err(e) => {
                    if config.error_handling == "fail-fast" {
                        return Err(e).with_context(|| {
                            format!("Failed to open dataset for extent scan: {}", path)
                        });
                    }
                    warn!(
                        source_index = idx + 1,
                        path = %path,
                        error = %e,
                        "Failed to open dataset for extent scan, skipping"
                    );
                    continue;
                }
            };

            // Collect layers to scan (same logic as read_file_source_with_error_handling)
            let layer_indices_or_names: Vec<LayerSelector> =
                if let Some(layers) = &input.layers {
                    if layers.is_empty() {
                        vec![LayerSelector::Index(0)]
                    } else {
                        layers.iter().map(|n| LayerSelector::Name(n.clone())).collect()
                    }
                } else {
                    vec![LayerSelector::Index(0)]
                };

            for selector in &layer_indices_or_names {
                let mut layer = match selector {
                    LayerSelector::Index(i) => match dataset.layer(*i) {
                        Ok(l) => l,
                        Err(e) => {
                            if config.error_handling == "fail-fast" {
                                return Err(e).with_context(|| {
                                    format!("Failed to access layer {} in: {}", i, path)
                                });
                            }
                            warn!(path = %path, layer = i, error = %e, "Failed to access layer, skipping");
                            continue;
                        }
                    },
                    LayerSelector::Name(name) => match dataset.layer_by_name(name) {
                        Ok(l) => l,
                        Err(e) => {
                            if config.error_handling == "fail-fast" {
                                return Err(e).with_context(|| {
                                    format!("Layer '{}' not found in: {}", name, path)
                                });
                            }
                            warn!(path = %path, layer = %name, error = %e, "Layer not found, skipping");
                            continue;
                        }
                    },
                };

                // Apply attribute filter if configured (affects get_extent() fallback path)
                if let Some(ref attr_filter) = input.attribute_filter {
                    let _ = layer.set_attribute_filter(attr_filter);
                }

                // Try fast extent first, fallback to full scan
                let envelope = match layer.try_get_extent() {
                    Ok(Some(env)) => env,
                    Ok(None) => {
                        debug!(path = %path, "try_get_extent() returned None, using get_extent()");
                        match layer.get_extent() {
                            Ok(env) => env,
                            Err(e) => {
                                if config.error_handling == "fail-fast" {
                                    return Err(e).with_context(|| {
                                        format!("Failed to get extent for: {}", path)
                                    });
                                }
                                warn!(path = %path, error = %e, "Failed to get extent, skipping");
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        if config.error_handling == "fail-fast" {
                            return Err(e).with_context(|| {
                                format!("Failed to get extent for: {}", path)
                            });
                        }
                        warn!(path = %path, error = %e, "try_get_extent() failed, skipping");
                        continue;
                    }
                };

                // Reproject envelope to target SRS if layer has a different SRS
                let mut bounds = [envelope.MinX, envelope.MinY, envelope.MaxX, envelope.MaxY];

                // Clamp extent to spatial filter envelope when configured.
                // The spatial filter envelope is in the filter source SRS (typically EPSG:2154).
                // The layer extent is in the layer's native SRS. When both SRS match,
                // we intersect the two bounding boxes directly. This prevents sources
                // like COURBE tiles from inflating the global extent beyond the clipping area.
                if let Some((sf_env, ref sf_srs)) = sf_envelopes.get(&idx) {
                    // Determine layer SRS for comparison
                    let layer_srs_def = input.source_srs.as_deref()
                        .map(|s| s.to_string())
                        .or_else(|| layer.spatial_ref().and_then(|srs| {
                            srs.auth_code().ok().map(|code| format!("EPSG:{}", code))
                        }));

                    let srs_match = match (&layer_srs_def, sf_srs) {
                        (Some(a), Some(b)) => a == b,
                        _ => false,
                    };

                    // Resolve the spatial filter envelope in the layer's SRS
                    let resolved_sf_env = if srs_match {
                        // Same SRS — use envelope directly
                        Some(*sf_env)
                    } else if let (Some(ref layer_srs_str), Some(ref filter_srs_str)) = (&layer_srs_def, sf_srs) {
                        // Different SRS — reproject the spatial filter envelope to the layer SRS
                        let mut src_srs = SpatialRef::from_definition(filter_srs_str)
                            .map_err(|e| { debug!(error = %e, "Failed to create SRS from filter def"); e }).ok();
                        let mut dst_srs = SpatialRef::from_definition(layer_srs_str)
                            .map_err(|e| { debug!(error = %e, "Failed to create SRS from layer def"); e }).ok();
                        match (&mut src_srs, &mut dst_srs) {
                            (Some(ref mut s), Some(ref mut d)) => {
                                s.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                                d.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                                match CoordTransform::new(s, d) {
                                    Ok(ct) => match ct.transform_bounds(sf_env, 21) {
                                        Ok(reprojected) => {
                                            debug!(
                                                path = %path,
                                                source_index = idx,
                                                filter_srs = %filter_srs_str,
                                                layer_srs = %layer_srs_str,
                                                original_env = ?sf_env,
                                                reprojected_env = ?reprojected,
                                                "Reprojected spatial filter envelope for extent clamping"
                                            );
                                            Some(reprojected)
                                        }
                                        Err(e) => {
                                            debug!(path = %path, error = %e, "Failed to reproject SF envelope bounds");
                                            None
                                        }
                                    }
                                    Err(e) => {
                                        debug!(path = %path, error = %e, "Failed to create CoordTransform for SF envelope");
                                        None
                                    }
                                }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if let Some(sf_bounds) = resolved_sf_env {
                        let clamped = [
                            bounds[0].max(sf_bounds[0]),
                            bounds[1].max(sf_bounds[1]),
                            bounds[2].min(sf_bounds[2]),
                            bounds[3].min(sf_bounds[3]),
                        ];
                        // Only clamp if the intersection is valid (non-empty)
                        if clamped[0] < clamped[2] && clamped[1] < clamped[3] {
                            debug!(
                                path = %path,
                                source_index = idx,
                                original = ?bounds,
                                clamped = ?clamped,
                                "Extent clamped by spatial filter envelope"
                            );
                            bounds = clamped;
                        } else {
                            debug!(
                                path = %path,
                                source_index = idx,
                                "Source extent does not intersect spatial filter envelope, skipping"
                            );
                            continue;
                        }
                    } else if !srs_match {
                        debug!(
                            path = %path,
                            source_index = idx,
                            layer_srs = ?layer_srs_def,
                            filter_srs = ?sf_srs,
                            "Could not reproject SF envelope, using raw extent"
                        );
                    }
                }

                // Story 9.4: Use explicit SRS from InputSource if available
                let bounds_wgs84 = if let Some(ref src_def) = input.source_srs {
                    // Explicit source_srs → use it for reprojection
                    let mut src = SpatialRef::from_definition(src_def)?;
                    src.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                    let mut dst = if let Some(ref dst_def) = input.target_srs {
                        SpatialRef::from_definition(dst_def)?
                    } else {
                        SpatialRef::from_epsg(4326)?
                    };
                    dst.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                    debug!(
                        path = %path,
                        source_srs = %src_def,
                        target_srs = input.target_srs.as_deref().unwrap_or("EPSG:4326"),
                        "Reprojecting extent with explicit SRS"
                    );
                    match CoordTransform::new(&src, &dst) {
                        Ok(transform) => match transform.transform_bounds(&bounds, 21) {
                            Ok(reprojected) => reprojected,
                            Err(e) => {
                                if config.error_handling == "fail-fast" {
                                    return Err(e).with_context(|| {
                                        format!("Failed to reproject extent for: {}", path)
                                    });
                                }
                                warn!(path = %path, error = %e, "Failed to reproject extent, skipping");
                                continue;
                            }
                        },
                        Err(e) => {
                            if config.error_handling == "fail-fast" {
                                return Err(e).with_context(|| {
                                    format!("Failed to create coordinate transform for: {}", path)
                                });
                            }
                            warn!(path = %path, error = %e, "Failed to create coordinate transform, skipping");
                            continue;
                        }
                    }
                } else if let Some(layer_srs) = layer.spatial_ref() {
                    // Auto-detect path (original behavior)
                    match layer_srs.auth_code() {
                        Ok(4326) => bounds, // Already WGS84
                        Ok(code) => {
                            // Non-WGS84 SRS — reproject extent
                            debug!(path = %path, srs = code, "Reprojecting extent to WGS84");
                            let wgs84 = SpatialRef::from_epsg(4326)?;
                            match CoordTransform::new(&layer_srs, &wgs84) {
                                Ok(transform) => match transform.transform_bounds(&bounds, 21) {
                                    Ok(reprojected) => reprojected,
                                    Err(e) => {
                                        if config.error_handling == "fail-fast" {
                                            return Err(e).with_context(|| {
                                                format!("Failed to reproject extent for: {}", path)
                                            });
                                        }
                                        warn!(path = %path, error = %e, "Failed to reproject extent, skipping");
                                        continue;
                                    }
                                },
                                Err(e) => {
                                    if config.error_handling == "fail-fast" {
                                        return Err(e).with_context(|| {
                                            format!("Failed to create coordinate transform for: {}", path)
                                        });
                                    }
                                    warn!(path = %path, error = %e, "Failed to create coordinate transform, skipping");
                                    continue;
                                }
                            }
                        }
                        Err(_) => {
                            // Cannot determine SRS authority — assume WGS84
                            warn!(path = %path, "Cannot determine layer SRS authority, assuming WGS84");
                            bounds
                        }
                    }
                } else {
                    // No SRS defined — assume WGS84 (same as load_features_from_layer behavior)
                    warn!(path = %path, "Layer has no SRS for extent, assuming WGS84");
                    bounds
                };

                min_x = min_x.min(bounds_wgs84[0]);
                min_y = min_y.min(bounds_wgs84[1]);
                max_x = max_x.max(bounds_wgs84[2]);
                max_y = max_y.max(bounds_wgs84[3]);
                layer_count += 1;
            }
        }

        if layer_count == 0 {
            anyhow::bail!("No valid source extents found");
        }

        info!(
            layer_count,
            min_x, min_y, max_x, max_y,
            "Extent scan completed"
        );

        Ok(GlobalExtent {
            min_x,
            min_y,
            max_x,
            max_y,
            layer_count,
        })
    }

    /// Build a spatial filter geometry from a reference shapefile.
    ///
    /// Opens the shapefile, unions all polygon geometries (O(n log n) binary tree),
    /// applies a buffer, and returns WKB + SRS + envelope for thread-safe sharing.
    ///
    /// # Arguments
    /// * `source_path` - Path to the shapefile containing clipping polygons
    /// * `buffer_distance` - Buffer distance in meters (in the source SRS). Negative = inward shrink.
    ///
    /// # Returns
    /// * `Result<SpatialFilterGeometry>` - WKB-encoded geometry with SRS and envelope
    pub fn build_spatial_filter_geometry(
        source_path: &str,
        buffer_distance: f64,
    ) -> Result<SpatialFilterGeometry> {
        let dataset = Dataset::open(source_path)
            .with_context(|| format!("Failed to open spatial filter source: {}", source_path))?;
        let mut layer = dataset.layer(0)
            .with_context(|| format!("Failed to access layer 0 in: {}", source_path))?;

        // Detect SRS from source layer
        let srs_def = layer.spatial_ref().and_then(|srs| {
            srs.auth_code().ok().map(|code| format!("EPSG:{}", code))
        });

        let feature_count = layer.feature_count();
        info!(
            source = %source_path,
            features = feature_count,
            buffer_m = buffer_distance,
            srs = srs_def.as_deref().unwrap_or("unknown"),
            "Building spatial filter geometry"
        );

        // Collect all geometries
        let mut geometries: Vec<gdal::vector::Geometry> = Vec::new();
        for feature in layer.features() {
            if let Some(g) = feature.geometry() {
                geometries.push(g.clone());
            }
        }

        if geometries.is_empty() {
            return Err(anyhow!("No geometries found in spatial filter source: {}", source_path));
        }

        let united_count = geometries.len();

        // Binary tree union: O(n log n) instead of O(n²) incremental union
        let geom = binary_tree_union(geometries)?;

        let buffered = if buffer_distance != 0.0 {
            geom.buffer(buffer_distance, 30)
                .with_context(|| format!("Buffer({}) failed on union geometry", buffer_distance))?
        } else {
            geom
        };

        let envelope = buffered.envelope();

        info!(
            source = %source_path,
            features_united = united_count,
            buffer_m = buffer_distance,
            "Spatial filter geometry built"
        );

        let wkb = buffered.wkb()
            .with_context(|| "Failed to serialize spatial filter geometry to WKB")?;

        Ok(SpatialFilterGeometry {
            wkb,
            srs: srs_def,
            envelope: [envelope.MinX, envelope.MinY, envelope.MaxX, envelope.MaxY],
        })
    }

    /// Build a spatial filter geometry from a pattern that may contain brace expansion
    /// or glob wildcards (e.g., `data/{D038,D069}/COMMUNE.shp`).
    ///
    /// Resolves the pattern to concrete file paths, loads geometries from each,
    /// unions everything into a single filter geometry, then applies the buffer.
    pub fn build_spatial_filter_from_pattern(
        source_pattern: &str,
        buffer_distance: f64,
    ) -> Result<SpatialFilterGeometry> {
        use crate::config::expand_braces;

        // If no brace/glob markers, delegate to the single-file method
        if !source_pattern.contains('{') && !source_pattern.contains('*') && !source_pattern.contains('?') {
            return Self::build_spatial_filter_geometry(source_pattern, buffer_distance);
        }

        // Expand braces then glob each resulting pattern
        let expanded = expand_braces(source_pattern);
        let mut resolved_paths: Vec<std::path::PathBuf> = Vec::new();
        for pat in &expanded {
            let matches: Vec<std::path::PathBuf> = glob::glob(pat)
                .with_context(|| format!("Invalid glob pattern in spatial_filter.source: {}", pat))?
                .filter_map(|e| e.ok())
                .collect();
            resolved_paths.extend(matches);
        }

        // Deduplicate in case brace expansion produces overlapping patterns
        resolved_paths.sort();
        resolved_paths.dedup();

        if resolved_paths.is_empty() {
            return Err(anyhow!(
                "No files matched spatial_filter.source pattern: {}",
                source_pattern
            ));
        }

        if resolved_paths.len() == 1 {
            return Self::build_spatial_filter_geometry(
                &resolved_paths[0].to_string_lossy(),
                buffer_distance,
            );
        }

        info!(
            pattern = %source_pattern,
            files = resolved_paths.len(),
            "Building multi-file spatial filter geometry"
        );

        // Collect geometries from all matched files
        let mut all_geometries: Vec<gdal::vector::Geometry> = Vec::new();
        let mut srs_def: Option<String> = None;

        for path in &resolved_paths {
            let path_str = path.to_string_lossy();
            let dataset = Dataset::open(path.as_path())
                .with_context(|| format!("Failed to open spatial filter source: {}", path_str))?;
            let mut layer = dataset.layer(0)
                .with_context(|| format!("Failed to access layer 0 in: {}", path_str))?;

            // Capture SRS from first file, warn if subsequent files differ
            let file_srs = layer.spatial_ref().and_then(|srs| {
                srs.auth_code().ok().map(|code| format!("EPSG:{}", code))
            });
            if srs_def.is_none() {
                srs_def = file_srs.clone();
            } else if let Some(ref current_srs) = file_srs {
                if srs_def.as_ref() != Some(current_srs) {
                    warn!(
                        file = %path_str,
                        expected = ?srs_def,
                        found = %current_srs,
                        "SRS mismatch in spatial filter sources — geometries may be incorrect"
                    );
                }
            }

            for feature in layer.features() {
                if let Some(g) = feature.geometry() {
                    all_geometries.push(g.clone());
                }
            }
        }

        if all_geometries.is_empty() {
            return Err(anyhow!(
                "No geometries found across {} spatial filter source files",
                resolved_paths.len()
            ));
        }

        let united_count = all_geometries.len();
        let geom = binary_tree_union(all_geometries)?;

        let buffered = if buffer_distance != 0.0 {
            geom.buffer(buffer_distance, 30)
                .with_context(|| format!("Buffer({}) failed on union geometry", buffer_distance))?
        } else {
            geom
        };

        let envelope = buffered.envelope();

        info!(
            pattern = %source_pattern,
            files = resolved_paths.len(),
            features_united = united_count,
            buffer_m = buffer_distance,
            "Multi-file spatial filter geometry built"
        );

        let wkb = buffered.wkb()
            .with_context(|| "Failed to serialize spatial filter geometry to WKB")?;

        Ok(SpatialFilterGeometry {
            wkb,
            srs: srs_def,
            envelope: [envelope.MinX, envelope.MinY, envelope.MaxX, envelope.MaxY],
        })
    }

    /// Load features intersecting a single tile from all sources.
    ///
    /// Uses `set_spatial_filter_rect()` to leverage GDAL's native spatial filtering.
    /// Only features whose bounding box intersects the tile are loaded.
    ///
    /// Note: Each call opens and closes all datasets. For N tiles × M sources,
    /// this is N×M dataset opens. This trades I/O overhead for memory safety
    /// (no long-lived dataset handles). If profiling shows this is a bottleneck,
    /// a future optimization can pre-open datasets and reuse them.
    ///
    /// # Arguments
    /// * `config` - Configuration with list of input sources
    /// * `tile_bounds` - Tile bounding box for spatial filtering
    /// * `spatial_filter_geometries` - Pre-built spatial filter geometries, keyed by source index
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)>` - Filtered features and stats
    pub fn read_features_for_tile(
        config: &Config,
        tile_bounds: &crate::pipeline::tiler::TileBounds,
        spatial_filter_geometries: &HashMap<usize, SpatialFilterGeometry>,
    ) -> Result<(Vec<Feature>, UnsupportedTypeStats, MultiGeometryStats)> {
        let mut all_features = Vec::new();
        let mut all_unsupported = UnsupportedTypeStats::default();
        let mut all_multi_geom = MultiGeometryStats::default();

        for (idx, input) in config.inputs.iter().enumerate() {
            let path = match input.path.as_ref() {
                Some(p) => p,
                None => continue,
            };

            let dataset = match Dataset::open(path) {
                Ok(ds) => ds,
                Err(e) => {
                    if config.error_handling == "fail-fast" {
                        return Err(e)
                            .with_context(|| format!("Failed to open dataset: {}", path));
                    }
                    warn!(
                        source_index = idx + 1,
                        path = %path,
                        error = %e,
                        "Failed to open dataset, skipping"
                    );
                    continue;
                }
            };

            let wgs84 = SpatialRef::from_epsg(4326)?;

            // Same layer selection logic
            let layer_selectors: Vec<LayerSelector> =
                if let Some(layers) = &input.layers {
                    if layers.is_empty() {
                        vec![LayerSelector::Index(0)]
                    } else {
                        layers.iter().map(|n| LayerSelector::Name(n.clone())).collect()
                    }
                } else {
                    vec![LayerSelector::Index(0)]
                };

            for selector in &layer_selectors {
                let mut layer = match selector {
                    LayerSelector::Index(i) => match dataset.layer(*i) {
                        Ok(l) => l,
                        Err(e) => {
                            if config.error_handling == "fail-fast" {
                                return Err(e).with_context(|| {
                                    format!("Failed to access layer {} in: {}", i, path)
                                });
                            }
                            warn!(path = %path, error = %e, "Failed to access layer, skipping");
                            continue;
                        }
                    },
                    LayerSelector::Name(name) => match dataset.layer_by_name(name) {
                        Ok(l) => l,
                        Err(e) => {
                            if config.error_handling == "fail-fast" {
                                return Err(e).with_context(|| {
                                    format!("Layer '{}' not found in: {}", name, path)
                                });
                            }
                            warn!(path = %path, error = %e, "Layer not found, skipping");
                            continue;
                        }
                    },
                };

                // Determine the native CRS definition string for this layer.
                let native_srs_def: String = if let Some(ref src_def) = input.source_srs {
                    src_def.clone()
                } else if let Some(layer_srs) = layer.spatial_ref() {
                    let code = layer_srs.auth_code().unwrap_or(4326);
                    format!("EPSG:{}", code)
                } else {
                    "EPSG:4326".to_string()
                };

                // Compute tile bounds in the layer's native CRS for spatial filtering.
                // GDAL spatial filter operates in the layer's native CRS.
                let (native_min_x, native_min_y, native_max_x, native_max_y) =
                    if let Some(ref src_def) = input.source_srs {
                        let mut target = if let Some(ref dst_def) = input.target_srs {
                            SpatialRef::from_definition(dst_def)?
                        } else {
                            SpatialRef::from_epsg(4326)?
                        };
                        target.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                        let mut source = SpatialRef::from_definition(src_def)?;
                        source.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                        let reverse_transform = CoordTransform::new(&target, &source)?;
                        let nb = reverse_transform.transform_bounds(
                            &[tile_bounds.min_lon, tile_bounds.min_lat, tile_bounds.max_lon, tile_bounds.max_lat],
                            21,
                        )?;
                        (nb[0], nb[1], nb[2], nb[3])
                    } else if native_srs_def == "EPSG:4326" {
                        (tile_bounds.min_lon, tile_bounds.min_lat, tile_bounds.max_lon, tile_bounds.max_lat)
                    } else {
                        let wgs84_srs = SpatialRef::from_definition("EPSG:4326")?;
                        let native_srs = SpatialRef::from_definition(&native_srs_def)?;
                        let reverse = CoordTransform::new(&wgs84_srs, &native_srs)?;
                        let mut xs = vec![tile_bounds.min_lon, tile_bounds.max_lon];
                        let mut ys = vec![tile_bounds.min_lat, tile_bounds.max_lat];
                        if reverse.transform_coords(&mut xs, &mut ys, &mut []).is_ok() {
                            (xs[0].min(xs[1]), ys[0].min(ys[1]), xs[0].max(xs[1]), ys[0].max(ys[1]))
                        } else {
                            (tile_bounds.min_lon, tile_bounds.min_lat, tile_bounds.max_lon, tile_bounds.max_lat)
                        }
                    };

                // Apply spatial filter: combine with clipping geometry if available
                let mut skip_layer = false;
                if let Some(sf_geom) = spatial_filter_geometries.get(&idx) {
                    // F5: Fast envelope pre-rejection — avoid WKB parse if tile is clearly outside
                    let env = &sf_geom.envelope;
                    let same_crs = sf_geom.srs.as_deref() == Some(&native_srs_def);
                    if same_crs
                        && (native_max_x < env[0] || native_min_x > env[2]
                            || native_max_y < env[1] || native_min_y > env[3])
                    {
                        trace!(
                            source_index = idx,
                            tile = %tile_bounds.tile_id(),
                            "Tile outside spatial filter envelope, skipping source"
                        );
                        skip_layer = true;
                    } else {
                        // Reconstruct clipping geometry from WKB
                        let clip_geom = gdal::vector::Geometry::from_wkb(&sf_geom.wkb)
                            .with_context(|| format!("Failed to deserialize spatial filter WKB for source {}", idx))?;

                        // F1: Reproject clipping geometry to layer's native CRS if CRS differ
                        if !same_crs {
                            if let Some(ref clip_srs_def) = sf_geom.srs {
                                let mut clip_srs = SpatialRef::from_definition(clip_srs_def)?;
                                clip_srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                                let mut target_srs = SpatialRef::from_definition(&native_srs_def)?;
                                target_srs.set_axis_mapping_strategy(gdal::spatial_ref::AxisMappingStrategy::TraditionalGisOrder);
                                let transform = CoordTransform::new(&clip_srs, &target_srs)?;
                                let rv = unsafe {
                                    gdal_sys::OGR_G_Transform(
                                        clip_geom.c_geometry(),
                                        transform.to_c_hct(),
                                    )
                                };
                                if rv != gdal_sys::OGRErr::OGRERR_NONE {
                                    return Err(anyhow!(
                                        "Failed to reproject spatial filter from {} to {}",
                                        clip_srs_def, native_srs_def
                                    ));
                                }
                            }
                        }

                        // Build tile polygon in native CRS via WKT
                        let tile_wkt = format!(
                            "POLYGON (({} {}, {} {}, {} {}, {} {}, {} {}))",
                            native_min_x, native_min_y,
                            native_max_x, native_min_y,
                            native_max_x, native_max_y,
                            native_min_x, native_max_y,
                            native_min_x, native_min_y,
                        );
                        let tile_poly = gdal::vector::Geometry::from_wkt(&tile_wkt)
                            .with_context(|| "Failed to create tile polygon from WKT")?;

                        // Intersect clipping geometry with tile polygon
                        match clip_geom.intersection(&tile_poly) {
                            Some(combined) if !combined.is_empty() => {
                                layer.set_spatial_filter(&combined);
                            }
                            _ => {
                                trace!(
                                    source_index = idx,
                                    tile = %tile_bounds.tile_id(),
                                    "Tile outside spatial filter area, skipping source"
                                );
                                skip_layer = true;
                            }
                        }
                    }
                } else {
                    // No spatial filter: use tile rect as before
                    layer.set_spatial_filter_rect(
                        native_min_x, native_min_y, native_max_x, native_max_y,
                    );
                }

                if skip_layer {
                    continue;
                }

                // Apply attribute filter if configured on this InputSource
                if let Some(ref attr_filter) = input.attribute_filter {
                    layer.set_attribute_filter(attr_filter)
                        .with_context(|| format!("Failed to set attribute filter '{}' on: {}", attr_filter, path))?;
                }

                // Story 9.4: Propagate explicit SRS from InputSource
                let (features, unsupported, multi_geom) =
                    Self::load_features_from_layer(
                        &mut layer,
                        path,
                        &wgs84,
                        input.source_srs.as_deref(),
                        input.target_srs.as_deref(),
                        None, // attribute_filter already set on layer above
                        input.layer_alias.as_deref(),
                        input.dedup_by_field.as_deref().or(config.default_dedup_by_field.as_deref()),
                    )?;

                // Clear attribute filter before clearing spatial filter
                if input.attribute_filter.is_some() {
                    layer.set_attribute_filter("")
                        .ok(); // Ignore error on cleanup
                }
                layer.clear_spatial_filter();

                all_features.extend(features);
                all_unsupported.merge(&unsupported);
                all_multi_geom.merge(&multi_geom);
            }
        }

        debug!(
            tile = %tile_bounds.tile_id(),
            features = all_features.len(),
            "Tile features loaded"
        );

        Ok((all_features, all_unsupported, all_multi_geom))
    }

    /// Read features from all sources and build R-tree spatial index.
    ///
    /// # Deprecated
    /// Use `scan_extents()` + `read_features_for_tile()` instead for memory-efficient
    /// tile-centric processing.
    ///
    /// # Arguments
    /// * `config` - Configuration with list of input sources
    ///
    /// # Returns
    /// * `Result<(Vec<Feature>, RTreeIndex, UnsupportedTypeStats, MultiGeometryStats)>` - All features, R-tree index, unsupported type stats, and multi-geometry stats
    ///
    /// # Errors
    /// * File not found or not readable (depending on error_handling mode)
    /// * GDAL errors
    /// * R-tree construction errors (should never happen in practice)
    pub fn read_all_sources(
        config: &Config,
    ) -> Result<(
        Vec<Feature>,
        RTreeIndex,
        UnsupportedTypeStats,
        MultiGeometryStats,
    )> {
        let mut all_features = Vec::new();
        let mut total_stats = ReaderStats::default();
        let mut all_unsupported = UnsupportedTypeStats::default();
        let mut all_multi_geom = MultiGeometryStats::default(); // Story 6.7 - Subtask 4.3

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

            match Self::read_file_source_with_error_handling(input, &config.error_handling, input.dedup_by_field.as_deref().or(config.default_dedup_by_field.as_deref())) {
                Ok((features, unsupported, multi_geom)) => {
                    let count = features.len();
                    all_unsupported.merge(&unsupported);
                    // Code Review M2 Fix: Use merge() for O(T) instead of O(N) loop
                    all_multi_geom.merge(&multi_geom);

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

        // Story 6.7 - Subtask 4.4: Log INFO summary for multi-geometries decomposed
        if all_multi_geom.total() > 0 {
            let breakdown: Vec<String> = all_multi_geom
                .by_type
                .iter()
                .map(|(type_name, entry)| format!("{}: {}", type_name, entry.count))
                .collect();
            info!(
                total = all_multi_geom.total(),
                breakdown = %breakdown.join(", "),
                "Multi-geometry features decomposed into simple geometries"
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

        Ok((all_features, rtree, all_unsupported, all_multi_geom))
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
    /// Source layer name (for rules engine matching by source_layer)
    pub source_layer: Option<String>,
}

impl Feature {
    /// Extract all attributes from a GDAL feature as string key-value pairs.
    /// Code Review H4 Fix: Shared helper to avoid duplication between from_gdal_feature and process_sub_geometry.
    fn extract_attributes(gdal_feature: &gdal::vector::Feature) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        for (field_name, field_value) in gdal_feature.fields() {
            let value_str = match field_value {
                Some(gdal::vector::FieldValue::StringValue(s)) => s.to_string(),
                Some(gdal::vector::FieldValue::IntegerValue(i)) => i.to_string(),
                Some(gdal::vector::FieldValue::Integer64Value(i)) => i.to_string(),
                Some(gdal::vector::FieldValue::RealValue(r)) => r.to_string(),
                Some(gdal::vector::FieldValue::DateValue(d)) => format!("{:?}", d),
                Some(gdal::vector::FieldValue::DateTimeValue(dt)) => format!("{:?}", dt),
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
        attributes
    }

    /// Decompose a GDAL multi-geometry into N simple geometry features.
    ///
    /// Story 6.7 - Task 2: Decompose MultiPoint, MultiLineString, MultiPolygon into simple geometries.
    /// Each sub-geometry inherits all attributes from the parent feature.
    ///
    /// # Arguments
    /// * `gdal_feature` - GDAL feature containing multi-geometry
    ///
    /// # Returns
    /// * `Result<Vec<Feature>>` - N features (one per sub-geometry), or empty vec if error
    ///
    /// # Errors
    /// * Feature has no geometry
    /// * Invalid sub-geometry structure
    /// Code Review M1 Fix: Added #[instrument] per Story 6.6 M5 learning.
    #[instrument(skip(gdal_feature))]
    fn decompose_multi_geometry(gdal_feature: &gdal::vector::Feature) -> Result<Vec<Feature>> {
        let geometry = gdal_feature
            .geometry()
            .ok_or_else(|| anyhow!("Feature has no geometry for multi-geometry decomposition"))?;

        // Get the multi-geometry type from the geometry itself
        let multi_type = geometry.geometry_type();

        // Subtask 2.2: Get count of sub-geometries
        let count = geometry.geometry_count();
        if count == 0 {
            debug!(
                multi_type = ?multi_type,
                "Multi-geometry has 0 sub-geometries, returning empty vec"
            );
            return Ok(vec![]);
        }

        let mut sub_features = Vec::with_capacity(count);

        // Determine simple geometry type from multi-type
        // Handle both standard and 25D variants
        let simple_geom_type = match multi_type {
            OGRwkbGeometryType::wkbMultiPoint | OGRwkbGeometryType::wkbMultiPoint25D => {
                GeometryType::Point
            }
            OGRwkbGeometryType::wkbMultiLineString | OGRwkbGeometryType::wkbMultiLineString25D => {
                GeometryType::LineString
            }
            OGRwkbGeometryType::wkbMultiPolygon | OGRwkbGeometryType::wkbMultiPolygon25D => {
                GeometryType::Polygon
            }
            _ => {
                warn!(
                    multi_type = ?multi_type,
                    "Unsupported multi-geometry type in decompose_multi_geometry"
                );
                return Ok(vec![]);
            }
        };

        // Subtask 2.3: Iterate over each sub-geometry
        for i in 0..count {
            let sub_geom = geometry.get_geometry(i);

            // Subtask 2.6: Handle errors with logging and skip problematic sub-geometries
            match Self::process_sub_geometry(&sub_geom, simple_geom_type, gdal_feature) {
                Ok(feature) => sub_features.push(feature),
                Err(e) => {
                    warn!(
                        sub_geometry_index = i,
                        error = %e,
                        "Skipping invalid sub-geometry in multi-geometry decomposition"
                    );
                    // Continue with next sub-geometry
                }
            }
        }

        debug!(
            multi_type = ?multi_type,
            input_count = count,
            output_count = sub_features.len(),
            "Multi-geometry decomposed"
        );

        Ok(sub_features)
    }

    /// Process a single sub-geometry from a multi-geometry.
    ///
    /// Helper function to extract coordinates and attributes for one sub-geometry.
    /// Code Review M4 Fix: Removed target_srs parameter (always None, SRS transform done upstream).
    /// Code Review M1 Fix: Added #[instrument] per Story 6.6 M5 learning.
    ///
    /// # Arguments
    /// * `sub_geom` - GDAL geometry (sub-geometry from multi)
    /// * `geom_type` - Simple geometry type (Point, LineString, or Polygon)
    /// * `parent_feature` - Parent GDAL feature (for attribute cloning)
    ///
    /// # Returns
    /// * `Result<Feature>` - Feature with geometry and cloned attributes
    #[instrument(skip(sub_geom, parent_feature))]
    fn process_sub_geometry(
        sub_geom: &gdal::vector::Geometry,
        geom_type: GeometryType,
        parent_feature: &gdal::vector::Feature,
    ) -> Result<Feature> {
        // Extract coordinates from sub-geometry
        let coords = Self::extract_coordinates(sub_geom, geom_type)?;

        // Subtask 2.4: Clone attributes from parent feature using shared helper
        let attributes = Self::extract_attributes(parent_feature);

        Ok(Feature {
            geometry_type: geom_type,
            geometry: coords,
            attributes,
            source_layer: None,
        })
    }

    /// Convert a GDAL feature to internal Feature representation.
    ///
    /// Story 6.7 - Task 3: Changed signature to return Vec<Feature> to support multi-geometry decomposition.
    ///
    /// # Arguments
    /// * `gdal_feature` - GDAL feature to convert
    ///
    /// # Returns
    /// * `Result<Vec<Feature>>` - vec![feature] for simple types, vec![f1, f2, ...fN] for multi-geometries, vec![] for unsupported types
    ///
    /// # Errors
    /// * Invalid geometry structure
    pub fn from_gdal_feature(gdal_feature: &gdal::vector::Feature) -> Result<Vec<Self>> {
        // 1. Extract and validate geometry type
        let geometry = gdal_feature
            .geometry()
            .ok_or_else(|| anyhow!("Feature has no geometry"))?;

        let geom_type = geometry.geometry_type();

        // Subtask 3.2, 3.3: Handle multi-geometries by decomposing them
        match geom_type {
            // Multi-geometries → decompose into N simple features
            OGRwkbGeometryType::wkbMultiPoint
            | OGRwkbGeometryType::wkbMultiPoint25D
            | OGRwkbGeometryType::wkbMultiLineString
            | OGRwkbGeometryType::wkbMultiLineString25D
            | OGRwkbGeometryType::wkbMultiPolygon
            | OGRwkbGeometryType::wkbMultiPolygon25D => {
                debug!(geometry_type = ?geom_type, "Decomposing multi-geometry");
                return Self::decompose_multi_geometry(gdal_feature);
            }
            // Subtask 3.5: GeometryCollection → filter with warning (including 25D variant)
            // Code Review M5 Fix: Handle GeometryCollection25D explicitly
            OGRwkbGeometryType::wkbGeometryCollection
            | OGRwkbGeometryType::wkbGeometryCollection25D => {
                warn!(
                    geometry_type = ?geom_type,
                    "GeometryCollection is not supported, filtering feature"
                );
                return Ok(vec![]);
            }
            // Simple geometries → continue with normal processing (including 25D variants)
            OGRwkbGeometryType::wkbPoint | OGRwkbGeometryType::wkbPoint25D => {}
            OGRwkbGeometryType::wkbLineString | OGRwkbGeometryType::wkbLineString25D => {}
            OGRwkbGeometryType::wkbPolygon | OGRwkbGeometryType::wkbPolygon25D => {}
            // Other unsupported types
            other => {
                debug!(geometry_type = ?other, "Skipping unsupported geometry type");
                return Ok(vec![]);
            }
        }

        // 2. Get simple geometry type for processing (including 25D variants)
        let geometry_type = match geom_type {
            OGRwkbGeometryType::wkbPoint | OGRwkbGeometryType::wkbPoint25D => GeometryType::Point,
            OGRwkbGeometryType::wkbLineString | OGRwkbGeometryType::wkbLineString25D => {
                GeometryType::LineString
            }
            OGRwkbGeometryType::wkbPolygon | OGRwkbGeometryType::wkbPolygon25D => {
                GeometryType::Polygon
            }
            _ => unreachable!("Should have been handled by match above"),
        };

        // 3. Extract coordinates
        let geometry_coords = Self::extract_coordinates(geometry, geometry_type)?;

        // 4. Extract all attributes from feature fields (using shared helper)
        let attributes = Self::extract_attributes(gdal_feature);

        // Log debug information about the feature
        debug!(
            geometry_type = ?geometry_type,
            coord_count = geometry_coords.len(),
            attr_count = attributes.len(),
            "Feature extracted from GDAL"
        );

        // Subtask 3.4: Wrap simple geometry in Vec for uniform return type
        Ok(vec![Feature {
            geometry_type,
            geometry: geometry_coords,
            attributes,
            source_layer: None,
        }])
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
            source_layer: None,
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
            source_layer: None,
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
            source_layer: None,
        };

        assert_eq!(feature.geometry_type, GeometryType::Polygon);
        assert!(feature.attributes.is_empty());
    }
}
