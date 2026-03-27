//! NOD subfile writer — routing graph (nodes, arcs, RouteCenters, road-to-node links).
//!
//! The NOD subfile stores:
//! - **NOD Header** (48 bytes): signature, section offsets, drive_on_right
//! - **NOD1**: RouteCenters with RouteNodes and Table A arcs (bearing, tabAInfo, NET1 offset)
//! - **NOD2**: Per-road bitstream marking which vertices are RouteNodes
//! - **NOD3**: Boundary nodes (empty for single-tile compilation)
//!
//! Without NOD, the GPS ignores NET attributes and cannot compute routes.
//!
//! Format references:
//! - mkgmap: `NodFile.java`, `RouteCenter.java`, `RouteNode.java`, `RoadDef.java`
//! - wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/NOD_Subfile_Format

use crate::parser::mp_types::MpPolyline;
use crate::routing::RoadNetwork;

// ── Constants ─────────────────────────────────────────────────────────────────

/// NOD header size in bytes.
const NOD_HEADER_SIZE: usize = 0x30; // 48

/// Maximum number of RouteNodes per RouteCenter (Table A).
const MAX_NODES_PER_CENTER: usize = 256;

// ── NodBuildResult ────────────────────────────────────────────────────────────

/// Result of `NodWriter::build`.
pub struct NodBuildResult {
    /// Complete NOD subfile binary: `[header || NOD1 || NOD2 || NOD3]`.
    pub data: Vec<u8>,
    /// `nod2_road_offsets[i]` = byte offset of the i-th road_def's NOD2 entry,
    /// relative to the start of the NOD2 section.
    /// Used to patch the 2-byte NOD2 placeholder in NET1 records.
    pub nod2_road_offsets: Vec<u32>,
    /// Number of boundary nodes detected (nodes within BOUNDARY_THRESHOLD of the tile bbox).
    pub boundary_node_count: usize,
}

// ── Coordinate conversion ─────────────────────────────────────────────────────

/// Convert WGS84 degrees to Garmin semicircle units (i32).
///
/// Formula: `(deg / 360.0 × 2^32).round() as i32`
fn wgs84_to_garmin(deg: f64) -> i32 {
    (deg / 360.0 * (1u64 << 32) as f64).round() as i32
}

// ── NOD Header ────────────────────────────────────────────────────────────────

/// Write the 48-byte NOD header into `buf`.
///
/// Binary layout:
/// ```text
/// 0x00  LE16  Header length = 0x0030 (48)
/// 0x02  u8    Type indicator = 0x00
/// 0x03  u8    Locked indicator = 0x00
/// 0x04  7B    Creation date (zeros)
/// 0x0B  10B   Signature "GARMIN NOD" (no null terminator)
/// 0x15  LE32  NOD1 section offset (= 48)
/// 0x19  LE32  NOD1 section length
/// 0x1D  u8    Node record size = 0x09 (informational)
/// 0x1E  LE32  NOD2 section offset (= 48 + nod1_len)
/// 0x22  LE32  NOD2 section length
/// 0x26  LE32  NOD3 section offset (= NOD2 offset + NOD2 len)
/// 0x2A  LE32  NOD3 section length (0 when no boundary nodes)
/// 0x2E  u8    Drive-on-right = 0x01 (France, circulation à droite)
/// 0x2F  u8    Flags = 0x00
/// ```
fn write_nod_header(buf: &mut Vec<u8>, nod1_len: u32, nod2_len: u32, nod3_len: u32) {
    let start_len = buf.len();
    // 0x00: header_length = 0x0030 (LE16)
    buf.extend_from_slice(&(NOD_HEADER_SIZE as u16).to_le_bytes());
    // 0x02: type indicator (u8)
    buf.push(0x00);
    // 0x03: locked indicator (u8)
    buf.push(0x00);
    // 0x04: creation date (7 bytes, all zero)
    buf.extend_from_slice(&[0u8; 7]);
    // 0x0B: signature "GARMIN NOD" — exactly 10 bytes (no null terminator)
    buf.extend_from_slice(b"GARMIN NOD");

    // 0x15: NOD1 section offset (LE32) = header length
    buf.extend_from_slice(&(NOD_HEADER_SIZE as u32).to_le_bytes());
    // 0x19: NOD1 section length (LE32)
    buf.extend_from_slice(&nod1_len.to_le_bytes());
    // 0x1D: node record size = 0x09 (informational, u8)
    buf.push(0x09);

    // 0x1E: NOD2 section offset (LE32) = header + nod1_len
    let nod2_offset = NOD_HEADER_SIZE as u32 + nod1_len;
    buf.extend_from_slice(&nod2_offset.to_le_bytes());
    // 0x22: NOD2 section length (LE32)
    buf.extend_from_slice(&nod2_len.to_le_bytes());

    // 0x26: NOD3 section offset (LE32) = nod2_offset + nod2_len
    let nod3_offset = nod2_offset + nod2_len;
    buf.extend_from_slice(&nod3_offset.to_le_bytes());
    // 0x2A: NOD3 section length (LE32)
    buf.extend_from_slice(&nod3_len.to_le_bytes());

    // 0x2E: drive_on_right = 0x01 (France)
    buf.push(0x01);
    // 0x2F: flags = 0x00
    buf.push(0x00);

    debug_assert_eq!(
        buf.len() - start_len,
        NOD_HEADER_SIZE,
        "write_nod_header must append exactly {} bytes",
        NOD_HEADER_SIZE
    );
}

