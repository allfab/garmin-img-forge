// NETFile + RoadDef, faithful to mkgmap NETFile.java, NETHeader.java, RoadDef.java

use super::common_header::{self, CommonHeader};

pub const NET_HEADER_LEN: u16 = 55;

// Access flags — mkgmap RoadDef.java
pub const NO_CAR: u16 = 0x0001;
pub const NO_BUS: u16 = 0x0002;
pub const NO_TAXI: u16 = 0x0004;
pub const NO_FOOT: u16 = 0x0010;
pub const NO_BIKE: u16 = 0x0020;
pub const NO_TRUCK: u16 = 0x0040;
pub const NO_DELIVERY: u16 = 0x4000;
pub const NO_EMERGENCY: u16 = 0x8000;

/// Road definition — mkgmap RoadDef.java
#[derive(Debug, Clone)]
pub struct RoadDef {
    pub label_offsets: Vec<u32>,
    pub road_class: u8,
    pub speed: u8,
    pub one_way: bool,
    pub toll: bool,
    pub access_flags: u16,
    pub road_length_meters: u32,
    pub net1_offset: u32,
    pub nod2_offset: Option<u32>,
    /// Per-level subdivision references: Vec<Vec<(polyline_num: u8, subdiv_num: u16)>>
    pub subdiv_refs: Vec<Vec<(u8, u16)>>,
}

impl RoadDef {
    pub fn new() -> Self {
        Self {
            label_offsets: Vec::new(),
            road_class: 0,
            speed: 0,
            one_way: false,
            toll: false,
            access_flags: 0,
            road_length_meters: 0,
            net1_offset: 0,
            nod2_offset: None,
            subdiv_refs: Vec::new(),
        }
    }

    /// Write NET1 record — variable length.
    /// Returns (data, level_div_offset) where level_div_offset is the byte position
    /// of the first polyline_num byte within the returned data.
    pub fn write_net1(&self) -> (Vec<u8>, usize) {
        let mut buf = Vec::new();

        // Labels (3B each, MSB set on last)
        for (i, &offset) in self.label_offsets.iter().enumerate() {
            let mut val = offset & 0x3FFFFF;
            if i == self.label_offsets.len() - 1 {
                val |= 0x800000; // last label flag
            }
            let b = val.to_le_bytes();
            buf.push(b[0]);
            buf.push(b[1]);
            buf.push(b[2]);
        }

        // Flags byte — mkgmap RoadDef.java
        // Access is NOT in NET1 flags — it's in NOD Table A
        let mut flags: u8 = 0;
        if self.one_way { flags |= 0x02; }
        flags |= 0x04; // NET_FLAG_UNK1 — always set in mkgmap
        if self.nod2_offset.is_some() { flags |= 0x40; } // NET_FLAG_NODINFO
        buf.push(flags);

        // Road length (3B = meters / 4.8)
        let encoded_len = (self.road_length_meters as f64 / 4.8) as u32;
        let lb = encoded_len.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        // Level counts + level divs (mkgmap RoadDef.writeLevelCount + writeLevelDivs).
        // imgforge currently emits routable geometry only at level 0. A single logical
        // road can still be split into multiple RGN polylines/subdivisions; BaseCamp
        // uses this table to map a clicked geometry segment back to the RoadDef.
        let refs = self.subdiv_refs.first().map(|v| v.as_slice()).unwrap_or(&[]);
        let count = refs.len().max(1).min(0x7f);
        buf.push(0x80 | (count as u8)); // level 0 is the max/last level
        let level_div_offset = buf.len();
        if refs.is_empty() {
            buf.push(0x00);
            buf.extend_from_slice(&0u16.to_le_bytes());
        } else {
            for &(polyline_num, subdiv_num) in refs.iter().take(0x7f) {
                buf.push(polyline_num);
                buf.extend_from_slice(&subdiv_num.to_le_bytes());
            }
        }

        // NOD2 offset — variable-length encoding (mkgmap RoadDef.java:296-306)
        if let Some(nod2) = self.nod2_offset {
            if nod2 < 0x7FFF {
                buf.push(0x01); // size_indicator = 1 → 2-byte offset
                buf.extend_from_slice(&(nod2 as u16).to_le_bytes());
            } else {
                buf.push(0x02); // size_indicator = 2 → 3-byte offset
                let b = nod2.to_le_bytes();
                buf.push(b[0]);
                buf.push(b[1]);
                buf.push(b[2]);
            }
        }

        (buf, level_div_offset)
    }
}

