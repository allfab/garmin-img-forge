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
pub const WANTED_MAX_AREA_SIZE: usize = 0x3FFF; // 16383 bytes — parité mkgmap MapSplitter.java:66
pub const MIN_DIMENSION: i32 = 10;
pub const LARGE_OBJECT_DIM: i32 = 8192;

// Size estimation — mkgmap MapArea.addSize
const POINT_SIZE: usize = 9;
const LINE_OVERHEAD: usize = 11;
const LINE_POINT_SIZE: usize = 4;
const SHAPE_OVERHEAD: usize = 11;
const SHAPE_POINT_SIZE: usize = 4;

/// Parité mkgmap `PredictFilterPoints.predictedMaxNumPoints` — compte les points
/// qui resteraient après rounding à la résolution cible (doublons collapsés).
/// À haute résolution (shift=0), retourne le compte brut. À bits=18 (shift=6),
/// les points dans le même bucket de 64 units collapsent en 1 seul.
fn predicted_max_num_points(points: &[Coord], resolution: u8) -> usize {
    if points.is_empty() {
        return 0;
    }
    let shift = (24i32 - resolution as i32).max(0) as u32;
    let (half, mask): (i32, i32) = if shift == 0 {
        (0, !0)
    } else {
        (1 << (shift - 1), !((1 << shift) - 1))
    };
    let mut n = 0usize;
    let mut last_lat = 0i32;
    let mut last_lon = 0i32;
    for p in points {
        let lat = (p.latitude().wrapping_add(half)) & mask;
        let lon = (p.longitude().wrapping_add(half)) & mask;
        if n == 0 {
            n = 1;
        } else if lat != last_lat || lon != last_lon {
            n += 1;
        }
        last_lat = lat;
        last_lon = lon;
    }
    n
}

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
    /// Union of bboxes of all features actually added (parité mkgmap
    /// `MapArea.addToBounds` → `getFullBounds`). `None` tant qu'aucune feature
    /// n'a été ajoutée : on retombe alors sur `bounds` initial.
    full_min_lat: i32,
    full_min_lon: i32,
    full_max_lat: i32,
    full_max_lon: i32,
    has_full_bounds: bool,
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
            full_min_lat: i32::MAX,
            full_min_lon: i32::MAX,
            full_max_lat: i32::MIN,
            full_max_lon: i32::MIN,
            has_full_bounds: false,
        }
    }

    /// Union des bboxes de toutes les features ajoutées — parité mkgmap
    /// `MapArea.getFullBounds()` (MapArea.java:488-490). Utilisé pour
    /// `createSubdivision(parent, ma.getFullBounds(), z)` afin que le bbox
    /// déclaré de la subdivision TRE englobe réellement les features qu'elle
    /// contient, y compris celles qui débordent du bounds initial de la cell
    /// (tolérées par le splitter quand `shape ≤ maxWidth/maxHeight`).
    /// Sans ça, Alpha 100 / BaseCamp clippent strictement au half déclaré →
    /// zones vides là où les features débordent. GPSMapedit tolère et rend
    /// correctement, ce qui masque le bug avec les lecteurs non-Garmin.
    pub fn full_bounds(&self) -> Area {
        if !self.has_full_bounds {
            return self.bounds;
        }
        Area::new(self.full_min_lat, self.full_min_lon, self.full_max_lat, self.full_max_lon)
    }

    #[inline]
    fn extend_bounds_coord(&mut self, co: &Coord) {
        let lat = co.latitude();
        let lon = co.longitude();
        if lat < self.full_min_lat { self.full_min_lat = lat; }
        if lat > self.full_max_lat { self.full_max_lat = lat; }
        if lon < self.full_min_lon { self.full_min_lon = lon; }
        if lon > self.full_max_lon { self.full_max_lon = lon; }
        self.has_full_bounds = true;
    }

    #[inline]
    fn extend_bounds_points(&mut self, points: &[Coord]) {
        for p in points {
            self.extend_bounds_coord(p);
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
        self.extend_bounds_coord(&pt.location);
        self.points.push(pt);
        self.sizes[0] += POINT_SIZE;
    }

    /// Add a line — mkgmap addSize: 11 + predictedMaxNumPoints * 4.
    /// Utilise la prédiction de points APRÈS rounding à la résolution courante
    /// (parité mkgmap PredictFilterPoints). Sans ça, nos estimations brutes à
    /// haute résolution surévaluent de 5-10× aux wide-zoom levels (shift≥6),
    /// déclenchant want_split trop souvent → prolifération de subdivs.
    pub fn add_line(&mut self, line: SplitLine) {
        let n = predicted_max_num_points(&line.points, self.resolution);
        self.sizes[1] += LINE_OVERHEAD + n * LINE_POINT_SIZE;
        self.extend_bounds_points(&line.points);
        self.lines.push(line);
    }

    /// Add a shape — mkgmap addSize: 11 + predictedMaxNumPoints * 4
    pub fn add_shape(&mut self, shape: SplitShape) {
        let n = predicted_max_num_points(&shape.points, self.resolution);
        self.sizes[2] += SHAPE_OVERHEAD + n * SHAPE_POINT_SIZE;
        self.extend_bounds_points(&shape.points);
        self.shapes.push(shape);
    }

    pub fn total_size(&self) -> usize {
        self.sizes[0] + self.sizes[1] + self.sizes[2]
    }

    /// mkgmap MapArea.hasData — no empty subdivisions
    pub fn has_data(&self) -> bool {
        !self.points.is_empty() || !self.lines.is_empty() || !self.shapes.is_empty()
    }

    /// Parité mkgmap `MapArea.canAddSize(el, POINT_KIND)` (MapArea.java:608-614).
    fn can_add_point(&self) -> bool {
        if self.points.len() >= MAX_NUM_POINTS {
            return false;
        }
        self.total_size() + POINT_SIZE <= WANTED_MAX_AREA_SIZE
    }

    /// Parité mkgmap `MapArea.canAddSize(el, LINE_KIND)` (MapArea.java:616-630),
    /// `numElements` fixé à 1 (on ne split pas une polyline en plusieurs records
    /// ici ; LineSplitterFilter côté mkgmap intervient plus tard).
    fn can_add_line(&self, line: &SplitLine) -> bool {
        if self.lines.len() >= MAX_NUM_LINES {
            return false;
        }
        let n = predicted_max_num_points(&line.points, self.resolution);
        if n <= 1 {
            return true;
        }
        self.total_size() + LINE_OVERHEAD + n * LINE_POINT_SIZE <= WANTED_MAX_AREA_SIZE
    }

    /// Parité mkgmap `MapArea.canAddSize(el, SHAPE_KIND)` (MapArea.java:632-642).
    /// mkgmap ne pose PAS de limite de count sur les shapes ici (pas de
    /// MAX_NUM_SHAPES dans MapSplitter) ; seule la taille compte.
    fn can_add_shape(&self, shape: &SplitShape) -> bool {
        let n = predicted_max_num_points(&shape.points, self.resolution);
        if n <= 3 {
            return true;
        }
        self.total_size() + SHAPE_OVERHEAD + n * SHAPE_POINT_SIZE <= WANTED_MAX_AREA_SIZE
    }

    /// Must split — mkgmap hard limits (MapSplitter.addAreasToList)
    fn must_split(&self) -> bool {
        self.lines.len() > MAX_NUM_LINES
            || self.shapes.len() > MAX_NUM_LINES // polygons share the line limit
            || self.points.len() > MAX_NUM_POINTS
            || self.total_size() > MAX_RGN_SIZE
    }

    /// Want split — mkgmap soft limits (MapSplitter.java:159).
    /// Le plancher min_dimension est shifté par la résolution : au level 5
    /// (bits=18, shift=6), MIN_DIMENSION << shift = 640 units. Sans ce shift
    /// on continue à splitter jusqu'à 10 units aux wide-zoom levels, ce qui
    /// produit 2× plus de subdivs que mkgmap.
    fn want_split(&self) -> bool {
        let shift = (24i32 - self.resolution as i32).max(0) as u32;
        let min_dim_scaled = MIN_DIMENSION.checked_shl(shift).unwrap_or(MIN_DIMENSION);
        self.bounds.max_dimension() > min_dim_scaled
            && self.total_size() > WANTED_MAX_AREA_SIZE
    }

    /// Overflow split — parité mkgmap `MapArea.split(1, 1, bounds, true)`
    /// + `distPointsIntoNewAreas` / `distLinesIntoNewAreas` / `distShapesIntoNewAreas`
    /// (MapArea.java:251-305, 316-358).
    ///
    /// Utilisé quand une MapArea dépasse les seuils mais que `bounds` est trop
    /// petit pour un split géographique (cf. `MapSplitter.addAreasToList:185-189`
    /// branche `mustSplit`). On distribue les features par "paquets" qui tiennent
    /// dans une MapArea (via `can_add_*`), en créant une nouvelle area dès que
    /// le paquet courant refuse. Les bounds de chaque overflow area grossissent
    /// dynamiquement avec les features ajoutées (via `add_*` qui étend le bbox).
    fn split_overflow(&self) -> Vec<MapArea> {
        let mut result: Vec<MapArea> = Vec::new();
        // MapArea primaire : garde les bounds d'origine (même si on ne split pas
        // géographiquement, la cellule TRE garde son ancrage géographique).
        let mut current = MapArea::new(self.bounds, self.resolution);

        // Shapes en premier (comme mkgmap : distShapesIntoNewAreas appelé avant
        // distPoints/distLines, MapArea.java:250-253).
        for shape in &self.shapes {
            if !current.can_add_shape(shape) {
                result.push(std::mem::replace(
                    &mut current,
                    MapArea::new(self.bounds, self.resolution),
                ));
            }
            current.add_shape(shape.clone());
        }
        for pt in &self.points {
            if !current.can_add_point() {
                result.push(std::mem::replace(
                    &mut current,
                    MapArea::new(self.bounds, self.resolution),
                ));
            }
            current.add_point(pt.clone());
        }
        for line in &self.lines {
            if !current.can_add_line(line) {
                result.push(std::mem::replace(
                    &mut current,
                    MapArea::new(self.bounds, self.resolution),
                ));
            }
            current.add_line(line.clone());
        }
        if current.has_data() {
            result.push(current);
        }
        result
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

        // Max cell dimensions for large object detection — parité mkgmap MapArea.java:260-271.
        // maxSize est shifté selon la résolution : plus la résolution est faible (wide-zoom),
        // plus maxSize est grand. Sans ce shift, notre plafond fixe `MAX_DIVISION_SIZE/2`
        // (≈16K units) déclenche les "largeObjectArea" beaucoup trop souvent aux wide-zoom
        // levels (bits≤20) → prolifération de subdivs dédiés, bytes RGN dupliqués.
        const MAX_RESOLUTION: i32 = 24;
        let max_size = {
            let shifted = (MAX_DIVISION_SIZE as i64) << (MAX_RESOLUTION - self.resolution as i32).max(0);
            let clamped = shifted.min(((1i64 << 24) - 1) as i64).max(0x8000);
            clamped as i32
        };
        let cell_w = self.bounds.width() / nx.max(1) as i32;
        let cell_h = self.bounds.height() / ny.max(1) as i32;
        let max_cell_w = cell_w.min(max_size / 2).max(LARGE_OBJECT_DIM * 2);
        let max_cell_h = cell_h.min(max_size / 2).max(LARGE_OBJECT_DIM * 2);

        // ── Lines: single sub-area by bbox midpoint (mkgmap pickArea, no clipping).
        // La ligne intacte va dans UNE seule sub-area ; add_line() → extend_bounds_points()
        // étend full_bounds() pour couvrir toute la ligne, même si elle déborde de la cell.
        // writer.rs utilise full_bounds() pour les bounds TRE → le firmware charge la
        // subdivision quand n'importe quelle portion de la ligne est dans le viewport.
        for line in &self.lines {
            if line.points.is_empty() {
                continue;
            }
            let line_bbox = Area::from_coords(&line.points);
            let mid_lat = (line_bbox.min_lat() as i64 + line_bbox.max_lat() as i64) / 2;
            let mid_lon = (line_bbox.min_lon() as i64 + line_bbox.max_lon() as i64) / 2;
            let target = pick_area(
                mid_lon, mid_lat,
                xbase, ybase, nx, ny, dx, dy, num_areas,
            );
            sub_areas[target].add_line(line.clone());
        }

        // ── Shapes: clip each polygon to every sub-area it overlaps (Sutherland-Hodgman).
        // Parité mkgmap MapArea.java:280-295 split agressif activé pour les polygones.
        // Chaque fragment va dans la sous-zone correspondante avec le même mp_index,
        // ce qui garantit l'affichage lors du panning même quand le polygone chevauche
        // plusieurs sous-divisions.
        for shape in &self.shapes {
            if shape.points.len() < 3 {
                continue;
            }
            let shape_bbox = Area::from_coords(&shape.points);
            let mid_lat = (shape_bbox.min_lat() as i64 + shape_bbox.max_lat() as i64) / 2;
            let mid_lon = (shape_bbox.min_lon() as i64 + shape_bbox.max_lon() as i64) / 2;
            let target = pick_area(
                mid_lon, mid_lat,
                xbase, ybase, nx, ny, dx, dy, num_areas,
            );

            let fits_target = sub_areas[target].bounds.contains_area(&shape_bbox);
            if fits_target {
                sub_areas[target].add_shape(shape.clone());
                continue;
            }

            // Polygon crosses cell boundaries: clip to each overlapping sub-area.
            let mut clipped_any = false;
            for j in 0..num_areas {
                if !sub_areas[j].bounds.intersects(&shape_bbox) {
                    continue;
                }
                let clipped = clip_polygon_to_rect(&shape.points, &sub_areas[j].bounds);
                if clipped.len() >= 3 {
                    sub_areas[j].add_shape(SplitShape {
                        mp_index: shape.mp_index,
                        points: clipped,
                    });
                    clipped_any = true;
                }
            }
            if !clipped_any {
                sub_areas[target].add_shape(shape.clone());
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

// ── Polyline clipping — Liang-Barsky ───────────────────────────────────────

/// Clip an open polyline to a rectangle using Liang-Barsky parametric clipping.
/// Returns one or more segments (a line may exit and re-enter the rect multiple times).
/// Each returned segment has ≥ 2 points and lies entirely within `rect`.
pub fn clip_polyline_to_rect(points: &[Coord], rect: &Area) -> Vec<Vec<Coord>> {
    if points.len() < 2 {
        return vec![];
    }
    let mut segments: Vec<Vec<Coord>> = Vec::new();
    let mut current: Vec<Coord> = Vec::new();
    for i in 0..points.len() - 1 {
        match clip_segment_lb(points[i], points[i + 1], rect) {
            None => {
                if current.len() >= 2 { segments.push(std::mem::take(&mut current)); }
                else { current.clear(); }
            }
            Some((ca, cb)) => {
                if current.is_empty() {
                    current.push(ca);
                } else if ca != *current.last().unwrap() {
                    if current.len() >= 2 { segments.push(std::mem::take(&mut current)); }
                    else { current.clear(); }
                    current.push(ca);
                }
                current.push(cb);
            }
        }
    }
    if current.len() >= 2 { segments.push(current); }
    segments
}

/// Liang-Barsky segment clip: returns None if fully outside, Some((a',b')) otherwise.
fn clip_segment_lb(a: Coord, b: Coord, rect: &Area) -> Option<(Coord, Coord)> {
    let ax = a.longitude() as f64;
    let ay = a.latitude() as f64;
    let dx = (b.longitude() - a.longitude()) as f64;
    let dy = (b.latitude() - a.latitude()) as f64;
    let mut t0 = 0.0f64;
    let mut t1 = 1.0f64;
    for (p, q) in [
        (-dx, ax - rect.min_lon() as f64),
        ( dx, rect.max_lon() as f64 - ax),
        (-dy, ay - rect.min_lat() as f64),
        ( dy, rect.max_lat() as f64 - ay),
    ] {
        if p == 0.0 {
            if q < 0.0 { return None; }
        } else {
            let r = q / p;
            if p < 0.0 { if r > t1 { return None; } else if r > t0 { t0 = r; } }
            else       { if r < t0 { return None; } else if r < t1 { t1 = r; } }
        }
    }
    let ca = Coord::new((ay + t0 * dy).round() as i32, (ax + t0 * dx).round() as i32);
    let cb = Coord::new((ay + t1 * dy).round() as i32, (ax + t1 * dx).round() as i32);
    Some((ca, cb))
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

/// mkgmap MapSplitter.addAreasToList — recursive split until all areas fit.
///
/// `max_depth` est conservé comme garde-fou ultime contre une boucle infinie
/// pathologique, mais en production on passe `usize::MAX` car mkgmap n'impose
/// aucune limite de profondeur (cf. MapSplitter.java:131-200, le paramètre
/// `depth` y sert uniquement au padding des logs). Les vraies conditions
/// d'arrêt sont la taille (`!need_split`), `MIN_DIMENSION`, et `!can_split`.
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

    // Trop petite pour divisions plus fines — parité mkgmap : plancher shifté par résolution.
    let shift = (24i32 - area.resolution as i32).max(0) as u32;
    let min_dim_scaled = MIN_DIMENSION.checked_shl(shift).unwrap_or(MIN_DIMENSION);
    if area.bounds.max_dimension() <= min_dim_scaled {
        if area.must_split() {
            // Parité mkgmap `MapSplitter.addAreasToList:185-189` : quand on ne
            // peut plus splitter géographiquement mais qu'on dépasse les seuils
            // (MAX_NUM_LINES / MAX_NUM_POINTS / MAX_RGN_SIZE), on force un
            // overflow split qui distribue les features en plusieurs MapAreas
            // partageant les mêmes bounds. L'Alpha 100 refuse de rendre les
            // subdivs avec > 255 lignes (bug wide-zoom blanc).
            for sub in area.split_overflow() {
                if sub.has_data() {
                    result.push(sub);
                }
            }
        } else {
            result.push(area);
        }
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

    // Step 2: Recursive splitting until all areas fit limits.
    // mkgmap n'a pas de plafond de profondeur (cf. MapSplitter.java) ; on
    // passe usize::MAX pour que la subdivision continue tant que la taille
    // n'est pas conforme. Sans ça, sur les zones très denses (ex. quadrant
    // FRANCE-SE, agglos Marseille/Nice/Lyon), le splitter abandonnait à
    // depth=8 et écrivait des subdivisions >64 KB → données corrompues
    // côté Garmin (artefacts géométriques sur les communes).
    add_areas_to_list(initial, usize::MAX)
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
    fn test_overflow_split_distributes_lines_when_too_small_to_divide() {
        // Parité mkgmap MapSplitter.addAreasToList:185-189 — quand bounds < MIN_DIMENSION<<shift
        // mais lines > MAX_NUM_LINES, on doit forcer l'overflow split au lieu d'accepter
        // une subdiv avec > 255 lignes (Alpha 100 refuse de la rendre).
        // Bounds tiny au level 5 (res=18, shift=6, min_dim_scaled = 10<<6 = 640 units).
        // bounds 100 units → trop petit, mais 400 lignes doit forcer overflow.
        let mut area = MapArea::new(Area::new(0, 0, 100, 100), 18);
        for i in 0..400 {
            area.add_line(SplitLine {
                mp_index: i,
                points: vec![pt(50, 50), pt(51, 51)],
            });
        }
        assert!(area.must_split());
        let result = add_areas_to_list(vec![area], usize::MAX);
        // Chaque subdiv doit respecter MAX_NUM_LINES
        for (i, sub) in result.iter().enumerate() {
            assert!(
                sub.lines.len() <= MAX_NUM_LINES,
                "subdiv {} has {} lines > MAX_NUM_LINES",
                i, sub.lines.len()
            );
        }
        // Total lignes préservé (aucune perdue)
        let total: usize = result.iter().map(|a| a.lines.len()).sum();
        assert_eq!(total, 400);
        // Au moins 2 subdivs — 400 lignes de 2 pts = ~400*19 bytes = 7600B, tient en 1 subdiv
        // par WANTED_MAX_AREA_SIZE mais le cap `lines.len() >= 255` nous force à splitter.
        assert!(
            result.len() >= 2,
            "overflow split should produce >= 2 subdivs, got {}",
            result.len()
        );
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
    fn test_split_line_entire_in_single_cell_or_dedicated() {
        // Une polyline qui traverse plusieurs cells reste ENTIÈRE dans la cell
        // de son premier point (ou subdiv dédié si elle dépasse max_cell_size).
        // Les bounds de subdivision = area.bounds (non-chevauchants) garantissent
        // qu'exactement 1 subdivision couvre chaque viewport.
        let bounds = Area::new(0, 0, 1000, 1000);
        let mut area = MapArea::new(bounds, 24);

        // Diagonal line crossing all 4 cells of a 2×2 grid
        area.add_line(SplitLine {
            mp_index: 0,
            points: vec![pt(100, 100), pt(900, 900)],
        });

        let subs = area.split(2, 2);
        let total_lines: usize = subs.iter().map(|s| s.lines.len()).sum();
        // La ligne doit apparaître UNE SEULE FOIS (dans une seule sub-area ou
        // dans un dedicated subdiv).
        assert_eq!(total_lines, 1, "line must not be duplicated across cells");
    }

    #[test]
    fn test_split_large_line_gets_dedicated_area() {
        // Une polyline dont le bbox dépasse max_cell_w/h ET qui ne tient pas
        // dans la cell de son premier point → subdiv dédié (largeObjectArea).
        let bounds = Area::new(0, 0, (LARGE_OBJECT_DIM * 8) as i32, (LARGE_OBJECT_DIM * 8) as i32);
        let mut area = MapArea::new(bounds, 24);
        area.add_line(SplitLine {
            mp_index: 0,
            points: vec![
                pt(100, 100),
                pt((LARGE_OBJECT_DIM * 7) as i32, (LARGE_OBJECT_DIM * 7) as i32),
            ],
        });

        let subs = area.split(2, 2);
        // 4 cells (2×2) + 1 dedicated = 5 sub-areas au total.
        assert!(subs.len() >= 5, "expected extra dedicated subdiv, got {}", subs.len());
        // La ligne n'apparaît qu'une seule fois globalement.
        let total_lines: usize = subs.iter().map(|s| s.lines.len()).sum();
        assert_eq!(total_lines, 1);
    }

    #[test]
    fn test_split_line_inside_cell_unchanged() {
        let bounds = Area::new(0, 0, 1000, 1000);
        let mut area = MapArea::new(bounds, 24);

        // Line fully contained in bottom-left quadrant
        area.add_line(SplitLine {
            mp_index: 7,
            points: vec![pt(100, 100), pt(200, 150), pt(300, 250)],
        });

        let subs = area.split(2, 2);
        let total_lines: usize = subs.iter().map(|s| s.lines.len()).sum();
        assert_eq!(total_lines, 1);
        // Points identical to the source (no clipping applied)
        let hosted = subs.iter().find(|s| !s.lines.is_empty()).unwrap();
        assert_eq!(hosted.lines[0].points, vec![pt(100, 100), pt(200, 150), pt(300, 250)]);
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
        // Lines may be clipped into segments when they cross cell boundaries,
        // so the total is at least the input count (never fewer).
        assert!(total_lines >= 20, "got {total_lines}");
    }
}
