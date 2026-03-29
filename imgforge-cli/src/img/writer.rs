// MapBuilder — build orchestrator, faithful to mkgmap MapBuilder.java

use crate::error::ImgError;
use crate::parser::mp_types::MpFile;
use super::area::Area;
use super::coord::Coord;
use super::filesystem::ImgFilesystem;
use super::labelenc::LabelEncoding;
use super::lbl::LblWriter;
use super::line_preparer;
use super::overview::{PointOverview, PolylineOverview, PolygonOverview};
use super::point::Point;
use super::polygon::Polygon;
use super::polyline::Polyline;
use super::rgn::RgnWriter;
use super::subdivision::Subdivision;
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

    // Collect all features with their labels
    let point_labels: Vec<u32> = mp.points.iter()
        .map(|p| lbl.add_label(&p.label))
        .collect();
    let line_labels: Vec<u32> = mp.polylines.iter()
        .map(|pl| lbl.add_label(&pl.label))
        .collect();
    let poly_labels: Vec<u32> = mp.polygons.iter()
        .map(|pg| lbl.add_label(&pg.label))
        .collect();

    // 2. Build zoom levels from header
    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate()
        .map(|(i, &res)| Zoom::new(i as u8, res))
        .collect();

    // 3. Compute bounds from all features
    let mut all_coords: Vec<Coord> = Vec::new();
    for p in &mp.points {
        all_coords.push(p.coord);
    }
    for pl in &mp.polylines {
        all_coords.extend_from_slice(&pl.points);
    }
    for pg in &mp.polygons {
        all_coords.extend_from_slice(&pg.points);
    }

    if all_coords.is_empty() {
        return Err(ImgError::InvalidFormat("No features to compile".into()));
    }

    let bounds = Area::from_coords(&all_coords);

    // 4. Build a single subdivision at the highest resolution (level 0)
    let resolution = levels.first().map(|z| z.resolution).unwrap_or(24);
    let shift = (24 - resolution) as i32;

    let mut subdiv = Subdivision::new(1, 0, resolution);
    subdiv.set_center(&bounds.center());
    subdiv.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    subdiv.is_last = true;

    // 5. Build RGN data for this subdivision
    let mut rgn = RgnWriter::new();

    // Encode points
    let mut points_data = Vec::new();
    for (i, mp_point) in mp.points.iter().enumerate() {
        let mut pt = Point::new(mp_point.type_code, mp_point.coord);
        pt.label_offset = point_labels[i];
        let encoded = pt.write(subdiv.center_lat, subdiv.center_lon, shift);
        points_data.extend_from_slice(&encoded);
    }

    // Encode polylines
    let mut polylines_data = Vec::new();
    for (i, mp_line) in mp.polylines.iter().enumerate() {
        if mp_line.points.len() < 2 { continue; }
        let mut pl = Polyline::new(mp_line.type_code, mp_line.points.clone());
        pl.label_offset = line_labels[i];
        pl.direction = mp_line.direction;

        // Compute deltas for LinePreparer
        let deltas = compute_deltas(&mp_line.points, &subdiv);
        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
            let encoded = pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, false);
            polylines_data.extend_from_slice(&encoded);
        }
    }

    // Encode polygons
    let mut polygons_data = Vec::new();
    for (i, mp_poly) in mp.polygons.iter().enumerate() {
        if mp_poly.points.len() < 3 { continue; }
        let mut pg = Polygon::new(mp_poly.type_code, mp_poly.points.clone());
        pg.label_offset = poly_labels[i];

        let deltas = compute_deltas(&mp_poly.points, &subdiv);
        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
            let encoded = pg.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream);
            polygons_data.extend_from_slice(&encoded);
        }
    }

    // Set subdivision flags
    if !points_data.is_empty() { subdiv.flags |= super::subdivision::HAS_POINTS; }
    if !polylines_data.is_empty() { subdiv.flags |= super::subdivision::HAS_POLYLINES; }
    if !polygons_data.is_empty() { subdiv.flags |= super::subdivision::HAS_POLYGONS; }

    let rgn_offset = rgn.write_subdivision(&points_data, &[], &polylines_data, &polygons_data);
    subdiv.rgn_offset = rgn_offset;

    // 6. Build TRE
    let mut tre = TreWriter::new();
    tre.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    tre.display_priority = mp.header.draw_priority;
    if copyright_label > 0 {
        tre.copyright_offsets.push(copyright_label);
    }

    // Only add level 0 for now — mark it as inherited (last level)
    if let Some(&z) = levels.first() {
        let mut level = z;
        level.inherited = true;
        tre.levels.push(level);
    }
    tre.subdivisions.push(subdiv);

    // Build overviews
    for mp_point in &mp.points {
        tre.point_overviews.push(PointOverview::new(mp_point.type_code as u8, 0, 0));
    }
    for mp_line in &mp.polylines {
        tre.polyline_overviews.push(PolylineOverview::new(mp_line.type_code as u8, 0));
    }
    for mp_poly in &mp.polygons {
        tre.polygon_overviews.push(PolygonOverview::new(mp_poly.type_code as u8, 0));
    }

    // Deduplicate overviews
    tre.polyline_overviews.sort();
    tre.polyline_overviews.dedup();
    tre.polygon_overviews.sort();
    tre.polygon_overviews.dedup();
    tre.point_overviews.sort();
    tre.point_overviews.dedup();

    // 7. Assemble into IMG filesystem
    let map_number = format!("{:08}", mp.header.id);
    let description = if mp.header.name.is_empty() {
        format!("Map {}", mp.header.id)
    } else {
        mp.header.name.clone()
    };

    let mut fs = ImgFilesystem::new(&description);
    fs.add_file(&map_number, "TRE", tre.build());
    fs.add_file(&map_number, "RGN", rgn.build());
    fs.add_file(&map_number, "LBL", lbl.build());

    fs.sync()
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

        // Verify IMG signatures
        assert_eq!(&img[0x10..0x17], b"DSKIMG\0");
        assert_eq!(&img[0x41..0x48], b"GARMIN\0");
        assert_eq!(img[0x1FE], 0x55);
        assert_eq!(img[0x1FF], 0xAA);
        assert!(img.len() > 512);
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
}
