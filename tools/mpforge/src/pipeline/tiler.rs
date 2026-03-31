//! Spatial tiling and grid management.

use crate::config::{ErrorMode, FilterConfig, GridConfig};
use crate::pipeline::geometry_validator::{validate_and_repair, ValidationResult, ValidationStats};
use crate::pipeline::reader::{Feature, GeometryType, RTreeIndex};
use gdal::spatial_ref::SpatialRef;
use gdal::vector::Geometry;
use rstar::AABB;
use tracing::{debug, info, instrument, warn};

/// Processes spatial tiling based on grid configuration.
#[derive(Debug)]
pub struct TileProcessor {
    grid: GridConfig,
}

impl TileProcessor {
    pub fn new(grid: GridConfig) -> Self {
        Self { grid }
    }

    /// Generate tile boundaries based on grid configuration and data extent.
    ///
    /// Creates a regular grid of tiles covering the data bounding box with configurable
    /// cell size and overlap. Tiles can be optionally filtered by a spatial bbox.
    ///
    /// # Arguments
    /// * `rtree` - Spatial index providing global data bounding box
    /// * `filters` - Optional spatial filter to exclude tiles outside bbox
    ///
    /// # Returns
    /// * `Vec<TileBounds>` - All tiles intersecting data (after filtering)
    ///
    /// # Algorithm
    /// 1. Check for empty data → return empty vec
    /// 2. Determine grid origin (config.origin or global_bbox.lower())
    /// 3. Calculate grid dimensions (cols, rows) from bbox extent
    /// 4. Generate all tile boundaries with overlap applied
    /// 5. Filter tiles by bbox if filter is configured
    #[instrument(skip(rtree))]
    pub fn generate_tiles(
        &self,
        rtree: &RTreeIndex,
        filters: &Option<FilterConfig>,
    ) -> Vec<TileBounds> {
        // 1. Check for empty data
        if rtree.tree_size() == 0 {
            info!("No features in R-tree, cannot generate grid");
            return Vec::new();
        }

        // 2. Determine grid origin
        let global_bbox = rtree.global_bbox();
        let origin = self
            .grid
            .origin
            .unwrap_or_else(|| [global_bbox.lower()[0], global_bbox.lower()[1]]);

        // 3. Calculate grid dimensions
        let width = global_bbox.upper()[0] - origin[0];
        let height = global_bbox.upper()[1] - origin[1];
        let num_cols = (width / self.grid.cell_size).ceil() as usize;
        let num_rows = (height / self.grid.cell_size).ceil() as usize;

        info!(
            origin = ?origin,
            cell_size = self.grid.cell_size,
            overlap = self.grid.overlap,
            num_cols,
            num_rows,
            theoretical_tiles = num_cols * num_rows,
            "Generating spatial grid"
        );

        // 4. Generate all tile boundaries
        let mut tiles = Vec::with_capacity(num_cols * num_rows);

        for row in 0..num_rows {
            for col in 0..num_cols {
                // Calculate tile bbox with overlap
                let min_lon =
                    origin[0] + (col as f64 * self.grid.cell_size) - (self.grid.overlap / 2.0);
                let min_lat =
                    origin[1] + (row as f64 * self.grid.cell_size) - (self.grid.overlap / 2.0);
                let max_lon = min_lon + self.grid.cell_size + self.grid.overlap;
                let max_lat = min_lat + self.grid.cell_size + self.grid.overlap;

                let tile = TileBounds {
                    col,
                    row,
                    min_lon,
                    min_lat,
                    max_lon,
                    max_lat,
                };

                // 5. Apply spatial filter if exists
                if let Some(filter) = filters {
                    if !tile.intersects_bbox(&filter.bbox) {
                        continue; // Skip tiles outside filter zone
                    }
                }

                tiles.push(tile);
            }
        }

        info!(
            tiles_generated = tiles.len(),
            filtered_out = (num_cols * num_rows) - tiles.len(),
            "Grid generation completed"
        );

        tiles
    }

