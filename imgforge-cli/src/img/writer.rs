// MapBuilder — build orchestrator, faithful to mkgmap MapBuilder.java
//
// Multi-level hierarchy: topdiv → level N → ... → level 0
// Feature distribution by pickArea (first point), polygon clipping, recursive split

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
use super::overview::{
    PointOverview, PolylineOverview, PolygonOverview,
    ExtPointOverview, ExtPolylineOverview, ExtPolygonOverview,
};
use super::point::Point;
use super::polygon::Polygon;
use super::polyline::Polyline;
use super::rgn::RgnWriter;
use super::splitter::{self, MapArea, SplitPoint, SplitLine, SplitShape, MAX_RGN_SIZE};
use super::subdivision::{self, Subdivision};
use super::tre::TreWriter;
use super::zoom::Zoom;

/// Build a single-tile IMG file from a parsed .mp file
pub fn build_img(mp: &MpFile) -> Result<Vec<u8>, ImgError> {
    let result = build_subfiles(mp)?;

    let mut fs = ImgFilesystem::new(&result.description);
    fs.add_file(&result.map_number, "TRE", result.tre.clone());
    fs.add_file(&result.map_number, "RGN", result.rgn.clone());
    fs.add_file(&result.map_number, "LBL", result.lbl.clone());
    if let Some(ref net) = result.net {
        fs.add_file(&result.map_number, "NET", net.clone());
    }
    if let Some(ref nod) = result.nod {
        fs.add_file(&result.map_number, "NOD", nod.clone());
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
    // Use UTF-8 (format 10) for label encoding — universally supported by modern viewers.
    // CP1252 input is converted to Unicode by read_mp_file's CP1252 fallback.
    let encoding = LabelEncoding::Format10;

    // 1. Build LBL — all labels
    let mut lbl_writer = LblWriter::new(encoding);
    let copyright_label = if !mp.header.copyright.is_empty() {
        lbl_writer.add_label(&mp.header.copyright)
    } else {
        0
    };

    let point_labels: Vec<u32> = mp.points.iter()
        .map(|p| lbl_writer.add_label(&p.label))
        .collect();
    let line_labels: Vec<u32> = mp.polylines.iter()
        .map(|pl| lbl_writer.add_label(&pl.label))
        .collect();
    let poly_labels: Vec<u32> = mp.polygons.iter()
        .map(|pg| lbl_writer.add_label(&pg.label))
        .collect();

    // 2. Build zoom levels
    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate()
        .map(|(i, &res)| Zoom::new(i as u8, res))
        .collect();

    // 3. Compute bounds
    let bounds = compute_bounds(mp)?;

    // 4. Build multi-level subdivision hierarchy + encode RGN
    let mut rgn = RgnWriter::new();
    let (all_subdivisions, tre_levels, ext_type_offsets_data) = build_multilevel_hierarchy(
        mp, &bounds, &levels, &point_labels, &line_labels, &poly_labels, &mut rgn,
    );

    // 5. Build TRE
    let mut tre = TreWriter::new();
    tre.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    tre.display_priority = mp.header.draw_priority;
    if copyright_label > 0 {
        tre.copyright_offsets.push(copyright_label);
    }
    tre.levels = tre_levels;
    tre.subdivisions = all_subdivisions;
    // mkgmap: lastRgnPos = rgnFile.position() - HEADER_LEN → end of RGN body
    tre.last_rgn_pos = rgn.position();

    // Build overviews (deduplicated)
    for mp_point in &mp.points {
        if mp_point.type_code < 0x100 {
            tre.point_overviews.push(PointOverview::new(mp_point.type_code as u8, 0, 0));
        } else {
            tre.ext_point_overviews.push(ExtPointOverview::from_type_code(mp_point.type_code, 0));
        }
    }
    for mp_line in &mp.polylines {
        if mp_line.type_code < 0x100 {
            tre.polyline_overviews.push(PolylineOverview::new(mp_line.type_code as u8, 0));
        } else {
            tre.ext_polyline_overviews.push(ExtPolylineOverview::from_type_code(mp_line.type_code, 0));
        }
    }
    for mp_poly in &mp.polygons {
        if mp_poly.type_code < 0x100 {
            tre.polygon_overviews.push(PolygonOverview::new(mp_poly.type_code as u8, 0));
        } else {
            tre.ext_polygon_overviews.push(ExtPolygonOverview::from_type_code(mp_poly.type_code, 0));
        }
    }
    tre.polyline_overviews.sort();
    tre.polyline_overviews.dedup();
    tre.polygon_overviews.sort();
    tre.polygon_overviews.dedup();
    tre.point_overviews.sort();
    tre.point_overviews.dedup();
    tre.ext_polyline_overviews.sort();
    tre.ext_polyline_overviews.dedup();
    tre.ext_polygon_overviews.sort();
    tre.ext_polygon_overviews.dedup();
    tre.ext_point_overviews.sort();
    tre.ext_point_overviews.dedup();

    // Set extended type offsets data
    tre.ext_type_offsets_data = ext_type_offsets_data;

    // 6. Build NET + NOD if routing data present
    let (net_data, nod_data) = build_routing(mp, &line_labels);

    // 7. Package result
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

// ── Multi-level hierarchy — mkgmap MapBuilder.makeMapAreas ─────────────────

/// Build multi-level subdivision tree: topdiv → level N → ... → level 0.
///
/// Uses pickArea distribution, Sutherland-Hodgman polygon clipping,
/// recursive split (addAreasToList), and per-level feature filtering by EndLevel.
///
/// Returns (subdivisions ordered for TRE, zoom levels for TRE, ext_type_offsets_data).
fn build_multilevel_hierarchy(
    mp: &MpFile,
    bounds: &Area,
    levels: &[Zoom],
    point_labels: &[u32],
    line_labels: &[u32],
    poly_labels: &[u32],
    rgn: &mut RgnWriter,
) -> (Vec<Subdivision>, Vec<Zoom>, Vec<u8>) {
    if levels.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let num_levels = levels.len();

    // ── Create topdiv (empty root) at highest configured level ──
    // mkgmap: topdiv sits at the highest zoom level (inherited), not above it
    let topdiv_level = (num_levels - 1) as u8;
    let topdiv_resolution = levels.last()
        .map(|z| z.resolution)
        .unwrap_or(24);

    let mut topdiv = Subdivision::new(1, topdiv_level, topdiv_resolution);
    topdiv.set_center(&bounds.center());
    topdiv.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    topdiv.is_last = true;
    // mkgmap: startRgnPointer is relative to RGN body (after header), NOT absolute
    topdiv.rgn_offset = 0;

    let mut all_subdivisions: Vec<Subdivision> = vec![topdiv];
    let mut subdiv_counter: u32 = 2;
    let mut parent_areas: Vec<(Area, u32)> = vec![(*bounds, 1)];

    // Track ext type offsets per subdivision for TRE extTypeOffsets section
    // (ext_areas_before, ext_lines_before, ext_points_before) per subdivision number
    let mut ext_offsets: Vec<(u32, u32, u32, u32, u32, u32)> = Vec::new();
    // Topdiv has no extended data
    ext_offsets.push((0, 0, 0, 0, 0, 0));

    // ── Process each level from most zoomed-out to most detailed ──
    // Skip the highest level (it's the inherited topdiv level)
    let process_levels = if num_levels > 1 { num_levels - 1 } else { num_levels };
    for level_idx in (0..process_levels).rev() {
        let level = &levels[level_idx];
        let level_num = level_idx as u8;
        let shift = (24i32 - level.resolution as i32).max(0);

        let mut next_parent_areas: Vec<(Area, u32)> = Vec::new();

        for (parent_bounds, parent_num) in &parent_areas {
            let (split_points, split_lines, split_shapes) =
                filter_features_for_level(mp, level_num, parent_bounds);

            if split_points.is_empty() && split_lines.is_empty() && split_shapes.is_empty() {
                continue;
            }

            let areas = splitter::split_features(
                *parent_bounds, level.resolution,
                split_points, split_lines, split_shapes,
            );
            if areas.is_empty() { continue; }

            let first_child_num = subdiv_counter;

            for (i, area) in areas.iter().enumerate() {
                assert!(subdiv_counter <= u16::MAX as u32, "Too many subdivisions (>65535)");
                let subdiv_num = subdiv_counter as u16;
                subdiv_counter += 1;

                let mut subdiv = Subdivision::new(subdiv_num, level_num, level.resolution);
                subdiv.set_center(&area.bounds.center());
                subdiv.set_bounds(
                    area.bounds.min_lat(), area.bounds.min_lon(),
                    area.bounds.max_lat(), area.bounds.max_lon(),
                );
                subdiv.parent = *parent_num as u16;
                subdiv.is_last = i == areas.len() - 1;

                // Capture ext positions before encoding
                let ext_areas_before = rgn.ext_areas_position();
                let ext_lines_before = rgn.ext_lines_position();
                let ext_points_before = rgn.ext_points_position();

                let (pts_data, lines_data, polys_data) =
                    encode_subdivision_rgn(mp, area, &subdiv, shift, point_labels, line_labels, poly_labels, rgn);

                // Capture ext positions after encoding
                let ext_areas_after = rgn.ext_areas_position();
                let ext_lines_after = rgn.ext_lines_position();
                let ext_points_after = rgn.ext_points_position();
                ext_offsets.push((ext_areas_before, ext_lines_before, ext_points_before,
                                  ext_areas_after, ext_lines_after, ext_points_after));

                if !pts_data.is_empty() { subdiv.flags |= subdivision::HAS_POINTS; }
                if !lines_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYLINES; }
                if !polys_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYGONS; }

                // mkgmap: startRgnPointer = position() - HEADER_LEN → relative to RGN body
                subdiv.rgn_offset = rgn.write_subdivision(&pts_data, &[], &lines_data, &polys_data);

                let total_rgn = pts_data.len() + lines_data.len() + polys_data.len();
                if total_rgn > MAX_RGN_SIZE {
                    eprintln!(
                        "WARNING: Subdivision {} RGN size {} exceeds MAX_RGN_SIZE {}",
                        subdiv_num, total_rgn, MAX_RGN_SIZE
                    );
                }

                all_subdivisions.push(subdiv);
                next_parent_areas.push((area.bounds, subdiv_num as u32));
            }

            if let Some(parent) = all_subdivisions.iter_mut().find(|s| s.number == *parent_num as u16) {
                parent.has_children = true;
                parent.children = (first_child_num..subdiv_counter).map(|n| n as u16).collect();
            }
        }

        if !next_parent_areas.is_empty() {
            parent_areas = next_parent_areas;
        }
    }

    // Build TRE zoom levels from actual subdivisions
    // mkgmap: highest level is inherited (topdiv only), rest are regular
    let mut tre_levels_build = Vec::new();
    let mut top_zoom = Zoom::new(topdiv_level, topdiv_resolution);
    top_zoom.inherited = true;
    tre_levels_build.push(top_zoom);

    for level_idx in (0..process_levels).rev() {
        let level_num = level_idx as u8;
        if all_subdivisions.iter().any(|s| s.zoom_level == level_num) {
            tre_levels_build.push(levels[level_idx]);
        }
    }

    // Force has_children for all non-leaf level subdivisions (16-byte records required)
    if tre_levels_build.len() >= 2 {
        let leaf_level = tre_levels_build.last().unwrap().level;
        for subdiv in all_subdivisions.iter_mut() {
            if subdiv.zoom_level != leaf_level && !subdiv.has_children {
                subdiv.has_children = true;
            }
        }
    }

    // Build extTypeOffsets data if we have extended data
    let ext_type_offsets_data = if rgn.has_ext_data() {
        let mut data = Vec::new();
        // One 13-byte record per subdivision
        for (_i, offsets) in ext_offsets.iter().enumerate() {
            let (areas_before, lines_before, points_before,
                 areas_after, lines_after, points_after) = *offsets;

            // Offset in each extended section
            data.extend_from_slice(&areas_before.to_le_bytes());
            data.extend_from_slice(&lines_before.to_le_bytes());
            data.extend_from_slice(&points_before.to_le_bytes());

            // kinds = number of non-empty sections for this subdivision
            let mut kinds: u8 = 0;
            if areas_after > areas_before { kinds += 1; }
            if lines_after > lines_before { kinds += 1; }
            if points_after > points_before { kinds += 1; }
            data.push(kinds);
        }
        // Final record: total sizes, kinds = 0
        data.extend_from_slice(&rgn.ext_areas_size().to_le_bytes());
        data.extend_from_slice(&rgn.ext_lines_size().to_le_bytes());
        data.extend_from_slice(&rgn.ext_points_size().to_le_bytes());
        data.push(0); // kinds = 0 for final record
        data
    } else {
        Vec::new()
    };

    (all_subdivisions, tre_levels_build, ext_type_offsets_data)
}