// ── NOD2 — Bitstream Road-to-Node Links ───────────────────────────────────────

/// Build the NOD2 section: one bitstream per road_def.
///
/// Each bitstream has N bits (one per vertex of the polyline):
/// - Bit = 1: vertex is a RouteNode (endpoint)
/// - Bit = 0: vertex is intermediate (geometry only)
///
/// For imgforge-cli: first and last vertices are always 1, intermediates are 0.
/// Bits are packed MSB-first. The stream is padded to the next byte boundary.
///
/// Returns `(nod2_data, nod2_offsets)` where `nod2_offsets[i]` = offset of
/// road_def i's entry within the returned data.
pub fn build_nod2_section(
    road_network: &RoadNetwork,
    polylines: &[MpPolyline],
) -> (Vec<u8>, Vec<u32>) {
    let mut data: Vec<u8> = Vec::new();
    let mut offsets: Vec<u32> = Vec::with_capacity(road_network.road_defs.len());

    for rd in &road_network.road_defs {
        let entry_offset = data.len() as u32;
        offsets.push(entry_offset);

        let n_vertices = if rd.polyline_idx < polylines.len() {
            polylines[rd.polyline_idx].coords.len()
        } else {
            2 // minimum: endpoints only
        };

        // Bits packed MSB-first: bit 0 = vertex 0 (MSB of first byte)
        let n_bytes = n_vertices.div_ceil(8);
        let mut bits = vec![0u8; n_bytes];

        // Vertex 0 is always a RouteNode (first endpoint)
        if n_vertices >= 1 {
            bits[0] |= 0x80; // bit 0 = MSB of byte 0
        }
        // Vertex N-1 is always a RouteNode (last endpoint)
        if n_vertices >= 2 {
            let last_bit = n_vertices - 1;
            let byte_idx = last_bit / 8;
            let bit_shift = 7 - (last_bit % 8); // MSB-first
            bits[byte_idx] |= 1u8 << bit_shift;
        }
        // All intermediate vertices remain 0

        data.extend_from_slice(&bits);
    }

    (data, offsets)
}

// ── NOD1 — RouteCenter Grouping ───────────────────────────────────────────────

/// Group node indices into RouteCenters of at most `MAX_NODES_PER_CENTER`.
///
/// Returns a list of groups; each group is a list of node indices into
/// `road_network.nodes`.
fn group_nodes_into_centers(nodes: &[crate::routing::RouteNode]) -> Vec<Vec<usize>> {
    if nodes.len() <= MAX_NODES_PER_CENTER {
        return vec![(0..nodes.len()).collect()];
    }
    split_nodes_recursive(nodes, &(0..nodes.len()).collect::<Vec<_>>())
}