    /// Generate tile boundaries from a direct bounding box.
    ///
    /// Same algorithm as `generate_tiles()` but accepts a `[min_x, min_y, max_x, max_y]`
    /// array instead of requiring an `RTreeIndex`. Used by the tile-centric pipeline
    /// where extents are obtained via `scan_extents()`.
    ///
    /// # Arguments
    /// * `global_bbox` - Bounding box `[min_x, min_y, max_x, max_y]`
    /// * `filters` - Optional spatial filter to exclude tiles outside bbox
    ///
    /// # Returns
    /// * `Vec<TileBounds>` - All tiles covering the bbox (after filtering)
    #[instrument(skip(self))]
    pub fn generate_tiles_from_bbox(
        &self,
        global_bbox: &[f64; 4],
        filters: &Option<FilterConfig>,
    ) -> Vec<TileBounds> {
        let [bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y] = *global_bbox;

        // Validate bbox
        if bbox_min_x >= bbox_max_x || bbox_min_y >= bbox_max_y {
            info!("Empty or degenerate bbox, cannot generate grid");
            return Vec::new();
        }

        // Determine grid origin
        let origin = self
            .grid
            .origin
            .unwrap_or([bbox_min_x, bbox_min_y]);

        // Calculate grid dimensions
        let width = bbox_max_x - origin[0];
        let height = bbox_max_y - origin[1];
        let num_cols = (width / self.grid.cell_size).ceil() as usize;
        let num_rows = (height / self.grid.cell_size).ceil() as usize;

        info!(
            origin = ?origin,
            cell_size = self.grid.cell_size,
            overlap = self.grid.overlap,
            num_cols,
            num_rows,
            theoretical_tiles = num_cols * num_rows,
            "Generating spatial grid from bbox"
        );

        // Generate all tile boundaries
        let mut tiles = Vec::with_capacity(num_cols * num_rows);

        for row in 0..num_rows {
            for col in 0..num_cols {
                let min_lon =
                    origin[0] + (col as f64 * self.grid.cell_size) - (self.grid.overlap / 2.0);
                let min_lat =
                    origin[1] + (row as f64 * self.grid.cell_size) - (self.grid.overlap / 2.0);
                let max_lon = min_lon + self.grid.cell_size + self.grid.overlap;
                let max_lat = min_lat + self.grid.cell_size + self.grid.overlap;

                let tile = TileBounds {
                    col,
                    row,
                    min_lon,
                    min_lat,
                    max_lon,
                    max_lat,
                };

                // Apply spatial filter if exists
                if let Some(filter) = filters {
                    if !tile.intersects_bbox(&filter.bbox) {
                        continue;
                    }
                }

                tiles.push(tile);
            }
        }

        info!(
            tiles_generated = tiles.len(),
            filtered_out = (num_cols * num_rows) - tiles.len(),
            "Grid generation from bbox completed"
        );

        tiles
    }

    /// Assign features to tiles using R-tree spatial queries.
    ///
    /// # Deprecated
    /// Use tile-centric pipeline with `read_features_for_tile()` instead.
    ///
    /// # Arguments
    /// * `rtree` - Spatial index for efficient queries
    /// * `tiles` - Tile boundaries to process
    ///
    /// # Returns
    /// * `Vec<(TileBounds, Vec<usize>)>` - Non-empty tiles with feature IDs
    #[instrument(skip(rtree, tiles))]
    pub fn assign_features_to_tiles(
        &self,
        rtree: &RTreeIndex,
        tiles: Vec<TileBounds>,
    ) -> Vec<(TileBounds, Vec<usize>)> {
        let mut result = Vec::new();
        let mut empty_count = 0;

        for tile in tiles {
            let tile_aabb = tile.to_aabb();
            let candidates = rtree.query_intersecting(&tile_aabb);

            if candidates.is_empty() {
                debug!(tile_id = %tile.tile_id(), "Tile has no features, skipping");
                empty_count += 1;
                continue;
            }

            result.push((tile, candidates));
        }

        info!(
            non_empty_tiles = result.len(),
            empty_tiles_skipped = empty_count,
            total_feature_refs = result.iter().map(|(_, f)| f.len()).sum::<usize>(),
            "Feature assignment completed"
        );

        result
    }
}

