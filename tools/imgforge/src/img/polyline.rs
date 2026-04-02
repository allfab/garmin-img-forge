// Polyline — polyline encoding, faithful to mkgmap Polyline.java

use super::coord::Coord;
use super::map_object::MapObject;

/// A polyline on the map
#[derive(Debug, Clone)]
pub struct Polyline {
    pub type_code: u32,
    pub label_offset: u32,
    pub points: Vec<Coord>,
    pub direction: bool,
    pub has_net_info: bool,
    pub net_offset: u32,
    pub road_id: Option<u32>,
}

impl MapObject for Polyline {
    fn type_code(&self) -> u32 { self.type_code }
    fn label_offset(&self) -> u32 { self.label_offset }
    fn coords(&self) -> &[Coord] { &self.points }
}

impl Polyline {
    pub fn new(type_code: u32, points: Vec<Coord>) -> Self {
        Self {
            type_code,
            label_offset: 0,
            points,
            direction: false,
            has_net_info: false,
            net_offset: 0,
            road_id: None,
        }
    }

    /// Write polyline record — mkgmap Polyline.java
    /// Format: type(1B) + label(3B) + delta_lon(i16) + delta_lat(i16) + blen(1-2B) + bitstream
    pub fn write(
        &self,
        subdiv_center_lat: i32,
        subdiv_center_lon: i32,
        shift: i32,
        bitstream: &[u8],
        extra_bit: bool,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + bitstream.len());

        // Total data after blen field: bitstream + NET offset (3B) if has_net_info
        let total_len = if self.has_net_info {
            bitstream.len() + 3
        } else {
            bitstream.len()
        };

        // Type byte with flags
        let mut type_byte = (self.type_code & 0xFF) as u8;
        if self.direction {
            type_byte |= 0x40;
        }
        if total_len > 256 {
            type_byte |= 0x80; // 2-byte length
        }
        buf.push(type_byte);

        // Label offset (3 bytes) with flags
        let mut lbl = self.label_offset;
        if extra_bit {
            lbl |= 0x400000;
        }
        if self.has_net_info {
            lbl |= 0x800000;
        }
        let lb = lbl.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        // First point delta from subdivision center (clamped to i16 range)
        let first = &self.points[0];
        let dx = ((first.longitude() - subdiv_center_lon) >> shift).clamp(-32768, 32767) as i16;
        let dy = ((first.latitude() - subdiv_center_lat) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());

        // Bitstream length — includes NET offset when has_net_info.
        // Decoder reads blen+1 bytes; last 3 are NET offset if has_net_info.
        let blen = total_len - 1;
        if blen >= 256 {
            buf.extend_from_slice(&(blen as u16).to_le_bytes());
        } else {
            buf.push(blen as u8);
        }

        // Bitstream data
        buf.extend_from_slice(bitstream);

        // NET1 offset (3 bytes, little-endian) — appended AFTER bitstream,
        // NOT counted in blen. Decoder reads these separately when has_net_info is set.
        if self.has_net_info {
            let nb = self.net_offset.to_le_bytes();
            buf.push(nb[0]);
            buf.push(nb[1]);
            buf.push(nb[2]);
        }

