//! Spatial tiling and grid management.

use crate::config::{FilterConfig, GridConfig};
use crate::pipeline::reader::RTreeIndex;
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
}

/// Placeholder for tile data with features.
/// TODO: Story 6.3 - Define complete tile data structure
#[allow(dead_code)] // Stub - will be implemented in Story 6.3
#[derive(Debug)]
pub struct TileData {
    pub tile_id: String,
}
