// NETFile + RoadDef, faithful to mkgmap NETFile.java, NETHeader.java, RoadDef.java

use super::common_header::CommonHeader;

pub const NET_HEADER_LEN: u16 = 55;

// Access flags — mkgmap RoadDef.java
pub const NO_CAR: u16 = 0x0001;
pub const NO_BUS: u16 = 0x0002;
pub const NO_TAXI: u16 = 0x0004;
pub const CARPOOL: u16 = 0x0008;
pub const NO_FOOT: u16 = 0x0010;
pub const NO_BIKE: u16 = 0x0020;
pub const NO_TRUCK: u16 = 0x0040;
pub const NO_THROUGHROUTE: u16 = 0x0080;
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
        }
    }

    /// Write NET1 record — variable length
    pub fn write_net1(&self) -> Vec<u8> {
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

        // Flags byte
        let mut flags: u8 = 0;
        if self.one_way { flags |= 0x02; }
        if self.nod2_offset.is_some() { flags |= 0x40; }
        if self.access_flags != 0 { flags |= 0x80; }
        buf.push(flags);

        // Access mask (2B, conditional)
        if self.access_flags != 0 {
            buf.extend_from_slice(&self.access_flags.to_le_bytes());
        }

        // Road length (3B = meters / 4.8)
        let encoded_len = (self.road_length_meters as f64 / 4.8) as u32;
        let lb = encoded_len.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        // NOD2 offset (conditional)
        if let Some(nod2) = self.nod2_offset {
            let b = nod2.to_le_bytes();
            buf.push(b[0]);
            buf.push(b[1]);
            buf.push(b[2]);
        }

        buf
    }
}

/// NET file writer
pub struct NetWriter {
    pub roads: Vec<RoadDef>,
}

impl NetWriter {
    pub fn new() -> Self {
        Self { roads: Vec::new() }
    }

    pub fn add_road(&mut self, road: RoadDef) -> usize {
        let idx = self.roads.len();
        self.roads.push(road);
        idx
    }

    /// Build complete NET subfile
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        let common = CommonHeader::new(NET_HEADER_LEN, "GARMIN NET");
        common.write(&mut buf);

        // Build NET1 data
        let mut net1_data = Vec::new();
        for road in &self.roads {
            net1_data.extend_from_slice(&road.write_net1());
        }

        // Build NET3 sorted index
        let net3_data = self.build_net3();

        // NET1 section: offset + size
        let net1_offset = NET_HEADER_LEN as u32;
        let net1_size = net1_data.len() as u32;
        buf.extend_from_slice(&net1_offset.to_le_bytes());
        buf.extend_from_slice(&net1_size.to_le_bytes());

        // NET2 section: empty
        let net2_offset = net1_offset + net1_size;
        buf.extend_from_slice(&net2_offset.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());

        // NET3 section
        let net3_offset = net2_offset;
        let net3_size = net3_data.len() as u32;
        buf.extend_from_slice(&net3_offset.to_le_bytes());
        buf.extend_from_slice(&net3_size.to_le_bytes());

        // Pad to header length
        while buf.len() < NET_HEADER_LEN as usize {
            buf.push(0x00);
        }
        buf.truncate(NET_HEADER_LEN as usize);

        // Section data
        buf.extend_from_slice(&net1_data);
        buf.extend_from_slice(&net3_data);

        buf
    }

    /// Build NET3 sorted index — 3B per road = NET1 offset, sorted by label
    fn build_net3(&self) -> Vec<u8> {
        let mut entries: Vec<(u32, u32)> = Vec::new(); // (label_offset_for_sort, net1_offset)
        let mut offset = 0u32;
        for road in &self.roads {
            let label = road.label_offsets.first().copied().unwrap_or(0);
            entries.push((label, offset));
            offset += road.write_net1().len() as u32;
        }
        entries.sort_by_key(|&(label, _)| label);

        let mut data = Vec::new();
        for (_label, net1_off) in entries {
            // NET3 record: 3 bytes = NET1 offset (sorted by label name)
            let b = net1_off.to_le_bytes();
            data.push(b[0]);
            data.push(b[1]);
            data.push(b[2]);
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
        let net1 = rd.write_net1();
        // label(3) + flags(1) + length(3) = 7
        assert_eq!(net1.len(), 7);
    }

    #[test]
    fn test_road_def_with_access() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.access_flags = NO_CAR | NO_TRUCK;
        rd.road_length_meters = 100;
        let net1 = rd.write_net1();
        // label(3) + flags(1) + access(2) + length(3) = 9
        assert_eq!(net1.len(), 9);
        assert!(net1[3] & 0x80 != 0); // access flag present
    }

    #[test]
    fn test_last_label_flag() {
        let mut rd = RoadDef::new();
        rd.label_offsets.push(100);
        rd.label_offsets.push(200);
        rd.road_length_meters = 0;
        let net1 = rd.write_net1();
        // Second label should have MSB set
        assert!(net1[5] & 0x80 != 0);
    }

    #[test]
    fn test_net_header() {
        let net = NetWriter::new();
        let data = net.build();
        assert_eq!(&data[2..12], b"GARMIN NET");
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, NET_HEADER_LEN);
    }
}