/// Recursively split a group of node indices by median latitude until each
/// group has at most `MAX_NODES_PER_CENTER` nodes.
fn split_nodes_recursive(
    nodes: &[crate::routing::RouteNode],
    indices: &[usize],
) -> Vec<Vec<usize>> {
    if indices.len() <= MAX_NODES_PER_CENTER {
        return vec![indices.to_vec()];
    }

    // Sort by latitude and split at median
    let mut sorted = indices.to_vec();
    sorted.sort_by(|&a, &b| {
        nodes[a]
            .coord
            .0
            .partial_cmp(&nodes[b].coord.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mid = sorted.len() / 2;

    let mut result = split_nodes_recursive(nodes, &sorted[..mid]);
    result.extend(split_nodes_recursive(nodes, &sorted[mid..]));
    result
}

// ── NOD1 — RouteCenters with Table A arcs ─────────────────────────────────────

/// Encode NOD1 data from pre-computed center groups (internal helper).
///
/// Separated from `build_nod1_section` so that `NodWriter::build` can compute
/// centers once and reuse the result for both encoding and logging.
fn encode_nod1_from_centers(
    road_network: &RoadNetwork,
    centers: &[Vec<usize>],
    net_road_offsets: &[u32],
) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();

    for center_node_indices in centers {
        // Compute centroid in WGS84
        let count = center_node_indices.len() as f64;
        let lat_center = center_node_indices
            .iter()
            .map(|&ni| road_network.nodes[ni].coord.0)
            .sum::<f64>()
            / count;
        let lon_center = center_node_indices
            .iter()
            .map(|&ni| road_network.nodes[ni].coord.1)
            .sum::<f64>()
            / count;

        let garmin_lat_center = wgs84_to_garmin(lat_center);
        let garmin_lon_center = wgs84_to_garmin(lon_center);

        // RouteCenter header: lat(4) + lon(4) + tabB_offset(2) = 10 bytes
        data.extend_from_slice(&garmin_lat_center.to_le_bytes());
        data.extend_from_slice(&garmin_lon_center.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes()); // tabB_offset = 0 (no Table B)

        // Encode each node: header (9 bytes) + Table A arcs (5 bytes each)
        for &ni in center_node_indices {
            let node = &road_network.nodes[ni];

            let garmin_lat = wgs84_to_garmin(node.coord.0);
            let garmin_lon = wgs84_to_garmin(node.coord.1);

            // delta_lat = node_lat_garmin - center_lat_garmin (signed LE24)
            let delta_lat = garmin_lat.wrapping_sub(garmin_lat_center);
            let delta_lon = garmin_lon.wrapping_sub(garmin_lon_center);

            data.push((delta_lat & 0xFF) as u8);
            data.push(((delta_lat >> 8) & 0xFF) as u8);
            data.push(((delta_lat >> 16) & 0xFF) as u8);

            data.push((delta_lon & 0xFF) as u8);
            data.push(((delta_lon >> 8) & 0xFF) as u8);
            data.push(((delta_lon >> 16) & 0xFF) as u8);

            // flags = 0x00 (non-boundary node)
            data.push(0x00);
            // arc_count = number of outgoing Table A arcs
            let arc_count = node.arcs.len().min(255) as u8;
            data.push(arc_count);
            // tabB_count = 0 (single-tile: no cross-center arcs in Table B)
            data.push(0x00);

            // Table A arcs: 5 bytes each
            for &arc_id in node.arcs.iter().take(255) {
                let arc = &road_network.arcs[arc_id as usize];
                let rd = &road_network.road_defs[arc.road_def_idx];

                // tabAInfo = (speed & 0x07) | (oneway << 3) | ((road_class & 0x07) << 4) | (toll << 7)
                let tab_a_info: u8 = (rd.speed & 0x07)
                    | ((rd.one_way as u8) << 3)
                    | ((rd.road_class & 0x07) << 4)
                    | ((rd.toll as u8) << 7);
                data.push(tab_a_info);

                // bearing = round(bearing_degrees × 256.0 / 360.0) as u8
                let bearing = (arc.bearing_degrees * 256.0 / 360.0).round() as u8;
                data.push(bearing);

                // net_offset_le24: NET1 offset of the road_def (bits 0-21, flags bits 22-23 = 0)
                let net_off = if arc.road_def_idx < net_road_offsets.len() {
                    net_road_offsets[arc.road_def_idx] & 0x3F_FFFF
                } else {
                    0
                };
                data.push((net_off & 0xFF) as u8);
                data.push(((net_off >> 8) & 0xFF) as u8);
                data.push(((net_off >> 16) & 0xFF) as u8);
            }
        }
    }

    data
}

/// Build the NOD1 section: list of RouteCenters.
///
/// Each RouteCenter contains a geographic group of RouteNodes with their
/// outgoing arcs (Table A). Table B (cross-center arcs) is empty (single-tile).
///
/// `net_road_offsets[i]` = NET1-relative offset of road_def i (from NetBuildResult).
pub fn build_nod1_section(
    road_network: &RoadNetwork,
    net_road_offsets: &[u32],
) -> Vec<u8> {
    if road_network.nodes.is_empty() {
        return Vec::new();
    }
    let centers = group_nodes_into_centers(&road_network.nodes);
    encode_nod1_from_centers(road_network, &centers, net_road_offsets)
}

// ── NOD3 — Boundary Node Detection ───────────────────────────────────────────

/// Distance threshold (degrees) for boundary node detection.
const BOUNDARY_THRESHOLD: f64 = 1e-4;

/// Bounding box derived from polyline coordinates.
struct BoundingBox {
    min_lat: f64,
    max_lat: f64,
    min_lon: f64,
    max_lon: f64,
}

/// Compute bounding box from polyline coordinates.
/// Returns `None` if no coordinates are present.
fn compute_bbox(polylines: &[MpPolyline]) -> Option<BoundingBox> {
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;
    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut has_coords = false;
    for pl in polylines {
        for &(lat, lon) in &pl.coords {
            if lat < min_lat {
                min_lat = lat;
            }
            if lat > max_lat {
                max_lat = lat;
            }
            if lon < min_lon {
                min_lon = lon;
            }
            if lon > max_lon {
                max_lon = lon;
            }
            has_coords = true;
        }
    }
    if has_coords {
        Some(BoundingBox { min_lat, max_lat, min_lon, max_lon })
    } else {
        None
    }
}

/// Detect RouteNode indices that lie within `BOUNDARY_THRESHOLD` degrees of the tile bbox.
fn detect_boundary_nodes(
    nodes: &[crate::routing::RouteNode],
    bbox: &BoundingBox,
) -> Vec<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            let (lat, lon) = n.coord;
            (lat - bbox.max_lat).abs() < BOUNDARY_THRESHOLD
                || (lat - bbox.min_lat).abs() < BOUNDARY_THRESHOLD
                || (lon - bbox.max_lon).abs() < BOUNDARY_THRESHOLD
                || (lon - bbox.min_lon).abs() < BOUNDARY_THRESHOLD
        })
        .map(|(i, _)| i)
        .collect()
}

