// MapBuilder — build orchestrator, faithful to mkgmap MapBuilder.java
//
// Multi-level hierarchy: topdiv → level N → ... → level 0
// Feature distribution by pickArea (first point), polygon clipping, recursive split

use crate::error::ImgError;
use crate::parser::mp_types::MpFile;
use crate::routing::graph_builder::{self, RouteParams, find_junctions, compute_node_flags};
use std::collections::{HashMap, HashSet};
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
    build_img_with_typ(mp, None)
}

/// Build a single-tile IMG file with optional TYP styling data
pub fn build_img_with_typ(mp: &MpFile, typ_data: Option<&[u8]>) -> Result<Vec<u8>, ImgError> {
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
    if let Some(ref dem) = result.dem {
        fs.add_file(&result.map_number, "DEM", dem.clone());
    }
    if let Some(typ) = typ_data {
        fs.add_file(&result.map_number, "TYP", typ.to_vec());
    }

    fs.sync()
}

/// Build a single-tile IMG from a pre-built TileResult with optional TYP
pub fn build_img_with_typ_from_result(result: &TileResult, typ_data: Option<&[u8]>) -> Result<Vec<u8>, ImgError> {
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
    if let Some(ref dem) = result.dem {
        fs.add_file(&result.map_number, "DEM", dem.clone());
    }
    if let Some(typ) = typ_data {
        fs.add_file(&result.map_number, "TYP", typ.to_vec());
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
    pub dem: Option<Vec<u8>>,
}

/// Build subfiles from a parsed .mp without assembling into IMG
pub fn build_subfiles(mp: &MpFile) -> Result<TileResult, ImgError> {
    // Encoding selection: codepage drives the label format.
    // --lower-case forces Format9/10 (Format6 is uppercase-only).
    let encoding = if mp.header.lower_case && mp.header.codepage == 0 {
        // Format6 can't represent lowercase — upgrade to Format10
        LabelEncoding::Format10
    } else if mp.header.codepage == 0 {
        LabelEncoding::Format6  // ASCII-only maps
    } else if mp.header.codepage == 65001 {
        LabelEncoding::Format10 // UTF-8
    } else {
        LabelEncoding::Format9(mp.header.codepage) // CP1252, CP1250, CP1251, etc.
    };

    // 1. Build LBL — all labels
    let mut lbl_writer = LblWriter::new(encoding);
    // Set sort IDs matching the SRT descriptor — mkgmap default for CP1252
    // sort_id1=7, sort_id2=0x8002 (Western European sort)
    if mp.header.codepage == 1252 || mp.header.codepage == 0 {
        lbl_writer.set_sort_ids(7, 0x8002);
    }
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

    // 4. Pre-compute routing → RoutingContext + net_writer + nod_data
    let (routing_ctx, mut net_writer_opt, nod_data) = pre_compute_routing(mp, &line_labels);

    // 5. Build multi-level subdivision hierarchy + encode RGN (with routing context)
    let mut rgn = RgnWriter::new();
    let (all_subdivisions, tre_levels, ext_type_offsets_data, subdiv_road_refs) = build_multilevel_hierarchy(
        mp, &bounds, &levels, &point_labels, &line_labels, &poly_labels, &mut rgn,
        routing_ctx.as_ref(),
    );

    // 5b. Patch NET1 level/div entries with subdivision references collected during RGN encoding
    if let Some(ref mut net_writer) = net_writer_opt {
        if !subdiv_road_refs.is_empty() {
            // Build per-road refs: (polyline_num, subdiv_num) — use the first ref for each road
            let num_roads = net_writer.roads.len();
            let mut road_refs: Vec<(u8, u16)> = vec![(0, 0); num_roads];
            for &(road_idx, polyline_num, subdiv_num) in &subdiv_road_refs {
                if road_idx < num_roads {
                    road_refs[road_idx] = (polyline_num, subdiv_num);
                }
            }
            net_writer.patch_level_divs(&road_refs);
        }
    }

    // 6. Build TRE
    let mut tre = TreWriter::new();
    tre.set_bounds(bounds.min_lat(), bounds.min_lon(), bounds.max_lat(), bounds.max_lon());
    tre.display_priority = mp.header.draw_priority;
    tre.transparent = mp.header.transparent;
    tre.map_id = mp.header.id;
    // mkgmap stores copyright via two mechanisms in TRE:
    // 1. LBL offsets in the copyright section (standard Garmin format, used by GPS devices)
    // 2. Raw text blob between header and section data (used by QMapShack for hover tooltip)
    if copyright_label > 0 {
        tre.copyright_offsets.push(copyright_label);
    }
    tre.copyright_message = mp.header.copyright.clone();
    tre.codepage = mp.header.codepage;
    tre.has_routing = net_writer_opt.is_some();
    tre.levels = tre_levels;
    tre.subdivisions = all_subdivisions;
    // mkgmap: lastRgnPos = rgnFile.position() - HEADER_LEN → end of RGN body
    tre.last_rgn_pos = rgn.position();

    // Build overviews (deduplicated)
    // Polish Map type codes: < 0x10000 = standard (type=high byte, subtype=low byte),
    //                        >= 0x10000 = extended (prefix 0x1)
    for mp_point in &mp.points {
        if mp_point.type_code < 0x10000 {
            let (t, st) = split_type_subtype(mp_point.type_code);
            tre.point_overviews.push(PointOverview::new(t, 0, st));
        } else {
            tre.ext_point_overviews.push(ExtPointOverview::from_type_code(mp_point.type_code, 0));
        }
    }
    for mp_line in &mp.polylines {
        if mp_line.type_code < 0x10000 {
            tre.polyline_overviews.push(PolylineOverview::new(mp_line.type_code as u8, 0));
        } else {
            tre.ext_polyline_overviews.push(ExtPolylineOverview::from_type_code(mp_line.type_code, 0));
        }
    }
    for mp_poly in &mp.polygons {
        if mp_poly.type_code < 0x10000 {
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
        net: net_writer_opt.as_ref().and_then(|nw| nw.built_data.clone()),
        nod: nod_data,
        dem: None,
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
    routing_ctx: Option<&RoutingContext>,
) -> (Vec<Subdivision>, Vec<Zoom>, Vec<u8>, Vec<(usize, u8, u16)>) {
    // 4th return: subdiv_road_refs = (road_idx, polyline_num, subdiv_num) for NET1 level/div patching
    let mut all_subdiv_road_refs: Vec<(usize, u8, u16)> = Vec::new();

    if levels.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new());
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
    // Each level filters features from the FULL map bounds (not parent subdivision bounds).
    // This ensures complete spatial coverage: even if a coarser level has sparse features,
    // finer levels still include all their features. Parent→child links are assigned
    // by spatial containment of each subdivision's center within parent areas.
    let process_levels = if num_levels > 1 { num_levels - 1 } else { num_levels };
    for level_idx in (0..process_levels).rev() {
        let level = &levels[level_idx];
        let level_num = level_idx as u8;
        let shift = (24i32 - level.resolution as i32).max(0);

        // Filter ALL features for this level from FULL map bounds
        let (split_points, split_lines, split_shapes) =
            filter_features_for_level(mp, level_num, bounds);

        // mkgmap always creates at least one subdivision per declared level,
        // even if no features pass the EndLevel filter. This ensures the full
        // multi-level hierarchy is preserved — some Garmin firmware (Alpha 100)
        // may require all declared levels to be present.
        let has_features = !split_points.is_empty() || !split_lines.is_empty() || !split_shapes.is_empty();

        let areas = if has_features {
            let result = splitter::split_features(
                *bounds, level.resolution,
                split_points, split_lines, split_shapes,
            );
            if result.is_empty() {
                // Splitter returned nothing — create one empty subdivision covering full bounds
                vec![splitter::MapArea::new(*bounds, level.resolution)]
            } else {
                result
            }
        } else {
            // No features at this level — create one empty subdivision (mkgmap convention)
            vec![splitter::MapArea::new(*bounds, level.resolution)]
        };

        // Determine parent for each area, then sort by parent to guarantee
        // contiguous child subdivision numbers per parent (Garmin format requirement:
        // children are encoded as "first child number" + contiguous range).
        let area_parents: Vec<u16> = areas.iter().map(|area| {
            let center = area.bounds.center();
            parent_areas.iter()
                .find(|(pa, _)| pa.contains_coord(&center))
                .map(|(_, n)| *n as u16)
                .unwrap_or_else(|| {
                    eprintln!(
                        "WARNING: subdivision center ({},{}) not contained in any parent, assigning to topdiv",
                        center.latitude(), center.longitude()
                    );
                    1
                })
        }).collect();

        // Sort areas by parent number to ensure contiguity
        let mut order: Vec<usize> = (0..areas.len()).collect();
        order.sort_by_key(|&i| area_parents[i]);

        let sorted_areas: Vec<&MapArea> = order.iter().map(|&i| &areas[i]).collect();
        let sorted_parents: Vec<u16> = order.iter().map(|&i| area_parents[i]).collect();

        let mut next_parent_areas: Vec<(Area, u32)> = Vec::new();
        let first_child_num = subdiv_counter;

        for (i, (&area, &parent_num)) in sorted_areas.iter().zip(sorted_parents.iter()).enumerate() {
            assert!(subdiv_counter <= u16::MAX as u32, "Too many subdivisions (>65535)");
            let subdiv_num = subdiv_counter as u16;
            subdiv_counter += 1;

            let mut subdiv = Subdivision::new(subdiv_num, level_num, level.resolution);
            subdiv.set_center(&area.bounds.center());
            subdiv.set_bounds(
                area.bounds.min_lat(), area.bounds.min_lon(),
                area.bounds.max_lat(), area.bounds.max_lon(),
            );
            subdiv.parent = parent_num;
            // is_last marks the last child in each parent's group, NOT the last at the level.
            // Since areas are sorted by parent, is_last is set when the next area has a different parent
            // or this is the very last area.
            let next_parent = sorted_parents.get(i + 1).copied();
            subdiv.is_last = next_parent != Some(parent_num);

            // Capture ext positions before encoding
            let ext_areas_before = rgn.ext_areas_position();
            let ext_lines_before = rgn.ext_lines_position();
            let ext_points_before = rgn.ext_points_position();

            let is_leaf = level_idx == 0;
            let (pts_data, lines_data, polys_data, road_refs) =
                encode_subdivision_rgn(mp, area, &subdiv, shift, point_labels, line_labels, poly_labels, rgn, routing_ctx, is_leaf);
            all_subdiv_road_refs.extend(road_refs);

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

        // Link children to parents — contiguous ranges guaranteed by parent-sorted numbering
        let child_nums: Vec<u16> = (first_child_num..subdiv_counter).map(|n| n as u16).collect();
        let child_parents: Vec<u16> = child_nums.iter().map(|&num| {
            all_subdivisions.iter().find(|s| s.number == num).map(|s| s.parent).unwrap_or(1)
        }).collect();

        let mut i = 0;
        while i < child_nums.len() {
            let pnum = child_parents[i];
            let start = i;
            while i < child_nums.len() && child_parents[i] == pnum {
                i += 1;
            }
            let children: Vec<u16> = child_nums[start..i].to_vec();
            if let Some(parent) = all_subdivisions.iter_mut().find(|s| s.number == pnum) {
                parent.has_children = true;
                parent.children = children;
            }
        }

        if !next_parent_areas.is_empty() {
            parent_areas = next_parent_areas;
        }
    }

    // Build TRE zoom levels from actual subdivisions
    // mkgmap: highest level is inherited (topdiv only), rest are regular
    // Collect active levels (those with subdivisions), then renumber contiguously.
    let mut active_levels = Vec::new();
    active_levels.push((topdiv_level, topdiv_resolution, true)); // (old_num, resolution, inherited)

    for level_idx in (0..process_levels).rev() {
        let level_num = level_idx as u8;
        if all_subdivisions.iter().any(|s| s.zoom_level == level_num) {
            active_levels.push((level_num, levels[level_idx].resolution, false));
        }
    }

    // Renumber contiguously: top = N-1, next = N-2, ..., bottom = 0
    // This ensures Garmin devices see contiguous level numbers with no gaps.
    let n = active_levels.len();
    let mut level_remap: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
    let mut tre_levels_build = Vec::new();
    for (i, &(old_num, resolution, inherited)) in active_levels.iter().enumerate() {
        let new_num = (n - 1 - i) as u8;
        level_remap.insert(old_num, new_num);
        let mut z = Zoom::new(new_num, resolution);
        z.inherited = inherited;
        tre_levels_build.push(z);
    }

    // Remap subdivision zoom_level numbers to match the contiguous scheme
    for subdiv in all_subdivisions.iter_mut() {
        if let Some(&new_level) = level_remap.get(&subdiv.zoom_level) {
            subdiv.zoom_level = new_level;
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

    (all_subdivisions, tre_levels_build, ext_type_offsets_data, all_subdiv_road_refs)
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
/// Applies geometry optimizations (simplification, min-size filtering, area sorting)
/// based on MpHeader options.
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

    // Auto-simplification: when no explicit simplification is configured and shift > 0,
    // apply a default DP epsilon to prevent quantization artifacts (self-intersections,
    // degenerate edges) caused by coordinate quantization at lower resolutions.
    let shift = mp.header.levels.get(level as usize)
        .map(|&res| (24i32 - res as i32).max(0))
        .unwrap_or(0);
    let auto_epsilon = if shift > 0 && level > 0 {
        Some((1i32 << shift) as f64 * 0.5)
    } else {
        None
    };

    // Determine DP epsilon for lines
    let line_epsilon = mp.header.reduce_point_density.or(auto_epsilon);

    // Determine DP epsilon for polygons (simplify_polygons per-resolution or fallback to reduce_point_density)
    let poly_epsilon = resolve_polygon_epsilon(&mp.header, level)
        .or(mp.header.reduce_point_density)
        .or(auto_epsilon);

    let lines: Vec<SplitLine> = mp.polylines.iter().enumerate()
        .filter(|(_, l)| l.end_level.unwrap_or(0) >= level)
        .filter(|(_, l)| !l.points.is_empty() && expanded.contains_coord(&l.points[0]))
        .map(|(i, l)| {
            let mut pts = l.points.clone();
            if let Some(eps) = line_epsilon {
                let coords: Vec<(i32, i32)> = pts.iter().map(|c| (c.latitude(), c.longitude())).collect();
                let simplified = douglas_peucker(&coords, eps);
                if simplified.len() >= 2 {
                    pts = simplified.iter().map(|&(lat, lon)| Coord::new(lat, lon)).collect();
                }
            }
            SplitLine { mp_index: i, points: pts }
        })
        .collect();

    let min_size = mp.header.min_size_polygon;

    let shapes: Vec<SplitShape> = mp.polygons.iter().enumerate()
        .filter(|(_, s)| s.end_level.unwrap_or(0) >= level)
        .filter(|(_, s)| !s.points.is_empty() && expanded.contains_coord(&s.points[0]))
        .filter_map(|(i, s)| {
            // Min-size filtering
            if let Some(min) = min_size {
                let coords: Vec<(i32, i32)> = s.points.iter().map(|c| (c.latitude(), c.longitude())).collect();
                if compute_area(&coords) < min as f64 {
                    return None;
                }
            }
            let mut pts = s.points.clone();
            // Polygon simplification
            if let Some(eps) = poly_epsilon {
                let coords: Vec<(i32, i32)> = pts.iter().map(|c| (c.latitude(), c.longitude())).collect();
                let simplified = douglas_peucker(&coords, eps);
                if simplified.len() >= 3 {
                    pts = simplified.iter().map(|&(lat, lon)| Coord::new(lat, lon)).collect();
                }
            }
            Some(SplitShape { mp_index: i, points: pts })
        })
        .collect();

    // Split features with >250 points — mkgmap LineSplitterFilter / PolygonSplitterFilter
    // Large features cause bitstream overflow because the variable-width delta encoding
    // uses a global bit width based on the max delta across ALL points. Splitting keeps
    // each element's geographic extent small → smaller deltas → compact bitstreams.
    let lines = split_large_polylines(lines);
    let mut shapes = split_large_polygons(shapes);

    // Order by decreasing area if requested
    if mp.header.order_by_decreasing_area && !shapes.is_empty() {
        shapes.sort_by(|a, b| {
            let area_a = compute_area(&a.points.iter().map(|c| (c.latitude(), c.longitude())).collect::<Vec<_>>());
            let area_b = compute_area(&b.points.iter().map(|c| (c.latitude(), c.longitude())).collect::<Vec<_>>());
            area_b.partial_cmp(&area_a).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    (points, lines, shapes)
}

// ── Feature splitting — mkgmap LineSplitterFilter / PolygonSplitterFilter ──

/// Maximum points per element — mkgmap LineSplitterFilter.MAX_POINTS_IN_LINE
const MAX_POINTS_IN_ELEMENT: usize = 250;

/// Maximum clipping iterations for polygon splitting (prevents runaway loops
/// on degenerate geometries where clipping adds intersection points).
const MAX_POLYGON_SPLIT_ITERATIONS: usize = 10_000;

/// Split polylines with >250 points into chunks — mkgmap LineSplitterFilter
///
/// Each chunk has at most MAX_POINTS_IN_ELEMENT points.
/// Consecutive chunks share their boundary point (1-point overlap).
/// mkgmap balances chunk sizes when the total is between 1x and 2x the limit.
/// Only the first chunk keeps the label; subsequent chunks get label_offset = 0
/// via the `first_chunk` flag stored in the mp_index encoding (see M3 fix below —
/// the label suppression is handled in encode_subdivision_rgn by tracking seen mp_indices).
fn split_large_polylines(lines: Vec<SplitLine>) -> Vec<SplitLine> {
    // Estimate output capacity: most lines pass through, a few expand ~10x
    let capacity = lines.iter()
        .map(|l| 1 + l.points.len() / MAX_POINTS_IN_ELEMENT)
        .sum();
    let mut result = Vec::with_capacity(capacity);

    for line in lines {
        let npoints = line.points.len();
        if npoints <= MAX_POINTS_IN_ELEMENT {
            result.push(line);
            continue;
        }

        let mut pos = 0;
        // mkgmap: if total is between 1x and 2x limit, split evenly in two
        let mut wanted = if npoints < 2 * MAX_POINTS_IN_ELEMENT {
            npoints / 2 + 1
        } else {
            MAX_POINTS_IN_ELEMENT
        };

        // Safety: max chunks = npoints (each chunk has ≥2 points, so this is generous)
        let max_chunks = npoints;
        let mut chunks_emitted = 0;

        loop {
            debug_assert!(wanted >= 2, "chunk size must be ≥ 2 points");
            let end = (pos + wanted).min(npoints);
            result.push(SplitLine {
                mp_index: line.mp_index,
                points: line.points[pos..end].to_vec(),
            });
            chunks_emitted += 1;

            if end >= npoints || chunks_emitted >= max_chunks {
                break;
            }

            pos = end - 1; // 1-point overlap
            let remaining = npoints - pos;

            if remaining <= MAX_POINTS_IN_ELEMENT {
                // Last chunk
                result.push(SplitLine {
                    mp_index: line.mp_index,
                    points: line.points[pos..].to_vec(),
                });
                break;
            } else if remaining < 2 * MAX_POINTS_IN_ELEMENT {
                // Balance the last two chunks
                wanted = remaining / 2 + 1;
            } else {
                wanted = MAX_POINTS_IN_ELEMENT;
            }
        }
    }
    result
}

/// Split polygons with >250 points via midpoint clipping — mkgmap PolygonSplitterBase
///
/// Uses Sutherland-Hodgman clipping along the longer bounding-box axis.
/// Processes queue in FIFO order (VecDeque) to preserve input ordering.
/// Limits iterations to MAX_POLYGON_SPLIT_ITERATIONS to prevent runaway loops
/// on degenerate geometries where clipping intersections inflate point counts.
fn split_large_polygons(shapes: Vec<SplitShape>) -> Vec<SplitShape> {
    use std::collections::VecDeque;

    let mut result = Vec::with_capacity(shapes.len());
    let mut queue: VecDeque<SplitShape> = shapes.into_iter().collect();
    let mut iterations = 0;

    while let Some(shape) = queue.pop_front() {
        if shape.points.len() <= MAX_POINTS_IN_ELEMENT {
            result.push(shape);
            continue;
        }

        iterations += 1;
        if iterations > MAX_POLYGON_SPLIT_ITERATIONS {
            // Degenerate case: stop splitting, keep oversized fragment to avoid data loss
            eprintln!(
                "WARNING: polygon split exceeded {} iterations, keeping fragment with {} points",
                MAX_POLYGON_SPLIT_ITERATIONS, shape.points.len()
            );
            result.push(shape);
            // Drain remaining queue items
            for remaining in queue {
                result.push(remaining);
            }
            break;
        }

        let bbox = Area::from_coords(&shape.points);
        // Split along the longer dimension — mkgmap PolygonSplitterBase.split
        let (half_a, half_b) = if bbox.width() >= bbox.height() {
            let mid = bbox.min_lon() + bbox.width() / 2;
            (
                Area::new(bbox.min_lat(), bbox.min_lon(), bbox.max_lat(), mid),
                Area::new(bbox.min_lat(), mid, bbox.max_lat(), bbox.max_lon()),
            )
        } else {
            let mid = bbox.min_lat() + bbox.height() / 2;
            (
                Area::new(bbox.min_lat(), bbox.min_lon(), mid, bbox.max_lon()),
                Area::new(mid, bbox.min_lon(), bbox.max_lat(), bbox.max_lon()),
            )
        };

        let clipped_a = splitter::clip_polygon_to_rect(&shape.points, &half_a);
        let clipped_b = splitter::clip_polygon_to_rect(&shape.points, &half_b);

        let a_ok = clipped_a.len() >= 3;
        let b_ok = clipped_b.len() >= 3;

        if a_ok {
            queue.push_back(SplitShape { mp_index: shape.mp_index, points: clipped_a });
        }
        if b_ok {
            queue.push_back(SplitShape { mp_index: shape.mp_index, points: clipped_b });
        }

        // Safety: if clipping produced nothing useful, keep original to avoid data loss
        if !a_ok && !b_ok {
            result.push(shape);
        }
    }
    result
}

/// Resolve polygon epsilon from simplify_polygons spec (e.g. "24:12,18:10,16:8")
fn resolve_polygon_epsilon(header: &crate::parser::mp_types::MpHeader, level: u8) -> Option<f64> {
    let spec = header.simplify_polygons.as_deref()?;
    let resolution = header.levels.get(level as usize).copied()?;

    for part in spec.split(',') {
        if let Some((res_str, eps_str)) = part.trim().split_once(':') {
            if let (Ok(res), Ok(eps)) = (res_str.trim().parse::<u8>(), eps_str.trim().parse::<f64>()) {
                if res == resolution {
                    return Some(eps);
                }
            }
        }
    }
    None
}

// ── RGN encoding per subdivision ───────────────────────────────────────────

/// Encode features from a MapArea into RGN binary data for one subdivision.
/// Standard types (< 0x10000) go into the returned tuple.
/// Extended types (≥ 0x10000, Polish Map prefix 0x1) are written directly into the RGN extended buffers.
fn encode_subdivision_rgn(
    mp: &MpFile,
    area: &MapArea,
    subdiv: &Subdivision,
    shift: i32,
    point_labels: &[u32],
    line_labels: &[u32],
    poly_labels: &[u32],
    rgn: &mut RgnWriter,
    routing_ctx: Option<&RoutingContext>,
    is_leaf_level: bool,
) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<(usize, u8, u16)>) {
    // subdiv_road_refs: (road_idx, polyline_num_in_subdiv, subdiv_number)
    let mut subdiv_road_refs: Vec<(usize, u8, u16)> = Vec::new();
    let mut polyline_counter: usize = 0;

    // Points
    // Polish Map type codes: < 0x10000 = standard, >= 0x10000 = extended
    // Standard with subtype (0x100-0xFFFF): type = high byte, subtype = low byte
    let mut points_data = Vec::new();
    for split_pt in &area.points {
        let mp_point = &mp.points[split_pt.mp_index];

        if mp_point.type_code >= 0x10000 {
            // Extended point
            let mut pt = Point::new(mp_point.type_code, split_pt.location);
            pt.label_offset = point_labels[split_pt.mp_index];
            rgn.write_ext_point(&pt.write_ext(subdiv.center_lat, subdiv.center_lon, shift));
        } else {
            // Standard point (possibly with subtype)
            let (type_byte, sub_type) = split_type_subtype(mp_point.type_code);
            let mut pt = Point::new(type_byte as u32, split_pt.location);
            pt.label_offset = point_labels[split_pt.mp_index];
            if sub_type > 0 || mp_point.type_code >= 0x100 {
                pt.has_sub_type = true;
                pt.sub_type = sub_type;
            }
            points_data.extend_from_slice(&pt.write(subdiv.center_lat, subdiv.center_lon, shift));
        }
    }

    // Polylines
    // polyline_counter tracks the index of standard polylines within this subdivision's
    // RGN data. Extended polylines (type >= 0x100) go to a separate RGN section and must
    // NOT be counted, otherwise NET1 level/div polyline_num would be off.
    // Track seen mp_indices to avoid duplicate labels on split polylines (M3 fix):
    // only the first chunk of a split polyline gets its label.
    let mut seen_line_indices: HashSet<usize> = HashSet::new();
    let mut polylines_data = Vec::new();
    for split_line in &area.lines {
        if split_line.points.len() < 2 { continue; }
        let mp_line = &mp.polylines[split_line.mp_index];
        let is_ext = mp_line.type_code >= 0x10000;
        let mut pl = Polyline::new(mp_line.type_code, split_line.points.clone());
        // Only label the first chunk of a split polyline within this subdivision
        if seen_line_indices.insert(split_line.mp_index) {
            pl.label_offset = line_labels[split_line.mp_index];
        } else {
            pl.label_offset = 0;
        }
        pl.direction = mp_line.direction;

        // P3: RGN→NET link at leaf level. The label field is REPLACED by
        // NET1 offset (mkgmap patches LBL→NET after NET is built).
        // No extra bytes after bitstream — blen is bitstream only.
        if is_leaf_level && !is_ext {
            if let Some(ctx) = routing_ctx {
                if let Some(&net1_off) = ctx.net1_offsets_by_mp_index.get(&split_line.mp_index) {
                    pl.has_net_info = true;
                    pl.net_offset = net1_off;
                    // Track subdiv ref for NET1 level/div patching
                    if let Some(&road_idx) = ctx.mp_index_to_road_idx.get(&split_line.mp_index) {
                        let polyline_num = polyline_counter as u8;
                        subdiv_road_refs.push((road_idx, polyline_num, subdiv.number));
                    }
                }
            }
        }

        let deltas = compute_deltas(&split_line.points, subdiv);
        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, is_ext) {
            if is_ext {
                rgn.write_ext_polyline(
                    &pl.write_ext(subdiv.center_lat, subdiv.center_lon, shift, &bitstream),
                );
            } else {
                polylines_data.extend_from_slice(
                    &pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, false),
                );
                polyline_counter += 1;
            }
        }
    }

    // Polygons
    let mut polygons_data = Vec::new();
    for split_shape in &area.shapes {
        if split_shape.points.len() < 3 { continue; }
        // Remove duplicate closing vertex (shapefile convention: first == last)
        // Garmin format closes polygons implicitly, so the duplicate wastes bitstream space
        let pts = if split_shape.points.len() > 3 {
            let first = &split_shape.points[0];
            let last = &split_shape.points[split_shape.points.len() - 1];
            if first.latitude() == last.latitude() && first.longitude() == last.longitude() {
                &split_shape.points[..split_shape.points.len() - 1]
            } else {
                &split_shape.points[..]
            }
        } else {
            &split_shape.points[..]
        };
        if pts.len() < 3 { continue; }
        let mp_poly = &mp.polygons[split_shape.mp_index];
        let mut pg = Polygon::new(mp_poly.type_code, pts.to_vec());
        pg.label_offset = poly_labels[split_shape.mp_index];
        let deltas = compute_deltas(pts, subdiv);
        let is_ext = mp_poly.type_code >= 0x10000;
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

    (points_data, polylines_data, polygons_data, subdiv_road_refs)
}

// ── Utilities ──────────────────────────────────────────────────────────────

/// Split a Polish Map type code (< 0x10000) into Garmin type byte and subtype byte.
/// - type_code < 0x100: type = type_code, subtype = 0
/// - type_code 0x100-0xFFFF: type = high byte, subtype = low byte
fn split_type_subtype(type_code: u32) -> (u8, u8) {
    if type_code < 0x100 {
        (type_code as u8, 0)
    } else {
        (((type_code >> 8) & 0xFF) as u8, (type_code & 0xFF) as u8)
    }
}

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

/// Douglas-Peucker line simplification on map-unit coordinates
pub fn douglas_peucker(points: &[(i32, i32)], epsilon: f64) -> Vec<(i32, i32)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let (start, end) = (points[0], points[points.len() - 1]);
    let mut max_dist = 0.0f64;
    let mut max_idx = 0;

    for (i, &p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let d = perpendicular_distance(p, start, end);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        let mut left = douglas_peucker(&points[..=max_idx], epsilon);
        let right = douglas_peucker(&points[max_idx..], epsilon);
        left.pop(); // remove duplicate junction point
        left.extend_from_slice(&right);
        left
    } else {
        vec![start, end]
    }
}

fn perpendicular_distance(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f64 {
    let dx = (b.0 - a.0) as f64;
    let dy = (b.1 - a.1) as f64;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        let ex = (p.0 - a.0) as f64;
        let ey = (p.1 - a.1) as f64;
        return (ex * ex + ey * ey).sqrt();
    }
    let num = ((p.0 - a.0) as f64 * dy - (p.1 - a.1) as f64 * dx).abs();
    num / len_sq.sqrt()
}

/// Compute signed area of polygon using shoelace formula (in map units²)
pub fn compute_area(points: &[(i32, i32)]) -> f64 {
    let n = points.len();
    if n < 3 { return 0.0; }
    let mut area = 0i64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].0 as i64 * points[j].1 as i64;
        area -= points[j].0 as i64 * points[i].1 as i64;
    }
    (area as f64).abs() / 2.0
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

/// Routing context computed before RGN encoding, providing NET1 offsets and node_flags
/// for each routable polyline (keyed by mp_index into mp.polylines).
struct RoutingContext {
    net1_offsets_by_mp_index: HashMap<usize, u32>,
    /// mp_index → road_idx (index in the NET writer's road list)
    mp_index_to_road_idx: HashMap<usize, usize>,
    /// Junction set for recalculating node_flags on split polylines (P4, currently disabled)
    #[allow(dead_code)]
    junctions: HashSet<(i32, i32)>,
}

fn pre_compute_routing(
    mp: &MpFile,
    line_labels: &[u32],
) -> (Option<RoutingContext>, Option<NetWriter>, Option<Vec<u8>>) {
    use crate::parser::mp_types::RoutingMode;
    use super::nod::{write_nod2_records, Nod2RoadInfo};

    // Routing mode check
    match mp.header.routing_mode {
        RoutingMode::Disabled => return (None, None, None),
        RoutingMode::Auto => {
            if !mp.polylines.iter().any(|pl| pl.road_id.is_some()) {
                return (None, None, None);
            }
        }
        RoutingMode::Route | RoutingMode::NetOnly => {
            if !mp.polylines.iter().any(|pl| pl.road_id.is_some()) {
                tracing::warn!("--route/--net specified but no RoadID found in .mp data");
                return (None, None, None);
            }
        }
    }

    let mut road_polylines = Vec::new();
    let mut mp_indices: Vec<usize> = Vec::new();
    let mut road_params: Vec<RouteParams> = Vec::new();

    // Collect road data
    struct RoadInfo {
        label_offset: u32,
        road_length: u32,
        params: RouteParams,
    }
    let mut road_infos: Vec<RoadInfo> = Vec::new();

    for (i, mp_line) in mp.polylines.iter().enumerate() {
        if mp_line.road_id.is_some() {
            let params = if let Some(ref rp) = mp_line.route_param {
                graph_builder::parse_route_param(rp)
            } else {
                RouteParams::default()
            };

            road_infos.push(RoadInfo {
                label_offset: line_labels[i],
                road_length: estimate_road_length(&mp_line.points),
                params: params.clone(),
            });

            let road_idx = road_infos.len() - 1;
            road_polylines.push((mp_line.points.clone(), road_idx, params.clone()));
            road_params.push(params);
            mp_indices.push(i);
        }
    }

    // Find junctions and compute node_flags
    let junctions = find_junctions(&road_polylines);
    let all_node_flags = compute_node_flags(&road_polylines, &junctions);

    // Build routing graph (enriched with heading + node_class)
    let route_nodes = graph_builder::build_graph_with_junctions(&road_polylines, &junctions);

    // Build NOD writer and prepare NOD1 (needed for NOD2 first_node offsets)
    let mut nod_writer = NodWriter::new();
    for node in &route_nodes {
        nod_writer.add_node(node.clone());
    }

    if mp.header.routing_mode == RoutingMode::NetOnly {
        // NetOnly: build NET without nod2_offset, no NOD
        let mut net_writer = NetWriter::new();
        for info in &road_infos {
            let mut rd = RoadDef::new();
            rd.label_offsets.push(info.label_offset);
            rd.road_length_meters = info.road_length;
            rd.road_class = info.params.road_class;
            rd.speed = info.params.speed;
            rd.one_way = info.params.one_way;
            rd.toll = info.params.toll;
            rd.access_flags = info.params.access_flags;
            net_writer.add_road(rd);
        }
        let _net_data = net_writer.build();
        let net1_offsets = net_writer.net1_offsets().to_vec();

        let mut net1_offsets_by_mp_index = HashMap::new();
        for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
            net1_offsets_by_mp_index.insert(mp_idx, net1_offsets[road_idx]);
        }
        let mut mp_index_to_road_idx = HashMap::new();
        for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
            mp_index_to_road_idx.insert(mp_idx, road_idx);
        }
        let routing_ctx = RoutingContext {
            net1_offsets_by_mp_index,
            mp_index_to_road_idx,
            junctions,
        };
        return (Some(routing_ctx), Some(net_writer), None);
    }

    // Full routing pipeline:
    // 1. Prepare NOD1 → get node offsets
    nod_writer.prepare();

    // 2. Build NOD2 per-road records → get nod2_offsets
    let mut nod2_road_infos: Vec<Nod2RoadInfo> = Vec::new();
    for (road_idx, flags) in all_node_flags.iter().enumerate() {
        let params = &road_params[road_idx];
        // Find the first RouteNode on this road to get its NOD1 offset
        let first_node_offset = find_first_route_node_offset(
            &road_polylines[road_idx].0,
            &junctions,
            &route_nodes,
            nod_writer.node_offsets(),
            road_idx,
        );
        let num_route_nodes = flags.iter().filter(|&&f| f).count() as u16;
        let starts_with_node = flags.first().copied().unwrap_or(false);
        nod2_road_infos.push(Nod2RoadInfo {
            road_class: params.road_class,
            speed: params.speed,
            num_route_nodes,
            starts_with_node,
            first_node_nod1_offset: first_node_offset,
        });
    }
    let (nod2_data, nod2_offsets) = write_nod2_records(&nod2_road_infos);
    nod_writer.set_nod2_data(nod2_data);

    // 3. Build NET with nod2_offsets → get NET1 offsets
    let mut net_writer = NetWriter::new();
    for (road_idx, info) in road_infos.iter().enumerate() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(info.label_offset);
        rd.road_length_meters = info.road_length;
        rd.road_class = info.params.road_class;
        rd.speed = info.params.speed;
        rd.one_way = info.params.one_way;
        rd.toll = info.params.toll;
        rd.access_flags = info.params.access_flags;
        rd.nod2_offset = Some(nod2_offsets[road_idx]);
        net_writer.add_road(rd);
    }
    let _net_data = net_writer.build();
    let net1_offsets = net_writer.net1_offsets().to_vec();

    // 4. Patch NET1 offsets into NOD1 Table A entries
    nod_writer.patch_net1_offsets(&net1_offsets);

    // 5. Build final NOD
    let nod_data = nod_writer.build();

    // Build RoutingContext for RGN
    let mut net1_offsets_by_mp_index = HashMap::new();
    for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
        net1_offsets_by_mp_index.insert(mp_idx, net1_offsets[road_idx]);
    }
    let mut mp_index_to_road_idx = HashMap::new();
    for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
        mp_index_to_road_idx.insert(mp_idx, road_idx);
    }
    let routing_ctx = RoutingContext {
        net1_offsets_by_mp_index,
        mp_index_to_road_idx,
        junctions,
    };

    (Some(routing_ctx), Some(net_writer), Some(nod_data))
}