/// Boundaries of a single tile in the spatial grid.
///
/// Story 6.2: Enhanced with full bbox and utility methods for grid tiling.
#[derive(Debug, Clone)]
pub struct TileBounds {
    /// Column index in the grid (0-based)
    pub col: usize,
    /// Row index in the grid (0-based)
    pub row: usize,
    /// Minimum longitude (west boundary with overlap)
    pub min_lon: f64,
    /// Minimum latitude (south boundary with overlap)
    pub min_lat: f64,
    /// Maximum longitude (east boundary with overlap)
    pub max_lon: f64,
    /// Maximum latitude (north boundary with overlap)
    pub max_lat: f64,
}

impl TileBounds {
    /// Create AABB for R-tree spatial queries.
    ///
    /// # Returns
    /// * `AABB<[f64; 2]>` - Axis-aligned bounding box with min/max corners
    pub fn to_aabb(&self) -> AABB<[f64; 2]> {
        AABB::from_corners([self.min_lon, self.min_lat], [self.max_lon, self.max_lat])
    }

    /// Generate unique tile identifier (format: "col_row").
    ///
    /// # Returns
    /// * `String` - Tile ID for file naming and logging
    ///
    /// # Examples
    /// ```
    /// # use mpforge::pipeline::tiler::TileBounds;
    /// let tile = TileBounds {
    ///     col: 15,
    ///     row: 42,
    ///     min_lon: 0.0,
    ///     min_lat: 0.0,
    ///     max_lon: 1.0,
    ///     max_lat: 1.0,
    /// };
    /// assert_eq!(tile.tile_id(), "15_42");
    /// ```
    pub fn tile_id(&self) -> String {
        format!("{}_{}", self.col, self.row)
    }

    /// Check if tile intersects a filter bounding box.
    ///
    /// Uses AABB intersection test: tiles are considered intersecting if they
    /// share any overlapping area (including edge contact).
    ///
    /// # Arguments
    /// * `filter_bbox` - Bounding box filter [min_lon, min_lat, max_lon, max_lat]
    ///
    /// # Returns
    /// * `bool` - true if tile intersects filter, false otherwise
    pub fn intersects_bbox(&self, filter_bbox: &[f64; 4]) -> bool {
        let [fmin_lon, fmin_lat, fmax_lon, fmax_lat] = *filter_bbox;

        // AABB intersection test: non-overlapping if any of these conditions are true
        !(self.max_lon < fmin_lon
            || self.min_lon > fmax_lon
            || self.max_lat < fmin_lat
            || self.min_lat > fmax_lat)
    }

    /// Create GDAL Polygon geometry from tile bounding box.
    ///
    /// Used for clipping features to tile boundaries via OGR_G_Intersection.
    /// Constructs a rectangular polygon in WGS84 (EPSG:4326) coordinate system.
    ///
    /// # Returns
    /// * `anyhow::Result<Geometry>` - Rectangular polygon ready for intersection
    ///
    /// # Errors
    /// * WKT parsing fails (should never happen for bbox)
    /// * SRS assignment fails
    /// * Geometry validation fails (indicates invalid bbox coordinates)
    ///
    /// # Examples
    /// ```
    /// # use mpforge::pipeline::tiler::TileBounds;
    /// let tile = TileBounds {
    ///     col: 0,
    ///     row: 0,
    ///     min_lon: 10.0,
    ///     min_lat: 20.0,
    ///     max_lon: 10.15,
    ///     max_lat: 20.15,
    /// };
    /// let polygon = tile.to_gdal_polygon().unwrap();
    /// assert!(polygon.is_valid());
    /// ```
    pub fn to_gdal_polygon(&self) -> anyhow::Result<Geometry> {
        // Construct WKT POLYGON ((minx miny, maxx miny, maxx maxy, minx maxy, minx miny))
        let wkt = format!(
            "POLYGON(({} {}, {} {}, {} {}, {} {}, {} {}))",
            self.min_lon,
            self.min_lat,
            self.max_lon,
            self.min_lat,
            self.max_lon,
            self.max_lat,
            self.min_lon,
            self.max_lat,
            self.min_lon,
            self.min_lat // Close ring
        );

        let mut geom = Geometry::from_wkt(&wkt)?;

        // Set WGS84 spatial reference
        let srs = SpatialRef::from_epsg(4326)?;
        geom.set_spatial_ref(srs);

        // Validate bbox geometry (should always be valid for proper bbox coords)
        if !geom.is_valid() {
            anyhow::bail!(
                "Invalid tile bbox geometry for tile {}: [{}, {}, {}, {}]",
                self.tile_id(),
                self.min_lon,
                self.min_lat,
                self.max_lon,
                self.max_lat
            );
        }

        Ok(geom)
    }
}

