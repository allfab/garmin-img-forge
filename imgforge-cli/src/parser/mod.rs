//! Polish Map (.mp) file parser.
//!
//! Implements a line-by-line state machine parser for the Polish Map format
//! used by cGPSmapper and compatible tools.

pub mod mp_types;

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::ParseError;
use mp_types::{MpFile, MpHeader, MpPoint, MpPolygon, MpPolyline, MpRoutingAttrs};

/// Parser for Polish Map (.mp) files.
pub struct MpParser;

/// Internal parser state machine states.
#[derive(Debug, PartialEq)]
enum ParseState {
    Idle,
    InHeader,
    InPoi,
    InPolyline,
    InPolygon,
}

/// Accumulated data while parsing a [POI] section.
#[derive(Default)]
struct PoiBuilder {
    type_code: Option<String>,
    label: Option<String>,
    end_level: Option<u8>,
    lat: Option<f64>,
    lon: Option<f64>,
    other_fields: HashMap<String, String>,
}

/// Accumulated data while parsing a [POLYLINE] section.
#[derive(Default)]
struct PolylineBuilder {
    type_code: Option<String>,
    label: Option<String>,
    end_level: Option<u8>,
    coords: Vec<(f64, f64)>,
    road_id: Option<String>,
    route_param: Option<String>,
    speed_type: Option<i32>,
    dir_indicator: Option<i32>,
    other_fields: HashMap<String, String>,
}

/// Accumulated data while parsing a [POLYGON] section.
#[derive(Default)]
struct PolygonBuilder {
    type_code: Option<String>,
    label: Option<String>,
    end_level: Option<u8>,
    outer: Vec<(f64, f64)>,
    /// Interior rings keyed by Data suffix index (Data1→1, Data2→2, …) for ordered collection.
    holes_map: BTreeMap<usize, Vec<(f64, f64)>>,
    other_fields: HashMap<String, String>,
}

/// Known canonical field names for [POI], [POLYLINE], [POLYGON] sections.
const FEATURE_KNOWN_FIELDS: &[&str] = &[
    "Type", "Label", "EndLevel", "Levels", "Marine", "Data0", "Data1", "Data2", "Data3", "Data4",
    "Data5", "Data6", "Data7", "Data8", "Data9",
];

/// Known routing field names for [POLYLINE] sections.
const ROUTING_FIELDS: &[&str] = &[
    "RoadID",
    "RouteParam",
    "SpeedType",
    "Speed",
    "DirIndicator",
    "Direction",
];

