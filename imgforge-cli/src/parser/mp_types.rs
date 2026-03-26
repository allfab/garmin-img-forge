//! Polish Map data structures representing parsed .mp file contents.

use std::collections::HashMap;

/// Represents a complete parsed Polish Map (.mp) file.
#[derive(Debug, Clone)]
pub struct MpFile {
    pub header: MpHeader,
    pub points: Vec<MpPoint>,
    pub polylines: Vec<MpPolyline>,
    pub polygons: Vec<MpPolygon>,
}

/// Header data from the [IMG ID] section.
#[derive(Debug, Clone)]
pub struct MpHeader {
    /// Map name (Name= field, required)
    pub name: String,
    /// Map ID (ID= field, required)
    pub id: String,
    /// Code page (CodePage=, default "1252")
    pub code_page: String,
    /// Number of zoom levels (Levels=)
    pub levels: Option<u8>,
    /// Level definitions (Level0=, Level1=, ... as bit values)
    pub level_defs: Vec<u8>,
    /// Tree size for index (TreeSize=)
    pub tree_size: Option<u32>,
    /// Region limit (RgnLimit=)
    pub rgn_limit: Option<u32>,
    /// Unknown or extended fields preserved for forward-compatibility
    pub other_fields: HashMap<String, String>,
}

impl Default for MpHeader {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: String::new(),
            code_page: "1252".to_string(),
            levels: None,
            level_defs: Vec::new(),
            tree_size: None,
            rgn_limit: None,
            other_fields: HashMap::new(),
        }
    }
}

/// A point feature from a [POI] section.
#[derive(Debug, Clone)]
pub struct MpPoint {
    /// Garmin type code (e.g. "0x2C00")
    pub type_code: String,
    /// Optional display label
    pub label: Option<String>,
    /// Highest zoom level at which this feature appears
    pub end_level: Option<u8>,
    /// Latitude (WGS84)
    pub lat: f64,
    /// Longitude (WGS84)
    pub lon: f64,
    /// Unknown or extended fields preserved for forward-compatibility
    pub other_fields: HashMap<String, String>,
}

/// A polyline feature from a [POLYLINE] section.
#[derive(Debug, Clone)]
pub struct MpPolyline {
    /// Garmin type code (e.g. "0x01")
    pub type_code: String,
    /// Optional display label
    pub label: Option<String>,
    /// Highest zoom level at which this feature appears
    pub end_level: Option<u8>,
    /// Ordered list of (lat, lon) coordinate pairs (WGS84)
    pub coords: Vec<(f64, f64)>,
    /// Optional routing attributes (present when RoadID/RouteParam/etc. are defined)
    pub routing: Option<MpRoutingAttrs>,
    /// Unknown or extended fields preserved for forward-compatibility
    pub other_fields: HashMap<String, String>,
}

/// A polygon feature from a [POLYGON] section.
#[derive(Debug, Clone)]
pub struct MpPolygon {
    /// Garmin type code (e.g. "0x50")
    pub type_code: String,
    /// Optional display label
    pub label: Option<String>,
    /// Highest zoom level at which this feature appears
    pub end_level: Option<u8>,
    /// Outer ring coordinates (lat, lon) — from Data0
    pub coords: Vec<(f64, f64)>,
    /// Interior rings (holes) — from Data1, Data2, ...
    pub holes: Vec<Vec<(f64, f64)>>,
    /// Unknown or extended fields preserved for forward-compatibility
    pub other_fields: HashMap<String, String>,
}

/// Routing attributes for road features in [POLYLINE] sections.
#[derive(Debug, Clone)]
pub struct MpRoutingAttrs {
    /// Road identifier (RoadID=)
    pub road_id: Option<String>,
    /// Route parameters in compact form: Speed,Class,Oneway,Toll,... (RouteParam=)
    pub route_param: Option<String>,
    /// Garmin speed type 0-7 (SpeedType= or Speed=)
    pub speed_type: Option<i32>,
    /// Direction indicator: 0=both, 1=forward, -1=reverse (DirIndicator= or Direction=)
    pub dir_indicator: Option<i32>,
    /// Story 14.1: Roundabout flag (Roundabout=1 when true)
    pub roundabout: Option<bool>,
    /// Story 14.1: Maximum height in centimeters (MaxHeight=, custom extension)
    pub max_height: Option<u32>,
    /// Story 14.1: Maximum weight in centithons (MaxWeight=, custom extension)
    pub max_weight: Option<u32>,
    /// Story 14.1: Maximum width in centimeters (MaxWidth=, custom extension)
    pub max_width: Option<u32>,
    /// Story 14.1: Maximum length in centimeters (MaxLength=, custom extension)
    pub max_length: Option<u32>,
}
