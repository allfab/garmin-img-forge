// MapSplitter — subdivision splitting, faithful to mkgmap MapSplitter.java + MapArea.java
//
// Key algorithms:
// - pickArea: distribution by first point (integer division in grid)
// - splitMaxSize: initial grid split to respect MAX_DIVISION_SIZE
// - addAreasToList: recursive re-split until all areas fit limits
// - Sutherland-Hodgman: polygon clipping against axis-aligned rectangles
// - Large objects: dedicated subdivision when feature exceeds cell dimensions

use super::area::Area;
use super::coord::Coord;

// ── Splitting limits from mkgmap MapSplitter.java ──────────────────────────

pub const MAX_DIVISION_SIZE: i32 = 0x7FFF;
pub const MAX_RGN_SIZE: usize = 0xFFF8; // 65528 bytes
pub const MAX_NUM_LINES: usize = 0xFF;
pub const MAX_NUM_POINTS: usize = 0xFF;
pub const WANTED_MAX_AREA_SIZE: usize = 0x3FFF; // 16383 bytes
pub const MIN_DIMENSION: i32 = 10;
pub const LARGE_OBJECT_DIM: i32 = 8192;

// Size estimation — mkgmap MapArea.addSize
const POINT_SIZE: usize = 9;
const LINE_OVERHEAD: usize = 11;
const LINE_POINT_SIZE: usize = 4;
const SHAPE_OVERHEAD: usize = 11;
const SHAPE_POINT_SIZE: usize = 4;

// ── Feature types for splitting ────────────────────────────────────────────

/// A point feature — carries original index for writer lookup
#[derive(Debug, Clone)]
pub struct SplitPoint {
    pub mp_index: usize,
    pub location: Coord,
}

/// A line feature — distributed by first point, never clipped
#[derive(Debug, Clone)]
pub struct SplitLine {
    pub mp_index: usize,
    pub points: Vec<Coord>,
}

/// A shape (polygon) feature — may be original or clipped fragment
#[derive(Debug, Clone)]
pub struct SplitShape {
    pub mp_index: usize,
    pub points: Vec<Coord>,
}

// ── MapArea — container with real features ─────────────────────────────────

/// A map area containing features, faithful to mkgmap MapArea.java
#[derive(Debug, Clone)]
pub struct MapArea {
    pub bounds: Area,
    pub points: Vec<SplitPoint>,
    pub lines: Vec<SplitLine>,
    pub shapes: Vec<SplitShape>,
    /// Estimated RGN sizes: [points, lines, shapes]
    sizes: [usize; 3],
    pub resolution: u8,
}

impl MapArea {
    pub fn new(bounds: Area, resolution: u8) -> Self {
        Self {
            bounds,
            points: Vec::new(),
            lines: Vec::new(),
            shapes: Vec::new(),
            sizes: [0; 3],
            resolution,
        }
    }

    /// Create from pre-built split features
    pub fn from_split_features(
        bounds: Area,
        resolution: u8,
        points: Vec<SplitPoint>,
        lines: Vec<SplitLine>,
        shapes: Vec<SplitShape>,
    ) -> Self {
        let mut area = Self::new(bounds, resolution);
        for pt in points { area.add_point(pt); }
        for line in lines { area.add_line(line); }
        for shape in shapes { area.add_shape(shape); }
        area
    }

    /// Add a point — mkgmap addSize: +9 bytes
    pub fn add_point(&mut self, pt: SplitPoint) {
        self.points.push(pt);
        self.sizes[0] += POINT_SIZE;
    }

    /// Add a line — mkgmap addSize: 11 + numPoints * 4
    pub fn add_line(&mut self, line: SplitLine) {
        self.sizes[1] += LINE_OVERHEAD + line.points.len() * LINE_POINT_SIZE;
        self.lines.push(line);
    }

    /// Add a shape — mkgmap addSize: 11 + numPoints * 4
    pub fn add_shape(&mut self, shape: SplitShape) {
        self.sizes[2] += SHAPE_OVERHEAD + shape.points.len() * SHAPE_POINT_SIZE;
        self.shapes.push(shape);
    }

    pub fn total_size(&self) -> usize {
        self.sizes[0] + self.sizes[1] + self.sizes[2]
    }

