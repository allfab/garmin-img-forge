// DEM Converter — interpolation and resampling to Garmin DEM grid
//
// Takes multiple ElevationGrids in WGS84, provides height queries
// with bilinear/bicubic interpolation, and generates rectangular
// height arrays for the DEM encoder.

use super::{ElevationGrid, InterpolationMethod};

/// UNDEF value for DEM cells without data
pub const UNDEF: i16 = -32768;

pub struct DemConverter {
    grids: Vec<ElevationGrid>,
    method: InterpolationMethod,
}

impl DemConverter {
    pub fn new(grids: Vec<ElevationGrid>, method: InterpolationMethod) -> Self {
        Self { grids, method }
    }

    /// Get the highest resolution (smallest cellsize) among all grids
    pub fn highest_resolution(&self) -> f64 {
        self.grids.iter()
            .map(|g| g.cellsize_lat.min(g.cellsize_lon))
            .fold(f64::INFINITY, f64::min)
    }

    /// Query height at a single WGS84 point.
    /// Returns None if no grid covers this point or all values are NODATA.
    pub fn get_height(&self, lat: f64, lon: f64) -> Option<i16> {
        for grid in &self.grids {
            if !grid.contains(lat, lon) {
                continue;
            }
            let result = match self.effective_method(grid) {
                InterpolationMethod::Bicubic => {
                    self.interpolate_bicubic(grid, lat, lon)
                        .or_else(|| self.interpolate_bilinear(grid, lat, lon))
                }
                _ => self.interpolate_bilinear(grid, lat, lon),
            };
            if let Some(val) = result {
                return Some(val.round() as i16);
            }
        }
        None
    }

    /// Generate a rectangular grid of heights for a DEM tile.
    /// top_lat/left_lon = NW corner, stepping south and east.
    /// dist_lat/dist_lon in DEM units (converted to degrees via FACTOR).
    pub fn get_heights(
        &self,
        top_lat: f64,
        left_lon: f64,
        rows: u32,
        cols: u32,
        step_lat: f64,
        step_lon: f64,
    ) -> Vec<i16> {
        let mut heights = Vec::with_capacity((rows * cols) as usize);
        for row in 0..rows {
            let lat = top_lat - row as f64 * step_lat;
            for col in 0..cols {
                let lon = left_lon + col as f64 * step_lon;
                heights.push(self.get_height(lat, lon).unwrap_or(UNDEF));
            }
        }
        heights
    }

    /// Determine effective interpolation method for a grid.
    /// Auto: use bicubic when DEM resolution is fine enough relative to source.
    fn effective_method(&self, _grid: &ElevationGrid) -> InterpolationMethod {
        match &self.method {
            InterpolationMethod::Auto => {
                // Default to bilinear for Auto (bicubic used when DEM res > 1/3 source res)
                InterpolationMethod::Bilinear
            }
            other => other.clone(),
        }
    }

    /// Bilinear interpolation using 4 surrounding points
    fn interpolate_bilinear(&self, grid: &ElevationGrid, lat: f64, lon: f64) -> Option<f64> {
        let col_f = (lon - grid.bounds.west) / grid.cellsize_lon;
        let row_f = (grid.bounds.north - lat) / grid.cellsize_lat;

        if col_f < 0.0 || row_f < 0.0 {
            return None;
        }

        let c0 = col_f.floor() as u32;
        let r0 = row_f.floor() as u32;

        if c0 + 1 >= grid.width || r0 + 1 >= grid.height {
            // Edge case: exact boundary → try nearest
            let c = col_f.round() as u32;
            let r = row_f.round() as u32;
            return grid.get(r, c);
        }

        let qx = col_f - c0 as f64;
        let qy = row_f - r0 as f64;

        let h00 = grid.get(r0, c0)?;
        let h10 = grid.get(r0, c0 + 1)?;
        let h01 = grid.get(r0 + 1, c0)?;
        let h11 = grid.get(r0 + 1, c0 + 1)?;

        Some(
            (1.0 - qy) * ((1.0 - qx) * h00 + qx * h10)
                + qy * ((1.0 - qx) * h01 + qx * h11)
        )
    }

