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
}

/// [POI] section
#[derive(Debug, Clone)]
pub struct MpPoint {
    pub type_code: u32,
    pub label: String,
    pub coord: Coord,
    pub end_level: Option<u8>,
    pub city_name: Option<String>,
    pub region_name: Option<String>,
    pub country_name: Option<String>,
    pub zip: Option<String>,
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