    /// mkgmap MapArea.hasData — no empty subdivisions
    pub fn has_data(&self) -> bool {
        !self.points.is_empty() || !self.lines.is_empty() || !self.shapes.is_empty()
    }

    pub fn has_points(&self) -> bool { !self.points.is_empty() }
    pub fn has_lines(&self) -> bool { !self.lines.is_empty() }
    pub fn has_shapes(&self) -> bool { !self.shapes.is_empty() }

    /// Must split — mkgmap hard limits (MapSplitter.addAreasToList)
    fn must_split(&self) -> bool {
        self.lines.len() > MAX_NUM_LINES
            || self.shapes.len() > MAX_NUM_LINES // polygons share the line limit
            || self.points.len() > MAX_NUM_POINTS
            || self.total_size() > MAX_RGN_SIZE
    }

    /// Want split — mkgmap soft limits
    fn want_split(&self) -> bool {
        self.bounds.max_dimension() > MIN_DIMENSION
            && self.total_size() > WANTED_MAX_AREA_SIZE
    }

    /// Split into nx*ny sub-areas with real feature distribution — mkgmap MapArea.split
    pub fn split(&self, nx: usize, ny: usize) -> Vec<MapArea> {
        let shift = (24i32 - self.resolution as i32).max(0);
        let sub_bounds = match self.bounds.split(nx, ny, shift) {
            Some(areas) if areas.len() > 1 => areas,
            _ => return vec![self.clone()],
        };

        let num_areas = sub_bounds.len();
        let mut sub_areas: Vec<MapArea> = sub_bounds
            .iter()
            .map(|b| MapArea::new(*b, self.resolution))
            .collect();

        // Grid parameters for pickArea (integer division)
        let xbase = self.bounds.min_lon() as i64;
        let ybase = self.bounds.min_lat() as i64;
        let dx = if nx > 1 {
            self.bounds.width() as i64 / nx as i64
        } else {
            self.bounds.width() as i64
        };
        let dy = if ny > 1 {
            self.bounds.height() as i64 / ny as i64
        } else {
            self.bounds.height() as i64
        };

        // ── Points: distribute by location ──
        for pt in &self.points {
            let idx = pick_area(
                pt.location.longitude() as i64,
                pt.location.latitude() as i64,
                xbase, ybase, nx, ny, dx, dy, num_areas,
            );
            sub_areas[idx].add_point(pt.clone());
        }

        // Max cell dimensions for large object detection — mkgmap MapArea.split
        let max_cell_w = (self.bounds.width() / nx.max(1) as i32)
            .min(MAX_DIVISION_SIZE / 2)
            .max(LARGE_OBJECT_DIM * 2);
        let max_cell_h = (self.bounds.height() / ny.max(1) as i32)
            .min(MAX_DIVISION_SIZE / 2)
            .max(LARGE_OBJECT_DIM * 2);

        // ── Lines: distribute by first point, no clipping ──
        for line in &self.lines {
            if line.points.is_empty() {
                continue;
            }
            let first = &line.points[0];
            let idx = pick_area(
                first.longitude() as i64,
                first.latitude() as i64,
                xbase, ybase, nx, ny, dx, dy, num_areas,
            );

            // Large object: if line bounds exceed cell dimensions, create dedicated area
            let line_bbox = Area::from_coords(&line.points);
            if line_bbox.width() > max_cell_w || line_bbox.height() > max_cell_h {
                let mut dedicated = MapArea::new(line_bbox, self.resolution);
                dedicated.add_line(line.clone());
                sub_areas.push(dedicated);
            } else {
                sub_areas[idx].add_line(line.clone());
            }
        }

        // ── Shapes: distribute by first point, clip if spanning multiple areas ──
        for shape in &self.shapes {
            if shape.points.len() < 3 {
                continue;
            }
            let shape_bbox = Area::from_coords(&shape.points);
            let first = &shape.points[0];
            let target = pick_area(
                first.longitude() as i64,
                first.latitude() as i64,
                xbase, ybase, nx, ny, dx, dy, num_areas,
            );

            if sub_areas[target].bounds.contains_area(&shape_bbox) {
                // Entire shape fits in target area — no clipping needed
                sub_areas[target].add_shape(shape.clone());
            } else {
                // Shape spans multiple areas — clip to ALL overlapping areas
                // including target (mkgmap splitIntoAreas behavior)
                for (i, bounds) in sub_bounds.iter().enumerate() {
                    if !bounds.intersects(&shape_bbox) {
                        continue;
                    }
                    let clipped = clip_polygon_to_rect(&shape.points, bounds);
                    if clipped.len() >= 3 {
                        sub_areas[i].add_shape(SplitShape {
                            mp_index: shape.mp_index,
                            points: clipped,
                        });
                    }
                }
            }
        }

        sub_areas
    }
}

