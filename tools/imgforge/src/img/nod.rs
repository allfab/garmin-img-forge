// NODFile + RouteNode/Arc/Center, faithful to mkgmap NODFile.java, NODHeader.java

use super::common_header::{self, CommonHeader};

pub const NOD_HEADER_LEN: u16 = 127;
pub const NOD_ALIGNMENT: usize = 64; // Tables aligned to 1<<6

// NOD1Part subdivision constraints (mkgmap NOD1Part.java)
const NOD1_MAX_SIZE: i32 = (1 << 16) - 0x800; // 0xF800 = 63488 (bbox max dimension)
const NOD1_MAX_TABA: usize = 0x100 - 0x8;     // 0xF8 = 248 unique roads per center
const NOD1_MAX_TABB: usize = 0x100 - 0x2;     // 0xFE = 254 external dest nodes per center
const NOD1_MAX_NODES_SIZE: usize = 0x2000;    // 8192 bytes (14-bit signed NOD1 offsets)
const NOD1_MAX_DEPTH: usize = 48;              // mkgmap discards at depth > 48 (subdivideHelper)

/// A node in the routing network
#[derive(Debug, Clone)]
pub struct RouteNode {
    pub lat: i32, // 24-bit map units
    pub lon: i32,
    pub arcs: Vec<RouteArc>,
    pub is_boundary: bool,
    pub node_class: u8,
}

/// An arc connecting two route nodes
#[derive(Debug, Clone)]
pub struct RouteArc {
    pub dest_node_index: usize,
    pub road_def_index: usize,
    pub length_meters: u32,
    pub forward: bool,
    pub road_class: u8,
    pub speed: u8,
    pub access: u16,
    pub toll: bool,
    pub one_way: bool,
    /// Initial heading in Garmin direction units: round(bearing_degrees * 256 / 360) as i8
    pub initial_heading: i8,
}

/// A group of nearby route nodes — mkgmap RouteCenter.java
#[derive(Debug, Clone)]
pub struct RouteCenter {
    pub center_lat: i32, // semicircles
    pub center_lon: i32,
    pub nodes: Vec<usize>, // indices into the node array
}

// ── NOD1 constants (mkgmap RouteNode.java / RouteArc.java) ──────────────

/// Node flags
const F_LARGE_OFFSETS: u8 = 0x20;
const F_ARCS: u8 = 0x40;
const F_BOUNDARY: u8 = 0x08;

/// Arc flagA
const FLAG_HASNET: u8 = 0x80;
const FLAG_FORWARD: u8 = 0x40;

/// Arc flagB
const FLAG_LAST_LINK: u8 = 0x80;

/// Distance multiplier: raw = round(meters / 4.8)
const DISTANCE_MULT: f64 = 4.8;

/// Encode arc length to variable-length format (mkgmap RouteArc.encodeLength).
/// Returns (flagA_bits_3_5, length_data_bytes).
/// `have_curve` is always false for MVP.
pub fn encode_arc_length(raw_length: u32, have_curve: bool) -> (u8, Vec<u8>) {
    let threshold = if have_curve { 0x300 } else { 0x400 };

    if raw_length < threshold as u32 {
        // 10-bit format: flagA bits 3-5 encode top 2 bits, 1 data byte
        let mut flag_bits: u8 = 0;
        if have_curve {
            flag_bits |= 0x20; // curve bit in flagA
        }
        flag_bits |= ((raw_length >> 5) as u8) & 0x18; // top 2 bits of length → bits 3-4
        debug_assert!((flag_bits & 0x38) != 0x38, "10-bit encoding must not set all bits 3-5");
        let data = vec![(raw_length & 0xFF) as u8];
        (flag_bits, data)
    } else {
        // Extended format: flagA bits 3-5 all set
        let flag_bits: u8 = 0x38;
        if raw_length >= (1 << 14) {
            // 22-bit format: 3 data bytes
            let data = vec![
                0xC0 | (raw_length & 0x3F) as u8,
                ((raw_length >> 6) & 0xFF) as u8,
                ((raw_length >> 14) & 0xFF) as u8,
            ];
            (flag_bits, data)
        } else if have_curve {
            // 15-bit format: 2 data bytes
            let data = vec![
                (raw_length & 0x7F) as u8,
                ((raw_length >> 7) & 0xFF) as u8,
            ];
            (flag_bits, data)
        } else {
            // 14-bit format: 2 data bytes
            let data = vec![
                0x80 | (raw_length & 0x3F) as u8,
                ((raw_length >> 6) & 0xFF) as u8,
            ];
            (flag_bits, data)
        }
    }
}

/// Convert meters to raw arc length: round(meters / 4.8)
pub fn meters_to_raw_length(meters: u32) -> u32 {
    ((meters as f64) / DISTANCE_MULT).round() as u32
}

/// Table A entry: one per unique road in a route center (5 bytes each).
/// Layout: NET1_offset|access_top(3B LE) + tabAInfo(1B) + access_low(1B)
#[derive(Debug, Clone)]
pub struct TableAEntry {
    pub net1_offset: u32,
    pub road_class: u8,
    pub speed: u8,
    pub toll: bool,
    pub one_way: bool,
    pub access: u16,
}

impl TableAEntry {
    pub fn write(&self, buf: &mut Vec<u8>) {
        // Bytes 0-2: NET1 offset (22 bits) | access_top (bits 22-23)
        let access_top = ((self.access as u32) & 0xC000) << 8; // bits 14-15 → 22-23
        let val = (self.net1_offset & 0x3FFFFF) | access_top;
        let b = val.to_le_bytes();
        buf.push(b[0]);
        buf.push(b[1]);
        buf.push(b[2]);
        // Byte 3: tabAInfo = toll(0x80) | (road_class << 4) | oneway(0x08) | speed
        let mut info: u8 = (self.road_class << 4) | self.speed;
        if self.toll { info |= 0x80; }
        if self.one_way { info |= 0x08; }
        buf.push(info);
        // Byte 4: access low byte (top 2 bits are in bytes 0-2, bits 8-13 unused in format)
        buf.push(self.access as u8);
    }
}

/// Table B entry: one per external node in a route center (3 bytes each).
/// Layout: NOD1 offset (3B LE)
#[derive(Debug, Clone)]
pub struct TableBEntry {
    pub nod1_offset: u32,
}