        buf
    }

    /// Write extended polyline record — mkgmap Polyline.java (extended type path)
    /// Format: type(2B BE) + dx(2B LE) + dy(2B LE) + len_encoded(1-2B) + bitstream + [label(3B)]
    pub fn write_ext(
        &self,
        subdiv_center_lat: i32,
        subdiv_center_lon: i32,
        shift: i32,
        bitstream: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + bitstream.len());

        let has_label = self.label_offset > 0;

        // Type: 2 bytes big-endian
        let type_high = ((self.type_code >> 8) & 0xFF) as u8;
        let mut type_low = (self.type_code & 0x1F) as u8;
        if has_label {
            type_low |= 0x20;
        }
        // no extra_bytes (0x80 not set)
        buf.push(type_high);
        buf.push(type_low);

        // First point delta from subdivision center (i16 LE)
        let first = &self.points[0];
        let dx = ((first.longitude() - subdiv_center_lon) >> shift).clamp(-32768, 32767) as i16;
        let dy = ((first.latitude() - subdiv_center_lat) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());

        // Extended type length encoding:
        // len < 0x80 → (len << 1) | 1 as single byte
        // else → (len << 2) | 2 as 2 bytes LE
        let len = bitstream.len();
        if len < 0x80 {
            buf.push(((len << 1) | 1) as u8);
        } else {
            let encoded = ((len << 2) | 2) as u16;
            buf.extend_from_slice(&encoded.to_le_bytes());
        }

        // Bitstream data
        buf.extend_from_slice(bitstream);

        // Label AFTER bitstream (extended type convention)
        if has_label {
            let lb = self.label_offset.to_le_bytes();
            buf.push(lb[0]);
            buf.push(lb[1]);
            buf.push(lb[2]);
        }

        buf
    }

    pub fn is_road(&self) -> bool {
        self.road_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polyline_write() {
        let points = vec![Coord::new(100, 200), Coord::new(110, 210)];
        let pl = Polyline::new(0x01, points);
        let bitstream = vec![0xAA, 0xBB];
        let buf = pl.write(0, 0, 0, &bitstream, false);

        assert_eq!(buf[0], 0x01); // type
        // label offset = 0
        assert_eq!(buf[1], 0);
        assert_eq!(buf[2], 0);
        assert_eq!(buf[3], 0);
        // dx = 200
        let dx = i16::from_le_bytes([buf[4], buf[5]]);
        assert_eq!(dx, 200);
        // dy = 100
        let dy = i16::from_le_bytes([buf[6], buf[7]]);
        assert_eq!(dy, 100);
        // blen = len-1 = 1 (Garmin convention: stored as actual_bytes - 1)
        assert_eq!(buf[8], 1);
        // bitstream
        assert_eq!(buf[9], 0xAA);
        assert_eq!(buf[10], 0xBB);
    }

    #[test]
    fn test_polyline_direction_flag() {
        let pl = Polyline {
            type_code: 0x01,
            label_offset: 0,
            points: vec![Coord::new(0, 0)],
            direction: true,
            has_net_info: false,
            net_offset: 0,
            road_id: None,
        };
        let buf = pl.write(0, 0, 0, &[0x00], false);
        assert!(buf[0] & 0x40 != 0);
    }

    #[test]
    fn test_polyline_ext_write() {
        let points = vec![Coord::new(100, 200), Coord::new(110, 210)];
        let pl = Polyline::new(0x10601, points);
        let bitstream = vec![0xAA, 0xBB];
        let buf = pl.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(1) + bs(2) = 9
        assert_eq!(buf.len(), 9);
        assert_eq!(buf[0], 0x06); // type high: (0x10601 >> 8) & 0xFF = 0x106 & 0xFF = 0x06
        assert_eq!(buf[1], 0x01); // type low: 0x10601 & 0x1F = 0x01, no label
        // len encoding: (2 << 1) | 1 = 5
        assert_eq!(buf[6], 5);
    }

    #[test]
    fn test_polyline_ext_with_label() {
        let points = vec![Coord::new(100, 200), Coord::new(110, 210)];
        let mut pl = Polyline::new(0x10601, points);
        pl.label_offset = 0x42;
        let bitstream = vec![0xAA, 0xBB];
        let buf = pl.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(1) + bs(2) + label(3) = 12
        assert_eq!(buf.len(), 12);
        assert_eq!(buf[1], 0x01 | 0x20); // type low + has_label
        assert_eq!(buf[9], 0x42); // label after bitstream
    }

    #[test]
    fn test_polyline_ext_boundary_127_bytes() {
        // 127 bytes = 0x7F → single-byte encoding: (127 << 1) | 1 = 0xFF
        let points = vec![Coord::new(0, 0), Coord::new(10, 10)];
        let pl = Polyline::new(0x10601, points);
        let bitstream = vec![0u8; 127];
        let buf = pl.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(1) + bs(127) = 134
        assert_eq!(buf.len(), 134);
        assert_eq!(buf[6], 0xFF); // (127 << 1) | 1 = 255 = 0xFF
    }

    #[test]
    fn test_polyline_ext_2byte_length_128() {
        // 128 bytes = 0x80 → 2-byte encoding: (128 << 2) | 2 = 514
        let points = vec![Coord::new(0, 0), Coord::new(10, 10)];
        let pl = Polyline::new(0x10601, points);
        let bitstream = vec![0u8; 128];
        let buf = pl.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(2) + bs(128) = 136
        assert_eq!(buf.len(), 136);
        let len_encoded = u16::from_le_bytes([buf[6], buf[7]]);
        assert_eq!(len_encoded, (128 << 2) | 2); // = 514
    }

    #[test]
    fn test_polyline_ext_2byte_length_large() {
        // 300 bytes → 2-byte encoding: (300 << 2) | 2 = 1202
        let points = vec![Coord::new(0, 0), Coord::new(10, 10)];
        let pl = Polyline::new(0x10601, points);
        let bitstream = vec![0u8; 300];
        let buf = pl.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(2) + bs(300) = 308
        assert_eq!(buf.len(), 308);
        let len_encoded = u16::from_le_bytes([buf[6], buf[7]]);
        assert_eq!(len_encoded, (300 << 2) | 2); // = 1202
    }

    #[test]
    fn test_write_with_net_info() {
        let points = vec![Coord::new(100, 200), Coord::new(110, 210)];
        let mut pl = Polyline::new(0x01, points);
        pl.has_net_info = true;
        pl.net_offset = 0x123456;
        let bitstream = vec![0xAA, 0xBB];
        let buf = pl.write(0, 0, 0, &bitstream, false);

        // label should have 0x800000 flag
        let lbl = u32::from_le_bytes([buf[1], buf[2], buf[3], 0]);
        assert!(lbl & 0x800000 != 0, "has_net_info flag should be set in label");

        // blen includes NET offset: (2 + 3) - 1 = 4
        assert_eq!(buf[8], 4, "blen should be bitstream(2) + net_offset(3) - 1 = 4");

        // Last 3 bytes should be net_offset (little-endian)
        let len = buf.len();
        assert_eq!(buf[len - 3], 0x56);
        assert_eq!(buf[len - 2], 0x34);
        assert_eq!(buf[len - 1], 0x12);

        // Total: type(1) + label(3) + dx(2) + dy(2) + blen(1) + bitstream(2) + net_offset(3) = 14
        assert_eq!(len, 14);
    }

    #[test]
    fn test_write_without_net_info() {
        let points = vec![Coord::new(100, 200), Coord::new(110, 210)];
        let pl = Polyline::new(0x01, points);
        let bitstream = vec![0xAA, 0xBB];
        let buf_without = pl.write(0, 0, 0, &bitstream, false);
        // type(1) + label(3) + dx(2) + dy(2) + blen(1) + bitstream(2) = 11
        assert_eq!(buf_without.len(), 11);
    }

    #[test]
    fn test_polyline_2byte_length() {
        let pl = Polyline::new(0x01, vec![Coord::new(0, 0)]);
        let bitstream = vec![0u8; 300];
        let buf = pl.write(0, 0, 0, &bitstream, false);
        assert!(buf[0] & 0x80 != 0); // 2-byte length flag
        let blen = u16::from_le_bytes([buf[8], buf[9]]);
        assert_eq!(blen, 299); // Garmin convention: stored as actual_bytes - 1
    }
}