// ── pickArea — distribution by first point ─────────────────────────────────

/// mkgmap MapArea.pickArea — integer division in grid
fn pick_area(
    x: i64, y: i64,
    xbase: i64, ybase: i64,
    nx: usize, ny: usize,
    dx: i64, dy: i64,
    num_areas: usize,
) -> usize {
    let xcell = if dx > 0 {
        ((x - xbase) / dx).clamp(0, (nx as i64) - 1) as usize
    } else {
        0
    };
    let ycell = if dy > 0 {
        ((y - ybase) / dy).clamp(0, (ny as i64) - 1) as usize
    } else {
        0
    };
    (xcell * ny + ycell).min(num_areas - 1)
}

// ── Polygon clipping — Sutherland-Hodgman ──────────────────────────────────

/// Sutherland-Hodgman polygon clipping against an axis-aligned rectangle
pub fn clip_polygon_to_rect(polygon: &[Coord], rect: &Area) -> Vec<Coord> {
    if polygon.len() < 3 {
        return Vec::new();
    }

    let mut output = polygon.to_vec();

    // Clip against each edge: left, right, bottom, top
    output = clip_edge(&output, Edge::Left, rect.min_lon());
    if output.len() < 3 { return Vec::new(); }

    output = clip_edge(&output, Edge::Right, rect.max_lon());
    if output.len() < 3 { return Vec::new(); }

    output = clip_edge(&output, Edge::Bottom, rect.min_lat());
    if output.len() < 3 { return Vec::new(); }

    output = clip_edge(&output, Edge::Top, rect.max_lat());

    output
}

#[derive(Clone, Copy)]
enum Edge {
    Left,   // x >= boundary (longitude)
    Right,  // x <= boundary
    Bottom, // y >= boundary (latitude)
    Top,    // y <= boundary
}

fn clip_edge(polygon: &[Coord], edge: Edge, boundary: i32) -> Vec<Coord> {
    if polygon.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::new();
    let n = polygon.len();

    for i in 0..n {
        let current = polygon[i];
        let next = polygon[(i + 1) % n];

        let cur_inside = is_inside(current, edge, boundary);
        let next_inside = is_inside(next, edge, boundary);

        if cur_inside {
            output.push(current);
            if !next_inside {
                if let Some(p) = intersect_edge(current, next, edge, boundary) {
                    output.push(p);
                }
            }
        } else if next_inside {
            if let Some(p) = intersect_edge(current, next, edge, boundary) {
                output.push(p);
            }
        }
    }

    output
}

fn is_inside(c: Coord, edge: Edge, boundary: i32) -> bool {
    match edge {
        Edge::Left => c.longitude() >= boundary,
        Edge::Right => c.longitude() <= boundary,
        Edge::Bottom => c.latitude() >= boundary,
        Edge::Top => c.latitude() <= boundary,
    }
}

/// Compute intersection of segment [a,b] with a clip edge.
/// Returns None when segment is parallel to the edge (dx=0 for L/R, dy=0 for B/T).
/// This is correct: in Sutherland-Hodgman, intersect is only called when one endpoint
/// is inside and one outside. A parallel segment cannot have one inside/one outside
/// for the same axis, so this path is unreachable in practice.
fn intersect_edge(a: Coord, b: Coord, edge: Edge, boundary: i32) -> Option<Coord> {
    let (ax, ay) = (a.longitude() as i64, a.latitude() as i64);
    let (bx, by) = (b.longitude() as i64, b.latitude() as i64);

    match edge {
        Edge::Left | Edge::Right => {
            let dx = bx - ax;
            if dx == 0 { return None; }
            let t_num = boundary as i64 - ax;
            let y = ay + t_num * (by - ay) / dx;
            Some(Coord::new(y as i32, boundary))
        }
        Edge::Bottom | Edge::Top => {
            let dy = by - ay;
            if dy == 0 { return None; }
            let t_num = boundary as i64 - ay;
            let x = ax + t_num * (bx - ax) / dy;
            Some(Coord::new(boundary, x as i32))
        }
    }
}