impl MpParser {
    /// Parse a Polish Map file from the given path.
    ///
    /// Returns a fully populated `MpFile` on success.
    /// Returns `ParseError::MissingImgId` if no `[IMG ID]` section is found.
    /// Returns `ParseError::InvalidFormat` with line number for format errors.
    pub fn parse_file(path: &Path) -> Result<MpFile, ParseError> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        Self::parse_reader(reader)
    }

    /// Parse from any `BufRead` source (used by tests).
    pub fn parse_reader<R: BufRead>(reader: R) -> Result<MpFile, ParseError> {
        let mut state = ParseState::Idle;
        let mut header_seen = false;

        let mut header = MpHeader::default();
        let mut points: Vec<MpPoint> = Vec::new();
        let mut polylines: Vec<MpPolyline> = Vec::new();
        let mut polygons: Vec<MpPolygon> = Vec::new();

        let mut poi_builder = PoiBuilder::default();
        let mut polyline_builder = PolylineBuilder::default();
        let mut polygon_builder = PolygonBuilder::default();

        let mut line_num: usize = 0;

        for line_result in reader.lines() {
            line_num += 1;
            let raw = line_result?;
            let line = raw.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            // --- Section header detection ---
            if line.starts_with('[') {
                match line {
                    "[IMG ID]" => {
                        state = ParseState::InHeader;
                        header = MpHeader::default();
                        continue;
                    }
                    "[END-IMG ID]" => {
                        if state == ParseState::InHeader {
                            if header.id.is_empty() {
                                return Err(ParseError::InvalidFormat {
                                    line: line_num,
                                    message: "missing required field ID in [IMG ID] section"
                                        .to_string(),
                                });
                            }
                            header_seen = true;
                            state = ParseState::Idle;
                        }
                        continue;
                    }
                    "[POI]" => {
                        state = ParseState::InPoi;
                        poi_builder = PoiBuilder::default();
                        continue;
                    }
                    "[POLYLINE]" => {
                        state = ParseState::InPolyline;
                        polyline_builder = PolylineBuilder::default();
                        continue;
                    }
                    "[POLYGON]" => {
                        state = ParseState::InPolygon;
                        polygon_builder = PolygonBuilder::default();
                        continue;
                    }
                    "[END]" => {
                        match state {
                            ParseState::InPoi => {
                                let b = std::mem::take(&mut poi_builder);
                                let lat = b.lat.ok_or_else(|| ParseError::InvalidFormat {
                                    line: line_num,
                                    message: "POI section missing Data0 coordinate".to_string(),
                                })?;
                                let lon = b.lon.ok_or_else(|| ParseError::InvalidFormat {
                                    line: line_num,
                                    message: "POI section missing Data0 coordinate".to_string(),
                                })?;
                                let type_code = b.type_code.unwrap_or_else(|| "0x00".to_string());
                                points.push(MpPoint {
                                    type_code,
                                    label: b.label,
                                    end_level: b.end_level,
                                    lat,
                                    lon,
                                    other_fields: b.other_fields,
                                });
                            }
                            ParseState::InPolyline => {
                                let b = std::mem::take(&mut polyline_builder);
                                if b.coords.is_empty() {
                                    return Err(ParseError::InvalidFormat {
                                        line: line_num,
                                        message: "POLYLINE section missing Data0 coordinates"
                                            .to_string(),
                                    });
                                }
                                let type_code = b.type_code.unwrap_or_else(|| "0x00".to_string());
                                let routing = if b.road_id.is_some()
                                    || b.route_param.is_some()
                                    || b.speed_type.is_some()
                                    || b.dir_indicator.is_some()
                                {
                                    Some(MpRoutingAttrs {
                                        road_id: b.road_id,
                                        route_param: b.route_param,
                                        speed_type: b.speed_type,
                                        dir_indicator: b.dir_indicator,
                                    })
                                } else {
                                    None
                                };
                                polylines.push(MpPolyline {
                                    type_code,
                                    label: b.label,
                                    end_level: b.end_level,
                                    coords: b.coords,
                                    routing,
                                    other_fields: b.other_fields,
                                });
                            }
                            ParseState::InPolygon => {
                                let b = std::mem::take(&mut polygon_builder);
                                if b.outer.is_empty() {
                                    return Err(ParseError::InvalidFormat {
                                        line: line_num,
                                        message: "POLYGON section missing Data0 coordinates"
                                            .to_string(),
                                    });
                                }
                                let type_code = b.type_code.unwrap_or_else(|| "0x00".to_string());
                                let holes: Vec<Vec<(f64, f64)>> =
                                    b.holes_map.into_values().collect();
                                polygons.push(MpPolygon {
                                    type_code,
                                    label: b.label,
                                    end_level: b.end_level,
                                    coords: b.outer,
                                    holes,
                                    other_fields: b.other_fields,
                                });
                            }
                            _ => {}
                        }
                        state = ParseState::Idle;
                        continue;
                    }
                    _ => {
                        // Unknown section header — skip
                        continue;
                    }
                }
            }

            // --- Key=value line parsing ---
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim();
                let value = line[eq_pos + 1..].trim();

                match state {
                    ParseState::InHeader => {
                        parse_header_field(&mut header, key, value, line_num)?;
                    }
                    ParseState::InPoi => {
                        parse_poi_field(&mut poi_builder, key, value, line_num)?;
                    }
                    ParseState::InPolyline => {
                        parse_polyline_field(&mut polyline_builder, key, value, line_num)?;
                    }
                    ParseState::InPolygon => {
                        parse_polygon_field(&mut polygon_builder, key, value, line_num)?;
                    }
                    ParseState::Idle => {
                        // Fields outside sections are ignored
                    }
                }
            }
        }

        if !header_seen {
            return Err(ParseError::MissingImgId);
        }

        Ok(MpFile {
            header,
            points,
            polylines,
            polygons,
        })
    }
}

