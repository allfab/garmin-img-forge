// MapBuilder — build orchestrator, faithful to mkgmap MapBuilder.java

use crate::error::ImgError;
use crate::parser::mp_types::MpFile;
use crate::routing::graph_builder::{self, RouteParams};
use super::area::Area;
use super::coord::Coord;
use super::filesystem::ImgFilesystem;
use super::labelenc::LabelEncoding;
use super::lbl::LblWriter;
use super::line_preparer;
use super::net::{NetWriter, RoadDef};
use super::nod::NodWriter;
use super::overview::{PointOverview, PolylineOverview, PolygonOverview};
use super::point::Point;
use super::polygon::Polygon;
use super::polyline::Polyline;
use super::rgn::RgnWriter;
use super::splitter::{MapArea, split_area};
use super::subdivision::{self, Subdivision};
use super::tre::TreWriter;
use super::zoom::Zoom;

/// Build a single-tile IMG file from a parsed .mp file
pub fn build_img(mp: &MpFile) -> Result<Vec<u8>, ImgError> {
    let charset = if mp.header.codepage == 0 || mp.header.codepage == 65001 {
        "utf-8"
    } else if mp.header.codepage == 1252 {
        "cp1252"
    } else {
        "ascii"
    };
    let encoding = LabelEncoding::from_charset(charset);

    // 1. Build LBL — add all labels
    let mut lbl = LblWriter::new(encoding);
    let copyright_label = if !mp.header.copyright.is_empty() {
        lbl.add_label(&mp.header.copyright)
    } else {
        0
    };

    let point_labels: Vec<u32> = mp.points.iter()
        .map(|p| lbl.add_label(&p.label))
        .collect();
    let line_labels: Vec<u32> = mp.polylines.iter()
        .map(|pl| lbl.add_label(&pl.label))
        .collect();
    let poly_labels: Vec<u32> = mp.polygons.iter()
        .map(|pg| lbl.add_label(&pg.label))
        .collect();

    // 2. Build zoom levels
    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate()
        .map(|(i, &res)| Zoom::new(i as u8, res))
        .collect();

    // 3. Compute bounds
    let mut min_lat = i32::MAX;
    let mut max_lat = i32::MIN;
    let mut min_lon = i32::MAX;
    let mut max_lon = i32::MIN;

    for p in &mp.points {
        update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, &p.coord);
    }
    for pl in &mp.polylines {
        for c in &pl.points {
            update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, c);
        }
    }
    for pg in &mp.polygons {
        for c in &pg.points {
            update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, c);
        }
    }

    if min_lat == i32::MAX {
        return Err(ImgError::InvalidFormat("No features to compile".into()));
    }

    let bounds = Area::new(min_lat, min_lon, max_lat, max_lon);

    // 4. Check if we need splitting
    let resolution = levels.first().map(|z| z.resolution).unwrap_or(24);
    let shift = (24 - resolution) as i32;

    let mut map_area = MapArea::new(bounds);
    map_area.num_points = mp.points.len();
    map_area.num_lines = mp.polylines.len();
    map_area.num_polygons = mp.polygons.len();

    let sub_areas = if map_area.needs_split() {
        split_area(&map_area, shift, 8)
    } else {
        vec![bounds]
    };

    // 5. Build subdivisions + RGN data
    let mut rgn = RgnWriter::new();
    let mut subdivisions = Vec::new();

    for (idx, area) in sub_areas.iter().enumerate() {
        let mut subdiv = Subdivision::new((idx + 1) as u16, 0, resolution);
        subdiv.set_center(&area.center());
        subdiv.set_bounds(area.min_lat(), area.min_lon(), area.max_lat(), area.max_lon());
        subdiv.is_last = idx == sub_areas.len() - 1;

        // Encode points in this subdivision's area
        let mut points_data = Vec::new();
        for (i, mp_point) in mp.points.iter().enumerate() {
            if area.contains_coord(&mp_point.coord) {
                let mut pt = Point::new(mp_point.type_code, mp_point.coord);
                pt.label_offset = point_labels[i];
                points_data.extend_from_slice(&pt.write(subdiv.center_lat, subdiv.center_lon, shift));
            }
        }

        // Encode polylines in this subdivision's area
        let mut polylines_data = Vec::new();
        for (i, mp_line) in mp.polylines.iter().enumerate() {
            if mp_line.points.len() < 2 { continue; }
            if !mp_line.points.iter().any(|c| area.contains_coord(c)) { continue; }

            let mut pl = Polyline::new(mp_line.type_code, mp_line.points.clone());
            pl.label_offset = line_labels[i];
            pl.direction = mp_line.direction;

            let deltas = compute_deltas(&mp_line.points, &subdiv);
            if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
                polylines_data.extend_from_slice(&pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, false));
            }
        }

        // Encode polygons in this subdivision's area
        let mut polygons_data = Vec::new();
        for (i, mp_poly) in mp.polygons.iter().enumerate() {
            if mp_poly.points.len() < 3 { continue; }
            if !mp_poly.points.iter().any(|c| area.contains_coord(c)) { continue; }

            let mut pg = Polygon::new(mp_poly.type_code, mp_poly.points.clone());
            pg.label_offset = poly_labels[i];

            let deltas = compute_deltas(&mp_poly.points, &subdiv);
            if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
                polygons_data.extend_from_slice(&pg.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream));
            }
        }

        // Set flags
        if !points_data.is_empty() { subdiv.flags |= subdivision::HAS_POINTS; }
        if !polylines_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYLINES; }
        if !polygons_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYGONS; }

        subdiv.rgn_offset = rgn.write_subdivision(&points_data, &[], &polylines_data, &polygons_data);
        subdivisions.push(subdiv);
    }

    // 6. Build TRE
    let mut tre = TreWriter::new();
    tre.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    tre.display_priority = mp.header.draw_priority;
    if copyright_label > 0 {
        tre.copyright_offsets.push(copyright_label);
    }

    if let Some(&z) = levels.first() {
        let mut level = z;
        level.inherited = true;
        tre.levels.push(level);
    }
    tre.subdivisions = subdivisions;

    // Build overviews (deduplicated)
    for mp_point in &mp.points {
        tre.point_overviews.push(PointOverview::new(mp_point.type_code as u8, 0, 0));
    }
    for mp_line in &mp.polylines {
        tre.polyline_overviews.push(PolylineOverview::new(mp_line.type_code as u8, 0));
    }
    for mp_poly in &mp.polygons {
        tre.polygon_overviews.push(PolygonOverview::new(mp_poly.type_code as u8, 0));
    }
    tre.polyline_overviews.sort();
    tre.polyline_overviews.dedup();
    tre.polygon_overviews.sort();
    tre.polygon_overviews.dedup();
    tre.point_overviews.sort();
    tre.point_overviews.dedup();

    // 7. Build NET + NOD if routing data present
    let has_routing = mp.polylines.iter().any(|pl| pl.road_id.is_some());
    let net_data;
    let nod_data;

    if has_routing {
        // Build road definitions
        let mut net_writer = NetWriter::new();
        let mut road_polylines = Vec::new();

        for (i, mp_line) in mp.polylines.iter().enumerate() {
            if let Some(_road_id) = mp_line.road_id {
                let mut road_def = RoadDef::new();
                road_def.label_offsets.push(line_labels[i]);
                road_def.road_length_meters = estimate_road_length(&mp_line.points);

                // Parse RouteParam if available
                let params = if let Some(ref rp) = mp_line.route_param {
                    let p = graph_builder::parse_route_param(rp);
                    road_def.road_class = p.road_class;
                    road_def.speed = p.speed;
                    road_def.one_way = p.one_way;
                    p
                } else {
                    RouteParams::default()
                };

                let road_idx = net_writer.add_road(road_def);
                road_polylines.push((mp_line.points.clone(), road_idx, params));
            }
        }

        // Build routing graph
        let route_nodes = graph_builder::build_graph(&road_polylines);

        // Build NOD
        let mut nod_writer = NodWriter::new();
        for node in route_nodes {
            nod_writer.add_node(node);
        }

        net_data = Some(net_writer.build());
        nod_data = Some(nod_writer.build());
    } else {
        net_data = None;
        nod_data = None;
    }

    // 8. Package subfiles
    let map_number = format!("{:08}", mp.header.id);
    let description = if mp.header.name.is_empty() {
        format!("Map {}", mp.header.id)
    } else {
        mp.header.name.clone()
    };

    let subfiles = TileResult {
        map_number: map_number.clone(),
        description,
        tre: tre.build(),
        rgn: rgn.build(),
        lbl: lbl.build(),
        net: net_data,
        nod: nod_data,
    };

    // Assemble into IMG filesystem
    let mut fs = ImgFilesystem::new(&subfiles.description);
    fs.add_file(&map_number, "TRE", subfiles.tre.clone());
    fs.add_file(&map_number, "RGN", subfiles.rgn.clone());
    fs.add_file(&map_number, "LBL", subfiles.lbl.clone());
    if let Some(ref net) = subfiles.net {
        fs.add_file(&map_number, "NET", net.clone());
    }
    if let Some(ref nod) = subfiles.nod {
        fs.add_file(&map_number, "NOD", nod.clone());
    }

    fs.sync()
}