// ── splitMaxSize — initial grid split ──────────────────────────────────────

/// mkgmap MapSplitter.splitMaxSize — divide into cells ≤ MAX_DIVISION_SIZE
pub fn split_max_size(area: &MapArea, shift: i32) -> Vec<MapArea> {
    let effective_width = if shift > 0 {
        area.bounds.width() >> shift
    } else {
        area.bounds.width()
    };
    let effective_height = if shift > 0 {
        area.bounds.height() >> shift
    } else {
        area.bounds.height()
    };

    let xsplit = (effective_width / MAX_DIVISION_SIZE + 1).max(1) as usize;
    let ysplit = (effective_height / MAX_DIVISION_SIZE + 1).max(1) as usize;

    if xsplit <= 1 && ysplit <= 1 {
        return vec![area.clone()];
    }

    area.split(xsplit, ysplit)
}

// ── addAreasToList — recursive post-split ──────────────────────────────────

/// mkgmap MapSplitter.addAreasToList — recursive split until all areas fit
pub fn add_areas_to_list(areas: Vec<MapArea>, max_depth: usize) -> Vec<MapArea> {
    let mut result = Vec::new();

    for area in areas {
        if !area.has_data() {
            continue;
        }
        add_area_recursive(area, max_depth, &mut result);
    }

    result
}

fn add_area_recursive(area: MapArea, depth: usize, result: &mut Vec<MapArea>) {
    let need_split = area.must_split() || area.want_split();

    if !need_split || depth == 0 {
        result.push(area);
        return;
    }

    if area.bounds.max_dimension() <= MIN_DIMENSION {
        // Too small to divide further (tooSmallToDivide)
        if area.must_split() {
            eprintln!(
                "WARNING: subdivision too small to divide but exceeds limits \
                 (pts={}, lines={}, shapes={}, rgn={}B)",
                area.points.len(), area.lines.len(), area.shapes.len(), area.total_size()
            );
        }
        result.push(area);
        return;
    }

    // Split in 2: horizontal or vertical based on aspect ratio
    let (nx, ny) = if area.bounds.width() > area.bounds.height() {
        (2, 1)
    } else {
        (1, 2)
    };

    let sub_areas = area.split(nx, ny);
    if sub_areas.len() <= 1 {
        result.push(sub_areas.into_iter().next().unwrap_or(area));
        return;
    }

    for sub in sub_areas {
        if sub.has_data() {
            add_area_recursive(sub, depth - 1, result);
        }
    }
}

// ── Public entry point ─────────────────────────────────────────────────────