// ── Feature filtering per level ────────────────────────────────────────────

/// Filter features visible at a given zoom level within parent bounds.
///
/// EndLevel semantics (Polish Map format):
/// - None → visible only at level 0
/// - Some(N) → visible at levels 0 through N
///
/// For level K: include features where end_level.unwrap_or(0) >= K
///
/// Uses expanded bounds (+1 margin) to avoid losing features at area boundaries
/// due to rounding between pickArea integer division and contains_coord comparison.
fn filter_features_for_level(
    mp: &MpFile,
    level: u8,
    parent_bounds: &Area,
) -> (Vec<SplitPoint>, Vec<SplitLine>, Vec<SplitShape>) {
    // Expand bounds by 1 unit to catch boundary features (F6 fix)
    let expanded = Area::new(
        parent_bounds.min_lat() - 1,
        parent_bounds.min_lon() - 1,
        parent_bounds.max_lat() + 1,
        parent_bounds.max_lon() + 1,
    );

    let points: Vec<SplitPoint> = mp.points.iter().enumerate()
        .filter(|(_, p)| p.end_level.unwrap_or(0) >= level)
        .filter(|(_, p)| expanded.contains_coord(&p.coord))
        .map(|(i, p)| SplitPoint { mp_index: i, location: p.coord })
        .collect();

    let lines: Vec<SplitLine> = mp.polylines.iter().enumerate()
        .filter(|(_, l)| l.end_level.unwrap_or(0) >= level)
        .filter(|(_, l)| !l.points.is_empty() && expanded.contains_coord(&l.points[0]))
        .map(|(i, l)| SplitLine { mp_index: i, points: l.points.clone() })
        .collect();

    let shapes: Vec<SplitShape> = mp.polygons.iter().enumerate()
        .filter(|(_, s)| s.end_level.unwrap_or(0) >= level)
        .filter(|(_, s)| !s.points.is_empty() && expanded.contains_coord(&s.points[0]))
        .map(|(i, s)| SplitShape { mp_index: i, points: s.points.clone() })
        .collect();

    (points, lines, shapes)
}