/// NET file writer
pub struct NetWriter {
    pub roads: Vec<RoadDef>,
    /// NET1 byte offsets per road, populated after build()
    net1_offsets: Vec<u32>,
    /// Built NET data (available after build() for patching)
    pub built_data: Option<Vec<u8>>,
    /// Per-road: byte offset within NET data of the first level div entry (polyline_num byte)
    /// = NET_HEADER_LEN + NET1 offset + (label bytes + flags + length + level_count_byte)
    level_div_positions: Vec<usize>,
}

impl NetWriter {
    pub fn new() -> Self {
        Self {
            roads: Vec::new(),
            net1_offsets: Vec::new(),
            built_data: None,
            level_div_positions: Vec::new(),
        }
    }

    /// NET1 byte offsets per road, available after build().
    pub fn net1_offsets(&self) -> &[u32] {
        &self.net1_offsets
    }

    pub fn add_road(&mut self, road: RoadDef) -> usize {
        let idx = self.roads.len();
        self.roads.push(road);
        idx
    }

    /// Build complete NET subfile
    pub fn build(&mut self) -> Vec<u8> {
        let mut buf = Vec::new();

        let common = CommonHeader::new(NET_HEADER_LEN, "GARMIN NET");
        common.write(&mut buf);

        // Build NET1 data + precompute per-road offsets for NET3
        let mut net1_data = Vec::new();
        let mut net1_offsets = Vec::with_capacity(self.roads.len());
        let mut level_div_positions = Vec::with_capacity(self.roads.len());
        for road in &self.roads {
            let record_start = net1_data.len();
            net1_offsets.push(record_start as u32);
            let (record_data, level_div_off) = road.write_net1();
            // Position within final NET data = header_len + record_start + level_div_off
            level_div_positions.push(NET_HEADER_LEN as usize + record_start + level_div_off);
            net1_data.extend_from_slice(&record_data);
        }
        // Build NET3 sorted index using precomputed offsets
        let net3_data = self.build_net3(&net1_offsets);

        // Store offsets for external access
        self.net1_offsets = net1_offsets;
        self.level_div_positions = level_div_positions;

        // Section descriptors — mkgmap NETHeader.java layout:
        // Each section = offset(4B) + size(4B) + flag(1B)
        let net1_offset = NET_HEADER_LEN as u32;
        let net1_size = net1_data.len() as u32;
        common_header::write_section(&mut buf, net1_offset, net1_size);
        buf.push(0x00); // @0x1D: addr_shift (road label addr multiplier)

        let net2_offset = net1_offset + net1_size;
        let net2_size = 0u32; // NET2 is empty for now
        common_header::write_section(&mut buf, net2_offset, net2_size);
        buf.push(0x00); // @0x26: NET2 flags

        let net3_offset = net2_offset + net2_size;
        let net3_size = net3_data.len() as u32;
        common_header::write_section(&mut buf, net3_offset, net3_size);
        buf.push(0x03); // @0x2F: NET3 record size

        // Remaining header fields @0x30-0x36 — mkgmap NETHeader
        buf.extend_from_slice(&0u32.to_le_bytes()); // @0x30: reserved
        buf.push(0x00);                              // @0x34: reserved
        buf.extend_from_slice(&1u16.to_le_bytes());  // @0x35: sort descriptor multiplier

        common_header::pad_to(&mut buf, NET_HEADER_LEN as usize);

        // Section data
        buf.extend_from_slice(&net1_data);
        buf.extend_from_slice(&net3_data);

        // TODO: callers in writer.rs discard the return value and read built_data instead.
        // Consider returning &[u8] or removing the return to avoid this clone.
        self.built_data = Some(buf.clone());
        buf
    }

