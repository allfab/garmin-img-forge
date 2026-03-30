pub mod mp_types;

use crate::error::ParseError;
use crate::img::coord::Coord;
use mp_types::*;

/// Parser state machine for Polish Map (.mp) files
pub fn parse_mp(content: &str) -> Result<MpFile, ParseError> {
    let mut header = MpHeader::default();
    header.codepage = 1252;
    header.draw_priority = 25;
    header.levels = vec![24, 22, 20, 18, 16];

    let mut points = Vec::new();
    let mut polylines = Vec::new();
    let mut polygons = Vec::new();

    let mut section = Section::None;
    let mut current_point: Option<MpPoint> = None;
    let mut current_polyline: Option<MpPolyline> = None;
    let mut current_polygon: Option<MpPolygon> = None;

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        // Section headers
        if line.eq_ignore_ascii_case("[IMG ID]") {
            section = Section::Header;
            continue;
        }
        if line.eq_ignore_ascii_case("[END]") || line.eq_ignore_ascii_case("[END-IMG ID]") {
            match section {
                Section::Point => {
                    if let Some(p) = current_point.take() {
                        points.push(p);
                    }
                }
                Section::Polyline => {
                    if let Some(pl) = current_polyline.take() {
                        polylines.push(pl);
                    }
                }
                Section::Polygon => {
                    if let Some(pg) = current_polygon.take() {
                        polygons.push(pg);
                    }
                }
                _ => {}
            }
            section = Section::None;
            continue;
        }
        if line.eq_ignore_ascii_case("[POI]") || line.eq_ignore_ascii_case("[RGN10]") || line.eq_ignore_ascii_case("[POINT]") {
            section = Section::Point;
            current_point = Some(MpPoint {
                type_code: 0,
                label: String::new(),
                coord: Coord::new(0, 0),
                end_level: None,
                city_name: None,
                region_name: None,
                country_name: None,
                zip: None,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("[POLYLINE]") || line.eq_ignore_ascii_case("[RGN40]") {
            section = Section::Polyline;
            current_polyline = Some(MpPolyline {
                type_code: 0,
                label: String::new(),
                points: Vec::new(),
                end_level: None,
                direction: false,
                road_id: None,
                route_param: None,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("[POLYGON]") || line.eq_ignore_ascii_case("[RGN80]") {
            section = Section::Polygon;
            current_polygon = Some(MpPolygon {
                type_code: 0,
                label: String::new(),
                points: Vec::new(),
                end_level: None,
            });
            continue;
        }

        // Key=Value parsing
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match section {
                Section::Header => parse_header_field(&mut header, key, value),
                Section::Point => {
                    if let Some(ref mut p) = current_point {
                        parse_point_field(p, key, value, line_num)?;
                    }
                }
                Section::Polyline => {
                    if let Some(ref mut pl) = current_polyline {
                        parse_polyline_field(pl, key, value, line_num)?;
                    }
                }
                Section::Polygon => {
                    if let Some(ref mut pg) = current_polygon {
                        parse_polygon_field(pg, key, value, line_num)?;
                    }
                }
                Section::None => {}
            }
        }
    }

    Ok(MpFile {
        header,
        points,
        polylines,
        polygons,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Section {
    None,
    Header,
    Point,
    Polyline,
    Polygon,
}

fn parse_header_field(header: &mut MpHeader, key: &str, value: &str) {
    match key.to_lowercase().as_str() {
        "id" => {
            header.id = value.parse().unwrap_or_else(|_| {
                tracing::warn!("Invalid map ID '{}', defaulting to 0", value);
                0
            });
        }
        "name" => header.name = value.to_string(),
        "copyright" => header.copyright = value.to_string(),
        "codepage" => header.codepage = value.parse().unwrap_or(1252),
        "datum" => header.datum = value.to_string(),
        "transparent" => header.transparent = value == "Y" || value == "1",
        "drawpriority" => header.draw_priority = value.parse().unwrap_or(25),
        "previewlat" => header.preview_lat = value.parse().unwrap_or(0.0),
        "previewlon" | "previewlong" => header.preview_lon = value.parse().unwrap_or(0.0),
        "levels" => {
            // "Levels=2" means number of levels, not resolution list
            // "Levels=24,20,16" is a comma-separated resolution list (imgforge-cli format)
            if value.contains(',') {
                header.levels = value
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();
            } else {
                // Single number = number of levels (cGPSmapper/BDTOPO format)
                // Actual resolutions come from Level0=, Level1=, etc.
                // Clear defaults and initialize with placeholder resolution
                let num_levels: usize = value.parse().unwrap_or(1);
                header.levels = vec![24; num_levels];
            }
        }
        k if k.starts_with("level") && k.len() > 5 => {
            // Level0=24, Level1=18, etc. — set resolution for specific level
            if let Ok(level_idx) = k[5..].parse::<usize>() {
                if let Ok(resolution) = value.parse::<u8>() {
                    if level_idx < header.levels.len() {
                        header.levels[level_idx] = resolution;
                    } else {
                        // Extend if needed
                        while header.levels.len() <= level_idx {
                            header.levels.push(24);
                        }
                        header.levels[level_idx] = resolution;
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_point_field(point: &mut MpPoint, key: &str, value: &str, line_num: usize) -> Result<(), ParseError> {
    match key.to_lowercase().as_str() {
        "type" => point.type_code = parse_type(value),
        "label" => point.label = value.to_string(),
        "endlevel" => point.end_level = value.parse().ok(),
        "cityname" => point.city_name = Some(value.to_string()),
        "regionname" => point.region_name = Some(value.to_string()),
        "countryname" => point.country_name = Some(value.to_string()),
        "zip" => point.zip = Some(value.to_string()),
        k if k.starts_with("data") => {
            let coords = parse_coords(value, line_num)?;
            if let Some(c) = coords.first() {
                point.coord = *c;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_polyline_field(pl: &mut MpPolyline, key: &str, value: &str, line_num: usize) -> Result<(), ParseError> {
    match key.to_lowercase().as_str() {
        "type" => pl.type_code = parse_type(value),
        "label" => pl.label = value.to_string(),
        "endlevel" => pl.end_level = value.parse().ok(),
        "dirindicator" => pl.direction = value == "1",
        "roadid" => pl.road_id = value.parse().ok(),
        "routeparam" => pl.route_param = Some(value.to_string()),
        k if k.starts_with("data") => {
            let coords = parse_coords(value, line_num)?;
            pl.points.extend(coords);
        }
        _ => {}
    }
    Ok(())
}

fn parse_polygon_field(pg: &mut MpPolygon, key: &str, value: &str, line_num: usize) -> Result<(), ParseError> {
    match key.to_lowercase().as_str() {
        "type" => pg.type_code = parse_type(value),
        "label" => pg.label = value.to_string(),
        "endlevel" => pg.end_level = value.parse().ok(),
        k if k.starts_with("data") => {
            let coords = parse_coords(value, line_num)?;
            pg.points.extend(coords);
        }
        _ => {}
    }
    Ok(())
}

fn parse_type(value: &str) -> u32 {
    if value.starts_with("0x") || value.starts_with("0X") {
        u32::from_str_radix(&value[2..], 16).unwrap_or(0)
    } else {
        value.parse().unwrap_or(0)
    }
}

/// Parse coordinate string: "(lat,lon),(lat,lon),..."
fn parse_coords(value: &str, line_num: usize) -> Result<Vec<Coord>, ParseError> {
    let mut coords = Vec::new();
    let mut rest = value.trim();

    while let Some(start) = rest.find('(') {
        let end = rest[start..].find(')').ok_or_else(|| ParseError::InvalidCoord(
            format!("Unclosed parenthesis at line {}", line_num + 1),
        ))?;
        let pair = &rest[start + 1..start + end];
        let parts: Vec<&str> = pair.split(',').collect();
        if parts.len() >= 2 {
            let lat: f64 = parts[0].trim().parse().map_err(|_| {
                ParseError::InvalidCoord(format!("Invalid lat '{}' at line {}", parts[0], line_num + 1))
            })?;
            let lon: f64 = parts[1].trim().parse().map_err(|_| {
                ParseError::InvalidCoord(format!("Invalid lon '{}' at line {}", parts[1], line_num + 1))
            })?;
            coords.push(Coord::from_degrees(lat, lon));
        }
        rest = &rest[start + end + 1..];
    }

    Ok(coords)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_mp() {
        let content = r#"
[IMG ID]
ID=63240001
Name=Test Map
[END-IMG ID]
[POI]
Type=0x2C00
Label=Test POI
Data0=(48.5734,7.7521)
[END]
[POLYLINE]
Type=0x01
Label=Main Street
Data0=(48.5734,7.7521),(48.5834,7.7621)
[END]
[POLYGON]
Type=0x03
Label=Forest
Data0=(48.57,7.75),(48.58,7.75),(48.58,7.76),(48.57,7.76)
[END]
"#;
        let mp = parse_mp(content).unwrap();
        assert_eq!(mp.header.id, 63240001);
        assert_eq!(mp.header.name, "Test Map");
        assert_eq!(mp.points.len(), 1);
        assert_eq!(mp.points[0].label, "Test POI");
        assert_eq!(mp.polylines.len(), 1);
        assert_eq!(mp.polylines[0].points.len(), 2);
        assert_eq!(mp.polygons.len(), 1);
        assert_eq!(mp.polygons[0].points.len(), 4);
    }

    #[test]
    fn test_parse_coords() {
        let coords = parse_coords("(48.5734,7.7521),(48.5834,7.7621)", 0).unwrap();
        assert_eq!(coords.len(), 2);
        assert!((coords[0].lat_degrees() - 48.5734).abs() < 0.001);
    }

    #[test]
    fn test_parse_type_hex() {
        assert_eq!(parse_type("0x2C00"), 0x2C00u32);
        assert_eq!(parse_type("0x01"), 1u32);
    }

    #[test]
    fn test_parse_type_extended() {
        assert_eq!(parse_type("0x10f04"), 0x10f04u32);
        assert_eq!(parse_type("0x2C04"), 0x2C04u32);
        assert_eq!(parse_type("0x1101C"), 0x1101Cu32);
    }

    #[test]
    fn test_parse_routing_attrs() {
        let content = r#"
[POLYLINE]
Type=0x06
Label=Highway
RoadID=1234
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(48.57,7.75),(48.58,7.76)
[END]
"#;
        let mp = parse_mp(content).unwrap();
        assert_eq!(mp.polylines[0].road_id, Some(1234));
        assert!(mp.polylines[0].route_param.is_some());
    }

    #[test]
    fn test_parse_empty() {
        let mp = parse_mp("").unwrap();
        assert!(mp.points.is_empty());
        assert!(mp.polylines.is_empty());
        assert!(mp.polygons.is_empty());
    }
}
