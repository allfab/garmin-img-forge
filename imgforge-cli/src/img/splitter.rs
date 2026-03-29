// MapSplitter — subdivision splitting, faithful to mkgmap MapSplitter.java

use super::area::Area;
use super::coord::Coord;

/// Splitting limits from mkgmap MapSplitter.java
pub const MAX_DIVISION_SIZE: i32 = 0x7FFF;
pub const MAX_RGN_SIZE: usize = 0xFFF8;
pub const MAX_NUM_LINES: usize = 0xFF;
pub const MAX_NUM_POINTS: usize = 0xFF;
pub const WANTED_MAX_AREA_SIZE: i32 = 0x3FFF;
pub const MIN_DIMENSION: i32 = 10;

/// A map area containing features, ready for splitting
#[derive(Debug, Clone)]
pub struct MapArea {
    pub bounds: Area,
    pub num_points: usize,
    pub num_lines: usize,
    pub num_polygons: usize,
    pub estimated_rgn_size: usize,
}

impl MapArea {
    pub fn new(bounds: Area) -> Self {
        Self {
            bounds,
            num_points: 0,
            num_lines: 0,
            num_polygons: 0,
            estimated_rgn_size: 0,
        }
    }

    /// Check if this area needs to be split
    pub fn needs_split(&self) -> bool {
        self.num_points > MAX_NUM_POINTS
            || self.num_lines > MAX_NUM_LINES
            || self.estimated_rgn_size > MAX_RGN_SIZE
            || self.bounds.max_dimension() > WANTED_MAX_AREA_SIZE
    }

    /// Split this area into sub-areas — mkgmap MapSplitter logic
    pub fn split(&self, resolution_shift: i32) -> Vec<Area> {
        let width = self.bounds.width();
        let height = self.bounds.height();

        let (xsplit, ysplit) = if width > height {
            (2, 1)
        } else {
            (1, 2)
        };

        if let Some(areas) = self.bounds.split(xsplit, ysplit, resolution_shift) {
            areas
        } else {
            vec![self.bounds]
        }
    }
}

/// Recursively split a map area until all sub-areas fit within limits
pub fn split_area(area: &MapArea, resolution_shift: i32, max_depth: usize) -> Vec<Area> {
    if !area.needs_split() || max_depth == 0 {
        return vec![area.bounds];
    }

    let sub_areas = area.split(resolution_shift);
    let mut result = Vec::new();

    for sub_bounds in sub_areas {
        if sub_bounds.width() < MIN_DIMENSION && sub_bounds.height() < MIN_DIMENSION {
            result.push(sub_bounds);
            continue;
        }

        // Create sub-area with proportionally distributed features
        let fraction = estimate_fraction(&area.bounds, &sub_bounds);
        let sub = MapArea {
            bounds: sub_bounds,
            num_points: (area.num_points as f64 * fraction) as usize,
            num_lines: (area.num_lines as f64 * fraction) as usize,
            num_polygons: (area.num_polygons as f64 * fraction) as usize,
            estimated_rgn_size: (area.estimated_rgn_size as f64 * fraction) as usize,
        };

        result.extend(split_area(&sub, resolution_shift, max_depth - 1));
    }

    result
}

fn estimate_fraction(parent: &Area, child: &Area) -> f64 {
    let parent_area = parent.width() as f64 * parent.height() as f64;
    if parent_area == 0.0 {
        return 1.0;
    }
    let child_area = child.width() as f64 * child.height() as f64;
    (child_area / parent_area).min(1.0)
}

/// Distribute coords into sub-areas by their center point
pub fn distribute_coords(coords: &[Coord], areas: &[Area]) -> Vec<Vec<usize>> {
    let mut distribution = vec![Vec::new(); areas.len()];
    for (idx, coord) in coords.iter().enumerate() {
        for (area_idx, area) in areas.iter().enumerate() {
            if area.contains_coord(coord) {
                distribution[area_idx].push(idx);
                break;
            }
        }
    }
    distribution
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_split_by_points() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100));
        area.num_points = 300;
        assert!(area.needs_split());
    }

    #[test]
    fn test_needs_split_by_area() {
        let area = MapArea::new(Area::new(0, 0, 0x5000, 0x5000));
        assert!(area.needs_split());
    }

    #[test]
    fn test_no_split_needed() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100));
        area.num_points = 10;
        area.num_lines = 10;
        assert!(!area.needs_split());
    }

    #[test]
    fn test_split_produces_multiple_areas() {
        let area = MapArea::new(Area::new(0, 0, 1000, 1000));
        let parts = area.split(0);
        assert!(parts.len() >= 2);
    }

    #[test]
    fn test_recursive_split() {
        let mut area = MapArea::new(Area::new(0, 0, 0x8000, 0x8000));
        area.num_points = 1000;
        area.estimated_rgn_size = 0x20000;
        let result = split_area(&area, 0, 10);
        assert!(result.len() > 1);
    }

    #[test]
    fn test_distribute_coords() {
        let areas = vec![
            Area::new(0, 0, 50, 50),
            Area::new(50, 50, 100, 100),
        ];
        let coords = vec![
            Coord::new(25, 25),  // area 0
            Coord::new(75, 75),  // area 1
        ];
        let dist = distribute_coords(&coords, &areas);
        assert_eq!(dist[0], vec![0]);
        assert_eq!(dist[1], vec![1]);
    }
}