/// Parse a key=value pair for the [IMG ID] header section.
fn parse_header_field(
    header: &mut MpHeader,
    key: &str,
    value: &str,
    line_num: usize,
) -> Result<(), ParseError> {
    match key {
        "Name" => header.name = value.to_string(),
        "ID" => header.id = value.to_string(),
        "CodePage" => header.code_page = value.to_string(),
        "Levels" => {
            header.levels = Some(value.parse::<u8>().map_err(|_| ParseError::InvalidFormat {
                line: line_num,
                message: format!("invalid Levels value: '{}'", value),
            })?);
        }
        "TreeSize" => {
            header.tree_size =
                Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| ParseError::InvalidFormat {
                            line: line_num,
                            message: format!("invalid TreeSize value: '{}'", value),
                        })?,
                );
        }
        "RgnLimit" => {
            header.rgn_limit =
                Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| ParseError::InvalidFormat {
                            line: line_num,
                            message: format!("invalid RgnLimit value: '{}'", value),
                        })?,
                );
        }
        k if k.starts_with("Level") && k.len() > 5 => {
            // Level0, Level1, ... Level9
            let suffix = &k[5..];
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                let bits = value.parse::<u8>().map_err(|_| ParseError::InvalidFormat {
                    line: line_num,
                    message: format!("invalid {} value: '{}'", k, value),
                })?;
                let idx = suffix.parse::<usize>().unwrap_or(0);
                if idx < 10 {
                    // Grow the vec as needed
                    while header.level_defs.len() <= idx {
                        header.level_defs.push(0);
                    }
                    header.level_defs[idx] = bits;
                }
            } else {
                header.other_fields.insert(k.to_string(), value.to_string());
            }
        }
        k => {
            // Store all other fields for forward-compatibility (includes extended known fields)
            header.other_fields.insert(k.to_string(), value.to_string());
        }
    }
    Ok(())
}

/// Parse a key=value pair for a [POI] section.
fn parse_poi_field(
    builder: &mut PoiBuilder,
    key: &str,
    value: &str,
    line_num: usize,
) -> Result<(), ParseError> {
    match key {
        "Type" => builder.type_code = Some(value.to_string()),
        "Label" => builder.label = Some(value.to_string()),
        "EndLevel" => {
            builder.end_level =
                Some(value.parse::<u8>().map_err(|_| ParseError::InvalidFormat {
                    line: line_num,
                    message: format!("invalid EndLevel value: '{}'", value),
                })?);
        }
        "Data0" => {
            let coords = parse_data_line(value, line_num)?;
            if coords.len() != 1 {
                return Err(ParseError::InvalidFormat {
                    line: line_num,
                    message: format!(
                        "POI Data0 must have exactly 1 coordinate pair, got {}",
                        coords.len()
                    ),
                });
            }
            let (lat, lon) = coords[0];
            builder.lat = Some(lat);
            builder.lon = Some(lon);
        }
        k if !FEATURE_KNOWN_FIELDS.contains(&k) => {
            builder
                .other_fields
                .insert(k.to_string(), value.to_string());
        }
        _ => {}
    }
    Ok(())
}

/// Parse a key=value pair for a [POLYLINE] section.
fn parse_polyline_field(
    builder: &mut PolylineBuilder,
    key: &str,
    value: &str,
    line_num: usize,
) -> Result<(), ParseError> {
    match key {
        "Type" => builder.type_code = Some(value.to_string()),
        "Label" => builder.label = Some(value.to_string()),
        "EndLevel" => {
            builder.end_level =
                Some(value.parse::<u8>().map_err(|_| ParseError::InvalidFormat {
                    line: line_num,
                    message: format!("invalid EndLevel value: '{}'", value),
                })?);
        }
        "Data0" => {
            builder.coords = parse_data_line(value, line_num)?;
        }
        "RoadID" => builder.road_id = Some(value.to_string()),
        "RouteParam" => builder.route_param = Some(value.to_string()),
        "SpeedType" | "Speed" => {
            if let Ok(n) = value.parse::<i32>() {
                builder.speed_type = Some(n);
            }
        }
        "DirIndicator" | "Direction" => {
            if let Ok(n) = value.parse::<i32>() {
                builder.dir_indicator = Some(n);
            }
        }
        k if !FEATURE_KNOWN_FIELDS.contains(&k) && !ROUTING_FIELDS.contains(&k) => {
            builder
                .other_fields
                .insert(k.to_string(), value.to_string());
        }
        _ => {}
    }
    Ok(())
}

