// Types for Polish Map (.mp) file format

use crate::img::coord::Coord;

/// Complete parsed .mp file
#[derive(Debug, Clone)]
pub struct MpFile {
    pub header: MpHeader,
    pub points: Vec<MpPoint>,
    pub polylines: Vec<MpPolyline>,
    pub polygons: Vec<MpPolygon>,
}

/// Routing generation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    /// Auto-detect: generate NET+NOD if road_id present
    Auto,
    /// Force NET+NOD generation
    Route,
    /// Generate NET only (address search, no turn-by-turn)
    NetOnly,
    /// Disable routing entirely
    Disabled,
}

impl Default for RoutingMode {
    fn default() -> Self {
        RoutingMode::Auto
    }
}

/// [IMG ID] section header
#[derive(Debug, Clone, Default)]
pub struct MpHeader {
    pub id: u32,
    pub name: String,
    pub copyright: String,
    pub levels: Vec<u8>,       // resolution per level
    pub codepage: u16,
    pub datum: String,
    pub transparent: bool,
    pub draw_priority: u32,
    pub preview_lat: f64,
    pub preview_lon: f64,
    // Encoding
    pub lower_case: bool,
    // Rendering
    pub order_by_decreasing_area: bool,
    // Geometry optimization
    pub reduce_point_density: Option<f64>,
    pub simplify_polygons: Option<String>,
    pub min_size_polygon: Option<i32>,
    pub merge_lines: bool,
    // Routing
    pub routing_mode: RoutingMode,
    // Metadata (TDB-level)
    pub country_name: String,
    pub country_abbr: String,
    pub region_name: String,
    pub region_abbr: String,
    pub product_version: u16,
}

/// [POI] section
#[derive(Debug, Clone)]
pub struct MpPoint {
    pub type_code: u32,
    pub label: String,
    pub coord: Coord,
    pub end_level: Option<u8>,
}

/// [POLYLINE] section
#[derive(Debug, Clone)]
pub struct MpPolyline {
    pub type_code: u32,
    pub label: String,
    pub points: Vec<Coord>,
    pub end_level: Option<u8>,
    pub direction: bool,
    pub road_id: Option<u32>,
    pub route_param: Option<String>,
}

/// [POLYGON] section
#[derive(Debug, Clone)]
pub struct MpPolygon {
    pub type_code: u32,
    pub label: String,
    pub points: Vec<Coord>,
    pub end_level: Option<u8>,
}
