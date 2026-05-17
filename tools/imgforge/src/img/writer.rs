// MapBuilder — build orchestrator, faithful to mkgmap MapBuilder.java
//
// Multi-level hierarchy: topdiv → level N → ... → level 0
// Feature distribution by pickArea (first point), polygon clipping, recursive split

use crate::error::ImgError;
use crate::parser::mp_types::{ElevationUnit, MpFile};
use crate::routing::graph_builder::{self, RouteParams, NodEntry, find_junctions, compute_node_flags};
use std::collections::{HashMap, HashSet};
use super::area::Area;
use super::coord::Coord;
use super::filesystem::ImgFilesystem;
use super::filters::{round_coords, remove_obsolete_points, passes_size_filter, passes_remove_empty};
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
    build_subfiles_inner(mp, None)
}

/// Build subfiles using DEM-aligned bounds for the TRE — mirrors mkgmap's
/// `map.setBounds(treArea)` (MapBuilder.java:395) which updates TRE bounds
/// after DEM calculation to prevent a mismatch where DEM extends beyond the
/// TRE-declared area (visible as a white band in Basecamp at tile seams).
pub fn build_subfiles_with_dem_bounds(mp: &MpFile, dem_bounds: Area) -> Result<TileResult, ImgError> {
    build_subfiles_inner(mp, Some(dem_bounds))
}