/// Placeholder for tile data with features.
/// TODO: Story 6.4 - Define complete tile data structure for export
#[allow(dead_code)] // Stub - will be fully implemented in Story 6.4
#[derive(Debug)]
pub struct TileData {
    pub tile_id: String,
}

// ============================================================================
// Task 2-3: Geometry Clipping Functions (Story 6.3)
// ============================================================================

/// Clip a feature to tile bounding box using GDAL Intersection.
///
/// This function performs geometric clipping of a feature's geometry to fit within
/// a tile's boundaries while preserving all attribute fields. Invalid or degenerate
/// geometries are handled according to the error_mode parameter.
///
/// # Arguments
/// * `feature` - Source feature with original geometry (internal Feature representation)
/// * `tile_bbox` - Tile bounding box as GDAL Polygon (WGS84)
/// * `error_mode` - How to handle invalid/degenerate geometries
///
/// # Returns
/// * `Ok(Some(Feature))` - Clipped feature with preserved attributes
/// * `Ok(None)` - Feature outside tile, intersection empty, or invalid (continue mode)
/// * `Err(_)` - Invalid geometry in fail-fast mode
///
/// # Performance
/// * O(n log n) where n = number of vertices in feature geometry (GEOS algorithm)
/// * Typical: ~1ms per feature (50 vertices average)
///
/// # Examples
/// ```ignore
/// let tile_bbox = tile.to_gdal_polygon()?;
/// let clipped_feature = clip_feature_to_tile(&feature, &tile_bbox, ErrorMode::Continue)?;
/// if let Some(clipped) = clipped_feature {
///     // Process clipped feature with preserved attributes
/// }
/// ```
#[instrument(skip(feature, tile_bbox, validation_stats))]
pub fn clip_feature_to_tile(
    feature: &Feature,
    tile_bbox: &Geometry,
    error_mode: ErrorMode,
    validation_stats: &mut ValidationStats,
) -> anyhow::Result<Option<Feature>> {
    // Story 6.5: Validate and optionally repair geometry before clipping
    let src_geom = match validate_and_repair(feature, validation_stats) {
        ValidationResult::Valid(geom) => geom,
        ValidationResult::Repaired(geom, strategy) => {
            debug!(strategy = ?strategy, "Using repaired geometry for clipping");
            geom
        }
        ValidationResult::Rejected(reason) => {
            // Logging already done by validate_and_repair() — no duplicate error log
            warn!("Feature rejected during validation: {}", reason);
            return handle_invalid_geometry(error_mode);
        }
    };

    // Early exit: Check if geometry intersects tile (O(1) bbox check)
    if !src_geom.intersects(tile_bbox) {
        debug!("Feature outside tile, skipping");
        return Ok(None);
    }

    // Special case: Points don't need clipping (overlap handles boundaries)
    if feature.geometry_type == GeometryType::Point {
        debug!("Point geometry, no clipping needed");
        // Return clone of original feature
        return Ok(Some(feature.clone()));
    }

    // Perform GDAL Intersection
    let clipped_geom = match src_geom.intersection(tile_bbox) {
        Some(geom) => {
            // Check for empty result
            if geom.is_empty() {
                debug!("Intersection empty, skipping");
                return Ok(None);
            }

            // Validate result
            if !geom.is_valid() {
                warn!("Intersection produced invalid geometry");
                return handle_invalid_geometry(error_mode);
            }

            geom
        }
        None => {
            // None indicates GDAL error during intersection (not empty result)
            // Empty intersections return Some(empty_geom) with is_empty() = true
            warn!("GDAL intersection operation failed");
            return handle_invalid_geometry(error_mode);
        }
    };

    // Convert clipped GDAL Geometry back to internal coordinates
    let clipped_coords = gdal_geometry_to_coords(&clipped_geom)?;

    // Create new Feature with clipped geometry + preserved attributes
    let clipped_feature = Feature {
        geometry_type: feature.geometry_type,
        geometry: clipped_coords,
        attributes: feature.attributes.clone(), // Preserve all attributes (Type, Label, etc.)
        source_layer: feature.source_layer.clone(),
    };

    debug!(
        geom_type = ?feature.geometry_type,
        original_coords = feature.geometry.len(),
        clipped_coords = clipped_feature.geometry.len(),
        "Feature clipped successfully"
    );

    Ok(Some(clipped_feature))
}