impl TableBEntry {
    pub fn write(&self, buf: &mut Vec<u8>) {
        let b = self.nod1_offset.to_le_bytes();
        buf.push(b[0]);
        buf.push(b[1]);
        buf.push(b[2]);
    }
}

/// NOD2 per-road record info for the mkgmap format
pub struct Nod2RoadInfo {
    pub road_class: u8,
    pub speed: u8,
    pub num_route_nodes: u16,         // count of RouteNodes on this road
    pub starts_with_node: bool,       // true if first vertex is a RouteNode
    pub first_node_nod1_offset: u32,  // NOD1 offset of first RouteNode on this road
}

/// Write NOD2 records in mkgmap per-road format.
/// Returns (nod2_data, nod2_offset_per_road).
/// Each record: nod2Flags(1B) + NOD1_ptr(3B LE) + nbits(2B LE) + bitstream
///
/// mkgmap semantics: `nbits = nnodes + (startsWithNode ? 0 : 1)`.
/// All bits are set to 1 (no house number support), except bit 0 = 0 if !startsWithNode.
pub fn write_nod2_records(roads: &[Nod2RoadInfo]) -> (Vec<u8>, Vec<u32>) {
    let mut data = Vec::new();
    let mut offsets = Vec::with_capacity(roads.len());

    for road in roads {
        offsets.push(data.len() as u32);

        // nod2Flags: bit0=always | (road_class << 4) | (speed << 1)
        let nod2_flags: u8 = 0x01 | ((road.road_class & 0x07) << 4) | ((road.speed & 0x07) << 1);
        data.push(nod2_flags);

        // NOD1 offset of first RouteNode (3B LE)
        let ptr_b = road.first_node_nod1_offset.to_le_bytes();
        data.push(ptr_b[0]);
        data.push(ptr_b[1]);
        data.push(ptr_b[2]);

        // nbits = nnodes + (startsWithNode ? 0 : 1)
        let nbits: u16 = road.num_route_nodes + if road.starts_with_node { 0 } else { 1 };
        data.extend_from_slice(&nbits.to_le_bytes());

        // Build bit array: all 1s (no house numbers), with leading 0 if !startsWithNode
        let mut bits: Vec<bool> = Vec::with_capacity(nbits as usize);
        if !road.starts_with_node {
            bits.push(false);
        }
        for _ in 0..road.num_route_nodes {
            bits.push(true);
        }

        // Pack bits into bytes, LSB first
        for chunk in bits.chunks(8) {
            let mut byte: u8 = 0;
            for (j, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << j;
                }
            }
            data.push(byte);
        }
    }

    (data, offsets)
}

/// Info needed to backpatch an internal arc's dest pointer after all nodes in a center are written
struct ArcPatchInfo {
    /// Byte offset in NOD1 data where the 2-byte dest pointer lives (flagB byte)
    flagb_pos: usize,
    /// Byte offset in NOD1 data of the source node (for relative offset calc)
    src_node_offset: usize,
    /// Global node index of the destination node
    dest_node_index: usize,
    /// flagB value (last_link, etc.)
    flag_b: u8,
}

/// Info needed to backpatch a Table B entry's NOD1 offset after all centers are written
struct TableBPatchInfo {
    /// Byte offset in NOD1 data where the 3-byte NOD1 offset is
    byte_pos: usize,
    /// Global node index of the external node
    node_index: usize,
}

/// Upper bound on the serialized size of one arc (mkgmap RouteArc.boundSize).
/// MVP: no curves, so curvedat.length = 0.
/// Formula: 1 (flagA) + 1-2 (dest ptr) + 1 (indexA) + 1 (heading) + lendat.length = 5 + lendat
fn arc_bound_size(arc: &RouteArc) -> usize {
    let raw_len = meters_to_raw_length(arc.length_meters);
    let (_, len_data) = encode_arc_length(raw_len, false);
    5 + len_data.len()
}

/// Upper bound on the serialized size of one node (mkgmap RouteNode.boundSize).
/// Always assumes large offsets (4 bytes) as mkgmap does.
fn node_bound_size(node: &RouteNode) -> usize {
    let arcs_size: usize = node.arcs.iter().map(arc_bound_size).sum();
    6 + arcs_size // 1 (table ptr) + 1 (flags) + 4 (large offsets assumed) + arcs
}

/// Recursive bbox-split subdivision (mkgmap NOD1Part.subdivideHelper).
///
/// Splits `indices` into RouteCenters satisfying all 5 constraints.
/// Center coord = bbox midpoint (mkgmap Area.getCenter).
/// Nodes are sorted lon-major then lat within each center (mkgmap Coord.compareTo).
fn subdivide_nodes(
    nodes: &[RouteNode],
    indices: &[usize],
    depth: usize,
    centers: &mut Vec<RouteCenter>,
) {
    if indices.is_empty() {
        return;
    }

    // Compute bboxActual: extend with BBox(co) which adds +1 to max (mkgmap convention)
    let mut min_lat = i32::MAX;
    let mut max_lat = i32::MIN;
    let mut min_lon = i32::MAX;
    let mut max_lon = i32::MIN;
    for &i in indices {
        let n = &nodes[i];
        if n.lat < min_lat { min_lat = n.lat; }
        if n.lat + 1 > max_lat { max_lat = n.lat + 1; }
        if n.lon < min_lon { min_lon = n.lon; }
        if n.lon + 1 > max_lon { max_lon = n.lon + 1; }
    }

    let width = max_lon - min_lon;
    let height = max_lat - min_lat;
    let bbox_max_dim = width.max(height);

    // tabA: unique road_def_indexes across all arcs in this group
    let mut road_defs = std::collections::HashSet::new();
    // tabB: external dest node indices (dest not in this group).
    // index_set is rebuilt each recursion level — O(n·depth) total. Acceptable for BDTOPO tile sizes.
    let index_set: std::collections::HashSet<usize> = indices.iter().copied().collect();
    let mut dest_nodes_ext = std::collections::HashSet::new();
    let mut nodes_size: usize = 0;
    for &i in indices {
        nodes_size += node_bound_size(&nodes[i]);
        for arc in &nodes[i].arcs {
            road_defs.insert(arc.road_def_index);
            if !index_set.contains(&arc.dest_node_index) {
                dest_nodes_ext.insert(arc.dest_node_index);
            }
        }
    }

    // tabC is always 0 in MVP (no turn restrictions)
    let constraints_ok = bbox_max_dim < NOD1_MAX_SIZE
        && road_defs.len() < NOD1_MAX_TABA
        && dest_nodes_ext.len() < NOD1_MAX_TABB
        && nodes_size < NOD1_MAX_NODES_SIZE;

    if constraints_ok {
        // Sort nodes lon-major then lat (mkgmap Coord.compareTo)
        let mut sorted = indices.to_vec();
        sorted.sort_by(|&a, &b| {
            nodes[a].lon.cmp(&nodes[b].lon).then(nodes[a].lat.cmp(&nodes[b].lat))
        });
        centers.push(RouteCenter {
            center_lat: (min_lat + max_lat) / 2,
            center_lon: (min_lon + max_lon) / 2,
            nodes: sorted,
        });
        return;
    }

    if depth > NOD1_MAX_DEPTH {
        tracing::error!(
            "NOD1Part: subdivision depth {} exceeded, discarding {} nodes near lat={} lon={}",
            depth, indices.len(), min_lat, min_lon
        );
        return;
    }

    // Split on widest dimension of bboxActual (mkgmap uses bboxActual, not bbox)
    let (left, right): (Vec<usize>, Vec<usize>) = if width > height {
        // Longitude split: left gets lon < mid, right gets lon >= mid
        let mid = (min_lon + max_lon) / 2;
        (
            indices.iter().copied().filter(|&i| nodes[i].lon < mid).collect(),
            indices.iter().copied().filter(|&i| nodes[i].lon >= mid).collect(),
        )
    } else {
        // Latitude split: bottom gets lat < mid, top gets lat >= mid
        let mid = (min_lat + max_lat) / 2;
        (
            indices.iter().copied().filter(|&i| nodes[i].lat < mid).collect(),
            indices.iter().copied().filter(|&i| nodes[i].lat >= mid).collect(),
        )
    };

    subdivide_nodes(nodes, &left, depth + 1, centers);
    subdivide_nodes(nodes, &right, depth + 1, centers);
}