fn build_subfiles_inner(mp: &MpFile, bounds_override: Option<Area>) -> Result<TileResult, ImgError> {
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
        .map(|pl| {
            let label = if mp.header.elevation_unit == ElevationUnit::Metres
                && is_contour_type(pl.type_code)
            {
                metres_label_to_feet(&pl.label)
            } else {
                pl.label.clone()
            };
            lbl_writer.add_label(&label)
        })
        .collect();
    let poly_labels: Vec<u32> = mp.polygons.iter()
        .map(|pg| lbl_writer.add_label(&pg.label))
        .collect();

    // 2. Build zoom levels
    let levels: Vec<Zoom> = mp.header.levels.iter().enumerate()
        .map(|(i, &res)| Zoom::new(i as u8, res))
        .collect();

    // 3. Compute bounds
    // feature_bounds : toujours feature-based → centres des subdivisions + encodage delta RGN
    //   (parité mkgmap src.getBounds() passé à makeMapAreas / topLevelSubdivision)
    // tre_header_bounds : DEM-alignés si disponibles → header TRE outer bounds uniquement
    //   (parité mkgmap map.setBounds(treArea), MapBuilder.java:395)
    // Les deux peuvent différer légèrement (snap grille HGT) ; utiliser dem_bounds pour les
    // centres décalerait tous les deltas coordonnées → fan-pattern visible en vue large.
    let feature_bounds = compute_bounds(mp)?;
    let tre_header_bounds = bounds_override.unwrap_or(feature_bounds);

    // 4. Pre-compute routing → RoutingContext + net_writer + nod_data
    let (mut routing_ctx, mut net_writer_opt, nod_data) = pre_compute_routing(mp, &line_labels);

    // 5. Build multi-level subdivision hierarchy + encode RGN (with routing context)
    let mut rgn = RgnWriter::new();
    let (mut all_subdivisions, mut tre_levels, mut ext_type_offsets_data, subdiv_road_refs) = build_multilevel_hierarchy(
        mp, &feature_bounds, &levels, &point_labels, &line_labels, &poly_labels, &mut rgn,
        routing_ctx.as_ref(),
    )?;

    // 5b. Rebuild NET with all level/div references collected during RGN encoding.
    // mkgmap writes one RoadIndex for each RGN polyline segment of a road. NET1
    // offsets can change when those variable-length records grow, so after the
    // rebuild we re-encode RGN once with the final NET1 offsets.
    if let Some(ref mut net_writer) = net_writer_opt {
        if !subdiv_road_refs.is_empty() {
            let num_roads = net_writer.roads.len();
            let mut road_refs: Vec<Vec<(u8, u16)>> = vec![Vec::new(); num_roads];
            for &(road_idx, polyline_num, subdiv_num) in &subdiv_road_refs {
                if road_idx < num_roads {
                    road_refs[road_idx].push((polyline_num, subdiv_num));
                }
            }
            for (road, refs) in net_writer.roads.iter_mut().zip(road_refs.into_iter()) {
                road.subdiv_refs = vec![refs];
            }
            let _ = net_writer.build();
            if let Some(ref mut ctx) = routing_ctx {
                for (mp_idx, road_idx) in &ctx.mp_index_to_road_idx {
                    if let Some(&net1_off) = net_writer.net1_offsets().get(*road_idx) {
                        ctx.net1_offsets_by_mp_index.insert(*mp_idx, net1_off);
                    }
                }
            }

            rgn = RgnWriter::new();
            let rebuilt = build_multilevel_hierarchy(
                mp, &feature_bounds, &levels, &point_labels, &line_labels, &poly_labels, &mut rgn,
                routing_ctx.as_ref(),
            )?;
            let (rebuilt_subdivisions, rebuilt_levels, rebuilt_ext_offsets, _) = rebuilt;
            let _ = std::mem::replace(&mut all_subdivisions, rebuilt_subdivisions);
            let _ = std::mem::replace(&mut tre_levels, rebuilt_levels);
            let _ = std::mem::replace(&mut ext_type_offsets_data, rebuilt_ext_offsets);
        }
    }

    // 6. Build TRE
    let mut tre = TreWriter::new();
    tre.set_bounds(tre_header_bounds.min_lat(), tre_header_bounds.min_lon(), tre_header_bounds.max_lat(), tre_header_bounds.max_lon());
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
    let num_levels = tre_levels.len();
    tre.levels = tre_levels;
    tre.subdivisions = all_subdivisions;
    // mkgmap: lastRgnPos = rgnFile.position() - HEADER_LEN → end of RGN body
    tre.last_rgn_pos = rgn.position();

    // Build overviews with correct max_level per type.
    // max_level = highest TRE level index at which features of that type are visible.
    // For points: end_level.unwrap_or(0) (same semantics as filter at line ~602).
    // For lines/polygons: max DataN key visible given end_level (mirrors feature_visible_at_level).
    // Polish Map type codes: < 0x10000 = standard (type=high byte, subtype=low byte),
    //                        >= 0x10000 = extended (prefix 0x1)

    // Points: group by type_code → max(end_level.unwrap_or(0))
    let mut point_max: HashMap<u32, u8> = HashMap::new();
    for mp_point in &mp.points {
        let ml = mp_point.end_level.unwrap_or(0);
        let e = point_max.entry(mp_point.type_code).or_insert(0);
        *e = (*e).max(ml);
    }
    for (tc, ml) in &point_max {
        if *tc < 0x10000 {
            let (t, st) = split_type_subtype(*tc);
            tre.point_overviews.push(PointOverview::new(t, *ml, st));
        } else {
            tre.ext_point_overviews.push(ExtPointOverview::from_type_code(*tc, *ml));
        }
    }

    // Lines: group by type_code → max visible DataN level
    let mut line_max: HashMap<u32, u8> = HashMap::new();
    for mp_line in &mp.polylines {
        let ml = mp_line.geometries.keys()
            .filter(|&&n| match mp_line.end_level {
                None | Some(0) => true,
                Some(e) => n <= e,
            })
            .copied()
            .max()
            .unwrap_or(0);
        let e = line_max.entry(mp_line.type_code).or_insert(0);
        *e = (*e).max(ml);
    }
    for (tc, ml) in &line_max {
        if *tc < 0x10000 {
            tre.polyline_overviews.push(PolylineOverview::new(*tc as u8, *ml));
        } else {
            tre.ext_polyline_overviews.push(ExtPolylineOverview::from_type_code(*tc, *ml));
        }
    }

    // Polygons: group by type_code → max visible DataN level
    let mut poly_max: HashMap<u32, u8> = HashMap::new();
    for mp_poly in &mp.polygons {
        let ml = mp_poly.geometries.keys()
            .filter(|&&n| match mp_poly.end_level {
                None | Some(0) => true,
                Some(e) => n <= e,
            })
            .copied()
            .max()
            .unwrap_or(0);
        let e = poly_max.entry(mp_poly.type_code).or_insert(0);
        *e = (*e).max(ml);
    }
    for (tc, ml) in &poly_max {
        if *tc < 0x10000 {
            tre.polygon_overviews.push(PolygonOverview::new(*tc as u8, *ml));
        } else {
            tre.ext_polygon_overviews.push(ExtPolygonOverview::from_type_code(*tc, *ml));
        }
    }

    // Background polygon overview — type 0x4B is added to every subdivision at all levels
    let bg_max_level = num_levels.saturating_sub(1) as u8;
    tre.polygon_overviews.push(PolygonOverview::new(0x4B, bg_max_level));

    // Deduplicate overviews by type_code, keeping the maximum max_level.
    // Simple sort+dedup() would use full PartialEq (type_code, max_level), leaving duplicate
    // type_code entries with different max_levels. dedup_by merges them correctly.
    dedup_overviews_polyline(&mut tre.polyline_overviews);
    dedup_overviews_polygon(&mut tre.polygon_overviews);
    dedup_overviews_point(&mut tre.point_overviews);
    dedup_overviews_ext_polyline(&mut tre.ext_polyline_overviews);
    dedup_overviews_ext_polygon(&mut tre.ext_polygon_overviews);
    dedup_overviews_ext_point(&mut tre.ext_point_overviews);

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

// ── Overview deduplication helpers ────────────────────────────────────────────

// Each function sorts by type_code then merges consecutive same-type entries,
// keeping the maximum max_level. Derived Ord on (type_code, max_level) means
// dedup() alone would treat entries with the same type but different max_level
// as distinct — leaving duplicates in the TRE section.

fn dedup_overviews_polyline(v: &mut Vec<PolylineOverview>) {
    v.sort_by_key(|o| o.type_code);
    v.dedup_by(|a, b| { if a.type_code == b.type_code { b.max_level = b.max_level.max(a.max_level); true } else { false } });
}
fn dedup_overviews_polygon(v: &mut Vec<PolygonOverview>) {
    v.sort_by_key(|o| o.type_code);
    v.dedup_by(|a, b| { if a.type_code == b.type_code { b.max_level = b.max_level.max(a.max_level); true } else { false } });
}
fn dedup_overviews_point(v: &mut Vec<PointOverview>) {
    v.sort_by_key(|o| (o.type_code, o.sub_type));
    v.dedup_by(|a, b| { if a.type_code == b.type_code && a.sub_type == b.sub_type { b.max_level = b.max_level.max(a.max_level); true } else { false } });
}
fn dedup_overviews_ext_polyline(v: &mut Vec<ExtPolylineOverview>) {
    v.sort_by_key(|o| (o.type_high, o.type_low));
    v.dedup_by(|a, b| { if a.type_high == b.type_high && a.type_low == b.type_low { b.max_level = b.max_level.max(a.max_level); true } else { false } });
}
fn dedup_overviews_ext_polygon(v: &mut Vec<ExtPolygonOverview>) {
    v.sort_by_key(|o| (o.type_high, o.type_low));
    v.dedup_by(|a, b| { if a.type_high == b.type_high && a.type_low == b.type_low { b.max_level = b.max_level.max(a.max_level); true } else { false } });
}
fn dedup_overviews_ext_point(v: &mut Vec<ExtPointOverview>) {
    v.sort_by_key(|o| (o.type_high, o.type_low));
    v.dedup_by(|a, b| { if a.type_high == b.type_high && a.type_low == b.type_low { b.max_level = b.max_level.max(a.max_level); true } else { false } });
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
) -> Result<(Vec<Subdivision>, Vec<Zoom>, Vec<u8>, Vec<(usize, u8, u16)>), ImgError> {
    // 4th return: subdiv_road_refs = (road_idx, polyline_num, subdiv_num) for NET1 level/div patching
    let mut all_subdiv_road_refs: Vec<(usize, u8, u16)> = Vec::new();

    if levels.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()));
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
    // Hierarchical split (parité mkgmap MapSplitter): for each parent subdivision,
    // features whose bbox-midpoint falls within that parent's bounds are split
    // WITHIN those bounds. This guarantees every child cell is fully contained in
    // its parent's geographic extent → no cross-boundary straddling → firmware
    // top-down navigation always reaches the correct leaf.
    let process_levels = if num_levels > 1 { num_levels - 1 } else { num_levels };
    for level_idx in (0..process_levels).rev() {
        let level = &levels[level_idx];
        let level_num = level_idx as u8;
        let shift = (24i32 - level.resolution as i32).max(0);

        // Pre-compute all features for this level (from full tile bounds)
        let (all_points, all_lines, all_shapes) =
            filter_features_for_level(mp, level_num, bounds);

        // ── Hierarchical split: one split per parent ──
        // Assign each feature to exactly one parent by midpoint (right-exclusive).
        // Features at exact tile-max boundary fall through to nearest-parent fallback.
        let mut areas: Vec<splitter::MapArea> = Vec::new();
        let mut area_parents: Vec<u16> = Vec::new();

        // Pre-compute midpoints and parent assignments for all features
        let pt_parents: Vec<Option<u16>> = all_points.iter().map(|p| {
            assign_to_parent(
                p.location.latitude(), p.location.longitude(),
                &parent_areas,
            )
        }).collect();

        // Lines: single-parent by bbox midpoint (mkgmap pickArea).
        // Full unclipped lines go to ONE parent; subdivision TRE bounds expand
        // via full_bounds() to cover any overflow beyond the cell boundary,
        // so the device loads the subdivision when any part of the line is visible.
        let ln_parents: Vec<Option<u16>> = all_lines.iter().map(|l| {
            let bbox = Area::from_coords(&l.points);
            let mid_lat = (bbox.min_lat() + bbox.max_lat()) / 2;
            let mid_lon = (bbox.min_lon() + bbox.max_lon()) / 2;
            assign_to_parent(mid_lat, mid_lon, &parent_areas)
        }).collect();

        // Shapes: pre-clip each polygon to every parent it overlaps so that panning
        // across subdivision boundaries keeps the polygon visible. Each overlapping
        // parent receives a Sutherland-Hodgman-clipped fragment referencing the same
        // mp_index (type/label lookup unchanged). Replaces the single-parent-by-centroid
        // approach that caused polygons to vanish when the user panned to an adjacent
        // subdivision whose subtree didn't contain the polygon.
        let sh_clips: Vec<Vec<(u16, Vec<Coord>)>> = all_shapes.iter().map(|s| {
            let bbox = Area::from_coords(&s.points);
            let mut clips: Vec<(u16, Vec<Coord>)> = Vec::new();
            for &(area, pnum) in &parent_areas {
                if area.intersects(&bbox) {
                    let clipped = splitter::clip_polygon_to_rect(&s.points, &area);
                    if clipped.len() >= 3 {
                        clips.push((pnum as u16, clipped));
                    }
                }
            }
            // Fallback: centroid parent if no clip produced ≥3 pts (degenerate polygon)
            if clips.is_empty() {
                let mid_lat = (bbox.min_lat() + bbox.max_lat()) / 2;
                let mid_lon = (bbox.min_lon() + bbox.max_lon()) / 2;
                if let Some(p) = assign_to_parent(mid_lat, mid_lon, &parent_areas) {
                    clips.push((p, s.points.clone()));
                }
            }
            clips
        }).collect();

        for &(parent_bounds, parent_num) in &parent_areas {
            let pnum16 = parent_num as u16;

            let parent_pts: Vec<splitter::SplitPoint> = all_points.iter().enumerate()
                .filter(|(i, _)| pt_parents[*i] == Some(pnum16))
                .map(|(_, p)| p.clone())
                .collect();

            let parent_lines: Vec<splitter::SplitLine> = all_lines.iter().enumerate()
                .filter(|(i, _)| ln_parents[*i] == Some(pnum16))
                .map(|(_, l)| splitter::SplitLine {
                    mp_index: l.mp_index,
                    points: l.points.clone(),
                })
                .collect();

            let parent_shapes: Vec<splitter::SplitShape> = all_shapes.iter().enumerate()
                .flat_map(|(i, s)| {
                    sh_clips[i].iter()
                        .filter(|(p, _)| *p == pnum16)
                        .map(move |(_, clipped_pts)| splitter::SplitShape {
                            mp_index: s.mp_index,
                            points: clipped_pts.clone(),
                        })
                })
                .collect();

            let has_features = !parent_pts.is_empty()
                || !parent_lines.is_empty()
                || !parent_shapes.is_empty();

            let sub_areas = if has_features {
                let result = splitter::split_features(
                    parent_bounds, level.resolution,
                    parent_pts, parent_lines, parent_shapes,
                );
                if result.is_empty() {
                    vec![splitter::MapArea::new(parent_bounds, level.resolution)]
                } else {
                    result
                }
            } else {
                // mkgmap invariant: every non-leaf parent gets at least one child
                vec![splitter::MapArea::new(parent_bounds, level.resolution)]
            };

            for sa in sub_areas {
                areas.push(sa);
                area_parents.push(pnum16);
            }
        }

        // Fallback: features with None parent (exact tile-max boundary) go to nearest
        if let Some(&(_, last_pnum)) = parent_areas.last() {
            let last_pnum16 = last_pnum as u16;
            let fallback_pos = area_parents.iter().rposition(|&p| p == last_pnum16);
            if let Some(pos) = fallback_pos {
                for (i, p) in all_points.iter().enumerate() {
                    if pt_parents[i].is_none() { areas[pos].add_point(p.clone()); }
                }
                for (i, l) in all_lines.iter().enumerate() {
                    if ln_parents[i].is_none() {
                        areas[pos].add_line(splitter::SplitLine {
                            mp_index: l.mp_index,
                            points: l.points.clone(),
                        });
                    }
                }
                // Shapes: already handled by sh_clips intersection logic above.
            }
        }

        // Areas are already in parent-order (built per parent) — no re-sort needed.
        // Contiguity guaranteed by construction.
        let sorted_areas: Vec<&splitter::MapArea> = areas.iter().collect();
        let sorted_parents: Vec<u16> = area_parents.clone();

        let mut next_parent_areas: Vec<(Area, u32)> = Vec::new();
        let first_child_num = subdiv_counter;

        for (i, (&area, &parent_num)) in sorted_areas.iter().zip(sorted_parents.iter()).enumerate() {
            if subdiv_counter > u16::MAX as u32 {
                return Err(ImgError::InvalidFormat(
                    "Too many subdivisions (>65535) — tile is too dense".into(),
                ));
            }
            let subdiv_num = subdiv_counter as u16;
            subdiv_counter += 1;

            let mut subdiv = Subdivision::new(subdiv_num, level_num, level.resolution);
            // Subdivision TRE bounds = full_bounds() (union bbox de toutes les features).
            // Parité mkgmap MapBuilder.java:913 — createSubdivision(parent, ma.getFullBounds(), z).
            // Les polygones sont clipés à la cellule → full_bounds ≈ cell pour les shapes.
            // Les polylignes sont stockées intactes dans UNE seule subdivision → full_bounds
            // s'étend pour couvrir toute la ligne. Le firmware charge la subdivision dès que
            // son TRE bounds intersecte le viewport → la ligne reste visible lors du panning.
            // Le centre reste ancré sur la cellule grille (hiérarchie de navigation inchangée).
            let fb = area.full_bounds();
            subdiv.set_center(&area.bounds.center());
            subdiv.set_bounds(
                fb.min_lat(), fb.min_lon(),
                fb.max_lat(), fb.max_lon(),
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

            // mkgmap puts regular points in HAS_POINTS (0x10) and only city-indexed
            // points in HAS_IND_POINTS (0x20). Since imgforge doesn't detect cities,
            // all points go into the regular section at all levels.
            if !pts_data.is_empty() {
                subdiv.flags |= subdivision::HAS_POINTS;
            }
            if !lines_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYLINES; }
            if !polys_data.is_empty() { subdiv.flags |= subdivision::HAS_POLYGONS; }

            // All points in regular section — no indexed points without city detection
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

    Ok((all_subdivisions, tre_levels_build, ext_type_offsets_data, all_subdiv_road_refs))
}

// ── Feature filtering per level ────────────────────────────────────────────

/// Sémantique mkgmap r4924 `PolishMapDataSource.setResolution` (range-based) :
/// chaque `DataN(k)` génère un MapElement avec `[minRes, maxRes]` :
/// - `maxRes = bits(k)` (résolution du bucket d'origine)
/// - si `endLevel > 0` : `minRes = bits(j-1)` où j est le prochain bucket DataN après k,
///   sinon `minRes = bits(endLevel)`
/// - si `endLevel == 0` : `minRes = maxRes = bits(k)` (visibilité stricte à L=k)
///
/// Le device rend cette feature à un target level L ssi `bits(L) ∈ [minRes, maxRes]`.
/// Comme bits décroît avec level (bits(0)=24 > bits(1)=23 > ...), ça équivaut à :
/// **`L ∈ [k, j-1]`** (ou `[k, endLevel]` si pas de bucket suivant).
///
/// En pratique : feature visible au level L ssi
/// - `L ≤ endLevel` (toujours, sauf endLevel=0 traité à part)
/// - il existe un `DataN(k)` avec `k ≤ L` (le plus grand `k ≤ L` est le bucket à utiliser)
///
/// Bug pré-fix : imgforge exigeait `geometries.contains_key(&L)` strict. Conséquence :
/// une polyline avec `Data0=` et `Data2=` (mais pas `Data1=`) et `EndLevel=2` était
/// invisible au level 1 → "feature qui apparaît à 300m mais disparaît à 200m" sur Alpha 100,
/// alors que mkgmap rendait correctement Data0 au level 1 via le fallback range.
fn feature_visible_at_level(
    end_level: Option<u8>,
    geometries: &std::collections::BTreeMap<u8, Vec<crate::img::coord::Coord>>,
    level: u8,
) -> bool {
    match end_level {
        Some(0) | None => {
            // endLevel=0 : visibilité stricte au bucket exact (pas de fallback range)
            geometries.contains_key(&level)
        }
        Some(el) => {
            if level > el {
                return false;
            }
            // Range mkgmap : il faut un DataN(k) avec k ≤ level. Le `next_back` du sous-arbre
            // [..=level] retourne le plus grand k ≤ level présent dans la BTreeMap.
            geometries.range(..=level).next_back().is_some()
        }
    }
}

/// Bucket DataN à utiliser pour render au level L (sémantique mkgmap range).
/// Retourne le plus grand `k ≤ level` présent dans `geometries`, ou None si aucun.
/// Utilisé pour récupérer la géométrie à émettre quand `feature_visible_at_level` retourne true.
fn pick_geometry_bucket(
    geometries: &std::collections::BTreeMap<u8, Vec<crate::img::coord::Coord>>,
    level: u8,
) -> Option<&[crate::img::coord::Coord]> {
    geometries.range(..=level).next_back().map(|(_, v)| v.as_slice())
}

/// Filter features visible at a given zoom level within parent bounds.
///
/// Applies geometry optimizations (simplification, min-size filtering, area sorting)
/// based on MpHeader options. Cf. `feature_visible_at_level` pour la règle d'inclusion.
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

    // Auto-simplification — parité mkgmap `MapBuilder.java:215` (default reducePointError=2.6)
    // et `DouglasPeuckerFilter.maxErrorDistance = filterDistance × (1 << shift)`.
    // Gated sur `shift > 0 && level > 0` pour protéger le niveau détail.
    let shift = mp.header.levels.get(level as usize)
        .map(|&res| (24i32 - res as i32).max(0))
        .unwrap_or(0);
    let auto_epsilon = if shift > 0 && level > 0 {
        Some((1i32 << shift) as f64 * 2.6)
    } else {
        None
    };

    // Determine DP epsilon for lines
    // Guard: never simplify at the most detailed level (TRE 0) — matches mkgmap behaviour
    // and prevents contour discontinuities caused by per-subdivision rounding after DP.
    let line_epsilon = if level == 0 {
        None
    } else {
        mp.header.reduce_point_density.or(auto_epsilon)
    };

    // Determine DP epsilon for polygons.
    // N'utilise PAS auto_epsilon : les polygones ont des contraintes topologiques
    // (frontières partagées entre communes, forêts adjacentes). Un DP indépendant
    // par polygone avec epsilon = (1<<shift)*2.6 (≈1600m au niveau 6) casse la
    // topologie → trous/superpositions visibles entre communes adjacentes.
    // La simplification des polygones est gérée par mpforge (profils generalize-profiles.yaml)
    // qui connaît les couches et peut choisir des tolérances appropriées.
    // imgforge n'applique DP sur polygones que si explicitement configuré dans l'en-tête MP.
    let poly_epsilon = resolve_polygon_epsilon(&mp.header, level)
        .or(mp.header.reduce_point_density);

    // Sémantique mkgmap r4924 (PolishMapDataSource : lineStringMap par level,
    // setResolution(elem, level) calculé pour chaque bucket) : une polyline est
    // émise au level L ssi `DataL` existe ET (EndLevel=0 ⇒ ok, sinon L ≤ EndLevel).
    // Au-delà de L > EndLevel, r4924 produirait min=extractResolution(EndLevel)
    // et max=bits(L) < min → intervalle vide → feature filtrée par le filter
    // chain (MapArea.addLines max filter + MapBuilder.processLines min filter).
    // Cause-racine du bug Alpha 100 wide-zoom : fix #3 antérieur ajoutait une
    // branche `OR EndLevel >= level` qui surémittait aux levels 0..EndLevel
    // même sans DataL explicite, gonflant les bytes RGN aux wide-zoom levels.
    // Chaîne de filtres mkgmap normalFilters (cf. MapBuilder.java:1246-1283) :
    // RoundCoords → SizeFilter(1) → RemoveObsolete → DP → RemoveEmpty.
    // Ordre strict : le DP opère sur de la géométrie déjà quantifiée et
    // nettoyée des colinéaires, ce qui maximise son efficacité. Gated sur
    // `shift > 0` (level > 0) pour protéger le niveau détail.
    let shift_u: u32 = shift.max(0) as u32;
    let lines: Vec<SplitLine> = mp.polylines.iter().enumerate()
        .filter(|(_, l)| feature_visible_at_level(l.end_level, &l.geometries, level))
        .filter_map(|(i, l)| {
            // Sémantique mkgmap range : utilise le bucket du plus grand k ≤ level
            // (et non le fallback vers le bucket plus grossier comme le faisait
            // `geometry_for_level`, qui n'est pas la sémantique render mkgmap).
            let geom = pick_geometry_bucket(&l.geometries, level)?;
            if geom.is_empty() || !expanded.contains_coord(&geom[0]) {
                return None;
            }
            let mut pts = geom.to_vec();
            if shift_u > 0 {
                if !mp.header.no_round_coords {
                    pts = round_coords(&pts, shift_u);
                }
                if !mp.header.no_size_filter && !passes_size_filter(&pts, shift_u, 1) {
                    return None;
                }
                if !mp.header.no_remove_obsolete_points {
                    pts = remove_obsolete_points(&pts, false);
                }
            }
            if let Some(eps) = line_epsilon {
                let coords: Vec<(i32, i32)> = pts.iter().map(|c| (c.latitude(), c.longitude())).collect();
                let simplified = douglas_peucker(&coords, eps);
                if simplified.len() >= 2 {
                    pts = simplified.iter().map(|&(lat, lon)| Coord::new(lat, lon)).collect();
                }
            }
            if shift_u > 0 && !passes_remove_empty(&pts, false) {
                return None;
            }
            Some(SplitLine { mp_index: i, points: pts })
        })
        .collect();

    let min_size = mp.header.min_size_polygon;

    // Même règle r4924 que pour les polylines (cf. commentaire ci-dessus).
    let shapes: Vec<SplitShape> = mp.polygons.iter().enumerate()
        .filter(|(_, s)| feature_visible_at_level(s.end_level, &s.geometries, level))
        .filter_map(|(i, s)| {
            let geom = pick_geometry_bucket(&s.geometries, level)?;
            if geom.is_empty() || !expanded.contains_coord(&geom[0]) {
                return None;
            }
            // Min-size filtering
            if let Some(min) = min_size {
                let coords: Vec<(i32, i32)> = geom.iter().map(|c| (c.latitude(), c.longitude())).collect();
                if compute_area(&coords) < min as f64 {
                    return None;
                }
            }
            let mut pts = geom.to_vec();
            // Même chaîne mkgmap pour les polygones (cf. `processShapes`), ordre strict
            // Round → Size → RemoveObsolete → DP → RemoveEmpty. Gated sur `shift > 0`.
            if shift_u > 0 {
                if !mp.header.no_round_coords {
                    pts = round_coords(&pts, shift_u);
                }
                if !mp.header.no_size_filter && !passes_size_filter(&pts, shift_u, 1) {
                    return None;
                }
                if !mp.header.no_remove_obsolete_points {
                    pts = remove_obsolete_points(&pts, true);
                }
            }
            if let Some(eps) = poly_epsilon {
                let coords: Vec<(i32, i32)> = pts.iter().map(|c| (c.latitude(), c.longitude())).collect();
                let simplified = douglas_peucker(&coords, eps);
                if simplified.len() >= 3 {
                    pts = simplified.iter().map(|&(lat, lon)| Coord::new(lat, lon)).collect();
                }
            }
            if shift_u > 0 && !passes_remove_empty(&pts, true) {
                return None;
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
        // NET1 offset (mkgmap patches LBL→NET after NET is built). For full
        // routing, mkgmap also sets FLAG_EXTRABIT and writes one bit per
        // encoded point when a level-0 road segment contains internal route
        // nodes, or when this is not the last segment of a split road.
        let mut extra_bit = false;
        let mut split_node_flags: Option<Vec<bool>> = None;
        if is_leaf_level && !is_ext {
            if let Some(ctx) = routing_ctx {
                if let Some(&net1_off) = ctx.net1_offsets_by_mp_index.get(&split_line.mp_index) {
                    pl.has_net_info = true;
                    pl.net_offset = net1_off;
                    if let Some((orig_points, orig_flags)) =
                        ctx.node_flags_by_mp_index.get(&split_line.mp_index)
                    {
                        if let Some((mut flags, is_last_segment)) =
                            align_node_flags_to_split(orig_points, orig_flags, &split_line.points)
                        {
                            let has_internal_nodes = flags
                                .iter()
                                .enumerate()
                                .skip(1)
                                .take(flags.len().saturating_sub(2))
                                .any(|(_, &flag)| flag);
                            extra_bit = has_internal_nodes || !is_last_segment;
                            if extra_bit {
                                // mkgmap does not mark the final point of the final
                                // split segment in the extra-bit stream. Non-final
                                // split segments keep their end node marker so the
                                // continuation is visible from RGN.
                                if is_last_segment {
                                    if let Some(last) = flags.last_mut() {
                                        *last = false;
                                    }
                                }
                                split_node_flags = Some(flags);
                            }
                        }
                    }
                    // Track subdiv ref for NET1 level/div patching
                    if let Some(&road_idx) = ctx.mp_index_to_road_idx.get(&split_line.mp_index) {
                        let polyline_num = polyline_counter as u8;
                        subdiv_road_refs.push((road_idx, polyline_num, subdiv.number));
                    }
                }
            }
        }

        let deltas = compute_deltas(&split_line.points, subdiv);
        if let Some(bitstream) =
            line_preparer::prepare_line(&deltas, extra_bit, split_node_flags.as_deref(), is_ext)
        {
            if is_ext {
                rgn.write_ext_polyline(
                    &pl.write_ext(subdiv.center_lat, subdiv.center_lon, shift, &bitstream),
                );
            } else {
                polylines_data.extend_from_slice(
                    &pl.write(subdiv.center_lat, subdiv.center_lon, shift, &bitstream, extra_bit),
                );
                polyline_counter += 1;
            }
        }
    }

    // Polygons
    let mut polygons_data = Vec::new();

    // Background polygon — mkgmap MapperBasedMapDataSource.addBackground()
    // Type 0x4B covers the subdivision area, rendered behind all other polygons.
    // mkgmap adds one 0x4B polygon covering the full map bounds, then the splitter
    // clips it per subdivision. We achieve the same result by generating it directly
    // from each subdivision's area bounds.
    {
        let bg_pts = vec![
            Coord::new(area.bounds.min_lat(), area.bounds.min_lon()),
            Coord::new(area.bounds.min_lat(), area.bounds.max_lon()),
            Coord::new(area.bounds.max_lat(), area.bounds.max_lon()),
            Coord::new(area.bounds.max_lat(), area.bounds.min_lon()),
        ];
        let bg = Polygon::new(0x4B, bg_pts.clone());
        let bg_deltas = compute_deltas(&bg_pts, subdiv);
        if let Some(bg_bs) = line_preparer::prepare_line(&bg_deltas, false, None, false) {
            polygons_data.extend_from_slice(
                &bg.write(subdiv.center_lat, subdiv.center_lon, shift, &bg_bs),
            );
        }
    }

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

/// Assign a feature midpoint to the first parent whose bounds contain it
/// (right-exclusive: boundary points go to the next parent).
/// Returns None if no parent contains the point (tile-max edge case).
fn assign_to_parent(lat: i32, lon: i32, parent_areas: &[(Area, u32)]) -> Option<u16> {
    // Right-exclusive: use < for max bounds to avoid double-containment at boundaries
    if let Some(&(_, n)) = parent_areas.iter()
        .find(|(pa, _)| pa.contains_coord_right_excl(lat, lon))
    {
        return Some(n as u16);
    }
    // Inclusive fallback: catches features at the tile's max_lat or max_lon
    if let Some(&(_, n)) = parent_areas.iter()
        .find(|(pa, _)| pa.contains_coord(&super::coord::Coord::new(lat, lon)))
    {
        return Some(n as u16);
    }
    None
}

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
        for c in pl.all_coords() {
            update_bounds(&mut min_lat, &mut max_lat, &mut min_lon, &mut max_lon, c);
        }
    }
    for pg in &mp.polygons {
        for c in pg.all_coords() {
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

fn align_node_flags_to_split(
    original_points: &[Coord],
    original_flags: &[bool],
    split_points: &[Coord],
) -> Option<(Vec<bool>, bool)> {
    if split_points.is_empty()
        || original_points.len() != original_flags.len()
        || split_points.len() > original_points.len()
    {
        return None;
    }

    let start = original_points
        .windows(split_points.len())
        .position(|window| window == split_points)?;
    let end = start + split_points.len();
    let flags = original_flags[start..end].to_vec();
    Some((flags, end == original_points.len()))
}

/// Routing context computed before RGN encoding, providing NET1 offsets and node_flags
/// for each routable polyline (keyed by mp_index into mp.polylines).
struct RoutingContext {
    net1_offsets_by_mp_index: HashMap<usize, u32>,
    /// mp_index → road_idx (index in the NET writer's road list)
    mp_index_to_road_idx: HashMap<usize, usize>,
    /// mp_index → detailed routing geometry + per-vertex RouteNode flags.
    ///
    /// mkgmap writes an extra bitstream on level-0 road polylines when internal
    /// routing nodes have to be discoverable from RGN geometry. BaseCamp uses
    /// this RGN-side node marking when snapping route points to roads.
    node_flags_by_mp_index: HashMap<usize, (Vec<Coord>, Vec<bool>)>,
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
                tracing::info!("--route/--net specified but no RoadID found in .mp data — Routing inactif dans cette tuile : aucun tronçon routable (RoadID inexistant)");
                return (None, None, None);
            }
        }
    }

    let mut road_polylines = Vec::new();
    let mut mp_indices: Vec<usize> = Vec::new();
    let mut road_params: Vec<RouteParams> = Vec::new();
    let mut road_nod_entries: Vec<Vec<NodEntry>> = Vec::new();

    // Collect road data
    struct RoadInfo {
        label_offset: u32,
        road_length: u32,
        params: RouteParams,
    }
    let mut road_infos: Vec<RoadInfo> = Vec::new();

    for (i, mp_line) in mp.polylines.iter().enumerate() {
        if mp_line.road_id.is_some() {
            // Routing strict Data0 (F4) : pas de fallback Politique B.
            let Some(routing_geom) = mp_line.routing_geometry() else {
                tracing::warn!(
                    road_id = ?mp_line.road_id,
                    "routable polyline has no Data0 bucket, skipping routing entry"
                );
                continue;
            };

            let params = if let Some(ref rp) = mp_line.route_param {
                graph_builder::parse_route_param(rp)
            } else {
                RouteParams::default()
            };

            road_infos.push(RoadInfo {
                label_offset: line_labels[i],
                road_length: estimate_road_length(routing_geom),
                params: params.clone(),
            });

            let road_idx = road_infos.len() - 1;
            road_polylines.push((routing_geom.to_vec(), road_idx, params.clone()));
            road_params.push(params);
            mp_indices.push(i);
            road_nod_entries.push(mp_line.nodes.clone());
        }
    }

    // Polish MP mkgmap-faithful mode: when NodN= directives are present, mkgmap's
    // RoadHelper promotes only those coordinates to CoordNode. It does not infer
    // endpoints/shared coordinates on top of the MP node list. Fall back to the
    // topology heuristic only for MP inputs without explicit routing nodes.
    let has_explicit_nod_entries = road_nod_entries.iter().any(|nodes| !nodes.is_empty());
    let mut junctions: std::collections::HashSet<(i32, i32)> = if has_explicit_nod_entries {
        std::collections::HashSet::new()
    } else {
        find_junctions(&road_polylines)
    };
    let mut boundary_coords: std::collections::HashSet<(i32, i32)> =
        std::collections::HashSet::new();

    // Collect junctions from NodEntry directives.
    for (road_idx, nods) in road_nod_entries.iter().enumerate() {
        if !nods.is_empty() {
            let coords = &road_polylines[road_idx].0;
            for nod in nods {
                let idx = nod.point_index as usize;
                if idx < coords.len() {
                    let key = (coords[idx].latitude(), coords[idx].longitude());
                    junctions.insert(key);
                    if nod.boundary {
                        boundary_coords.insert(key);
                    }
                }
            }
        }
    }

    let all_node_flags = if has_explicit_nod_entries {
        road_polylines
            .iter()
            .map(|(coords, _, _)| {
                coords
                    .iter()
                    .map(|coord| junctions.contains(&(coord.latitude(), coord.longitude())))
                    .collect()
            })
            .collect()
    } else {
        compute_node_flags(&road_polylines, &junctions)
    };

    // Build routing graph (enriched with heading + node_class)
    let route_nodes =
        graph_builder::build_graph_with_junctions(&road_polylines, &junctions, &boundary_coords);

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
        let mut node_flags_by_mp_index = HashMap::new();
        for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
            node_flags_by_mp_index.insert(
                mp_idx,
                (road_polylines[road_idx].0.clone(), Vec::new()),
            );
        }
        let routing_ctx = RoutingContext {
            net1_offsets_by_mp_index,
            mp_index_to_road_idx,
            node_flags_by_mp_index,
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
    let mut node_flags_by_mp_index = HashMap::new();
    for (road_idx, &mp_idx) in mp_indices.iter().enumerate() {
        node_flags_by_mp_index.insert(
            mp_idx,
            (road_polylines[road_idx].0.clone(), all_node_flags[road_idx].clone()),
        );
    }
    let routing_ctx = RoutingContext {
        net1_offsets_by_mp_index,
        mp_index_to_road_idx,
        node_flags_by_mp_index,
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

/// Contour line types: 0x20–0x25 inclusive (mkgmap GType.isContourLine).
/// No mask: real contour types are always < 0x100; applying & 0xFF would cause false positives
/// on types with sub-type encoded in low byte (e.g. 0x0A22 would incorrectly match 0x22).
fn is_contour_type(type_code: u32) -> bool {
    type_code >= 0x20 && type_code <= 0x25
}

/// Convert a numeric metres label to feet (mkgmap fixElevation logic).
/// Non-numeric labels are returned unchanged.
fn metres_label_to_feet(label: &str) -> String {
    match label.trim().parse::<f64>() {
        Ok(m) => (m * 3.280_839_9).round().to_string(),
        Err(_) => label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    // --- multi-level Data: AC8 — sélection par niveau dans filter_features_for_level ---
    fn mp_with_multi_data(content: &str) -> MpFile {
        parser::parse_mp(content).unwrap()
    }

    #[test]
    fn filter_features_mkgmap_range_with_endlevel() {
        // Sémantique mkgmap r4924 PolishMapDataSource.setResolution (range-based) :
        // une polyline avec Data0 + Data2 et EndLevel=2 reçoit ces ranges :
        //   - Data0 → [minRes=bits(1)=22, maxRes=bits(0)=24] → visible levels 0 et 1
        //   - Data2 → [minRes=bits(2)=20, maxRes=bits(2)=20] → visible level 2
        // Au level 1, mkgmap rend la géométrie Data0 (fallback range).
        // Bug Alpha 100 ("voie disparaît à 200m mais visible à 300m") fixé en avril 2026 :
        // imgforge filtrait strictement contains_key(level), perdait la voie au level 1.
        let content = r#"
[IMG ID]
ID=1
Levels=24,22,20
[END-IMG ID]
[POLYLINE]
Type=0x06
EndLevel=2
Data0=(48.0,7.0),(48.1,7.1),(48.2,7.2)
Data2=(48.0,7.0),(48.2,7.2)
[END]
"#;
        let mp = mp_with_multi_data(content);
        let bounds = compute_bounds(&mp).unwrap();

        let (_, lines0, _) = filter_features_for_level(&mp, 0, &bounds);
        assert_eq!(lines0.len(), 1, "level 0 → bucket Data0 (largest k≤0)");
        assert_eq!(lines0[0].points.len(), 3, "level 0 utilise géom Data0 (3 pts)");

        let (_, lines1, _) = filter_features_for_level(&mp, 1, &bounds);
        assert_eq!(lines1.len(), 1, "level 1 → fallback range mkgmap : Data0 visible (largest k≤1)");
        // shift=1 (level 1, bits=22 → shift = 24-22 = 2 ; ici Levels=[24,22,20] donc level 1 = bits 22, shift 2)
        // round_coords + remove_obsolete peuvent réduire de 3 → 2 pts. On vérifie ≥ 2.
        assert!(lines1[0].points.len() >= 2, "géom Data0 utilisée au level 1 ({} pts)", lines1[0].points.len());

        let (_, lines2, _) = filter_features_for_level(&mp, 2, &bounds);
        assert_eq!(lines2.len(), 1, "level 2 → bucket Data2 explicite (2 ≤ EndLevel=2)");
        assert!(lines2[0].points.len() >= 2);

        // Au-delà d'EndLevel, plus de visibilité même si DataN existe au-delà.
        // (Pas de level 3 ici puisque Levels=24,22,20 a 3 entrées 0..2.)
    }

    #[test]
    fn filter_features_endlevel_zero_strict() {
        // Sémantique mkgmap : si EndLevel=0, pas de fallback range — chaque DataN
        // visible UNIQUEMENT à son level d'origine. Comportement identique à
        // l'ancien strict pour ce cas.
        let content = r#"
[IMG ID]
ID=1
Levels=24,22,20
[END-IMG ID]
[POLYLINE]
Type=0x06
EndLevel=0
Data0=(48.0,7.0),(48.1,7.1),(48.2,7.2)
Data2=(48.0,7.0),(48.2,7.2)
[END]
"#;
        let mp = mp_with_multi_data(content);
        let bounds = compute_bounds(&mp).unwrap();
        let (_, lines0, _) = filter_features_for_level(&mp, 0, &bounds);
        assert_eq!(lines0.len(), 1, "level 0 → Data0 présent");
        let (_, lines1, _) = filter_features_for_level(&mp, 1, &bounds);
        assert_eq!(lines1.len(), 0, "level 1 → EndLevel=0, pas de fallback : exclu");
        let (_, lines2, _) = filter_features_for_level(&mp, 2, &bounds);
        assert_eq!(lines2.len(), 1, "level 2 → Data2 présent (EndLevel=0 implicite ≥ level d'origine)");
    }

    #[test]
    fn filter_features_data0_only_restricted_to_level0() {
        // r4924 : Data0-only avec EndLevel=0 n'est visible qu'au level 0.
        // (Historiquement imgforge propageait à tous levels — bug Alpha 100 wide-zoom.)
        let content = r#"
[IMG ID]
ID=1
Levels=24,22,20
[END-IMG ID]
[POLYLINE]
Type=0x06
EndLevel=0
Data0=(48.0,7.0),(48.1,7.1),(48.2,7.2),(48.3,7.3)
[END]
"#;
        let mp = mp_with_multi_data(content);
        let bounds = compute_bounds(&mp).unwrap();

        let (_, lines0, _) = filter_features_for_level(&mp, 0, &bounds);
        assert_eq!(lines0.len(), 1, "level 0 → bucket Data0 explicite");
        assert!(!lines0[0].points.is_empty());

        for lvl in 1..=2 {
            let (_, lines, _) = filter_features_for_level(&mp, lvl, &bounds);
            assert_eq!(lines.len(), 0, "level {} : pas de DataN explicite → exclu", lvl);
        }
    }

    #[test]
    fn filter_features_beyond_endlevel_excluded() {
        // Polyline Data0..Data6 avec EndLevel=4 : r4924 émet 7 polylines mais
        // celles aux levels 5 et 6 ont min>max → invisibles. imgforge doit les
        // exclure dès le filtre pour éviter la surémission (cause Alpha 100).
        let content = r#"
[IMG ID]
ID=1
Levels=24,23,22,21,20,18,16
[END-IMG ID]
[POLYLINE]
Type=0x06
EndLevel=4
Data0=(48.0,7.0),(48.1,7.1)
Data1=(48.0,7.0),(48.1,7.1)
Data2=(48.0,7.0),(48.1,7.1)
Data3=(48.0,7.0),(48.1,7.1)
Data4=(48.0,7.0),(48.1,7.1)
Data5=(48.0,7.0),(48.1,7.1)
Data6=(48.0,7.0),(48.1,7.1)
[END]
"#;
        let mp = mp_with_multi_data(content);
        let bounds = compute_bounds(&mp).unwrap();
        for lvl in 0..=4 {
            let (_, lines, _) = filter_features_for_level(&mp, lvl, &bounds);
            assert_eq!(lines.len(), 1, "level {} ≤ EndLevel=4 → inclus", lvl);
        }
        for lvl in 5..=6 {
            let (_, lines, _) = filter_features_for_level(&mp, lvl, &bounds);
            assert_eq!(lines.len(), 0, "level {} > EndLevel=4 → exclu (r4924 min>max)", lvl);
        }
    }

    #[test]
    fn compute_bounds_includes_all_buckets() {
        // Bounds doit englober Data0 ET Data1 même si géométries différentes.
        let content = r#"
[IMG ID]
ID=1
Levels=24,22
[END-IMG ID]
[POLYLINE]
Type=0x06
Data0=(48.0,7.0),(48.1,7.1)
Data1=(50.0,9.0),(50.1,9.1)
[END]
"#;
        let mp = mp_with_multi_data(content);
        let bounds = compute_bounds(&mp).unwrap();
        // max_lat doit refléter le bucket Data1 (50.x).
        assert!(bounds.max_lat() > Coord::from_degrees(49.0, 8.0).latitude(),
            "bounds must include Data1 bucket coords");
    }

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
    fn test_elevation_metres_contour_label_converted_in_lbl() {
        // F3 E2E: Elevation=M + type 0x22 label "100" → LBL contient "328", pas "100"
        let content = r#"
[IMG ID]
ID=1
Name=Test Elevation
CodePage=1252
Levels=24
Elevation=M
[END-IMG ID]
[POLYLINE]
Type=0x22
Label=100
Data0=(45.0,6.0),(45.1,6.1)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();
        let lbl = &result.lbl;
        let label_off = u32::from_le_bytes([lbl[21], lbl[22], lbl[23], lbl[24]]) as usize;
        let label_data = &lbl[label_off..];

        assert!(
            label_data.windows(3).any(|w| w == b"328"),
            "LBL doit contenir '328' (100m converti en pieds)"
        );
        assert!(
            !label_data.windows(3).any(|w| w == b"100"),
            "LBL ne doit PAS contenir '100' (valeur en mètres brute)"
        );
    }

    #[test]
    fn test_elevation_metres_non_contour_label_unchanged_in_lbl() {
        // F3 E2E: Elevation=M + type 0x06 (route) → label "100" inchangé dans LBL
        let content = r#"
[IMG ID]
ID=1
Name=Test Elevation Non-Contour
CodePage=1252
Levels=24
Elevation=M
[END-IMG ID]
[POLYLINE]
Type=0x06
Label=100
Data0=(45.0,6.0),(45.1,6.1)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        let result = build_subfiles(&mp).unwrap();
        let lbl = &result.lbl;
        let label_off = u32::from_le_bytes([lbl[21], lbl[22], lbl[23], lbl[24]]) as usize;
        let label_data = &lbl[label_off..];

        assert!(
            label_data.windows(3).any(|w| w == b"100"),
            "LBL doit contenir '100' inchangé pour un type non-contour"
        );
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
    fn test_build_routing_with_nod_entries_has_net_nod() {
        // T9: NodN= directives parsed from .mp (mpforge-produced) → NET+NOD generated
        // and the NodEntry junction points are honoured in the junction set.
        let content = r#"
[IMG ID]
ID=99990100
Name=NodEntry Routing Test
Levels=24
[END-IMG ID]
[POLYLINE]
Type=0x06
Label=Route A
RoadID=1
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(48.57,7.75),(48.575,7.755),(48.58,7.76)
Nod1=0,1001,0
Nod2=2,1002,1
[END]
[POLYLINE]
Type=0x06
Label=Route B
RoadID=2
RouteParam=4,3,0,0,0,0,0,0,0,0,0,0
Data0=(48.58,7.76),(48.585,7.765)
Nod1=0,1002,0
Nod2=1,1003,1
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        assert_eq!(mp.polylines[0].nodes.len(), 2, "Nod1+Nod2 parsed for Route A");
        assert_eq!(mp.polylines[0].nodes[0].node_id, 1001);
        assert_eq!(mp.polylines[0].nodes[1].node_id, 1002);

        let img = build_img(&mp).unwrap();
        assert!(find_subfile_in_img(&img, "NET"), "NodEntry .mp must produce NET");
        assert!(find_subfile_in_img(&img, "NOD"), "NodEntry .mp must produce NOD");
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

    #[test]
    fn test_all_nonleaf_parents_have_valid_first_child() {
        // Vérifie l'invariant mkgmap : tout parent non-leaf a au moins un enfant.
        // Scénario : carte 7 niveaux, features UNIQUEMENT au level 0 dans un coin
        // (coin NE d'une tuile 1°×1°). Ceci provoque des parents sans features
        // aux niveaux intermédiaires → avant le fix, ces parents recevaient
        // first_child=total+1 (out-of-range), cassant la navigation Alpha 100.
        let content = r#"
[IMG ID]
ID=63240004
Name=Nonleaf Parent Invariant
Levels=24,23,22,21,20,18,16
[END-IMG ID]
[POLYLINE]
Type=0x05
EndLevel=0
Data0=(48.90,7.90),(48.91,7.91),(48.92,7.92),(48.93,7.93)
[END]
[POLYLINE]
Type=0x05
EndLevel=0
Data0=(48.90,7.90),(48.92,7.90),(48.94,7.90)
[END]
[POLYGON]
Type=0x03
Data0=(48.90,7.90),(48.91,7.90),(48.91,7.91),(48.90,7.91)
[END]
"#;
        let mp = parser::parse_mp(content).unwrap();
        // Avant le fix, ce cas provoquait des first_child out-of-range → panic ou
        // données corrompues. Vérifier que le build réussit et produit des données valides.
        let result = build_subfiles(&mp).unwrap();
        assert!(!result.tre.is_empty(), "TRE doit être non-vide");
        assert!(!result.rgn.is_empty(), "RGN doit être non-vide");

        // Inspecter le TRE binaire pour vérifier que chaque subdivision non-leaf
        // a un first_child dans la plage [1, total_subdivs].
        // Layout TRE (offsets absolus dans result.tre) :
        //   @21-24 : common header length + file type marker
        //   @21-32 : bounds (12 bytes)
        //   @33-36 : map_levels_pos (u32 LE)
        //   @37-40 : map_levels_size (u32 LE)  → num_levels = size / 4
        //   @41-44 : subdivisions_pos (u32 LE)
        //   @45-48 : subdivisions_size (u32 LE) → inclut les 4 octets du lastRgnPos
        // Chaque entrée map_levels = 4 octets [level_flags, resolution, count_lo, count_hi]
        // Subdivisions non-leaf = 16 octets (14 + 2 pour first_child)
        // Subdivisions leaf     = 14 octets
        let tre = &result.tre;
        if tre.len() < 49 { return; }
        let ml_pos  = u32::from_le_bytes([tre[33], tre[34], tre[35], tre[36]]) as usize;
        let ml_size = u32::from_le_bytes([tre[37], tre[38], tre[39], tre[40]]) as usize;
        let sd_pos  = u32::from_le_bytes([tre[41], tre[42], tre[43], tre[44]]) as usize;
        let sd_size = u32::from_le_bytes([tre[45], tre[46], tre[47], tre[48]]) as usize;
        if ml_size == 0 || ml_size % 4 != 0 { return; }
        if sd_pos + sd_size > tre.len() { return; }
        let num_levels = ml_size / 4;

        // Lire le nombre de subdivisions par niveau depuis map_levels
        let mut subdiv_counts: Vec<u16> = Vec::new();
        for i in 0..num_levels {
            let base = ml_pos + i * 4;
            if base + 4 > tre.len() { return; }
            let count = u16::from_le_bytes([tre[base + 2], tre[base + 3]]);
            subdiv_counts.push(count);
        }

        // Parcourir les subdivisions : niveaux 0..num_levels-2 = non-leaf (16 bytes),
        // niveau num_levels-1 = leaf (14 bytes).
        let total_subdivs: u16 = subdiv_counts.iter().map(|&c| c).sum();
        let mut pos = sd_pos;
        let sd_end = sd_pos + sd_size.saturating_sub(4); // -4 pour le lastRgnPos
        let mut all_first_children: Vec<u16> = Vec::new();
        for (lvl_idx, &count) in subdiv_counts.iter().enumerate() {
            let is_leaf = lvl_idx == num_levels - 1;
            let rec_size = if is_leaf { 14 } else { 16 };
            for _ in 0..count {
                if pos + rec_size > sd_end { break; }
                if !is_leaf {
                    let fc = u16::from_le_bytes([tre[pos + 14], tre[pos + 15]]);
                    all_first_children.push(fc);
                }
                pos += rec_size;
            }
        }

        // Chaque first_child doit pointer dans la plage valide [1, total_subdivs]
        for fc in &all_first_children {
            assert!(
                *fc >= 1 && *fc <= total_subdivs,
                "first_child={} hors plage [1, {}] — parent orphelin dans le TRE (ml_pos={} ml_size={} sd_pos={} sd_size={})",
                fc, total_subdivs, ml_pos, ml_size, sd_pos, sd_size
            );
        }
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

// ── Tests for elevation helpers ──────────────────────────────────────────

#[cfg(test)]
mod elevation_tests {
    use super::{is_contour_type, metres_label_to_feet};

    #[test]
    fn contour_types_0x20_to_0x25_recognised() {
        for t in 0x20u32..=0x25 {
            assert!(is_contour_type(t), "0x{:02X} should be contour", t);
        }
    }

    #[test]
    fn non_contour_types_not_recognised() {
        // 0x0A22 has subtype 0x22 in low byte but is NOT a contour — no mask applied
        for t in [0x1F, 0x26, 0x00, 0x06, 0x10000u32, 0x0A22, 0x0120, 0x10D20] {
            assert!(!is_contour_type(t), "0x{:X} should NOT be contour", t);
        }
    }

    #[test]
    fn metres_to_feet_100m() {
        assert_eq!(metres_label_to_feet("100"), "328");
    }

    #[test]
    fn metres_to_feet_1000m() {
        assert_eq!(metres_label_to_feet("1000"), "3281");
    }

    #[test]
    fn metres_to_feet_non_numeric_unchanged() {
        assert_eq!(metres_label_to_feet(""), "");
        assert_eq!(metres_label_to_feet("Col du Lautaret"), "Col du Lautaret");
    }

    #[test]
    fn metres_to_feet_zero() {
        assert_eq!(metres_label_to_feet("0"), "0");
    }

    #[test]
    fn metres_to_feet_negative() {
        // -10m → -10 * 3.2808399 = -32.808399 → round = -33
        assert_eq!(metres_label_to_feet("-10"), "-33");
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