    /// Bicubic (Catmull-Rom) interpolation using 16 surrounding points (4×4)
    fn interpolate_bicubic(&self, grid: &ElevationGrid, lat: f64, lon: f64) -> Option<f64> {
        let col_f = (lon - grid.bounds.west) / grid.cellsize_lon;
        let row_f = (grid.bounds.north - lat) / grid.cellsize_lat;

        if col_f < 1.0 || row_f < 1.0 {
            return None;
        }

        let c1 = col_f.floor() as i32;
        let r1 = row_f.floor() as i32;

        // Need 4×4 grid: (r1-1..r1+2) × (c1-1..c1+2)
        if c1 < 1 || r1 < 1 || c1 + 2 >= grid.width as i32 || r1 + 2 >= grid.height as i32 {
            return None;
        }

        let qx = col_f - c1 as f64;
        let qy = row_f - r1 as f64;

        // Get 4×4 values
        let mut vals = [[0.0f64; 4]; 4];
        for dr in 0..4i32 {
            for dc in 0..4i32 {
                let r = (r1 - 1 + dr) as u32;
                let c = (c1 - 1 + dc) as u32;
                vals[dr as usize][dc as usize] = grid.get(r, c)?;
            }
        }

        // Catmull-Rom spline in x for each row, then in y
        let mut row_vals = [0.0f64; 4];
        for i in 0..4 {
            row_vals[i] = catmull_rom(qx, vals[i][0], vals[i][1], vals[i][2], vals[i][3]);
        }

        Some(catmull_rom(qy, row_vals[0], row_vals[1], row_vals[2], row_vals[3]))
    }
}

/// Catmull-Rom spline interpolation
fn catmull_rom(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dem::GeoBounds;

    fn make_grid(width: u32, height: u32, data: Vec<f64>) -> ElevationGrid {
        ElevationGrid {
            width,
            height,
            data,
            nodata: -99999.0,
            bounds: GeoBounds {
                south: 45.0,
                west: 5.0,
                north: 45.0 + (height - 1) as f64 * 0.001,
                east: 5.0 + (width - 1) as f64 * 0.001,
            },
            cellsize_lat: 0.001,
            cellsize_lon: 0.001,
        }
    }

    #[test]
    fn test_bilinear_interpolation_center() {
        // 2×2 grid: [[100, 200], [300, 400]]
        let grid = make_grid(2, 2, vec![100.0, 200.0, 300.0, 400.0]);
        let conv = DemConverter::new(vec![grid], InterpolationMethod::Bilinear);

        // Center point should be average of all 4
        let h = conv.get_height(45.0005, 5.0005);
        assert!(h.is_some());
        let val = h.unwrap();
        assert_eq!(val, 250); // (100+200+300+400)/4
    }

    #[test]
    fn test_bilinear_interpolation_corner() {
        let grid = make_grid(2, 2, vec![100.0, 200.0, 300.0, 400.0]);
        let conv = DemConverter::new(vec![grid], InterpolationMethod::Bilinear);

        // NW corner (exact grid point)
        let h = conv.get_height(45.001, 5.0);
        assert!(h.is_some());
        assert_eq!(h.unwrap(), 100);
    }

    #[test]
    fn test_bicubic_interpolation() {
        // 6×6 grid for bicubic (needs 4×4 neighborhood)
        let mut data = Vec::new();
        for r in 0..6 {
            for c in 0..6 {
                data.push((r * 100 + c * 10) as f64);
            }
        }
        let grid = make_grid(6, 6, data);
        let conv = DemConverter::new(vec![grid], InterpolationMethod::Bicubic);

        // Query a point inside the valid bicubic range
        let h = conv.get_height(45.0035, 5.0025);
        assert!(h.is_some());
    }

    #[test]
    fn test_get_heights_grid() {
        let grid = make_grid(2, 2, vec![100.0, 200.0, 300.0, 400.0]);
        let conv = DemConverter::new(vec![grid], InterpolationMethod::Bilinear);

        let heights = conv.get_heights(
            45.001, 5.0, // NW corner
            2, 2,        // 2×2 output
            0.001, 0.001 // step = cellsize
        );
        assert_eq!(heights.len(), 4);
        assert_eq!(heights[0], 100); // NW
        assert_eq!(heights[1], 200); // NE
    }

    #[test]
    fn test_no_coverage() {
        let grid = make_grid(2, 2, vec![100.0, 200.0, 300.0, 400.0]);
        let conv = DemConverter::new(vec![grid], InterpolationMethod::Bilinear);

        // Point outside grid bounds
        let h = conv.get_height(50.0, 10.0);
        assert!(h.is_none());
    }

    #[test]
    fn test_catmull_rom_midpoint() {
        // For uniform values, midpoint should return the same value
        let result = catmull_rom(0.5, 100.0, 100.0, 100.0, 100.0);
        assert!((result - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_multiple_grids_priority() {
        // First grid should have priority
        let grid1 = make_grid(2, 2, vec![100.0, 100.0, 100.0, 100.0]);
        let grid2 = make_grid(2, 2, vec![999.0, 999.0, 999.0, 999.0]);
        let conv = DemConverter::new(vec![grid1, grid2], InterpolationMethod::Bilinear);

        let h = conv.get_height(45.0005, 5.0005);
        assert_eq!(h.unwrap(), 100); // First grid wins
    }
}