/// Build the NOD3 section: 6 bytes per boundary node.
///
/// Format per entry (wiki.openstreetmap.org/wiki/OSM_Map_On_Garmin/NOD_Subfile_Format#NOD3):
/// ```text
/// +0  3 bytes  Garmin lat semicircle (24-bit LE)
/// +3  3 bytes  Garmin lon semicircle (24-bit LE)
/// ```
fn build_nod3_section(
    nodes: &[crate::routing::RouteNode],
    boundary_indices: &[usize],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(boundary_indices.len() * 6);
    for &ni in boundary_indices {
        let (lat, lon) = nodes[ni].coord;
        let garmin_lat = wgs84_to_garmin(lat);
        let garmin_lon = wgs84_to_garmin(lon);
        // 3 bytes lat (24-bit LE)
        data.push((garmin_lat & 0xFF) as u8);
        data.push(((garmin_lat >> 8) & 0xFF) as u8);
        data.push(((garmin_lat >> 16) & 0xFF) as u8);
        // 3 bytes lon (24-bit LE)
        data.push((garmin_lon & 0xFF) as u8);
        data.push(((garmin_lon >> 8) & 0xFF) as u8);
        data.push(((garmin_lon >> 16) & 0xFF) as u8);
    }
    data
}

// ── NOD2 Patch Function ───────────────────────────────────────────────────────

/// Patch the 2-byte NOD2 placeholder offsets in the NET data buffer.
///
/// For each road_def i, writes `nod2_road_offsets[i]` as LE16 at the position
/// `nod2_patch_positions[i]` in `net_data`.
///
/// The indicator byte immediately before each placeholder (0x01) is left unchanged.
pub fn patch_nod2_offsets(
    net_data: &mut [u8],
    nod2_patch_positions: &[usize],
    nod2_road_offsets: &[u32],
) {
    debug_assert_eq!(
        nod2_patch_positions.len(),
        nod2_road_offsets.len(),
        "nod2_patch_positions and nod2_road_offsets must have identical lengths"
    );
    let n = nod2_patch_positions.len().min(nod2_road_offsets.len());
    for i in 0..n {
        let pos = nod2_patch_positions[i];
        debug_assert!(
            nod2_road_offsets[i] <= u16::MAX as u32,
            "NOD2 offset {} for road {} exceeds u16::MAX — NOD2 section too large for NET1 2-byte field",
            nod2_road_offsets[i],
            i
        );
        debug_assert!(
            pos + 1 < net_data.len(),
            "NOD2 patch position {} out of bounds (net_data.len() = {})",
            pos,
            net_data.len()
        );
        if pos + 1 < net_data.len() {
            let offset_u16 = nod2_road_offsets[i] as u16;
            net_data[pos] = (offset_u16 & 0xFF) as u8;
            net_data[pos + 1] = ((offset_u16 >> 8) & 0xFF) as u8;
        }
    }
}

// ── NodWriter ─────────────────────────────────────────────────────────────────

/// Builds the NOD subfile binary from the road network and NET1 offsets.
pub struct NodWriter;