/// Result of compiling a single tile — subfiles available separately
pub struct TileResult {
    pub map_number: String,
    pub description: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
    pub net: Option<Vec<u8>>,
    pub nod: Option<Vec<u8>>,
}

/// Build subfiles from a parsed .mp without assembling into IMG
pub fn build_subfiles(mp: &MpFile) -> Result<TileResult, ImgError> {
    // Reuse build_img's logic but capture subfiles before assembly
    // For now, we call build_img internally and also build subfiles
    // TODO: refactor to avoid double build
    build_img_internal(mp)
}

fn build_img_internal(mp: &MpFile) -> Result<TileResult, ImgError> {
    // This is the same logic as build_img but returns TileResult
    // We duplicate the construction here to avoid circular dependency
    let charset = if mp.header.codepage == 0 || mp.header.codepage == 65001 {
        "utf-8"
    } else if mp.header.codepage == 1252 {
        "cp1252"
    } else {
        "ascii"
    };
    let encoding = LabelEncoding::from_charset(charset);

    let mut lbl_writer = LblWriter::new(encoding);
    let _copyright_label = if !mp.header.copyright.is_empty() {
        lbl_writer.add_label(&mp.header.copyright)
    } else {
        0
    };

    let point_labels: Vec<u32> = mp.points.iter()
        .map(|p| lbl_writer.add_label(&p.label)).collect();
    let line_labels: Vec<u32> = mp.polylines.iter()
        .map(|pl| lbl_writer.add_label(&pl.label)).collect();
    let poly_labels: Vec<u32> = mp.polygons.iter()
        .map(|pg| lbl_writer.add_label(&pg.label)).collect();

    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate()
        .map(|(i, &res)| Zoom::new(i as u8, res)).collect();

    let mut min_lat = i32::MAX;
    let mut max_lat = i32::MIN;
    let mut min_lon = i32::MAX;
    let mut max_lon = i32::MIN;

    for p in &mp.points { update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, &p.coord); }
    for pl in &mp.polylines { for c in &pl.points { update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, c); } }
    for pg in &mp.polygons { for c in &pg.points { update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, c); } }

    if min_lat == i32::MAX {
        return Err(ImgError::InvalidFormat("No features to compile".into()));
    }
    let bounds = Area::new(min_lat, min_lon, max_lat, max_lon);
    let resolution = levels.first().map(|z| z.resolution).unwrap_or(24);
    let shift = (24 - resolution) as i32;

    let mut map_area = MapArea::new(bounds);
    map_area.num_points = mp.points.len();
    map_area.num_lines = mp.polylines.len();
    map_area.num_polygons = mp.polygons.len();

    let sub_areas = if map_area.needs_split() {
        split_area(&map_area, shift, 8)
    } else {
        vec![bounds]
    };

    let mut rgn = RgnWriter::new();
    let mut subdivisions = Vec::new();

    for (idx, area) in sub_areas.iter().enumerate() {
        let mut subdiv = Subdivision::new((idx + 1) as u16, 0, resolution);
        subdiv.set_center(&area.center());
        subdiv.set_bounds(area.min_lat(), area.min_lon(), area.max_lat(), area.max_lon());
        subdiv.is_last = idx == sub_areas.len() - 1;

        let mut points_data = Vec::new();
        for (i, mp_point) in mp.points.iter().enumerate() {
            if area.contains_coord(&mp_point.coord) {
                let mut pt = Point::new(mp_point.type_code, mp_point.coord);
                pt.label_offset = point_labels[i];
                points_data.extend_from_slice(&pt.write(subdiv.center_lat, subdiv.center_lon, shift));
            }
        }

        let mut polylines_data = Vec::new();
        for (i, mp_line) in mp.polylines.iter().enumerate() {
            if mp_line.points.len() < 2 { continue; }
            if !mp_line.points.iter().any(|c| area.contains_coord(c)) { continue; }
            let mut pl = Polyline::new(mp_line.type_code, mp_line.points.clone());
            pl.label_offset = line_labels[i];
            pl.direction = mp_line.direction;
            let deltas = compute_deltas(&mp_line.points, &subdiv);
            if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
                polylines_data.extend_from_slice(&pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, false));
            }
        }

        let mut polygons_data = Vec::new();
        for (i, mp_poly) in mp.polygons.iter().enumerate() {
            if mp_poly.points.len() < 3 { continue; }
            if !mp_poly.points.iter().any(|c| area.contains_coord(c)) { continue; }
            let mut pg = Polygon::new(mp_poly.type_code, mp_poly.points.clone());
            pg.label_offset = poly_labels[i];
            let deltas = compute_deltas(&mp_poly.points, &subdiv);
            if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
                polygons_data.extend_from_slice(&pg.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream));
            }
        }

        if !points_data.is_empty() { subdiv.flags |= subdivision::HAS_POINTS; }
        if !polylines_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYLINES; }
        if !polygons_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYGONS; }

        subdiv.rgn_offset = rgn.write_subdivision(&points_data, &[], &polylines_data, &polygons_data);
        subdivisions.push(subdiv);
    }

    let mut tre = TreWriter::new();
    tre.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    tre.display_priority = mp.header.draw_priority;
    if let Some(&z) = levels.first() {
        let mut level = z;
        level.inherited = true;
        tre.levels.push(level);
    }
    tre.subdivisions = subdivisions;

    for mp_point in &mp.points { tre.point_overviews.push(PointOverview::new(mp_point.type_code as u8, 0, 0)); }
    for mp_line in &mp.polylines { tre.polyline_overviews.push(PolylineOverview::new(mp_line.type_code as u8, 0)); }
    for mp_poly in &mp.polygons { tre.polygon_overviews.push(PolygonOverview::new(mp_poly.type_code as u8, 0)); }
    tre.polyline_overviews.sort(); tre.polyline_overviews.dedup();
    tre.polygon_overviews.sort(); tre.polygon_overviews.dedup();
    tre.point_overviews.sort(); tre.point_overviews.dedup();

    let has_routing = mp.polylines.iter().any(|pl| pl.road_id.is_some());
    let net_data;
    let nod_data;

    if has_routing {
        let mut net_writer = NetWriter::new();
        let mut road_polylines = Vec::new();
        for (i, mp_line) in mp.polylines.iter().enumerate() {
            if mp_line.road_id.is_some() {
                let mut road_def = RoadDef::new();
                road_def.label_offsets.push(line_labels[i]);
                road_def.road_length_meters = estimate_road_length(&mp_line.points);
                let params = if let Some(ref rp) = mp_line.route_param {
                    let p = graph_builder::parse_route_param(rp);
                    road_def.road_class = p.road_class;
                    road_def.speed = p.speed;
                    road_def.one_way = p.one_way;
                    p
                } else { RouteParams::default() };
                let road_idx = net_writer.add_road(road_def);
                road_polylines.push((mp_line.points.clone(), road_idx, params));
            }
        }
        let route_nodes = graph_builder::build_graph(&road_polylines);
        let mut nod_writer = NodWriter::new();
        for node in route_nodes { nod_writer.add_node(node); }
        net_data = Some(net_writer.build());
        nod_data = Some(nod_writer.build());
    } else {
        net_data = None;
        nod_data = None;
    }

    let map_number = format!("{:08}", mp.header.id);
    let description = if mp.header.name.is_empty() {
        format!("Map {}", mp.header.id)
    } else {
        mp.header.name.clone()
    };

    Ok(TileResult {
        map_number,
        description,
        tre: tre.build(),
        rgn: rgn.build(),
        lbl: lbl_writer.build(),
        net: net_data,
        nod: nod_data,
    })
}