/// NOD file writer — mkgmap-faithful binary format
pub struct NodWriter {
    pub nodes: Vec<RouteNode>,
    pub centers: Vec<RouteCenter>,
    pub drive_on_left: bool,
    pub nod2_data: Vec<u8>,
    /// (byte_position_in_nod1 of Table A entry, road_def_index) for NET1 patching
    table_a_positions: Vec<(usize, usize)>,
    /// Serialized NOD1 data, built by prepare(), patched before final build
    nod1_data: Option<Vec<u8>>,
    /// Node offsets within NOD1 (node_index → byte offset in NOD1 section)
    node_offsets: Vec<u32>,
    /// Max node offset per class (for NOD header class boundaries)
    class_boundaries: [u32; 5],
}

impl NodWriter {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            centers: Vec::new(),
            drive_on_left: false,
            nod2_data: Vec::new(),
            table_a_positions: Vec::new(),
            nod1_data: None,
            node_offsets: Vec::new(),
            // mkgmap initializes with Integer.MAX_VALUE (Java signed int = 0x7FFFFFFF).
            // Using u32::MAX (0xFFFFFFFF) here caused the written delta classBoundaries[1] − [0]
            // to be 0xFFFFFFFF, which the Garmin firmware reads as int32 = -1 and rejects the
            // routing graph. Match mkgmap exactly.
            class_boundaries: [i32::MAX as u32; 5],
        }
    }

    pub fn add_node(&mut self, node: RouteNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// Group nodes into RouteCenters using mkgmap bbox-split subdivision (NOD1Part.subdivideHelper).
    ///
    /// mkgmap-faithful (RoadNetwork.splitCenters:240-265) : les nodes sont
    /// regroupés par class **avant** subdivision, dans l'ordre arc-less,
    /// class 0, 1, 2, 3, 4. Les centers résultants sont donc écrits dans
    /// cet ordre, condition nécessaire pour que `class_boundaries[c]` =
    /// position du premier center contenant un node de class > c (sinon
    /// le tracking `min(center_start)` collapse tout à 0 si un node de
    /// haute class apparaît dans le 1er center).
    pub fn build_centers(&mut self) {
        self.centers.clear();
        if self.nodes.is_empty() {
            return;
        }
        let mut centers = Vec::new();

        // 1) Nodes sans arcs (mkgmap RoadNetwork.java:252)
        let arc_less: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| self.nodes[i].arcs.is_empty())
            .collect();
        if !arc_less.is_empty() {
            subdivide_nodes(&self.nodes, &arc_less, 0, &mut centers);
        }

        // 2) Nodes groupés par node_class (0..=4) (mkgmap RoadNetwork.java:257-265)
        for group in 0u8..=4u8 {
            let group_nodes: Vec<usize> = (0..self.nodes.len())
                .filter(|&i| !self.nodes[i].arcs.is_empty() && self.nodes[i].node_class == group)
                .collect();
            if !group_nodes.is_empty() {
                subdivide_nodes(&self.nodes, &group_nodes, 0, &mut centers);
            }
        }

        tracing::debug!("NOD1Part: {} nodes → {} RouteCenters", self.nodes.len(), centers.len());
        self.centers = centers;
    }

    /// Set NOD2 data (populated by caller)
    pub fn set_nod2_data(&mut self, data: Vec<u8>) {
        self.nod2_data = data;
    }

    /// Patch NET1 offsets into Table A entries after NET subfile is built.
    /// `net1_offsets` maps road_def_index → NET1 byte offset.
    pub fn patch_net1_offsets(&mut self, net1_offsets: &[u32]) {
        let nod1 = self.nod1_data.as_mut()
            .expect("patch_net1_offsets called before prepare()");

        for &(byte_pos, road_idx) in &self.table_a_positions {
            if road_idx >= net1_offsets.len() {
                tracing::warn!("NOD patch: road_def_index {} out of range, Table A at byte {} unpatched",
                    road_idx, byte_pos);
                continue;
            }
            let offset = net1_offsets[road_idx];
            if offset > 0x3FFFFF {
                tracing::warn!("NOD patch: NET1 offset 0x{:X} for road {} exceeds 22-bit limit, truncated",
                    offset, road_idx);
            }
            // Read existing 3 bytes (contain access top bits in bits 22-23)
            let existing = (nod1[byte_pos] as u32)
                | ((nod1[byte_pos + 1] as u32) << 8)
                | ((nod1[byte_pos + 2] as u32) << 16);
            let patched = (offset & 0x3FFFFF) | existing;
            let b = patched.to_le_bytes();
            nod1[byte_pos] = b[0];
            nod1[byte_pos + 1] = b[1];
            nod1[byte_pos + 2] = b[2];
        }
    }

    /// Build NOD1 data and store internally for later patching.
    /// Node offsets within NOD1, available after prepare().
    pub fn node_offsets(&self) -> &[u32] {
        &self.node_offsets
    }

    /// Table A positions: (byte_offset, road_def_index), available after prepare().
    pub fn table_a_positions(&self) -> &[(usize, usize)] {
        &self.table_a_positions
    }

    pub fn prepare(&mut self) {
        if self.centers.is_empty() {
            self.build_centers();
        }
        let (nod1_data, node_offsets, table_a_pos) = self.build_nod1();
        self.nod1_data = Some(nod1_data);
        self.node_offsets = node_offsets;
        self.table_a_positions = table_a_pos;
    }

    /// Build complete NOD subfile (header + NOD1 + NOD2 + NOD3 + NOD4).
    pub fn build(&mut self) -> Vec<u8> {
        if self.nod1_data.is_none() {
            self.prepare();
        }

        let mut buf = Vec::new();
        let common = CommonHeader::new(NOD_HEADER_LEN, "GARMIN NOD");
        common.write(&mut buf);

        let nod1_data = self.nod1_data.take().unwrap();
        let nod2_data = self.nod2_data.clone();
        let nod3_data = self.build_nod3(&self.node_offsets);
        let nod4_data = self.build_nod4(&self.node_offsets);

        // ── Section descriptors ──
        // NOD1 section @0x15
        let nod1_offset = NOD_HEADER_LEN as u32;
        let nod1_size = nod1_data.len() as u32;
        common_header::write_section(&mut buf, nod1_offset, nod1_size);
        // Flags = 0x0227: bit 0 (always) | bit 1 (restrictions) | bits 5-7 (DISTANCE_MULT_SHIFT=1)
        // | bit 9 (drive-on-left conditional)
        let mut nod_flags: u32 = 0x0227;
        if self.drive_on_left { nod_flags |= 0x0100; }
        let fl = nod_flags.to_le_bytes();
        buf.push(fl[0]); // @0x1D
        buf.push(fl[1]); // @0x1E
        buf.push(fl[2]); // @0x1F
        buf.push(fl[3]); // @0x20
        buf.push(0x06);  // @0x21: alignment shift (1<<6 = 64)
        buf.push(0x00);  // @0x22: pointer multiplier
        buf.extend_from_slice(&0x0005u16.to_le_bytes()); // @0x23-0x24: Table A record size

        // NOD2 section @0x25
        let nod2_offset = nod1_offset + nod1_size;
        common_header::write_section(&mut buf, nod2_offset, nod2_data.len() as u32);
        buf.extend_from_slice(&0u32.to_le_bytes()); // @0x2D-0x30: reserved

        // NOD3 section @0x31 — boundary nodes
        // mkgmap layout: writeSectionInfo (offset+size+itemSize = 10B) + put4(2) (4B)
        let nod3_offset = nod2_offset + nod2_data.len() as u32;
        common_header::write_section(&mut buf, nod3_offset, nod3_data.len() as u32);
        buf.extend_from_slice(&0x0009u16.to_le_bytes()); // @0x39: itemSize = 9 (boundary record)
        buf.extend_from_slice(&0x00000002u32.to_le_bytes()); // @0x3B: mystery field (always 2 in mkgmap)

        // NOD4 section @0x3F — high-class boundary nodes
        // mkgmap layout: writeSectionInfo (offset+size only = 8B), no itemSize
        let nod4_offset = nod3_offset + nod3_data.len() as u32;
        common_header::write_section(&mut buf, nod4_offset, nod4_data.len() as u32);

        // Clamp class_boundaries to nodes_len (mkgmap NODFile.writeNodes:103-106).
        // Unused class slots are initialized to i32::MAX and never updated by the
        // min-semantics pass in build_nod1; without this clamp, the delta encoding
        // produces wrap-around values that the firmware reads as nonsensical class
        // ranges → BaseCamp routing hangs on "Initialisation de l'itinéraire".
        for i in 0..5 {
            if self.class_boundaries[i] > nod1_size {
                self.class_boundaries[i] = nod1_size;
            }
        }

        // Class boundaries: 5 × u32 (first absolute, then deltas)
        let mut prev: u32 = 0;
        for i in 0..5 {
            if i == 0 {
                buf.extend_from_slice(&self.class_boundaries[i].to_le_bytes());
                prev = self.class_boundaries[i];
            } else {
                let delta = self.class_boundaries[i].wrapping_sub(prev);
                buf.extend_from_slice(&delta.to_le_bytes());
                prev = self.class_boundaries[i];
            }
        }

        // Pad header
        common_header::pad_to(&mut buf, NOD_HEADER_LEN as usize);

        // Section data
        buf.extend_from_slice(&nod1_data);
        buf.extend_from_slice(&nod2_data);
        buf.extend_from_slice(&nod3_data);
        buf.extend_from_slice(&nod4_data);

        buf
    }

    /// Build NOD1 — mkgmap-faithful route center format.
    /// Returns (data, node_offsets, table_a_positions_for_net1_patching).
    fn build_nod1(&mut self) -> (Vec<u8>, Vec<u32>, Vec<(usize, usize)>) {
        use std::collections::{HashMap, HashSet};

        let mut data = Vec::new();
        let mut node_offsets = vec![0u32; self.nodes.len()];
        let mut table_a_positions: Vec<(usize, usize)> = Vec::new();
        let mut table_b_patches: Vec<TableBPatchInfo> = Vec::new();

        for center in &self.centers {
            let center_start = data.len();

            // ── Phase 0: Determine internal/external nodes ──
            let center_node_set: HashSet<usize> = center.nodes.iter().copied().collect();

            // Build Table B index: external dest nodes → position in Table B
            let mut table_b_index: HashMap<usize, u8> = HashMap::new();
            let mut table_b_nodes: Vec<usize> = Vec::new();
            for &ni in &center.nodes {
                for arc in &self.nodes[ni].arcs {
                    if !center_node_set.contains(&arc.dest_node_index)
                        && !table_b_index.contains_key(&arc.dest_node_index)
                    {
                        let idx = table_b_nodes.len() as u8;
                        table_b_index.insert(arc.dest_node_index, idx);
                        table_b_nodes.push(arc.dest_node_index);
                    }
                }
            }

            // ── Phase 1: Write nodes + arcs ──
            struct NodeInfo {
                offset: usize,
                byte0_pos: usize,
            }
            let mut node_infos: Vec<NodeInfo> = Vec::new();
            let mut arc_patches: Vec<ArcPatchInfo> = Vec::new();

            // Build Table A index: unique road_def_index → position in table
            let mut table_a_index: HashMap<usize, u8> = HashMap::new();
            let mut table_a_roads: Vec<usize> = Vec::new();
            for &ni in &center.nodes {
                for arc in &self.nodes[ni].arcs {
                    if !table_a_index.contains_key(&arc.road_def_index) {
                        let idx = table_a_roads.len() as u8;
                        table_a_index.insert(arc.road_def_index, idx);
                        table_a_roads.push(arc.road_def_index);
                    }
                }
            }

            // Write each node
            for &node_idx in &center.nodes {
                let node_start = data.len();
                node_offsets[node_idx] = node_start as u32;

                // Track class boundaries: mkgmap uses center_start (min semantics)
                let nc = (self.nodes[node_idx].node_class as usize).min(4);
                if nc > 0 {
                    for c in (0..nc).rev() {
                        if (center_start as u32) < self.class_boundaries[c] {
                            self.class_boundaries[c] = center_start as u32;
                        }
                    }
                }

                let node = &self.nodes[node_idx];

                // Byte 0: placeholder (backpatched with calcLowByte later)
                let byte0_pos = data.len();
                data.push(0x00);

                // Byte 1: flags
                // Deltas in 24-bit map units (same units as center)
                let delta_lat = node.lat - center.center_lat;
                let delta_lon = node.lon - center.center_lon;
                let delta_large = delta_lat.unsigned_abs() > 0x7FF || delta_lon.unsigned_abs() > 0x7FF;
                let mut flags: u8 = node.node_class & 0x07;
                if !node.arcs.is_empty() { flags |= F_ARCS; }
                if delta_large { flags |= F_LARGE_OFFSETS; }
                if node.is_boundary { flags |= F_BOUNDARY; }
                // mkgmap: avoid byte0=0 + flags=0 for isolated class-0 nodes
                if flags == 0 { flags |= F_LARGE_OFFSETS; }
                data.push(flags);

                // Coords: 3B (12-bit packed) or 4B (16-bit packed), LE
                let use_large = (flags & F_LARGE_OFFSETS) != 0;
                if use_large {
                    let packed = ((delta_lat as i32) << 16) | ((delta_lon as i32) & 0xFFFF);
                    data.extend_from_slice(&packed.to_le_bytes());
                } else {
                    let packed = ((delta_lat & 0xFFF) << 12) | (delta_lon & 0xFFF);
                    let b = packed.to_le_bytes();
                    data.push(b[0]);
                    data.push(b[1]);
                    data.push(b[2]);
                }

                // Write arcs
                let num_arcs = node.arcs.len();
                let mut last_index_a: Option<u8> = None;
                for (arc_i, arc) in node.arcs.iter().enumerate() {
                    let is_last = arc_i == num_arcs - 1;
                    let is_internal = center_node_set.contains(&arc.dest_node_index);
                    let raw_len = meters_to_raw_length(arc.length_meters);
                    let (len_flag_bits, len_data) = encode_arc_length(raw_len, false);

                    // flagA: dest_class(0-2) | length_bits(3-5) | forward(6) | hasnet(7)
                    // dest_class = min(arc.road_class, dest_node.node_class)
                    // (mkgmap RouteArc.getArcDestClass — RouteArc.java:235-237)
                    let dest_node_class = self.nodes[arc.dest_node_index].node_class;
                    let dest_class = arc.road_class.min(dest_node_class);
                    let mut flag_a: u8 = dest_class & 0x07;
                    flag_a |= len_flag_bits;
                    if arc.forward { flag_a |= FLAG_FORWARD; }
                    // FLAG_HASNET (mkgmap RouteArc.java:237-246):
                    // First arc: only if useCompactDirs (false for MVP)
                    // Subsequent arcs: only if road_def changed
                    if arc_i > 0 {
                        let prev_arc = &node.arcs[arc_i - 1];
                        if arc.road_def_index != prev_arc.road_def_index {
                            flag_a |= FLAG_HASNET;
                        }
                    }
                    data.push(flag_a);

                    // Dest pointer — format depends on internal vs external
                    let mut flag_b: u8 = 0;
                    if is_last { flag_b |= FLAG_LAST_LINK; }

                    if is_internal {
                        // Internal: 2 bytes (flagB + placeholder), backpatched later
                        let flagb_pos = data.len();
                        data.push(flag_b);
                        data.push(0x00);
                        arc_patches.push(ArcPatchInfo {
                            flagb_pos,
                            src_node_offset: node_start,
                            dest_node_index: arc.dest_node_index,
                            flag_b,
                        });
                    } else {
                        // External (mkgmap RouteArc.java:266-273):
                        // flagB has bit 6 set (external marker)
                        // Low 6 bits = Table B index (or 0x3F + extra byte)
                        let index_b = *table_b_index.get(&arc.dest_node_index).unwrap();
                        flag_b |= 0x40; // external flag
                        if index_b >= 0x3F {
                            data.push(flag_b | 0x3F);
                            data.push(index_b);
                        } else {
                            data.push(flag_b | index_b);
                        }
                    }

                    // indexA (conditional: first arc or different from previous)
                    let index_a = *table_a_index.get(&arc.road_def_index).unwrap();
                    if last_index_a != Some(index_a) {
                        data.push(index_a);
                        last_index_a = Some(index_a);
                    }

                    // Length data (1-3 bytes)
                    data.extend_from_slice(&len_data);

                    // Heading (conditional: first arc, or indexA/forward changed)
                    if arc_i == 0 {
                        data.push(arc.initial_heading as u8);
                    } else {
                        let prev_arc = &node.arcs[arc_i - 1];
                        let prev_idx_a = *table_a_index.get(&prev_arc.road_def_index).unwrap();
                        if index_a != prev_idx_a || arc.forward != prev_arc.forward {
                            data.push(arc.initial_heading as u8);
                        }
                    }
                }

                node_infos.push(NodeInfo {
                    offset: node_start,
                    byte0_pos,
                });
            }

            // ── Phase 2: Pad to NEXT 64-byte boundary ──
            let tables_offset = (data.len() + NOD_ALIGNMENT) & !(NOD_ALIGNMENT - 1);
            while data.len() < tables_offset {
                data.push(0x00);
            }

            // ── Phase 3: Backpatch byte 0 of each node (calcLowByte) ──
            for info in &node_infos {
                let low = calc_low_byte(info.offset, tables_offset);
                data[info.byte0_pos] = low;
            }

            // ── Phase 4: Backpatch internal arc dest pointers ──
            // Internal arc format: 2 bytes = flagB (high) | signed offset (low 14 bits).
            // flagB bits 0-5 are always 0 for internal arcs (bit 6 = external flag,
            // bit 7 = last_link), so the OR with diff bits 8-13 is safe.
            for patch in &arc_patches {
                let dest_offset = node_offsets[patch.dest_node_index] as usize;
                let diff = (dest_offset as i32) - (patch.src_node_offset as i32);
                let val = ((patch.flag_b as u16) << 8) | ((diff as u16) & 0x3FFF);
                data[patch.flagb_pos] = (val >> 8) as u8;
                data[patch.flagb_pos + 1] = (val & 0xFF) as u8;
            }

            // ── Phase 5: Tables header ──
            data.push(0x00); // tabC_format (no Table C)
            let lon_b = center.center_lon.to_le_bytes();
            data.push(lon_b[0]);
            data.push(lon_b[1]);
            data.push(lon_b[2]);
            let lat_b = center.center_lat.to_le_bytes();
            data.push(lat_b[0]);
            data.push(lat_b[1]);
            data.push(lat_b[2]);
            data.push(table_a_roads.len() as u8);
            data.push(table_b_nodes.len() as u8);

            // ── Phase 6: Table A (5B per road) ──
            for &road_idx in &table_a_roads {
                let ta_pos = data.len();
                let arc = center.nodes.iter()
                    .flat_map(|&ni| self.nodes[ni].arcs.iter())
                    .find(|a| a.road_def_index == road_idx)
                    .unwrap();
                let entry = TableAEntry {
                    net1_offset: 0, // patched later by patch_net1_offsets()
                    road_class: arc.road_class,
                    speed: arc.speed,
                    toll: arc.toll,
                    one_way: arc.one_way,
                    access: arc.access,
                };
                entry.write(&mut data);
                table_a_positions.push((ta_pos, road_idx));
            }

            // ── Phase 7: Table B (3B per external node) — placeholders ──
            for &ext_node_idx in &table_b_nodes {
                let pos = data.len();
                data.push(0x00);
                data.push(0x00);
                data.push(0x00);
                table_b_patches.push(TableBPatchInfo {
                    byte_pos: pos,
                    node_index: ext_node_idx,
                });
            }
        }

        // ── Final: Backpatch all Table B entries with NOD1 offsets ──
        for patch in &table_b_patches {
            let off = node_offsets[patch.node_index];
            let b = off.to_le_bytes();
            data[patch.byte_pos] = b[0];
            data[patch.byte_pos + 1] = b[1];
            data[patch.byte_pos + 2] = b[2];
        }

        (data, node_offsets, table_a_positions)
    }

    fn build_nod3(&self, node_offsets: &[u32]) -> Vec<u8> {
        let mut data = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if node.is_boundary {
                let lon_b = node.lon.to_le_bytes();
                data.push(lon_b[0]);
                data.push(lon_b[1]);
                data.push(lon_b[2]);
                let lat_b = node.lat.to_le_bytes();
                data.push(lat_b[0]);
                data.push(lat_b[1]);
                data.push(lat_b[2]);
                let off = if i < node_offsets.len() { node_offsets[i] } else { 0 };
                let ob = off.to_le_bytes();
                data.push(ob[0]);
                data.push(ob[1]);
                data.push(ob[2]);
            }
        }
        data
    }

    fn build_nod4(&self, node_offsets: &[u32]) -> Vec<u8> {
        let mut data = Vec::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if node.is_boundary && node.node_class > 0 {
                let lon_b = node.lon.to_le_bytes();
                data.push(lon_b[0]);
                data.push(lon_b[1]);
                data.push(lon_b[2]);
                let lat_b = node.lat.to_le_bytes();
                data.push(lat_b[0]);
                data.push(lat_b[1]);
                data.push(lat_b[2]);
                let off = if i < node_offsets.len() { node_offsets[i] } else { 0 };
                let ob = off.to_le_bytes();
                data.push(ob[0]);
                data.push(ob[1]);
                data.push(ob[2]);
            }
        }
        data
    }
}

