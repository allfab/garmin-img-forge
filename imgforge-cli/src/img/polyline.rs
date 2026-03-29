// Polyline — polyline encoding, faithful to mkgmap Polyline.java

use super::coord::Coord;
use super::map_object::MapObject;

/// A polyline on the map
#[derive(Debug, Clone)]
pub struct Polyline {
    pub type_code: u16,
    pub label_offset: u32,
    pub points: Vec<Coord>,
    pub direction: bool,
    pub has_net_info: bool,
    pub net_offset: u32,
    pub road_id: Option<u32>,
}

impl MapObject for Polyline {
    fn type_code(&self) -> u16 { self.type_code }
    fn label_offset(&self) -> u32 { self.label_offset }
    fn coords(&self) -> &[Coord] { &self.points }
}

impl Polyline {
    pub fn new(type_code: u16, points: Vec<Coord>) -> Self {
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

        // Type byte with flags
        let mut type_byte = (self.type_code & 0xFF) as u8;
        if self.direction {
            type_byte |= 0x40;
        }
        if bitstream.len() >= 256 {
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

        // First point delta from subdivision center
        let first = &self.points[0];
        let dx = ((first.longitude() - subdiv_center_lon) >> shift) as i16;
        let dy = ((first.latitude() - subdiv_center_lat) >> shift) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());

        // Bitstream length
        if bitstream.len() >= 256 {
            buf.extend_from_slice(&(bitstream.len() as u16).to_le_bytes());
        } else {
            buf.push(bitstream.len() as u8);
        }

        // Bitstream data
        buf.extend_from_slice(bitstream);

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
        // blen = 2
        assert_eq!(buf[8], 2);
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
    fn test_polyline_2byte_length() {
        let pl = Polyline::new(0x01, vec![Coord::new(0, 0)]);
        let bitstream = vec![0u8; 300];
        let buf = pl.write(0, 0, 0, &bitstream, false);
        assert!(buf[0] & 0x80 != 0); // 2-byte length flag
        let blen = u16::from_le_bytes([buf[8], buf[9]]);
        assert_eq!(blen, 300);
    }
}