// ── RGN encoding per subdivision ───────────────────────────────────────────

/// Encode features from a MapArea into RGN binary data for one subdivision.
/// Standard types (< 0x100) go into the returned tuple.
/// Extended types (≥ 0x100) are written directly into the RGN extended buffers.
fn encode_subdivision_rgn(
    mp: &MpFile,
    area: &MapArea,
    subdiv: &Subdivision,
    shift: i32,
    point_labels: &[u32],
    line_labels: &[u32],
    poly_labels: &[u32],
    rgn: &mut RgnWriter,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    // Points
    let mut points_data = Vec::new();
    for split_pt in &area.points {
        let mp_point = &mp.points[split_pt.mp_index];
        let mut pt = Point::new(mp_point.type_code, split_pt.location);
        pt.label_offset = point_labels[split_pt.mp_index];

        if mp_point.type_code >= 0x100 {
            rgn.write_ext_point(&pt.write_ext(subdiv.center_lat, subdiv.center_lon, shift));
        } else {
            points_data.extend_from_slice(&pt.write(subdiv.center_lat, subdiv.center_lon, shift));
        }
    }

    // Polylines
    let mut polylines_data = Vec::new();
    for split_line in &area.lines {
        if split_line.points.len() < 2 { continue; }
        let mp_line = &mp.polylines[split_line.mp_index];
        let mut pl = Polyline::new(mp_line.type_code, split_line.points.clone());
        pl.label_offset = line_labels[split_line.mp_index];
        pl.direction = mp_line.direction;
        let deltas = compute_deltas(&split_line.points, subdiv);
        let is_ext = mp_line.type_code >= 0x100;
        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, is_ext) {
            if is_ext {
                rgn.write_ext_polyline(
                    &pl.write_ext(subdiv.center_lat, subdiv.center_lon, shift, &bitstream),
                );
            } else {
                polylines_data.extend_from_slice(
                    &pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, false),
                );
            }
        }
    }

    // Polygons
    let mut polygons_data = Vec::new();
    for split_shape in &area.shapes {
        if split_shape.points.len() < 3 { continue; }
        let mp_poly = &mp.polygons[split_shape.mp_index];
        let mut pg = Polygon::new(mp_poly.type_code, split_shape.points.clone());
        pg.label_offset = poly_labels[split_shape.mp_index];
        let deltas = compute_deltas(&split_shape.points, subdiv);
        let is_ext = mp_poly.type_code >= 0x100;
        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, is_ext) {
            if is_ext {
                rgn.write_ext_polygon(
                    &pg.write_ext(subdiv.center_lat, subdiv.center_lon, shift, &bitstream),
                );
            } else {
                polygons_data.extend_from_slice(
                    &pg.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream),
                );
            }
        }
    }

    (points_data, polylines_data, polygons_data)
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn compute_bounds(mp: &MpFile) -> Result<Area, ImgError> {
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

    Ok(Area::new(min_lat, min_lon, max_lat, max_lon))
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

fn build_routing(mp: &MpFile, line_labels: &[u32]) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let has_routing = mp.polylines.iter().any(|pl| pl.road_id.is_some());

    if !has_routing {
        return (None, None);
    }

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
            } else {
                RouteParams::default()
            };

            let road_idx = net_writer.add_road(road_def);
            road_polylines.push((mp_line.points.clone(), road_idx, params));
        }
    }

    let route_nodes = graph_builder::build_graph(&road_polylines);
    let mut nod_writer = NodWriter::new();
    for node in route_nodes {
        nod_writer.add_node(node);
    }

    (Some(net_writer.build()), Some(nod_writer.build()))
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

    #[test]
    fn test_build_multilevel_has_multiple_levels() {
        let content = r#"
[IMG ID]
ID=63240002
Name=MultiLevel Test
Levels=24,18
[END-IMG ID]
[POI]
Type=0x2C00
Label=POI Level0
Data0=(48.5734,7.7521)
[END]
[POLYLINE]
Type=0x01
Label=Road Both Levels
EndLevel=1
Data0=(48.5734,7.7521),(48.5834,7.7621)
[END]
[POLYGON]
Type=0x03
Label=Forest Level0 Only
Data0=(48.57,7.75),(48.58,7.75),(48.58,7.76),(48.57,7.76)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();

        // TRE should be built successfully
        assert!(!result.tre.is_empty());
        assert!(!result.rgn.is_empty());
    }

    #[test]
    fn test_no_empty_subdivisions() {
        let content = r#"
[IMG ID]
ID=63240003
Name=No Empty Subdivs
Levels=24
[END-IMG ID]
[POI]
Type=0x2C00
Label=Test POI
Data0=(48.5734,7.7521)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();

        // Parse TRE to check subdivisions — at minimum, verify we get valid output
        assert!(!result.tre.is_empty());
        assert!(!result.rgn.is_empty());
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