/// Split features into subdivisions — mkgmap MapSplitter.split
///
/// Returns a list of MapArea, each containing the features for one subdivision.
/// Empty areas are filtered out (mkgmap hasData).
pub fn split_features(
    bounds: Area,
    resolution: u8,
    points: Vec<SplitPoint>,
    lines: Vec<SplitLine>,
    shapes: Vec<SplitShape>,
) -> Vec<MapArea> {
    let area = MapArea::from_split_features(bounds, resolution, points, lines, shapes);

    if !area.has_data() {
        return Vec::new();
    }

    let shift = (24i32 - resolution as i32).max(0);

    // Step 1: Split to max subdivision size
    let initial = split_max_size(&area, shift);

    // Step 2: Recursive splitting until all areas fit limits
    add_areas_to_list(initial, 8)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(lat: i32, lon: i32) -> Coord {
        Coord::new(lat, lon)
    }

    // ── pickArea tests ──

    #[test]
    fn test_pick_area_2x2_grid() {
        // 4 cells in a 100x100 area
        let idx = pick_area(25, 25, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 0); // cell (0,0)

        let idx = pick_area(75, 25, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 2); // cell (1,0)

        let idx = pick_area(25, 75, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 1); // cell (0,1)

        let idx = pick_area(75, 75, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 3); // cell (1,1)
    }

    #[test]
    fn test_pick_area_clamped() {
        // Out of bounds → clamped
        let idx = pick_area(-10, -10, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 0);

        let idx = pick_area(200, 200, 0, 0, 2, 2, 50, 50, 4);
        assert_eq!(idx, 3);
    }

    // ── MapArea tests ──

    #[test]
    fn test_add_size_estimation() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);

        area.add_point(SplitPoint { mp_index: 0, location: pt(50, 50) });
        assert_eq!(area.total_size(), 9); // POINT_SIZE

        area.add_line(SplitLine {
            mp_index: 0,
            points: vec![pt(0, 0), pt(10, 10), pt(20, 20)],
        });
        assert_eq!(area.total_size(), 9 + 11 + 3 * 4); // 9 + 23 = 32

        area.add_shape(SplitShape {
            mp_index: 0,
            points: vec![pt(0, 0), pt(10, 0), pt(10, 10), pt(0, 10)],
        });
        assert_eq!(area.total_size(), 32 + 11 + 4 * 4); // 32 + 27 = 59
    }

    #[test]
    fn test_has_data() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);
        assert!(!area.has_data());

        area.add_point(SplitPoint { mp_index: 0, location: pt(50, 50) });
        assert!(area.has_data());
    }

    #[test]
    fn test_must_split_by_points() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);
        for i in 0..=MAX_NUM_POINTS {
            area.add_point(SplitPoint { mp_index: i, location: pt(50, 50) });
        }
        assert!(area.must_split());
    }

    #[test]
    fn test_must_split_by_rgn_size() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);
        // Each 100-point line = 11 + 100*4 = 411 bytes. 160 lines ≈ 65760 > MAX_RGN_SIZE
        for i in 0..160 {
            area.add_line(SplitLine {
                mp_index: i,
                points: (0..100).map(|j| pt(j, j)).collect(),
            });
        }
        assert!(area.must_split());
    }

    #[test]
    fn test_no_split_small_area() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);
        area.add_point(SplitPoint { mp_index: 0, location: pt(50, 50) });
        assert!(!area.must_split());
        assert!(!area.want_split());
    }

    // ── split distribution tests ──

    #[test]
    fn test_split_distributes_points() {
        let bounds = Area::new(0, 0, 1000, 1000);
        let mut area = MapArea::new(bounds, 24);

        // 4 points in 4 quadrants
        area.add_point(SplitPoint { mp_index: 0, location: pt(250, 250) });
        area.add_point(SplitPoint { mp_index: 1, location: pt(750, 250) });
        area.add_point(SplitPoint { mp_index: 2, location: pt(250, 750) });
        area.add_point(SplitPoint { mp_index: 3, location: pt(750, 750) });

        let subs = area.split(2, 2);
        assert!(subs.len() >= 2);

        let total: usize = subs.iter().map(|s| s.points.len()).sum();
        assert_eq!(total, 4);

        // No empty subdivisions (all have data)
        for sub in &subs {
            assert!(sub.has_data());
        }
    }

    #[test]
    fn test_split_lines_by_first_point() {
        let bounds = Area::new(0, 0, 1000, 1000);
        let mut area = MapArea::new(bounds, 24);

        // Line starts in bottom-left, extends to top-right
        area.add_line(SplitLine {
            mp_index: 0,
            points: vec![pt(100, 100), pt(900, 900)],
        });

        let subs = area.split(2, 2);
        let total_lines: usize = subs.iter().map(|s| s.lines.len()).sum();
        assert_eq!(total_lines, 1); // line assigned to exactly one area
    }

    // ── Sutherland-Hodgman tests ──

    #[test]
    fn test_clip_polygon_inside() {
        let rect = Area::new(0, 0, 100, 100);
        let poly = vec![pt(10, 10), pt(10, 90), pt(90, 90), pt(90, 10)];
        let clipped = clip_polygon_to_rect(&poly, &rect);
        assert_eq!(clipped.len(), 4); // entirely inside
    }

    #[test]
    fn test_clip_polygon_outside() {
        let rect = Area::new(0, 0, 100, 100);
        let poly = vec![pt(200, 200), pt(200, 300), pt(300, 300), pt(300, 200)];
        let clipped = clip_polygon_to_rect(&poly, &rect);
        assert!(clipped.len() < 3); // entirely outside
    }

    #[test]
    fn test_clip_polygon_straddling() {
        let rect = Area::new(0, 0, 100, 100);
        // Square from (-50,-50) to (50,50) — half inside
        let poly = vec![pt(-50, -50), pt(-50, 50), pt(50, 50), pt(50, -50)];
        let clipped = clip_polygon_to_rect(&poly, &rect);
        assert!(clipped.len() >= 3);

        // All clipped points should be within rect
        for c in &clipped {
            assert!(c.latitude() >= 0);
            assert!(c.longitude() >= 0);
            assert!(c.latitude() <= 100);
            assert!(c.longitude() <= 100);
        }
    }

    #[test]
    fn test_clip_triangle_corner() {
        let rect = Area::new(0, 0, 100, 100);
        // Triangle with one vertex inside, two outside
        let poly = vec![pt(50, 50), pt(150, 50), pt(50, 150)];
        let clipped = clip_polygon_to_rect(&poly, &rect);
        assert!(clipped.len() >= 3);
    }

    // ── split_max_size tests ──

    #[test]
    fn test_split_max_size_small_area() {
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 24);
        area.add_point(SplitPoint { mp_index: 0, location: pt(50, 50) });
        let result = split_max_size(&area, 0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_split_max_size_large_area() {
        let mut area = MapArea::new(Area::new(0, 0, 0x10000, 0x10000), 24);
        area.add_point(SplitPoint { mp_index: 0, location: pt(50, 50) });
        let result = split_max_size(&area, 0);
        assert!(result.len() > 1);
    }

    // ── add_areas_to_list tests ──

    #[test]
    fn test_add_areas_filters_empty() {
        let areas = vec![
            MapArea::new(Area::new(0, 0, 100, 100), 24), // empty
        ];
        let result = add_areas_to_list(areas, 8);
        assert!(result.is_empty());
    }

    #[test]
    fn test_add_areas_splits_oversized() {
        let mut area = MapArea::new(Area::new(0, 0, 10000, 10000), 24);
        // Add 300 points (> MAX_NUM_POINTS) spread across the area
        for i in 0..300 {
            let lat = (i * 33) % 10000;
            let lon = (i * 47) % 10000;
            area.add_point(SplitPoint { mp_index: i, location: pt(lat as i32, lon as i32) });
        }

        let result = add_areas_to_list(vec![area], 8);
        assert!(result.len() > 1);

        let total: usize = result.iter().map(|a| a.points.len()).sum();
        assert_eq!(total, 300);

        // All results should have data
        for a in &result {
            assert!(a.has_data());
        }
    }

    // ── split_features integration test ──

    #[test]
    fn test_split_features_preserves_all() {
        let bounds = Area::new(0, 0, 50000, 50000);
        let mut points = Vec::new();
        let mut lines = Vec::new();
        let mut shapes = Vec::new();

        for i in 0..50 {
            let lat = (i * 997) % 50000;
            let lon = (i * 1013) % 50000;
            points.push(SplitPoint { mp_index: i, location: pt(lat as i32, lon as i32) });
        }

        for i in 0..20 {
            let y = (i * 2500) % 50000;
            lines.push(SplitLine {
                mp_index: i,
                points: vec![pt(y as i32, 100), pt(y as i32 + 100, 200)],
            });
        }

        for i in 0..10 {
            let y = (i * 5000) % 50000;
            let x = (i * 3000) % 50000;
            shapes.push(SplitShape {
                mp_index: i,
                points: vec![
                    pt(y as i32, x as i32),
                    pt(y as i32 + 100, x as i32),
                    pt(y as i32 + 100, x as i32 + 100),
                    pt(y as i32, x as i32 + 100),
                ],
            });
        }

        let result = split_features(bounds, 24, points, lines, shapes);

        // All areas have data
        for area in &result {
            assert!(area.has_data());
        }

        // Points and lines preserved (shapes may be clipped → possibly more fragments)
        let total_points: usize = result.iter().map(|a| a.points.len()).sum();
        let total_lines: usize = result.iter().map(|a| a.lines.len()).sum();
        assert_eq!(total_points, 50);
        assert_eq!(total_lines, 20);
    }
}
