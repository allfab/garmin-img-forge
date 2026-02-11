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

    /// Assign features to tiles using R-tree spatial queries.
    ///
    /// For each tile, queries the R-tree to find candidate features whose bboxes
    /// intersect the tile bbox (with overlap). Empty tiles are automatically skipped.
    ///
    /// # Arguments
    /// * `rtree` - Spatial index for efficient queries
    /// * `tiles` - Tile boundaries to process
    ///
    /// # Returns
    /// * `Vec<(TileBounds, Vec<usize>)>` - Non-empty tiles with feature IDs
    ///
    /// # Performance
    /// * Query complexity: O(log n + k) per tile where k = candidates
    ///   (vs O(n) naive iteration over all features, where n = total feature count)
    /// * Empty tiles are filtered out to avoid processing overhead downstream
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
    /// # use mpforge_cli::pipeline::tiler::TileBounds;
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
    /// # use mpforge_cli::pipeline::tiler::TileBounds;
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
        ValidationResult::Rejected(_) => {
            // Logging already done by validate_and_repair() — no duplicate error log
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

    Geometry::from_wkt(&wkt).map_err(|e| anyhow::anyhow!("WKT conversion failed: {}", e))
}

/// Extract coordinates from GDAL Geometry to internal format.
///
/// # Limitations
/// This is a simplified WKT parser that handles Point, LineString, and simple Polygon.
/// It may fail on:
/// - MultiPolygon, MultiLineString, GeometryCollection
/// - Polygons with interior rings (holes)
/// - Nested complex geometries
///
/// For MVP, this is acceptable as GDAL Intersection typically returns simple geometries
/// for tile clipping. Future improvement: use GDAL API `get_point()` directly.
fn gdal_geometry_to_coords(geom: &Geometry) -> anyhow::Result<Vec<(f64, f64)>> {
    let wkt = geom.wkt().map_err(|e| anyhow::anyhow!("Failed to get WKT: {}", e))?;

    // Parse WKT to extract coordinates
    // This is a simplified parser - handles Point, LineString, and Polygon
    let coords_str = wkt
        .split_once('(')
        .and_then(|(_, rest)| rest.rsplit_once(')'))
        .map(|(coords, _)| coords)
        .ok_or_else(|| anyhow::anyhow!("Invalid WKT format"))?;

    // Remove extra parentheses for Polygon
    let coords_str = coords_str.trim_start_matches('(').trim_end_matches(')');

    // Parse coordinate pairs
    let mut coords = Vec::new();
    for pair in coords_str.split(',') {
        let parts: Vec<&str> = pair.split_whitespace().collect();
        if parts.len() >= 2 {
            let x: f64 = parts[0].parse()?;
            let y: f64 = parts[1].parse()?;
            coords.push((x, y));
        }
    }

    if coords.is_empty() {
        anyhow::bail!("No coordinates found in geometry");
    }

    Ok(coords)
}

// Removed copy_attributes() - no longer needed as we use reader::Feature
// which has attributes as HashMap<String, String> that can be cloned directly