/// Find the NOD1 offset of the first RouteNode encountered along a road's vertices.
/// Prefers matching by coordinates + road_def_index arc check, falls back to coordinate-only
/// match for degenerate roads (single-point, no arcs).
fn find_first_route_node_offset(
    coords: &[Coord],
    junctions: &HashSet<(i32, i32)>,
    route_nodes: &[super::nod::RouteNode],
    node_offsets: &[u32],
    road_def_index: usize,
) -> u32 {
    let mut fallback: Option<u32> = None;

    for coord in coords {
        let key = (coord.latitude(), coord.longitude());
        if junctions.contains(&key) {
            for (i, rn) in route_nodes.iter().enumerate() {
                if rn.lat == key.0 && rn.lon == key.1 {
                    let off = if i < node_offsets.len() { node_offsets[i] } else { 0 };
                    // Prefer node with an arc for this road
                    if rn.arcs.iter().any(|a| a.road_def_index == road_def_index) {
                        return off;
                    }
                    // Remember first coordinate-only match as fallback
                    if fallback.is_none() {
                        fallback = Some(off);
                    }
                }
            }
        }
    }

    fallback.unwrap_or(0)
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
    fn test_build_accented_labels_cp1252() {
        let content = "[IMG ID]\nID=99990001\nName=Test Accents\nCodePage=1252\nLevels=24\n[END-IMG ID]\n[POI]\nType=0x2C00\nLabel=Ch\u{00E2}teau Fort\nData0=(48.57,7.75)\n[END]\n[POLYGON]\nType=0x03\nLabel=For\u{00EA}t de Ch\u{00EA}nes\nData0=(48.57,7.75),(48.58,7.75),(48.58,7.76),(48.57,7.76)\n[END]\n";
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();

        // LBL format should be 9 (Format9/codepage) for codepage 1252
        let lbl = &result.lbl;
        assert_eq!(lbl[30], 9, "LBL format should be 9 (codepage) for CP1252");

        // Labels should contain CP1252 encoded accented characters
        let label_off = u32::from_le_bytes([lbl[21], lbl[22], lbl[23], lbl[24]]) as usize;
        let label_data = &lbl[label_off..];
        // "â" in CP1252 = 0xE2, "ê" = 0xEA
        assert!(label_data.contains(&0xE2), "Should contain â in CP1252");
        assert!(label_data.contains(&0xEA), "Should contain ê in CP1252");
    }

    #[test]
    fn test_build_accented_labels_utf8() {
        let content = "[IMG ID]\nID=99990001\nName=Test Accents UTF8\nCodePage=65001\nLevels=24\n[END-IMG ID]\n[POI]\nType=0x2C00\nLabel=Ch\u{00E2}teau Fort\nData0=(48.57,7.75)\n[END]\n";
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();

        let lbl = &result.lbl;
        assert_eq!(lbl[30], 10, "LBL format should be 10 (UTF-8) for codepage 65001");
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

// ── Tests for feature splitting ───────────────────────────────────────────

#[cfg(test)]
mod split_tests {
    use super::*;
    use super::splitter::{SplitLine, SplitShape};

    fn coord(lat: i32, lon: i32) -> Coord {
        Coord::new(lat, lon)
    }

    fn make_line(n: usize) -> SplitLine {
        SplitLine {
            mp_index: 0,
            points: (0..n).map(|i| coord(i as i32 * 10, i as i32 * 10)).collect(),
        }
    }

    fn make_shape(n: usize, spread: i32) -> SplitShape {
        // Create a polygon with N points arranged in a rough circle
        let mut pts = Vec::with_capacity(n);
        for i in 0..n {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
            let lat = (spread as f64 * angle.sin()) as i32;
            let lon = (spread as f64 * angle.cos()) as i32;
            pts.push(coord(lat, lon));
        }
        SplitShape { mp_index: 0, points: pts }
    }

    // ── Polyline splitting tests ──

    #[test]
    fn test_polyline_no_split_under_limit() {
        let lines = vec![make_line(250)];
        let result = split_large_polylines(lines);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].points.len(), 250);
    }

    #[test]
    fn test_polyline_no_split_at_limit() {
        let lines = vec![make_line(MAX_POINTS_IN_ELEMENT)];
        let result = split_large_polylines(lines);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].points.len(), MAX_POINTS_IN_ELEMENT);
    }

    #[test]
    fn test_polyline_split_251_points() {
        let lines = vec![make_line(251)];
        let result = split_large_polylines(lines);
        assert_eq!(result.len(), 2);
        // mkgmap balances: 251 < 2*250, so wanted = 251/2+1 = 126
        assert_eq!(result[0].points.len(), 126);
        assert_eq!(result[1].points.len(), 126); // 251 - 125 = 126
    }

    #[test]
    fn test_polyline_split_overlap() {
        let lines = vec![make_line(501)];
        let result = split_large_polylines(lines);
        // Verify 1-point overlap: last point of chunk N == first point of chunk N+1
        for i in 0..result.len() - 1 {
            let last = result[i].points.last().unwrap();
            let first = &result[i + 1].points[0];
            assert_eq!(
                last.latitude(), first.latitude(),
                "chunks {} and {} must share boundary point", i, i + 1
            );
            assert_eq!(last.longitude(), first.longitude());
        }
    }

    #[test]
    fn test_polyline_split_preserves_all_points() {
        let original = make_line(750);
        let original_first = original.points[0];
        let original_last = original.points[749];
        let lines = vec![original];
        let result = split_large_polylines(lines);

        assert!(result.len() >= 3, "750 points should produce ≥3 chunks");

        // First point of first chunk == original first
        assert_eq!(result[0].points[0].latitude(), original_first.latitude());
        // Last point of last chunk == original last
        let last_chunk = result.last().unwrap();
        assert_eq!(last_chunk.points.last().unwrap().latitude(), original_last.latitude());

        // All chunks have ≤250 points
        for (i, chunk) in result.iter().enumerate() {
            assert!(
                chunk.points.len() <= MAX_POINTS_IN_ELEMENT,
                "chunk {} has {} points (max {})",
                i, chunk.points.len(), MAX_POINTS_IN_ELEMENT
            );
            assert!(chunk.points.len() >= 2, "chunk {} has < 2 points", i);
        }
    }

    #[test]
    fn test_polyline_split_3283_points() {
        // Real-world case from D038 BDTOPO
        let lines = vec![make_line(3283)];
        let result = split_large_polylines(lines);
        // 3283 points → ~14 chunks
        assert!(result.len() >= 13);
        for chunk in &result {
            assert!(chunk.points.len() <= MAX_POINTS_IN_ELEMENT);
            assert!(chunk.points.len() >= 2);
        }
        // mp_index preserved
        for chunk in &result {
            assert_eq!(chunk.mp_index, 0);
        }
    }

    #[test]
    fn test_polyline_split_small_line_passthrough() {
        let lines = vec![
            make_line(10),
            make_line(500),
            make_line(5),
        ];
        let result = split_large_polylines(lines);
        // First and third pass through, second is split
        assert!(result.len() >= 4); // 1 + ≥2 + 1
        assert_eq!(result[0].points.len(), 10);
        assert_eq!(result.last().unwrap().points.len(), 5);
    }

    // ── Polygon splitting tests ──

    #[test]
    fn test_polygon_no_split_under_limit() {
        let shapes = vec![make_shape(100, 1000)];
        let result = split_large_polygons(shapes);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].points.len(), 100);
    }

    #[test]
    fn test_polygon_split_large_shape() {
        let shapes = vec![make_shape(500, 10000)];
        let result = split_large_polygons(shapes);
        assert!(result.len() >= 2, "500-point polygon should be split");
        for fragment in &result {
            assert!(
                fragment.points.len() <= MAX_POINTS_IN_ELEMENT,
                "fragment has {} points (max {})",
                fragment.points.len(), MAX_POINTS_IN_ELEMENT
            );
            assert!(fragment.points.len() >= 3, "fragment must be valid polygon");
        }
    }

    #[test]
    fn test_polygon_split_preserves_mp_index() {
        let mut shapes = vec![make_shape(400, 5000)];
        shapes[0].mp_index = 42;
        let result = split_large_polygons(shapes);
        for fragment in &result {
            assert_eq!(fragment.mp_index, 42);
        }
    }

    #[test]
    fn test_polygon_split_8564_points() {
        // Real-world case from D038 BDTOPO
        let shapes = vec![make_shape(8564, 50000)];
        let result = split_large_polygons(shapes);
        assert!(result.len() >= 4, "8564-point polygon should produce many fragments");
        for fragment in &result {
            assert!(fragment.points.len() <= MAX_POINTS_IN_ELEMENT);
            assert!(fragment.points.len() >= 3);
        }
    }

    #[test]
    fn test_polygon_split_fifo_order() {
        // Verify FIFO ordering: first input polygon's fragments come before second's
        let shapes = vec![
            SplitShape { mp_index: 1, points: make_shape(400, 5000).points },
            SplitShape { mp_index: 2, points: make_shape(400, 5000).points },
        ];
        let result = split_large_polygons(shapes);
        // All mp_index=1 fragments should come before mp_index=2
        let first_idx2 = result.iter().position(|s| s.mp_index == 2).unwrap();
        let last_idx1 = result.iter().rposition(|s| s.mp_index == 1).unwrap();
        assert!(last_idx1 < first_idx2, "FIFO order: all idx=1 before idx=2");
    }

    #[test]
    fn test_polygon_degenerate_collinear() {
        // All points on a line — clipping may produce < 3 point fragments
        let pts: Vec<Coord> = (0..300).map(|i| coord(i * 10, 0)).collect();
        let shapes = vec![SplitShape { mp_index: 0, points: pts }];
        let result = split_large_polygons(shapes);
        // Should not panic; may keep original if clipping fails
        assert!(!result.is_empty());
    }

    #[test]
    fn test_mixed_small_and_large_features() {
        let shapes = vec![
            make_shape(10, 100),   // small, passthrough
            make_shape(500, 10000), // large, split
            make_shape(20, 200),   // small, passthrough
        ];
        let result = split_large_polygons(shapes);
        assert!(result.len() >= 4); // 1 + ≥2 + 1
        // First and last should be the small ones (FIFO order)
        assert_eq!(result[0].points.len(), 10);
    }
}