fn update_bounds(min_lat: &mut i32, max_lat: &mut i32, min_lon: &mut i32, max_lon: &mut i32, coord: &Coord) {
    let lat = coord.latitude();
    let lon = coord.longitude();
    if lat < *min_lat { *min_lat = lat; }
    if lat > *max_lat { *max_lat = lat; }
    if lon < *min_lon { *min_lon = lon; }
    if lon > *max_lon { *max_lon = lon; }
}

fn estimate_road_length(points: &[Coord]) -> u32 {
    let mut total = 0.0;
    for i in 1..points.len() {
        total += points[i - 1].distance(&points[i]);
    }
    total as u32
}

/// Compute coordinate deltas for LinePreparer from a list of points
fn compute_deltas(points: &[Coord], subdiv: &Subdivision) -> Vec<(i32, i32)> {
    let mut deltas = Vec::new();
    if points.len() < 2 { return deltas; }

    let mut last_lat = subdiv.round_lat_to_local_shifted(points[0].latitude()) as i32;
    let mut last_lon = subdiv.round_lon_to_local_shifted(points[0].longitude()) as i32;

    for point in &points[1..] {
        let lat = subdiv.round_lat_to_local_shifted(point.latitude()) as i32;
        let lon = subdiv.round_lon_to_local_shifted(point.longitude()) as i32;
        deltas.push((lon - last_lon, lat - last_lat));
        last_lon = lon;
        last_lat = lat;
    }

    deltas
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn test_build_minimal_img() {
        let content = r#"
[IMG ID]
ID=63240001
Name=Test
Levels=24,20,16
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
        let mp = parser::parse_mp(content).unwrap();
        let img = build_img(&mp).unwrap();

        assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
        assert_eq!(&img[0x41..0x48], b"GARMIN\0");
        assert_eq!(img[0x1FE], 0x55);
        assert_eq!(img[0x1FF], 0xAA);
        assert!(img.len() > 512);
    }

    #[test]
    fn test_build_routing_img_has_net_nod() {
        let content = r#"
[IMG ID]
ID=99990099
Name=Routing Test
Levels=24
[END-IMG ID]
[POLYLINE]
Type=0x06
Label=Test Road
RoadID=1
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(48.57,7.75),(48.58,7.76)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        let img = build_img(&mp).unwrap();

        // Must contain NET and NOD subfiles
        let has_net = find_subfile_in_img(&img, "NET");
        let has_nod = find_subfile_in_img(&img, "NOD");
        assert!(has_net, "Routing IMG must contain NET subfile");
        assert!(has_nod, "Routing IMG must contain NOD subfile");
    }

    #[test]
    fn test_build_empty_fails() {
        let mp = MpFile {
            header: crate::parser::mp_types::MpHeader::default(),
            points: Vec::new(),
            polylines: Vec::new(),
            polygons: Vec::new(),
        };
        assert!(build_img(&mp).is_err());
    }

    fn find_subfile_in_img(img: &[u8], ext: &str) -> bool {
        let dir_start = 2 * 512;
        let mut pos = dir_start;
        while pos + 512 <= img.len() {
            if img[pos] == 0x01 {
                let file_ext = std::str::from_utf8(&img[pos + 9..pos + 12]).unwrap_or("");
                let part = u16::from_le_bytes([img[pos + 0x11], img[pos + 0x12]]);
                if file_ext == ext && part == 0 {
                    return true;
                }
            }
            pos += 512;
        }
        false
    }
}