impl NodWriter {
    /// Build the complete NOD subfile.
    ///
    /// # Arguments
    /// - `road_network`: The road network graph (from graph builder)
    /// - `net_road_offsets`: NET1-relative offsets for each road_def (from NetBuildResult)
    /// - `polylines`: Original polyline features (for vertex count in NOD2 and bbox computation)
    ///
    /// Boundary nodes are detected geometrically: RouteNodes within `BOUNDARY_THRESHOLD`
    /// (1e-4 degrees) of the tile bounding box are written to NOD3.
    pub fn build(
        road_network: &RoadNetwork,
        net_road_offsets: &[u32],
        polylines: &[MpPolyline],
    ) -> NodBuildResult {
        let (nod2_data, nod2_road_offsets) = build_nod2_section(road_network, polylines);

        // Detect boundary nodes from the polyline bounding box.
        let boundary_indices = if let Some(bbox) = compute_bbox(polylines) {
            detect_boundary_nodes(&road_network.nodes, &bbox)
        } else {
            Vec::new()
        };
        let nod3_data = build_nod3_section(&road_network.nodes, &boundary_indices);
        let boundary_node_count = boundary_indices.len();

        // Compute centers once — used for both NOD1 encoding and logging.
        let (n_centers, nod1_data) = if road_network.nodes.is_empty() {
            (0, Vec::new())
        } else {
            let centers = group_nodes_into_centers(&road_network.nodes);
            let n = centers.len();
            (n, encode_nod1_from_centers(road_network, &centers, net_road_offsets))
        };

        let nod1_len = nod1_data.len() as u32;
        let nod2_len = nod2_data.len() as u32;
        let nod3_len = nod3_data.len() as u32;

        let total = NOD_HEADER_SIZE + nod1_data.len() + nod2_data.len() + nod3_data.len();
        let mut data = Vec::with_capacity(total);
        write_nod_header(&mut data, nod1_len, nod2_len, nod3_len);
        data.extend_from_slice(&nod1_data);
        data.extend_from_slice(&nod2_data);
        data.extend_from_slice(&nod3_data);

        let total_arcs: usize = road_network.nodes.iter().map(|n| n.arcs.len()).sum();

        tracing::info!(
            route_centers = n_centers,
            route_nodes = road_network.nodes.len(),
            table_a_arcs = total_arcs,
            road_defs = road_network.road_defs.len(),
            nod1_size = nod1_len,
            nod2_size = nod2_len,
            nod3_size = nod3_len,
            boundary_nodes = boundary_node_count,
            total_size = data.len(),
            "NOD subfile built"
        );

        NodBuildResult {
            data,
            nod2_road_offsets,
            boundary_node_count,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{RoadDef, RoadNetwork, RouteArc, RouteNode};

    // ── Task 1: NOD Header ──────────────────────────────────────────────────

    #[test]
    fn test_nod_header_size() {
        let mut buf = Vec::new();
        write_nod_header(&mut buf, 0, 0, 0);
        assert_eq!(buf.len(), 48, "NOD header must be exactly 48 bytes");
    }

    #[test]
    fn test_nod_header_signature() {
        let mut buf = Vec::new();
        write_nod_header(&mut buf, 100, 50, 0);
        // Signature "GARMIN NOD" at offset 0x0B (11)
        assert_eq!(&buf[0x0B..0x15], b"GARMIN NOD");
    }

    #[test]
    fn test_nod_header_drive_on_right() {
        let mut buf = Vec::new();
        write_nod_header(&mut buf, 100, 50, 0);
        // drive_on_right at offset 0x2E
        assert_eq!(buf[0x2E], 0x01, "drive_on_right must be 0x01 (France)");
    }

    #[test]
    fn test_nod_header_offsets() {
        let nod1_len = 200u32;
        let nod2_len = 80u32;
        let nod3_len = 12u32;
        let mut buf = Vec::new();
        write_nod_header(&mut buf, nod1_len, nod2_len, nod3_len);

        // NOD1 offset at 0x15 = 48
        let nod1_off = u32::from_le_bytes([buf[0x15], buf[0x16], buf[0x17], buf[0x18]]);
        assert_eq!(nod1_off, 48, "NOD1 offset must be 48 (= header size)");

        // NOD1 length at 0x19
        let nod1_len_r = u32::from_le_bytes([buf[0x19], buf[0x1A], buf[0x1B], buf[0x1C]]);
        assert_eq!(nod1_len_r, 200);

        // node_size at 0x1D = 0x09
        assert_eq!(buf[0x1D], 0x09, "node_size must be 0x09");

        // NOD2 offset at 0x1E = 48 + nod1_len = 248
        let nod2_off = u32::from_le_bytes([buf[0x1E], buf[0x1F], buf[0x20], buf[0x21]]);
        assert_eq!(nod2_off, 48 + 200, "NOD2 offset = header + NOD1 len");

        // NOD2 length at 0x22
        let nod2_len_r = u32::from_le_bytes([buf[0x22], buf[0x23], buf[0x24], buf[0x25]]);
        assert_eq!(nod2_len_r, 80);

        // NOD3 offset at 0x26 = NOD2 offset + NOD2 len = 328
        let nod3_off = u32::from_le_bytes([buf[0x26], buf[0x27], buf[0x28], buf[0x29]]);
        assert_eq!(nod3_off, 248 + 80, "NOD3 offset = NOD2 offset + NOD2 len");

        // NOD3 length at 0x2A
        let nod3_len_r = u32::from_le_bytes([buf[0x2A], buf[0x2B], buf[0x2C], buf[0x2D]]);
        assert_eq!(nod3_len_r, 12, "NOD3 length reflects boundary node data");
    }

    // ── Task 2: NOD2 bitstream ──────────────────────────────────────────────

    fn make_polylines_for_nod2(vertex_counts: &[usize]) -> Vec<crate::parser::mp_types::MpPolyline> {
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
        use std::collections::HashMap;
        vertex_counts
            .iter()
            .enumerate()
            .map(|(i, &n)| MpPolyline {
                type_code: "0x02".to_string(),
                label: None,
                end_level: None,
                coords: (0..n).map(|j| (45.0 + j as f64 * 0.001, 5.0)).collect(),
                routing: Some(MpRoutingAttrs {
                    road_id: Some(i.to_string()),
                    route_param: Some("5,2,0,0,0,0,0,0,0,0,0,0".to_string()),
                    speed_type: None,
                    dir_indicator: Some(0),
                    roundabout: None,
                    max_height: None,
                    max_weight: None,
                    max_width: None,
                    max_length: None,
                }),
                other_fields: HashMap::new(),
            })
            .collect()
    }

    fn make_road_defs(n: usize) -> Vec<RoadDef> {
        (0..n)
            .map(|i| RoadDef {
                road_id: i as u32,
                polyline_idx: i,
                speed: 5,
                road_class: 2,
                one_way: false,
                toll: false,
                roundabout: false,
                access_mask: 0,
                label: None,
            })
            .collect()
    }

    #[test]
    fn test_nod2_2_vertices_one_byte() {
        // Route à 2 vertices: bits = 1 1 → byte 0x11000000 = 0xC0
        let polylines = make_polylines_for_nod2(&[2]);
        let road_defs = make_road_defs(1);
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs };
        let (data, offsets) = build_nod2_section(&network, &polylines);
        assert_eq!(data.len(), 1, "2 vertices → 1 byte");
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], 0);
        // Bits: vertex 0 (MSB) = 1, vertex 1 = 1 → 0b1100_0000 = 0xC0
        assert_eq!(data[0], 0xC0, "2-vertex bitstream must be 0xC0 (bits 7,6 set)");
    }

    #[test]
    fn test_nod2_3_vertices_one_byte() {
        // Route à 3 vertices: bits = 1 0 1 [0 0 0 0 0] → 0b1010_0000 = 0xA0
        let polylines = make_polylines_for_nod2(&[3]);
        let road_defs = make_road_defs(1);
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs };
        let (data, _offsets) = build_nod2_section(&network, &polylines);
        assert_eq!(data.len(), 1, "3 vertices → 1 byte");
        assert_eq!(data[0], 0xA0, "3-vertex bitstream = 0b1010_0000 = 0xA0");
    }

    #[test]
    fn test_nod2_9_vertices_two_bytes() {
        // Route à 9 vertices: bits = 1 0 0 0 0 0 0 0 | 1 0 0 0 0 0 0 0 → 0x80 0x80
        let polylines = make_polylines_for_nod2(&[9]);
        let road_defs = make_road_defs(1);
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs };
        let (data, _offsets) = build_nod2_section(&network, &polylines);
        assert_eq!(data.len(), 2, "9 vertices → 2 bytes");
        assert_eq!(data[0], 0x80, "byte 0: vertex 0 set = 0x80");
        assert_eq!(data[1], 0x80, "byte 1: vertex 8 (MSB) = 0x80");
    }

    #[test]
    fn test_nod2_5_vertices_one_byte() {
        // Route à 5 vertices: bits = 1 0 0 0 1 [0 0 0] → 0b1000_1000 = 0x88
        // (as per spec example in story)
        let polylines = make_polylines_for_nod2(&[5]);
        let road_defs = make_road_defs(1);
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs };
        let (data, _offsets) = build_nod2_section(&network, &polylines);
        assert_eq!(data.len(), 1, "5 vertices → 1 byte");
        assert_eq!(data[0], 0x88, "5-vertex bitstream = 0b1000_1000 = 0x88");
    }

    #[test]
    fn test_nod2_multiple_roads_offsets() {
        // 3 roads: 2 vertices (1B), 3 vertices (1B), 9 vertices (2B)
        let polylines = make_polylines_for_nod2(&[2, 3, 9]);
        let road_defs = make_road_defs(3);
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs };
        let (data, offsets) = build_nod2_section(&network, &polylines);
        assert_eq!(data.len(), 4, "total: 1+1+2 = 4 bytes");
        assert_eq!(offsets[0], 0, "road 0 starts at offset 0");
        assert_eq!(offsets[1], 1, "road 1 starts at offset 1");
        assert_eq!(offsets[2], 2, "road 2 starts at offset 2");
    }

    // ── Task 4: NOD1 RouteCenters ───────────────────────────────────────────

    fn make_two_node_network() -> RoadNetwork {
        // 2 nodes, 1 arc forward, road with speed=6, class=3, oneway=true, toll=true
        let nodes = vec![
            RouteNode { id: 0, coord: (45.0, 5.0), level: 0, arcs: vec![0] },
            RouteNode { id: 1, coord: (45.001, 5.001), level: 0, arcs: vec![] },
        ];
        let arcs = vec![RouteArc {
            id: 0,
            from_node: 0,
            to_node: 1,
            road_def_idx: 0,
            forward: true,
            length_meters: 150.0,
            bearing_degrees: 0.0, // North
        }];
        let road_defs = vec![RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 6,
            road_class: 3,
            one_way: true,
            toll: true,
            roundabout: false,
            access_mask: 0,
            label: Some("D1075".to_string()),
        }];
        RoadNetwork { nodes, arcs, road_defs }
    }

    #[test]
    fn test_nod1_two_nodes_size() {
        let network = make_two_node_network();
        let net_road_offsets = vec![0u32];
        let data = build_nod1_section(&network, &net_road_offsets);

        // 1 RouteCenter:
        //   header: 4 (lat) + 4 (lon) + 2 (tabB_offset) = 10 bytes
        //   node 0: 3 (delta_lat) + 3 (delta_lon) + 1 (flags) + 1 (arc_count=1) + 1 (tabB_count=0) = 9 bytes
        //           arc: 1 (tabAInfo) + 1 (bearing) + 3 (net_offset) = 5 bytes → total node 0 = 14 bytes
        //   node 1: 3 + 3 + 1 + 1 + 1 = 9 bytes (arc_count=0, no arcs)
        //   total: 10 + 14 + 9 = 33 bytes
        assert_eq!(data.len(), 33, "2-node, 1-arc network NOD1 must be 33 bytes");
    }

    #[test]
    fn test_nod1_tab_a_info_encoding() {
        // speed=6, class=3, oneway=true, toll=true → tabAInfo = 0xBE
        // 0x06 | (1<<3) | (0x03<<4) | (1<<7) = 0x06 | 0x08 | 0x30 | 0x80 = 0xBE
        let network = make_two_node_network();
        let net_road_offsets = vec![0u32];
        let data = build_nod1_section(&network, &net_road_offsets);

        // RouteCenter header: 10 bytes
        // Node 0 header: 9 bytes → arc starts at offset 10 + 9 = 19
        let arc_offset = 10 + 9;
        assert_eq!(data[arc_offset], 0xBE, "tabAInfo for speed=6/class=3/oneway/toll must be 0xBE");
    }

    #[test]
    fn test_nod1_bearing_north() {
        let network = make_two_node_network(); // bearing = 0.0°
        let net_road_offsets = vec![0u32];
        let data = build_nod1_section(&network, &net_road_offsets);

        let arc_offset = 10 + 9;
        let bearing = data[arc_offset + 1];
        // North = 0° → 0.0 × 256/360 = 0
        assert_eq!(bearing, 0, "bearing north (0°) must encode to 0");
    }

    #[test]
    fn test_nod1_tab_a_info_autoroute_peage() {
        // Autoroute à péage : speed=7, class=4, oneway=true, toll=true
        // tabAInfo = 0x07 | (1<<3) | (0x04<<4) | (1<<7) = 0x07 | 0x08 | 0x40 | 0x80 = 0xCF
        let nodes = vec![
            RouteNode { id: 0, coord: (45.0, 5.0), level: 0, arcs: vec![0] },
            RouteNode { id: 1, coord: (45.001, 5.001), level: 0, arcs: vec![] },
        ];
        let arcs = vec![RouteArc {
            id: 0,
            from_node: 0,
            to_node: 1,
            road_def_idx: 0,
            forward: true,
            length_meters: 150.0,
            bearing_degrees: 0.0,
        }];
        let road_defs = vec![RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 7,
            road_class: 4,
            one_way: true,
            toll: true,
            roundabout: false,
            access_mask: 0,
            label: None,
        }];
        let network = RoadNetwork { nodes, arcs, road_defs };
        let data = build_nod1_section(&network, &[0u32]);
        let arc_offset = 10 + 9;
        assert_eq!(
            data[arc_offset], 0xCF,
            "autoroute à péage: speed=7/class=4/oneway/toll → tabAInfo = 0xCF"
        );
    }

    #[test]
    fn test_nod1_empty_network() {
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs: vec![] };
        let data = build_nod1_section(&network, &[]);
        assert_eq!(data.len(), 0, "empty network → empty NOD1");
    }

    // ── NodWriter full build ────────────────────────────────────────────────

    #[test]
    fn test_nod_build_empty_network() {
        let network = RoadNetwork { nodes: vec![], arcs: vec![], road_defs: vec![] };
        let result = NodWriter::build(&network, &[], &[]);

        assert_eq!(result.data.len(), 48, "empty network → header only (48 bytes)");
        assert_eq!(&result.data[0x0B..0x15], b"GARMIN NOD", "signature must be 'GARMIN NOD'");
        assert_eq!(result.data[0x2E], 0x01, "drive_on_right must be 0x01");
        assert_eq!(result.nod2_road_offsets.len(), 0);
    }

    #[test]
    fn test_nod_build_two_node_network() {
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
        use std::collections::HashMap;
        let network = make_two_node_network();
        // Nodes at (45.0, 5.0) and (45.001, 5.001) — both at corners of bbox
        // → both are boundary nodes → NOD3 = 2 × 6 = 12 bytes
        let polylines = vec![MpPolyline {
            type_code: "0x02".to_string(),
            label: Some("D1075".to_string()),
            end_level: None,
            coords: vec![(45.0, 5.0), (45.001, 5.001)],
            routing: Some(MpRoutingAttrs {
                road_id: Some("1".to_string()),
                route_param: Some("6,3,1,1,0,0,0,0,0,0,0,0".to_string()),
                speed_type: None,
                dir_indicator: Some(1),
                roundabout: None,
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields: HashMap::new(),
        }];

        let result = NodWriter::build(&network, &[0u32], &polylines);

        // Header = 48, NOD1 = 33, NOD2 = 1, NOD3 = 12 (2 boundary nodes × 6 bytes)
        assert_eq!(result.data.len(), 48 + 33 + 1 + 12, "total size = 94 bytes");
        assert_eq!(result.nod2_road_offsets.len(), 1);
        assert_eq!(result.nod2_road_offsets[0], 0, "road 0 NOD2 offset = 0");
        assert_eq!(result.boundary_node_count, 2, "both nodes are boundary nodes");

        // Verify NOD1 offset in header
        let nod1_off = u32::from_le_bytes([
            result.data[0x15], result.data[0x16], result.data[0x17], result.data[0x18],
        ]);
        assert_eq!(nod1_off, 48, "NOD1 offset = 48");

        // Verify NOD2 offset in header
        let nod2_off = u32::from_le_bytes([
            result.data[0x1E], result.data[0x1F], result.data[0x20], result.data[0x21],
        ]);
        assert_eq!(nod2_off, 48 + 33, "NOD2 offset = 48 + nod1_len");

        // Verify NOD3 offset in header = NOD2 offset + NOD2 len
        let nod3_off = u32::from_le_bytes([
            result.data[0x26], result.data[0x27], result.data[0x28], result.data[0x29],
        ]);
        assert_eq!(nod3_off, 48 + 33 + 1, "NOD3 offset = header + NOD1 + NOD2");

        // Verify NOD3 length in header
        let nod3_len = u32::from_le_bytes([
            result.data[0x2A], result.data[0x2B], result.data[0x2C], result.data[0x2D],
        ]);
        assert_eq!(nod3_len, 12, "NOD3 length = 2 boundary nodes × 6 bytes");
    }

    // ── Task 5: NOD3 boundary nodes ─────────────────────────────────────────

    #[test]
    fn test_nod3_empty_for_no_boundary_nodes() {
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
        use std::collections::HashMap;
        // Large tile: bbox is [45.0, 46.0] × [5.0, 6.0]
        // Node at (45.5, 5.5) — well inside, no boundary detection
        let network = RoadNetwork {
            nodes: vec![
                RouteNode { id: 0, coord: (45.5, 5.5), level: 0, arcs: vec![] },
            ],
            arcs: vec![],
            road_defs: vec![],
        };
        let polylines = vec![MpPolyline {
            type_code: "0x02".to_string(),
            label: None,
            end_level: None,
            coords: vec![(45.0, 5.0), (46.0, 6.0)],
            routing: Some(MpRoutingAttrs {
                road_id: Some("1".to_string()),
                route_param: Some("5,2,0,0,0,0,0,0,0,0,0,0".to_string()),
                speed_type: None,
                dir_indicator: None,
                roundabout: None,
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields: HashMap::new(),
        }];
        let result = NodWriter::build(&network, &[], &polylines);
        assert_eq!(result.boundary_node_count, 0, "interior node → no boundary nodes");
        // NOD3 length in header should be 0
        let nod3_len = u32::from_le_bytes([
            result.data[0x2A], result.data[0x2B], result.data[0x2C], result.data[0x2D],
        ]);
        assert_eq!(nod3_len, 0, "NOD3 length must be 0 for no boundary nodes");
    }

    #[test]
    fn test_nod3_non_empty_for_boundary_node() {
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
        use std::collections::HashMap;
        // Node at (45.01, 5.5) — bbox max_lat = 45.01, distance = 0 < 1e-4 → boundary
        let network = RoadNetwork {
            nodes: vec![
                RouteNode { id: 0, coord: (45.01, 5.5), level: 0, arcs: vec![] },
            ],
            arcs: vec![],
            road_defs: vec![],
        };
        let polylines = vec![MpPolyline {
            type_code: "0x02".to_string(),
            label: None,
            end_level: None,
            coords: vec![(45.0, 5.0), (45.01, 6.0)],
            routing: Some(MpRoutingAttrs {
                road_id: Some("1".to_string()),
                route_param: Some("5,2,0,0,0,0,0,0,0,0,0,0".to_string()),
                speed_type: None,
                dir_indicator: None,
                roundabout: None,
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields: HashMap::new(),
        }];
        let result = NodWriter::build(&network, &[], &polylines);
        assert_eq!(result.boundary_node_count, 1, "node at max_lat → boundary node detected");
        // NOD3 length in header should be 6 (1 node × 6 bytes)
        let nod3_len = u32::from_le_bytes([
            result.data[0x2A], result.data[0x2B], result.data[0x2C], result.data[0x2D],
        ]);
        assert_eq!(nod3_len, 6, "NOD3 length = 1 boundary node × 6 bytes");
    }

    // ── Task 3: patch_nod2_offsets ──────────────────────────────────────────

    #[test]
    fn test_patch_nod2_offsets_basic() {
        let mut net_data = vec![0u8; 20];
        // Put placeholder at positions 5 and 10
        let patch_positions = vec![5usize, 10usize];
        let nod2_offsets = vec![0x1234u32, 0x5678u32];

        patch_nod2_offsets(&mut net_data, &patch_positions, &nod2_offsets);

        let p0 = u16::from_le_bytes([net_data[5], net_data[6]]);
        let p1 = u16::from_le_bytes([net_data[10], net_data[11]]);
        assert_eq!(p0, 0x1234, "road 0: patched LE16 = 0x1234");
        assert_eq!(p1, 0x5678, "road 1: patched LE16 = 0x5678");
    }

    #[test]
    fn test_patch_nod2_offsets_zero_remains_untouched_when_no_offsets() {
        let mut net_data = vec![0xFFu8; 20];
        patch_nod2_offsets(&mut net_data, &[], &[]);
        // Nothing patched, all bytes remain 0xFF
        assert!(net_data.iter().all(|&b| b == 0xFF));
    }
}
