// NODFile + RouteNode/Arc/Center, faithful to mkgmap NODFile.java, NODHeader.java

use super::common_header::{self, CommonHeader};
use super::coord;

pub const NOD_HEADER_LEN: u16 = 127;
pub const NOD_ALIGNMENT: usize = 64; // Tables aligned to 1<<6

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
}

/// A group of nearby route nodes — mkgmap RouteCenter.java
#[derive(Debug, Clone)]
pub struct RouteCenter {
    pub center_lat: i32, // semicircles
    pub center_lon: i32,
    pub nodes: Vec<usize>, // indices into the node array
}

/// Build NOD2 bitstream from node_flags for all roads.
/// Returns (nod2_data, nod2_byte_offset_per_road).
/// For each road (in order), writes 1 bit per vertex: 1=RouteNode, 0=geometry.
pub fn build_nod2_bitstream(all_node_flags: &[Vec<bool>]) -> (Vec<u8>, Vec<u32>) {
    use super::bit_writer::BitWriter;

    let total_bits: usize = all_node_flags.iter().map(|f| f.len()).sum();
    let mut bw = BitWriter::with_capacity((total_bits + 7) / 8 + 8);
    let mut offsets = Vec::with_capacity(all_node_flags.len());

    for flags in all_node_flags {
        // Pad to byte boundary so each road starts at a clean byte offset
        while bw.bit_position() % 8 != 0 {
            bw.put1(false);
        }
        // Record byte offset at the start of this road's bits
        offsets.push((bw.bit_position() / 8) as u32);
        for &is_node in flags {
            bw.put1(is_node);
        }
    }

    (bw.bytes().to_vec(), offsets)
}

/// NOD file writer
pub struct NodWriter {
    pub nodes: Vec<RouteNode>,
    pub centers: Vec<RouteCenter>,
    pub drive_on_left: bool,
    pub nod2_data: Vec<u8>,
    /// (byte_position_in_nod1, road_def_index) for each Table A arc
    arc_net1_positions: Vec<(usize, usize)>,
    /// Serialized NOD1 data, built by build_nod1(), patched before final build
    nod1_data: Option<Vec<u8>>,
    /// Node offsets from build_nod1()
    node_offsets: Vec<u32>,
}

