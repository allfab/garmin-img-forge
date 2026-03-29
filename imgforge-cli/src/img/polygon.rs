// Polygon — polygon encoding, faithful to mkgmap Polygon.java
// Polygons are encoded identically to polylines in RGN format

use super::coord::Coord;
use super::map_object::MapObject;

/// A polygon on the map
#[derive(Debug, Clone)]
pub struct Polygon {
    pub type_code: u16,
    pub label_offset: u32,
    pub points: Vec<Coord>,
}

impl MapObject for Polygon {
    fn type_code(&self) -> u16 { self.type_code }
    fn label_offset(&self) -> u32 { self.label_offset }
    fn coords(&self) -> &[Coord] { &self.points }
}

impl Polygon {
    pub fn new(type_code: u16, points: Vec<Coord>) -> Self {
        Self {
            type_code,
            label_offset: 0,
            points,
        }
    }

    /// Write polygon record — same format as polyline
    pub fn write(
        &self,
        subdiv_center_lat: i32,
        subdiv_center_lon: i32,
        shift: i32,
        bitstream: &[u8],
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8 + bitstream.len());

        let mut type_byte = (self.type_code & 0xFF) as u8;
        if bitstream.len() >= 256 {
            type_byte |= 0x80;
        }
        buf.push(type_byte);

        let lb = self.label_offset.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        let first = &self.points[0];
        let dx = ((first.longitude() - subdiv_center_lon) >> shift) as i16;
        let dy = ((first.latitude() - subdiv_center_lat) >> shift) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());

        if bitstream.len() >= 256 {
            buf.extend_from_slice(&(bitstream.len() as u16).to_le_bytes());
        } else {
            buf.push(bitstream.len() as u8);
        }

        buf.extend_from_slice(bitstream);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polygon_write() {
        let points = vec![
            Coord::new(0, 0),
            Coord::new(100, 0),
            Coord::new(100, 100),
            Coord::new(0, 100),
        ];
        let pg = Polygon::new(0x03, points);
        let bitstream = vec![0x12, 0x34];
        let buf = pg.write(0, 0, 0, &bitstream);
        assert_eq!(buf[0], 0x03);
        assert_eq!(buf.len(), 11); // type(1)+label(3)+dx(2)+dy(2)+blen(1)+bs(2)
    }
}