    /// Build NET3 sorted index — 3B per road = NET1 offset, sorted by label
    fn build_net3(&self, net1_offsets: &[u32]) -> Vec<u8> {
        let mut entries: Vec<(u32, u32)> = self.roads.iter()
            .zip(net1_offsets.iter())
            .map(|(road, &off)| {
                let label = road.label_offsets.first().copied().unwrap_or(0);
                (label, off)
            })
            .collect();
        entries.sort_by_key(|&(label, _)| label);

        let mut data = Vec::with_capacity(entries.len() * 3);
        for (_label, net1_off) in entries {
            common_header::write_u24(&mut data, net1_off);
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_def_basic() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.road_length_meters = 480;
        let (net1, level_div_pos) = rd.write_net1();
        // label(3) + flags(1) + length(3) + levelCount(1) + levelDiv(3) = 11
        assert_eq!(net1.len(), 11);
        assert_eq!(net1[7], 0x81); // 1 polyline at level 0, last level
        assert_eq!(level_div_pos, 8); // polyline_num placeholder at byte 8
    }

    #[test]
    fn test_road_def_with_access() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.access_flags = NO_CAR | NO_TRUCK;
        rd.road_length_meters = 100;
        let (net1, _) = rd.write_net1();
        // label(3) + flags(1) + length(3) + levelCount(1) + levelDiv(3) = 11
        // Access is NOT in NET1 (it's in NOD Table A)
        assert_eq!(net1.len(), 11);
        assert_eq!(net1[3] & 0x80, 0); // no access flag in NET1
    }

    #[test]
    fn test_last_label_flag() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.label_offsets.push(200);
        rd.road_length_meters = 0;
        let (net1, _) = rd.write_net1();
        // Second label should have MSB set
        assert!(net1[5] & 0x80 != 0);
    }

    #[test]
    fn test_net_header() {
        let mut net = NetWriter::new();
        let data = net.build();
        assert_eq!(&data[2..12], b"GARMIN NET");
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, NET_HEADER_LEN);
    }

    /// AC10: NET1 offsets are accessible after build
    #[test]
    fn test_net1_offsets_exposed() {
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

        let mut rd2 = RoadDef::new();
        rd2.label_offsets.push(30);
        rd2.road_length_meters = 300;
        net.add_road(rd2);

        net.build();

        assert_eq!(net.net1_offsets().len(), 3);
        assert_eq!(net.net1_offsets()[0], 0);
        // road 0: label(3) + flags(1) + length(3) + levelCount(1) + levelDiv(3) = 11
        assert_eq!(net.net1_offsets()[1], 11);
        // road 1: same layout (no access in NET1) = 11
        assert_eq!(net.net1_offsets()[2], 11 + 11);
    }

    /// AC7: nod2_offset < 0x7FFF → size_indicator(1) + offset(2) = 3 bytes
    #[test]
    fn test_nod2_offset_small() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.road_length_meters = 100;
        rd.nod2_offset = Some(100);
        let (net1, _) = rd.write_net1();
        // label(3) + flags(1) + length(3) + levelCount(1) + levelDiv(3) + nod2[size(1)+offset(2)] = 14
        assert_eq!(net1.len(), 14);
        assert_eq!(net1[3] & 0x44, 0x44);
        assert_eq!(net1[7], 0x81); // 1 polyline, last level
        // nod2_offset after level div placeholder (3B)
        assert_eq!(net1[11], 0x01); // size indicator
        assert_eq!(u16::from_le_bytes([net1[12], net1[13]]), 100);
    }

    /// nod2_offset >= 0x7FFF → size_indicator(2) + offset(3) = 4 bytes
    #[test]
    fn test_nod2_offset_large() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.road_length_meters = 100;
        rd.nod2_offset = Some(0x10000);
        let (net1, _) = rd.write_net1();
        // label(3) + flags(1) + length(3) + levelCount(1) + levelDiv(3) + nod2[size(1)+offset(3)] = 15
        assert_eq!(net1.len(), 15);
        assert_eq!(net1[7], 0x81); // 1 polyline, last level
        assert_eq!(net1[11], 0x02); // size indicator
        let offset = (net1[12] as u32) | ((net1[13] as u32) << 8) | ((net1[14] as u32) << 16);
        assert_eq!(offset, 0x10000);
    }
}