/// Handle invalid geometry based on error mode.
fn handle_invalid_geometry(error_mode: ErrorMode) -> anyhow::Result<Option<Feature>> {
    match error_mode {
        ErrorMode::Continue => {
            debug!("Skipping invalid geometry (continue mode)");
            Ok(None)
        }
        ErrorMode::FailFast => {
            anyhow::bail!("Invalid geometry encountered in fail-fast mode")
        }
    }
}

/// Convert internal Feature to GDAL Geometry.
#[instrument(skip(feature))]
pub fn feature_to_gdal_geometry(feature: &Feature) -> anyhow::Result<Geometry> {
    let wkt = match feature.geometry_type {
        GeometryType::Point => {
            if feature.geometry.is_empty() {
                anyhow::bail!("Point feature has no coordinates");
            }
            let (x, y) = feature.geometry[0];
            format!("POINT({} {})", x, y)
        }
        GeometryType::LineString => {
            if feature.geometry.len() < 2 {
                anyhow::bail!("LineString must have at least 2 points");
            }
            let coords: Vec<String> = feature
                .geometry
                .iter()
                .map(|(x, y)| format!("{} {}", x, y))
                .collect();
            format!("LINESTRING({})", coords.join(", "))
        }
        GeometryType::Polygon => {
            if feature.geometry.len() < 3 {
                anyhow::bail!("Polygon must have at least 3 points");
            }
            let coords: Vec<String> = feature
                .geometry
                .iter()
                .map(|(x, y)| format!("{} {}", x, y))
                .collect();
            // Close ring if not already closed
            let first = feature.geometry[0];
            let last = *feature.geometry.last().unwrap();
            let ring = if (first.0 - last.0).abs() < 1e-9 && (first.1 - last.1).abs() < 1e-9 {
                coords.join(", ")
            } else {
                format!("{}, {} {}", coords.join(", "), first.0, first.1)
            };
            format!("POLYGON(({}))", ring)
        }
    };

    // Story 6.6 Fix: Verify WKT doesn't contain NaN/Inf before parsing
    // This can happen if source features have invalid coordinates that passed initial validation
    let wkt_lower = wkt.to_lowercase();
    if wkt_lower.contains("nan") || wkt_lower.contains("inf") || wkt_lower.contains("-1.#ind") {
        anyhow::bail!(
            "Cannot convert feature to GDAL geometry: WKT contains NaN/Inf ({})",
            wkt
        );
    }

    Geometry::from_wkt(&wkt).map_err(|e| anyhow::anyhow!("WKT conversion failed: {}", e))
}