/// calcLowByte — mkgmap RouteCenter.java:159-171
/// Formula: (tables_offset >> 6) - (node_offset >> 6) - 1
pub fn calc_low_byte(node_offset: usize, tables_offset: usize) -> u8 {
    let align = 6; // NOD_ALIGNMENT = 1 << 6
    let low = (tables_offset >> align) as i32 - (node_offset >> align) as i32 - 1;
    debug_assert!(low >= 0 && low < 256, "calcLowByte out of range: {}", low);
    low as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nod_header() {
        let mut nod = NodWriter::new();
        let data = nod.build();
        assert_eq!(&data[2..12], b"GARMIN NOD");
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, NOD_HEADER_LEN);
    }

    #[test]
    fn test_nod_empty() {
        let mut nod = NodWriter::new();
        let data = nod.build();
        assert!(data.len() >= NOD_HEADER_LEN as usize);
    }

    #[test]
    fn test_nod_with_nodes() {
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000,
            lon: 200000,
            arcs: Vec::new(),
            is_boundary: false,
            node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100,
            lon: 200100,
            arcs: Vec::new(),
            is_boundary: false,
            node_class: 0,
        });
        let data = nod.build();
        assert!(data.len() > NOD_HEADER_LEN as usize);
    }

    #[test]
    fn test_nod3_boundary_nodes() {
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000,
            lon: 200000,
            arcs: Vec::new(),
            is_boundary: true,
            node_class: 0,
        });
        let data = nod.build();
        // Should have NOD3 data (9 bytes per boundary node)
        assert!(data.len() >= NOD_HEADER_LEN as usize + 9);
    }

    #[test]
    fn test_alignment() {
        assert_eq!(NOD_ALIGNMENT, 64);
    }

    /// Helper: find Table A entry position in NOD1 data after prepare().
    fn find_table_a_start(nod: &NodWriter) -> usize {
        nod.table_a_positions()[0].0
    }

    /// Table A byte 3 (tabAInfo) encodes toll and oneway
    #[test]
    fn test_table_a_encoding_toll_oneway() {
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1,
                road_def_index: 0,
                length_meters: 100,
                forward: true,
                road_class: 2,
                speed: 5,
                access: 0,
                toll: true,
                one_way: true,
                initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        let ta_start = find_table_a_start(&nod);
        // Byte 3 of Table A entry = tabAInfo
        let tab_a_info = nod1[ta_start + 3];
        // Expected: toll(0x80) | (2 << 4) | oneway(0x08) | 5 = 0xAD
        assert_eq!(tab_a_info, 0xAD);
    }

    /// Table A byte 4 encodes access flags low byte
    #[test]
    fn test_table_a_encoding_access_low() {
        use crate::img::net::{NO_CAR, NO_FOOT, NO_BIKE};
        let access = NO_CAR | NO_FOOT | NO_BIKE; // 0x0031
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 0, speed: 0,
                access, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        let ta_start = find_table_a_start(&nod);
        assert_eq!(nod1[ta_start + 4], 0x31);
    }

    /// Table A bytes 0-2 encode access flags high bits
    #[test]
    fn test_table_a_encoding_access_high() {
        use crate::img::net::{NO_EMERGENCY, NO_CAR};
        let access = NO_EMERGENCY | NO_CAR; // 0x8001
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 0, speed: 0,
                access, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        let ta_start = find_table_a_start(&nod);
        // access_top = (0x8000 << 8) = 0x800000 in bits 22-23
        assert_eq!(nod1[ta_start], 0x00);
        assert_eq!(nod1[ta_start + 1], 0x00);
        assert_eq!(nod1[ta_start + 2], 0x80);
    }

    /// patch_net1_offsets correctly patches NET1 offsets into Table A entries
    #[test]
    fn test_patch_net1_offsets() {
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 0, speed: 0,
                access: 0, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();

        // Patch: road 1 at offset 42
        nod.patch_net1_offsets(&[0, 42]);

        let nod1 = nod.nod1_data.as_ref().unwrap();
        let ta_start = find_table_a_start(&nod);
        let patched = (nod1[ta_start] as u32)
            | ((nod1[ta_start + 1] as u32) << 8)
            | ((nod1[ta_start + 2] as u32) << 16);
        assert_eq!(patched & 0x3FFFFF, 42);
    }

    /// R8: Full pipeline test — prepare → patch with real NetWriter offsets → build
    #[test]
    fn test_full_prepare_patch_build_pipeline() {
        use crate::img::net::{NetWriter, RoadDef, NO_CAR};

        // Build NET with 2 roads (different sizes to get distinct offsets)
        let mut net = NetWriter::new();
        let mut rd0 = RoadDef::new();
        rd0.label_offsets.push(10);
        rd0.road_length_meters = 100;
        net.add_road(rd0);

        let mut rd1 = RoadDef::new();
        rd1.label_offsets.push(20);
        rd1.road_length_meters = 200;
        rd1.access_flags = NO_CAR;
        net.add_road(rd1);

        let _net_data = net.build();
        assert_eq!(net.net1_offsets().len(), 2);
        let expected_offset_1 = net.net1_offsets()[1];
        assert!(expected_offset_1 > 0);

        // Build NOD with an arc referencing road 1
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 2, speed: 3,
                access: 0, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });

        nod.prepare();
        nod.patch_net1_offsets(&net.net1_offsets());
        let nod_data = nod.build();

        // NOD file should be valid (header present)
        assert_eq!(&nod_data[2..12], b"GARMIN NOD");
        assert!(nod_data.len() > NOD_HEADER_LEN as usize);
    }

    // ── T11: New unit tests for mkgmap-faithful NOD1 format ──

    #[test]
    fn test_encode_arc_length_short() {
        // length < 0x300 → 10-bit format, 1 data byte
        let (flag_bits, data) = encode_arc_length(100, false);
        assert_eq!(data.len(), 1);
        assert_ne!(flag_bits & 0x38, 0x38, "10-bit format must not set all bits 3-5");
        // Reconstruct: top 2 bits from flag_bits[3-4], low 8 bits from data[0]
        let top = ((flag_bits & 0x18) as u32) << 5;
        let reconstructed = top | (data[0] as u32);
        assert_eq!(reconstructed, 100);
    }

    #[test]
    fn test_encode_arc_length_medium() {
        // For no-curve: threshold=0x400. Use length=2000 (>= 0x400, < 16384) → 14-bit format
        let (flag_bits, data) = encode_arc_length(2000, false);
        assert_eq!(flag_bits & 0x38, 0x38, "Extended format has all bits 3-5 set");
        assert_eq!(data.len(), 2);
        // 14-bit: data[0] = 0x80 | (len & 0x3F), data[1] = (len >> 6)
        assert!(data[0] & 0x80 != 0, "14-bit format has 0x80 marker");
        let reconstructed = ((data[0] & 0x3F) as u32) | ((data[1] as u32) << 6);
        assert_eq!(reconstructed, 2000);
    }

    #[test]
    fn test_encode_arc_length_long() {
        // length >= 16384 → 22-bit format, 3 data bytes
        let (flag_bits, data) = encode_arc_length(20000, false);
        assert_eq!(flag_bits & 0x38, 0x38);
        assert_eq!(data.len(), 3);
        assert_eq!(data[0] & 0xC0, 0xC0, "22-bit format has 0xC0 marker");
        let reconstructed = ((data[0] & 0x3F) as u32)
            | ((data[1] as u32) << 6)
            | ((data[2] as u32) << 14);
        assert_eq!(reconstructed, 20000);
    }

    #[test]
    fn test_write_node_small_offsets() {
        // mkgmap rule: node with no arcs + class 0 + no boundary → flags==0 →
        // F_LARGE_OFFSETS is forced to avoid byte0=0 AND flags=0 both zero.
        // Node with arcs (F_ARCS set) should NOT have F_LARGE_OFFSETS for small deltas.
        let mut nod = NodWriter::new();
        // Node 0: has an arc → F_ARCS set → flags ≠ 0 → small offsets stay 3B
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 50,
                forward: true, road_class: 0, speed: 0,
                access: 0, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        // Node 1: no arcs → flags==0 → F_LARGE_OFFSETS forced (mkgmap behavior)
        nod.add_node(RouteNode {
            lat: 100010, lon: 200010, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();

        // Node 0 has F_ARCS → flags ≠ 0 → no forced F_LARGE_OFFSETS for small delta
        let n0 = nod.node_offsets()[0] as usize;
        let flags0 = nod1[n0 + 1];
        assert_ne!(flags0 & F_ARCS, 0, "Node 0 should have F_ARCS");
        assert_eq!(flags0 & F_LARGE_OFFSETS, 0, "Node 0: small offsets, F_ARCS set → no forced F_LARGE_OFFSETS");

        // Node 1: class 0, no arcs, no boundary → flags==0 → F_LARGE_OFFSETS forced (mkgmap)
        let n1 = nod.node_offsets()[1] as usize;
        let flags1 = nod1[n1 + 1];
        assert_ne!(flags1 & F_LARGE_OFFSETS, 0, "Node 1: flags==0 case → F_LARGE_OFFSETS forced per mkgmap");
    }

    #[test]
    fn test_write_node_large_offsets() {
        // Force large delta: lat diff > 0x7FF semicircles
        // Semicircles scale: 1 map unit ≈ 360/2^24 degrees
        // Need delta_lat in semicircles > 0x7FF = 2047
        // coord::to_semicircles(x) = x (identity for 24-bit map units)
        // With center = average, delta = 50000 map units → way above 0x7FF
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 0, lon: 0, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100000, lon: 100000, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        // At least one node should have F_LARGE_OFFSETS
        let n0 = nod.node_offsets()[0] as usize;
        let n1 = nod.node_offsets()[1] as usize;
        let f0 = nod1[n0 + 1];
        let f1 = nod1[n1 + 1];
        assert!(
            (f0 & F_LARGE_OFFSETS != 0) || (f1 & F_LARGE_OFFSETS != 0),
            "At least one node should have F_LARGE_OFFSETS"
        );
    }

    #[test]
    fn test_write_route_center_layout() {
        // Verify: nodes → padding 64B → tables header → Table A
        let mut nod = NodWriter::new();
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 0, length_meters: 100,
                forward: true, road_class: 1, speed: 3,
                access: 0, toll: false, one_way: false, initial_heading: 0,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        let ta_start = nod.table_a_positions()[0].0;

        // Tables header is 9 bytes before first Table A entry
        let header_start = ta_start - 9;
        // Header start should be aligned to 64B (or after 64B-aligned block)
        assert_eq!(header_start % NOD_ALIGNMENT, 0, "Tables header should follow 64B-aligned padding");

        // Table A entry is 5 bytes
        assert!(nod1.len() >= ta_start + 5, "NOD1 should contain Table A entry");
    }

    #[test]
    fn test_calc_low_byte() {
        assert_eq!(calc_low_byte(10, 128), 1); // (128>>6) - (10>>6) - 1 = 2 - 0 - 1 = 1
        assert_eq!(calc_low_byte(0, 64), 0);   // (64>>6) - (0>>6) - 1 = 1 - 0 - 1 = 0
        assert_eq!(calc_low_byte(0, 128), 1);  // (128>>6) - (0>>6) - 1 = 2 - 0 - 1 = 1
    }

    #[test]
    fn test_nod2_record_format() {
        // Test mkgmap per-road NOD2 format: 3 RouteNodes, starts with node
        let roads = vec![
            Nod2RoadInfo {
                road_class: 2,
                speed: 5,
                num_route_nodes: 3,
                starts_with_node: true,
                first_node_nod1_offset: 42,
            },
        ];
        let (data, offsets) = write_nod2_records(&roads);
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], 0);

        // nod2Flags = 0x01 | (2 << 4) | (5 << 1) = 0x2B
        assert_eq!(data[0], 0x2B);
        // NOD1 offset = 42 in 3B LE
        assert_eq!(data[1], 42);
        assert_eq!(data[2], 0);
        assert_eq!(data[3], 0);
        // nbits = 3 (starts with node → no extra bit)
        assert_eq!(u16::from_le_bytes([data[4], data[5]]), 3);
        // Bitstream: 3 bits all 1 → LSB first = 0b111 = 0x07
        assert_eq!(data[6] & 0x07, 0x07);
    }

    #[test]
    fn test_nod2_record_not_starts_with_node() {
        // Road doesn't start with a RouteNode → leading 0 bit
        let roads = vec![
            Nod2RoadInfo {
                road_class: 0,
                speed: 0,
                num_route_nodes: 2,
                starts_with_node: false,
                first_node_nod1_offset: 0,
            },
        ];
        let (data, offsets) = write_nod2_records(&roads);
        // nbits = 2 + 1 = 3
        assert_eq!(u16::from_le_bytes([data[4], data[5]]), 3);
        // Bitstream: bit0=0, bit1=1, bit2=1 → 0b110 = 0x06
        assert_eq!(data[6] & 0x07, 0x06);
    }

    #[test]
    fn test_table_a_entry_direct() {
        // Direct test of TableAEntry::write
        let mut buf = Vec::new();
        let entry = TableAEntry {
            net1_offset: 0x1234,
            road_class: 3,
            speed: 5,
            toll: true,
            one_way: false,
            access: 0x8001, // NO_EMERGENCY | NO_CAR
        };
        entry.write(&mut buf);
        assert_eq!(buf.len(), 5);
        // Bytes 0-2: 0x1234 | (0x8000 << 8 → bit 23) = 0x1234 | 0x800000
        let val = (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16);
        assert_eq!(val & 0x3FFFFF, 0x1234);
        assert_eq!(val & 0xC00000, 0x800000); // access top bit
        // Byte 3: tabAInfo = toll(0x80) | (3<<4) | 5 = 0x80 | 0x30 | 0x05 = 0xB5
        assert_eq!(buf[3], 0xB5);
        // Byte 4: access low = 0x0001 (NO_CAR, with top 2 bits cleared)
        assert_eq!(buf[4], 0x01);
    }
}