impl NodWriter {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            centers: Vec::new(),
            drive_on_left: false,
            nod2_data: Vec::new(),
            arc_net1_positions: Vec::new(),
            nod1_data: None,
            node_offsets: Vec::new(),
        }
    }

    pub fn add_node(&mut self, node: RouteNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// Group nodes into route centers (max 256 nodes per center)
    pub fn build_centers(&mut self) {
        self.centers.clear();
        if self.nodes.is_empty() {
            return;
        }

        let mut current_nodes = Vec::new();
        for i in 0..self.nodes.len() {
            current_nodes.push(i);
            if current_nodes.len() >= 256 {
                let center = self.make_center(&current_nodes);
                self.centers.push(center);
                current_nodes.clear();
            }
        }
        if !current_nodes.is_empty() {
            let center = self.make_center(&current_nodes);
            self.centers.push(center);
        }
    }

    /// Set NOD2 bitstream data (1 bit per polyline vertex: 1=RouteNode, 0=geometry)
    pub fn set_nod2_data(&mut self, data: Vec<u8>) {
        self.nod2_data = data;
    }

    /// Patch NET1 offsets into Table A arcs after NET subfile is built.
    /// `net1_offsets` maps road_def_index → NET1 byte offset.
    /// Must be called after prepare() and before build().
    pub fn patch_net1_offsets(&mut self, net1_offsets: &[u32]) {
        let nod1 = self.nod1_data.as_mut()
            .expect("patch_net1_offsets called before prepare()");

        for &(byte_pos, road_idx) in &self.arc_net1_positions {
            if road_idx >= net1_offsets.len() {
                tracing::warn!("NOD patch: road_def_index {} out of range (max {}), arc at byte {} left unpatched",
                    road_idx, net1_offsets.len(), byte_pos);
                continue;
            }
            let offset = net1_offsets[road_idx];
            // Read existing 3 bytes (contain access top bits in bits 22-23)
            let existing = (nod1[byte_pos] as u32)
                | ((nod1[byte_pos + 1] as u32) << 8)
                | ((nod1[byte_pos + 2] as u32) << 16);
            // OR the NET1 offset (22 low bits) with existing access bits
            let patched = (offset & 0x3FFFFF) | existing;
            let b = patched.to_le_bytes();
            nod1[byte_pos] = b[0];
            nod1[byte_pos + 1] = b[1];
            nod1[byte_pos + 2] = b[2];
        }
    }

    fn make_center(&self, node_indices: &[usize]) -> RouteCenter {
        let mut sum_lat: i64 = 0;
        let mut sum_lon: i64 = 0;
        for &idx in node_indices {
            sum_lat += coord::to_semicircles(self.nodes[idx].lat) as i64;
            sum_lon += coord::to_semicircles(self.nodes[idx].lon) as i64;
        }
        let n = node_indices.len() as i64;
        RouteCenter {
            center_lat: (sum_lat / n) as i32,
            center_lon: (sum_lon / n) as i32,
            nodes: node_indices.to_vec(),
        }
    }

    /// Build NOD1 data and store internally for later patching.
    /// Must be called before patch_net1_offsets() and finalize().
    pub fn prepare(&mut self) {
        if self.centers.is_empty() {
            self.build_centers();
        }
        let (nod1_data, node_offsets, arc_positions) = self.build_nod1();
        self.nod1_data = Some(nod1_data);
        self.node_offsets = node_offsets;
        self.arc_net1_positions = arc_positions;
    }

    /// Build complete NOD subfile.
    /// Consumes prepared NOD1 data — must not be called twice after patch_net1_offsets().
    pub fn build(&mut self) -> Vec<u8> {
        // If prepare() wasn't called, do it now (no patching needed)
        if self.nod1_data.is_none() {
            self.prepare();
        }

        let mut buf = Vec::new();

        let common = CommonHeader::new(NOD_HEADER_LEN, "GARMIN NOD");
        common.write(&mut buf);

        let nod1_data = self.nod1_data.take().unwrap();
        let node_offsets = &self.node_offsets;

        // Build NOD2 data (bitstream)
        let nod2_data = self.build_nod2();

        // Build NOD3 data (boundary nodes with correct NOD1 offsets)
        let nod3_data = self.build_nod3(&node_offsets);

        // Section descriptors
        let nod1_offset = NOD_HEADER_LEN as u32;
        let nod1_size = nod1_data.len() as u32;
        common_header::write_section(&mut buf, nod1_offset, nod1_size);

        let nod2_offset = nod1_offset + nod1_size;
        common_header::write_section(&mut buf, nod2_offset, nod2_data.len() as u32);

        let nod3_offset = nod2_offset + nod2_data.len() as u32;
        common_header::write_section(&mut buf, nod3_offset, nod3_data.len() as u32);

        // Drive on left flag
        buf.push(if self.drive_on_left { 1 } else { 0 });

        common_header::pad_to(&mut buf, NOD_HEADER_LEN as usize);

        // Section data
        buf.extend_from_slice(&nod1_data);
        buf.extend_from_slice(&nod2_data);
        buf.extend_from_slice(&nod3_data);

        buf
    }

    /// Build NOD1 and return (data, node_index→nod1_offset map, arc_net1_positions)
    fn build_nod1(&self) -> (Vec<u8>, Vec<u32>, Vec<(usize, usize)>) {
        let mut data = Vec::new();
        let mut node_offsets = vec![0u32; self.nodes.len()];
        let mut arc_positions: Vec<(usize, usize)> = Vec::new();

        for center in &self.centers {
            // Center coords (4B + 4B semicircles)
            data.extend_from_slice(&center.center_lat.to_le_bytes());
            data.extend_from_slice(&center.center_lon.to_le_bytes());

            // Table B offset placeholder (2B)
            data.extend_from_slice(&0u16.to_le_bytes());

            // Route nodes with delta coords + Table A arcs
            for &node_idx in &center.nodes {
                node_offsets[node_idx] = data.len() as u32;
                let node = &self.nodes[node_idx];
                let delta_lat = coord::to_semicircles(node.lat) - center.center_lat;
                let delta_lon = coord::to_semicircles(node.lon) - center.center_lon;
                data.extend_from_slice(&(delta_lat as i16).to_le_bytes());
                data.extend_from_slice(&(delta_lon as i16).to_le_bytes());

                // Number of arcs
                data.push(node.arcs.len() as u8);

                // Table A arcs (5B each) — mkgmap TableA.java:202-214
                for arc in &node.arcs {
                    // Record byte position for NET1 offset patching
                    let arc_byte_pos = data.len();
                    arc_positions.push((arc_byte_pos, arc.road_def_index));

                    // Bytes 0-2: NET1 offset placeholder (patched later)
                    // + top 2 bits of access (NO_EMERGENCY, NO_DELIVERY) packed in bits 22-23
                    let access_top = ((arc.access as u32) & 0xC000) << 8;
                    let net1_placeholder = 0u32 | access_top;
                    let b = net1_placeholder.to_le_bytes();
                    data.push(b[0]);
                    data.push(b[1]);
                    data.push(b[2]);
                    // Byte 3: tabAInfo = toll(0x80) | (road_class << 4) | oneway(0x08) | speed
                    let mut tab_a_info: u8 = (arc.road_class << 4) | arc.speed;
                    if arc.toll { tab_a_info |= 0x80; }
                    if arc.one_way { tab_a_info |= 0x08; }
                    data.push(tab_a_info);
                    // Byte 4: access low byte (bits 8-13 not in Table A,
                    // full 16-bit access_flags stored in NET1 record)
                    data.push((arc.access & 0x00FF) as u8);
                }
            }

            // Align to 64 bytes
            while data.len() % NOD_ALIGNMENT != 0 {
                data.push(0x00);
            }
        }

        (data, node_offsets, arc_positions)
    }

    fn build_nod2(&self) -> Vec<u8> {
        // NOD2: 1 bit per vertex of each road polyline
        // 1 = vertex is a RouteNode, 0 = geometry-only vertex
        // For now, generate a bitstream marking all node positions.
        // Each road's polyline contributes vertex_count bits.
        // This is populated by the caller via set_nod2_data if available.
        self.nod2_data.clone()
    }

    fn build_nod3(&self, node_offsets: &[u32]) -> Vec<u8> {
        // NOD3: boundary nodes 9B each (lon 3B + lat 3B + NOD1 offset 3B)
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
                // NOD1 offset for this node
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

    /// AC4: Table A byte 3 encodes toll and oneway
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
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        // Find the arc's byte 3 (tabAInfo): after center(8B) + tableB(2B) + delta(4B) + narcs(1B) + bytes0-2(3B)
        // = 8 + 2 + 4 + 1 + 3 = 18
        let tab_a_info = nod1[18];
        // Expected: toll(0x80) | (2 << 4) | oneway(0x08) | 5 = 0x80 | 0x20 | 0x08 | 0x05 = 0xAD
        assert_eq!(tab_a_info, 0xAD);
    }

    /// AC5: Table A byte 4 encodes access flags low byte
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
                access, toll: false, one_way: false,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        // byte 4 = offset 19
        assert_eq!(nod1[19], 0x31);
    }

    /// AC6: Table A bytes 0-2 encode access flags high bits
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
                access, toll: false, one_way: false,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();
        let nod1 = nod.nod1_data.as_ref().unwrap();
        // bytes 0-2 of arc at offset 15: access_top = (0x8000 << 8) = 0x800000
        // NET1 placeholder = 0, so bytes = [0x00, 0x00, 0x80]
        assert_eq!(nod1[15], 0x00);
        assert_eq!(nod1[16], 0x00);
        assert_eq!(nod1[17], 0x80);
    }

    /// AC7: patch_net1_offsets correctly patches NET1 offsets into NOD1
    #[test]
    fn test_patch_net1_offsets() {
        let mut nod = NodWriter::new();
        // Node 0 has arc to road_def_index 1
        nod.add_node(RouteNode {
            lat: 100000, lon: 200000,
            arcs: vec![RouteArc {
                dest_node_index: 1, road_def_index: 1, length_meters: 100,
                forward: true, road_class: 0, speed: 0,
                access: 0, toll: false, one_way: false,
            }],
            is_boundary: false, node_class: 0,
        });
        nod.add_node(RouteNode {
            lat: 100100, lon: 200100, arcs: Vec::new(),
            is_boundary: false, node_class: 0,
        });
        nod.prepare();

        // Patch: road 0 at offset 0, road 1 at offset 42
        nod.patch_net1_offsets(&[0, 42]);

        let nod1 = nod.nod1_data.as_ref().unwrap();
        // Arc bytes 0-2 at offset 15 should now contain offset 42
        let patched = (nod1[15] as u32) | ((nod1[16] as u32) << 8) | ((nod1[17] as u32) << 16);
        assert_eq!(patched & 0x3FFFFF, 42);
    }

    #[test]
    fn test_build_nod2_bitstream_simple() {
        // 1 road, 3 vertices: node, geometry, node → bits: 1, 0, 1
        let flags = vec![vec![true, false, true]];
        let (data, offsets) = build_nod2_bitstream(&flags);
        assert_eq!(offsets.len(), 1);
        assert_eq!(offsets[0], 0);
        assert!(!data.is_empty());
        // First byte: bits 101 (LSB-first) = 0b00000101 = 0x05
        assert_eq!(data[0] & 0x07, 0x05);
    }

    #[test]
    fn test_build_nod2_offsets() {
        // 2 roads: road0 has 3 vertices, road1 has 5 vertices
        let flags = vec![
            vec![true, false, true],       // 3 bits → padded to 8 bits (1 byte)
            vec![true, false, false, false, true], // 5 bits
        ];
        let (data, offsets) = build_nod2_bitstream(&flags);
        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0], 0);
        // After 3 bits + 5 padding bits = 1 full byte, road1 starts at byte 1
        assert_eq!(offsets[1], 1);
        assert!(!data.is_empty());
        // Total: 8 bits (road0 padded) + 5 bits (road1) = 13 bits → 2 bytes
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_build_nod2_offsets_cross_byte() {
        // Road0: 9 vertices (9 bits → padded to 16 bits = 2 bytes)
        // Road1: starts at byte 2
        let flags = vec![
            vec![true; 9],
            vec![true, false, true],
        ];
        let (data, offsets) = build_nod2_bitstream(&flags);
        assert_eq!(offsets[0], 0);
        assert_eq!(offsets[1], 2); // 9 bits padded to 16 bits = 2 bytes
        assert!(data.len() >= 3);
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
                access: 0, toll: false, one_way: false,
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
}