/// Extract coordinates from GDAL Geometry to internal format using GDAL native API.
///
/// Handles simple geometries (Point, LineString, Polygon) and multi-geometries
/// (MultiPolygon, MultiLineString) by extracting the largest sub-geometry.
/// For Polygons, only the exterior ring is extracted.
fn gdal_geometry_to_coords(geom: &Geometry) -> anyhow::Result<Vec<(f64, f64)>> {
    use gdal::vector::OGRwkbGeometryType;

    let geom_type = geom.geometry_type();

    match geom_type {
        // Point
        OGRwkbGeometryType::wkbPoint | OGRwkbGeometryType::wkbPoint25D => {
            let (x, y, _) = geom.get_point(0);
            validate_coords(x, y)?;
            Ok(vec![(x, y)])
        }
        // LineString
        OGRwkbGeometryType::wkbLineString | OGRwkbGeometryType::wkbLineString25D => {
            extract_linestring_coords(geom)
        }
        // Polygon — extract exterior ring only
        OGRwkbGeometryType::wkbPolygon | OGRwkbGeometryType::wkbPolygon25D => {
            extract_polygon_exterior_coords(geom)
        }
        // MultiPolygon — extract largest sub-polygon's exterior ring
        OGRwkbGeometryType::wkbMultiPolygon | OGRwkbGeometryType::wkbMultiPolygon25D => {
            let count = geom.geometry_count();
            if count == 0 {
                anyhow::bail!("MultiPolygon has no sub-geometries");
            }
            // Find the sub-polygon with the most points (largest component)
            let mut best_coords = Vec::new();
            for i in 0..count {
                let sub = geom.get_geometry(i);
                if let Ok(coords) = extract_polygon_exterior_coords(&sub) {
                    if coords.len() > best_coords.len() {
                        best_coords = coords;
                    }
                }
            }
            if best_coords.is_empty() {
                anyhow::bail!("MultiPolygon: no valid sub-polygon found");
            }
            debug!(
                sub_count = count,
                coords = best_coords.len(),
                "MultiPolygon: extracted largest sub-polygon"
            );
            Ok(best_coords)
        }
        // MultiLineString — extract longest sub-linestring
        OGRwkbGeometryType::wkbMultiLineString | OGRwkbGeometryType::wkbMultiLineString25D => {
            let count = geom.geometry_count();
            if count == 0 {
                anyhow::bail!("MultiLineString has no sub-geometries");
            }
            let mut best_coords = Vec::new();
            for i in 0..count {
                let sub = geom.get_geometry(i);
                if let Ok(coords) = extract_linestring_coords(&sub) {
                    if coords.len() > best_coords.len() {
                        best_coords = coords;
                    }
                }
            }
            if best_coords.is_empty() {
                anyhow::bail!("MultiLineString: no valid sub-linestring found");
            }
            debug!(
                sub_count = count,
                coords = best_coords.len(),
                "MultiLineString: extracted longest sub-linestring"
            );
            Ok(best_coords)
        }
        other => {
            anyhow::bail!("Unsupported geometry type from intersection: {:?}", other);
        }
    }
}

/// Extract coordinates from a LineString geometry.
fn extract_linestring_coords(geom: &Geometry) -> anyhow::Result<Vec<(f64, f64)>> {
    let point_count = geom.point_count();
    if point_count == 0 {
        anyhow::bail!("LineString has no points");
    }
    let mut coords = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let (x, y, _) = geom.get_point(i as i32);
        validate_coords(x, y)?;
        coords.push((x, y));
    }
    Ok(coords)
}

/// Extract exterior ring coordinates from a Polygon geometry.
fn extract_polygon_exterior_coords(geom: &Geometry) -> anyhow::Result<Vec<(f64, f64)>> {
    let ring_count = geom.geometry_count();
    if ring_count == 0 {
        anyhow::bail!("Polygon has no rings");
    }
    let ring = geom.get_geometry(0); // exterior ring
    let point_count = ring.point_count();
    if point_count == 0 {
        anyhow::bail!("Polygon exterior ring has no points");
    }
    let mut coords = Vec::with_capacity(point_count);
    for i in 0..point_count {
        let (x, y, _) = ring.get_point(i as i32);
        validate_coords(x, y)?;
        coords.push((x, y));
    }
    Ok(coords)
}

/// Validate that coordinates are finite (not NaN or Inf).
fn validate_coords(x: f64, y: f64) -> anyhow::Result<()> {
    if !x.is_finite() || !y.is_finite() {
        anyhow::bail!("Invalid coordinates: x={}, y={} (NaN/Inf detected)", x, y);
    }
    Ok(())
}

// Removed copy_attributes() - no longer needed as we use reader::Feature
// which has attributes as HashMap<String, String> that can be cloned directly
