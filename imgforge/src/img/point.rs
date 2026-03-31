// Point — POI encoding, faithful to mkgmap Point.java

use super::coord::Coord;
use super::map_object::MapObject;

/// A point/POI on the map
#[derive(Debug, Clone)]
pub struct Point {
    pub type_code: u32,
    pub sub_type: u8,
    pub label_offset: u32,
    pub coord: Coord,
    pub is_poi: bool,
    pub has_sub_type: bool,
}

impl MapObject for Point {
    fn type_code(&self) -> u32 { self.type_code }
    fn sub_type(&self) -> u8 { self.sub_type }
    fn label_offset(&self) -> u32 { self.label_offset }
    fn coords(&self) -> &[Coord] { std::slice::from_ref(&self.coord) }
}

impl Point {
    pub fn new(type_code: u32, coord: Coord) -> Self {
        Self {
            type_code,
            sub_type: 0,
            label_offset: 0,
            coord,
            is_poi: false,
            has_sub_type: false,
        }
    }

    /// Write standard point record — mkgmap Point.java
    /// Format: type(1B) + label_offset(3B) + delta_lon(i16) + delta_lat(i16) + [subtype(1B)]
    pub fn write(&self, subdiv_center_lat: i32, subdiv_center_lon: i32, shift: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(9);

        // Type (1 byte, low byte of type_code)
        buf.push(self.type_code as u8);

        // Label offset (3 bytes) with flags
        let mut lbl = self.label_offset;
        if self.is_poi {
            lbl |= 0x400000;
        }
        if self.has_sub_type {
            lbl |= 0x800000;
        }
        let lb = lbl.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        // Delta longitude (i16 LE, clamped to avoid overflow)
        let dx = ((self.coord.longitude() - subdiv_center_lon) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());

        // Delta latitude (i16 LE, clamped to avoid overflow)
        let dy = ((self.coord.latitude() - subdiv_center_lat) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dy.to_le_bytes());

        // Subtype if flagged
        if self.has_sub_type {
            buf.push(self.sub_type);
        }

        buf
    }

    /// Write extended point record — mkgmap Point.java (extended type path)
    /// Format: type(2B big-endian) + dx(2B LE) + dy(2B LE) + [label(3B)]
    pub fn write_ext(&self, subdiv_center_lat: i32, subdiv_center_lon: i32, shift: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(10);

        // Type: 2 bytes big-endian
        // First byte: high byte of type_code (type_code >> 8) & 0xFF
        // Second byte: (type_code & 0x1F) | flags
        let type_high = ((self.type_code >> 8) & 0xFF) as u8;
        let mut type_low = (self.type_code & 0x1F) as u8;
        let has_label = self.label_offset > 0;
        if has_label {
            type_low |= 0x20;
        }
        buf.push(type_high);
        buf.push(type_low);

        // Delta longitude (i16 LE)
        let dx = ((self.coord.longitude() - subdiv_center_lon) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());

        // Delta latitude (i16 LE)
        let dy = ((self.coord.latitude() - subdiv_center_lat) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dy.to_le_bytes());

        // Label (3 bytes) if has_label
        if has_label {
            let lb = self.label_offset.to_le_bytes();
            buf.push(lb[0]);
            buf.push(lb[1]);
            buf.push(lb[2]);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_basic_8_bytes() {
        let p = Point::new(0x2C, Coord::new(100, 200));
        let buf = p.write(0, 0, 0);
        assert_eq!(buf.len(), 8); // type + label(3) + dx(2) + dy(2)
    }

    #[test]
    fn test_point_with_subtype_9_bytes() {
        let mut p = Point::new(0x2C, Coord::new(100, 200));
        p.has_sub_type = true;
        p.sub_type = 0x05;
        let buf = p.write(0, 0, 0);
        assert_eq!(buf.len(), 9);
        assert_eq!(buf[8], 0x05);
    }

    #[test]
    fn test_point_type_byte() {
        let p = Point::new(0x2C, Coord::new(0, 0));
        let buf = p.write(0, 0, 0);
        assert_eq!(buf[0], 0x2C);
    }

    #[test]
    fn test_point_poi_flag() {
        let mut p = Point::new(0x01, Coord::new(0, 0));
        p.is_poi = true;
        let buf = p.write(0, 0, 0);
        // Label offset byte 2 should have bit 6 set (0x40)
        assert!(buf[3] & 0x40 != 0);
    }

    #[test]
    fn test_point_ext_no_label() {
        // Type 0x2C04 → high=0x2C, low=0x04
        let p = Point::new(0x2C04, Coord::new(100, 200));
        let buf = p.write_ext(0, 0, 0);
        assert_eq!(buf.len(), 6); // type(2) + dx(2) + dy(2)
        assert_eq!(buf[0], 0x2C); // type high
        assert_eq!(buf[1], 0x04); // type low, no flags
    }

    #[test]
    fn test_point_ext_with_label() {
        let mut p = Point::new(0x2C04, Coord::new(100, 200));
        p.label_offset = 0x42;
        let buf = p.write_ext(0, 0, 0);
        assert_eq!(buf.len(), 9); // type(2) + dx(2) + dy(2) + label(3)
        assert_eq!(buf[0], 0x2C); // type high
        assert_eq!(buf[1], 0x04 | 0x20); // type low + has_label flag
        assert_eq!(buf[6], 0x42); // label offset low byte
    }

    #[test]
    fn test_point_delta_coords() {
        let p = Point::new(0x01, Coord::new(150, 250));
        let buf = p.write(100, 200, 0);
        let dx = i16::from_le_bytes([buf[4], buf[5]]);
        let dy = i16::from_le_bytes([buf[6], buf[7]]);
        assert_eq!(dx, 50);
        assert_eq!(dy, 50);
    }
}