/// Parse a key=value pair for a [POLYGON] section.
fn parse_polygon_field(
    builder: &mut PolygonBuilder,
    key: &str,
    value: &str,
    line_num: usize,
) -> Result<(), ParseError> {
    match key {
        "Type" => builder.type_code = Some(value.to_string()),
        "Label" => builder.label = Some(value.to_string()),
        "EndLevel" => {
            builder.end_level =
                Some(value.parse::<u8>().map_err(|_| ParseError::InvalidFormat {
                    line: line_num,
                    message: format!("invalid EndLevel value: '{}'", value),
                })?);
        }
        "Data0" => {
            builder.outer = parse_data_line(value, line_num)?;
        }
        k if k.starts_with("Data") && k.len() > 4 => {
            // Data1, Data2, ... are interior rings (holes) — keyed by index for ordered collection
            let suffix = &k[4..];
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                let idx: usize = suffix.parse().unwrap_or(0);
                let hole_coords = parse_data_line(value, line_num)?;
                builder.holes_map.insert(idx, hole_coords);
            } else {
                builder
                    .other_fields
                    .insert(k.to_string(), value.to_string());
            }
        }
        k if !FEATURE_KNOWN_FIELDS.contains(&k) => {
            builder
                .other_fields
                .insert(k.to_string(), value.to_string());
        }
        _ => {}
    }
    Ok(())
}

/// Parse a Data0 coordinate string into a list of (lat, lon) pairs.
///
/// Input format: `(lat1,lon1),(lat2,lon2),...`
///
/// Returns `ParseError::InvalidFormat` with the line number on parse failure.
pub fn parse_data_line(value: &str, line_num: usize) -> Result<Vec<(f64, f64)>, ParseError> {
    let mut coords = Vec::new();
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(ParseError::InvalidFormat {
            line: line_num,
            message: "empty coordinate value".to_string(),
        });
    }

    // Split on '),(' to separate coordinate pairs
    // Input: "(45.1880,5.7245),(45.1890,5.7255)"
    for pair in trimmed.split("),(") {
        let pair = pair.trim_matches(|c| c == '(' || c == ')').trim();
        let parts: Vec<&str> = pair.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(ParseError::InvalidFormat {
                line: line_num,
                message: format!("malformed coordinate pair: '{}'", pair),
            });
        }
        let lat = parts[0]
            .trim()
            .parse::<f64>()
            .map_err(|_| ParseError::InvalidFormat {
                line: line_num,
                message: format!("invalid latitude value: '{}'", parts[0].trim()),
            })?;
        let lon = parts[1]
            .trim()
            .parse::<f64>()
            .map_err(|_| ParseError::InvalidFormat {
                line: line_num,
                message: format!("invalid longitude value: '{}'", parts[1].trim()),
            })?;
        coords.push((lat, lon));
    }

    if coords.is_empty() {
        return Err(ParseError::InvalidFormat {
            line: line_num,
            message: "no valid coordinates found".to_string(),
        });
    }

    Ok(coords)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse_str(input: &str) -> Result<MpFile, ParseError> {
        MpParser::parse_reader(Cursor::new(input))
    }

    // ----------------------------------------------------------------
    // Header tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_header() {
        let input = r#"
