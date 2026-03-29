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
    pub access: u8,
}

/// A group of nearby route nodes — mkgmap RouteCenter.java
#[derive(Debug, Clone)]
pub struct RouteCenter {
    pub center_lat: i32, // semicircles
    pub center_lon: i32,
    pub nodes: Vec<usize>, // indices into the node array
}

/// NOD file writer
pub struct NodWriter {
    pub nodes: Vec<RouteNode>,
    pub centers: Vec<RouteCenter>,
    pub drive_on_left: bool,
    pub nod2_data: Vec<u8>,
}

impl NodWriter {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            centers: Vec::new(),
            drive_on_left: false,
            nod2_data: Vec::new(),
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
    pub fn patch_net1_offsets(&mut self, _net1_offsets: &[u32]) {
        // TODO: Implement NET1 offset backpatching into serialized NOD1 data.
        // This requires tracking the byte positions of each Table A arc's NET1 offset
        // field during build_nod1(), then overwriting them with the correct values
        // from NetWriter after NET is built.
        // For now, arcs reference NET1 offset 0 which points to the first road.
        tracing::debug!("NOD NET1 offset patching: not yet implemented (arcs default to offset 0)");
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

    /// Build complete NOD subfile
    pub fn build(&mut self) -> Vec<u8> {
        if self.centers.is_empty() {
            self.build_centers();
        }

        let mut buf = Vec::new();

        let common = CommonHeader::new(NOD_HEADER_LEN, "GARMIN NOD");
        common.write(&mut buf);

        // Build NOD1 data (route centers + nodes + tables)
        let (nod1_data, node_offsets) = self.build_nod1();

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

    /// Build NOD1 and return (data, node_index→nod1_offset map)
    fn build_nod1(&self) -> (Vec<u8>, Vec<u32>) {
        let mut data = Vec::new();
        let mut node_offsets = vec![0u32; self.nodes.len()];

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

                // Table A arcs (5B each)
                for arc in &node.arcs {
                    // NET1 offset (3B) — placeholder, patched later
                    data.push(0x00);
                    data.push(0x00);
                    data.push(0x00);
                    // Road class/speed/flags (1B)
                    data.push((arc.road_class << 4) | arc.speed);
                    // Access (1B)
                    data.push(arc.access);
                }
            }

            // Align to 64 bytes
            while data.len() % NOD_ALIGNMENT != 0 {
                data.push(0x00);
            }
        }

        (data, node_offsets)
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
}