[IMG ID]
Name=Ma Carte
ID=63240001
CodePage=1252
Levels=2
Level0=24
Level1=18
TreeSize=3000
RgnLimit=1024
[END-IMG ID]
"#;
        let mp = parse_str(input).unwrap();
        assert_eq!(mp.header.name, "Ma Carte");
        assert_eq!(mp.header.id, "63240001");
        assert_eq!(mp.header.code_page, "1252");
        assert_eq!(mp.header.levels, Some(2));
        assert_eq!(mp.header.level_defs, vec![24, 18]);
        assert_eq!(mp.header.tree_size, Some(3000));
        assert_eq!(mp.header.rgn_limit, Some(1024));
    }

    #[test]
    fn test_parse_header_defaults() {
        let input = "[IMG ID]\nName=Test\nID=00000001\n[END-IMG ID]\n";
        let mp = parse_str(input).unwrap();
        assert_eq!(mp.header.code_page, "1252");
        assert!(mp.header.levels.is_none());
        assert!(mp.header.tree_size.is_none());
    }

    // ----------------------------------------------------------------
    // POI tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_poi() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
[END-IMG ID]

[POI]
Type=0x2C00
Label=Mairie
Data0=(45.1880,5.7245)
EndLevel=4
[END]
"#;
        let mp = parse_str(input).unwrap();
        assert_eq!(mp.points.len(), 1);
        let poi = &mp.points[0];
        assert_eq!(poi.type_code, "0x2C00");
        assert_eq!(poi.label.as_deref(), Some("Mairie"));
        assert_eq!(poi.end_level, Some(4));
        assert!((poi.lat - 45.1880).abs() < 1e-6);
        assert!((poi.lon - 5.7245).abs() < 1e-6);
    }

    // ----------------------------------------------------------------
    // Polyline tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_polyline() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
[END-IMG ID]

[POLYLINE]
Type=0x01
Label=Autoroute A480
Data0=(45.2000,5.7000),(45.2100,5.7100),(45.2200,5.7200)
EndLevel=4
[END]
"#;
        let mp = parse_str(input).unwrap();
        assert_eq!(mp.polylines.len(), 1);
        let poly = &mp.polylines[0];
        assert_eq!(poly.type_code, "0x01");
        assert_eq!(poly.label.as_deref(), Some("Autoroute A480"));
        assert_eq!(poly.coords.len(), 3);
        assert!((poly.coords[0].0 - 45.2000).abs() < 1e-6);
        assert!((poly.coords[2].1 - 5.7200).abs() < 1e-6);
        assert!(poly.routing.is_none());
    }

    // ----------------------------------------------------------------
    // Polygon with hole tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_polygon_with_hole() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
[END-IMG ID]

[POLYGON]
Type=0x50
Label=Forêt
Data0=(45.2000,5.7000),(45.2100,5.7000),(45.2100,5.7100),(45.2000,5.7100),(45.2000,5.7000)
Data1=(45.2020,5.7020),(45.2080,5.7020),(45.2080,5.7080),(45.2020,5.7080),(45.2020,5.7020)
EndLevel=3
[END]
"#;
        let mp = parse_str(input).unwrap();
        assert_eq!(mp.polygons.len(), 1);
        let poly = &mp.polygons[0];
        assert_eq!(poly.type_code, "0x50");
        assert_eq!(poly.coords.len(), 5); // outer ring
        assert_eq!(poly.holes.len(), 1);
        assert_eq!(poly.holes[0].len(), 5); // inner ring
    }

    // ----------------------------------------------------------------
    // Routing attributes tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_routing_attrs() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
[END-IMG ID]

[POLYLINE]
Type=0x01
Label=Autoroute A480
Data0=(45.2000,5.7000),(45.2100,5.7100),(45.2200,5.7200)
RoadID=A480_001
RouteParam=7,4,0,1,0,0,0,0,0
DirIndicator=0
[END]
"#;
        let mp = parse_str(input).unwrap();
        let poly = &mp.polylines[0];
        let routing = poly.routing.as_ref().unwrap();
        assert_eq!(routing.road_id.as_deref(), Some("A480_001"));
        assert_eq!(routing.route_param.as_deref(), Some("7,4,0,1,0,0,0,0,0"));
        assert_eq!(routing.dir_indicator, Some(0));
    }

    // ----------------------------------------------------------------
    // Error tests
    // ----------------------------------------------------------------

    #[test]
    fn test_error_missing_img_id() {
        let input = "[POI]\nType=0x2C00\nData0=(45.0,5.0)\n[END]\n";
        let result = parse_str(input);
        assert!(matches!(result, Err(ParseError::MissingImgId)));
    }

    #[test]
    fn test_error_invalid_coords() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
[END-IMG ID]

[POI]
Type=0x2C00
Data0=(not_a_number,5.7245)
[END]
"#;
        let result = parse_str(input);
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat {
                line: _,
                message: _
            })
        ));
        if let Err(ParseError::InvalidFormat { line, .. }) = result {
            assert!(line > 0, "line number must be positive");
        }
    }

    // ----------------------------------------------------------------
    // Unknown fields (other_fields forward-compat)
    // ----------------------------------------------------------------

    #[test]
    fn test_unknown_fields_in_other_fields() {
        let input = r#"
[IMG ID]
Name=Test
ID=00000001
CustomHeaderField=custom_value
[END-IMG ID]

[POI]
Type=0x2C00
Data0=(45.0,5.0)
CustomPOIField=poi_custom
[END]
"#;
        let mp = parse_str(input).unwrap();
        assert_eq!(
            mp.header
                .other_fields
                .get("CustomHeaderField")
                .map(|s| s.as_str()),
            Some("custom_value")
        );
        assert_eq!(
            mp.points[0]
                .other_fields
                .get("CustomPOIField")
                .map(|s| s.as_str()),
            Some("poi_custom")
        );
    }

    // ----------------------------------------------------------------
    // Coordinate parsing tests
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_data_line_single() {
        let result = parse_data_line("(45.1880,5.7245)", 1).unwrap();
        assert_eq!(result.len(), 1);
        assert!((result[0].0 - 45.188).abs() < 1e-6);
        assert!((result[0].1 - 5.7245).abs() < 1e-6);
    }

    #[test]
    fn test_parse_data_line_multiple() {
        let result =
            parse_data_line("(45.2000,5.7000),(45.2100,5.7100),(45.2200,5.7200)", 1).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_parse_data_line_invalid_lat() {
        let result = parse_data_line("(abc,5.7245)", 10);
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat { line: 10, .. })
        ));
    }

    #[test]
    fn test_parse_data_line_invalid_lon() {
        let result = parse_data_line("(45.0,xyz)", 20);
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat { line: 20, .. })
        ));
    }

    #[test]
    fn test_parse_data_line_missing_comma() {
        let result = parse_data_line("(45.0)", 5);
        assert!(matches!(
            result,
            Err(ParseError::InvalidFormat { line: 5, .. })
        ));
    }

    // ----------------------------------------------------------------
    // New validation tests (added by code review fixes)
    // ----------------------------------------------------------------

    #[test]
    fn test_error_invalid_levels_value() {
        let input = "[IMG ID]\nName=Test\nID=00000001\nLevels=invalid\n[END-IMG ID]\n";
        let result = parse_str(input);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
        if let Err(ParseError::InvalidFormat { message, .. }) = result {
            assert!(
                message.contains("Levels"),
                "message should mention field: {}",
                message
            );
        }
    }

    #[test]
    fn test_error_invalid_end_level_poi() {
        let input = "[IMG ID]\nName=Test\nID=00000001\n[END-IMG ID]\n\
                     [POI]\nType=0x2C00\nData0=(45.0,5.0)\nEndLevel=abc\n[END]\n";
        let result = parse_str(input);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
    }

    #[test]
    fn test_error_poi_multiple_coords() {
        let input = "[IMG ID]\nName=Test\nID=00000001\n[END-IMG ID]\n\
                     [POI]\nType=0x2C00\nData0=(45.0,5.0),(46.0,6.0)\n[END]\n";
        let result = parse_str(input);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
        if let Err(ParseError::InvalidFormat { message, .. }) = result {
            assert!(
                message.contains("exactly 1"),
                "message should mention single point: {}",
                message
            );
        }
    }

    #[test]
    fn test_error_missing_id_field() {
        let input = "[IMG ID]\nName=Test\n[END-IMG ID]\n";
        let result = parse_str(input);
        assert!(matches!(result, Err(ParseError::InvalidFormat { .. })));
        if let Err(ParseError::InvalidFormat { message, .. }) = result {
            assert!(
                message.contains("ID"),
                "message should mention ID field: {}",
                message
            );
        }
    }
}
