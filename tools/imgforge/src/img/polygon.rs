// Polygon — polygon encoding, faithful to mkgmap Polygon.java
// Polygons are encoded identically to polylines in RGN format

use super::coord::Coord;
/// A polygon on the map
#[derive(Debug, Clone)]
pub struct Polygon {
    pub type_code: u32,
    pub label_offset: u32,
    pub points: Vec<Coord>,
}

impl Polygon {
    pub fn new(type_code: u32, points: Vec<Coord>) -> Self {
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
        if bitstream.len() > 256 {
            type_byte |= 0x80; // 2-byte length (blen = len-1 >= 256)
        }
        buf.push(type_byte);

        let lb = self.label_offset.to_le_bytes();
        buf.push(lb[0]);
        buf.push(lb[1]);
        buf.push(lb[2]);

        // First point delta with rounding bias — must match compute_deltas
        let first = &self.points[0];
        let half = (1i32 << shift) / 2;
        let dx = ((first.longitude() - subdiv_center_lon + half) >> shift).clamp(-32768, 32767) as i16;
        let dy = ((first.latitude() - subdiv_center_lat + half) >> shift).clamp(-32768, 32767) as i16;
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());

        // Bitstream length — Garmin convention: stored as (actual_bytes - 1)
        let blen = bitstream.len() - 1;
        if blen >= 256 {
            buf.extend_from_slice(&(blen as u16).to_le_bytes());
        } else {
            buf.push(blen as u8);
        }

        buf.extend_from_slice(bitstream);
        buf
    }

    /// Write extended polygon record — same format as extended polyline
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
        buf.push(type_high);
        buf.push(type_low);

        // First point delta with rounding bias — must match compute_deltas
        let first = &self.points[0];
        let half = (1i32 << shift) / 2;
        let dx = ((first.longitude() - subdiv_center_lon + half) >> shift).clamp(-32768, 32767) as i16;
        let dy = ((first.latitude() - subdiv_center_lat + half) >> shift).clamp(-32768, 32767) as i16;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polygon_ext_write() {
        let points = vec![
            Coord::new(0, 0),
            Coord::new(100, 0),
            Coord::new(100, 100),
            Coord::new(0, 100),
        ];
        let pg = Polygon::new(0x10f04, points);
        let bitstream = vec![0x12, 0x34];
        let buf = pg.write_ext(0, 0, 0, &bitstream);

        // type(2) + dx(2) + dy(2) + len(1) + bs(2) = 9
        assert_eq!(buf.len(), 9);
        assert_eq!(buf[0], 0x0f); // type high: (0x10f04 >> 8) & 0xFF = 0x0f
        assert_eq!(buf[1], 0x04); // type low: 0x10f04 & 0x1F = 0x04, no label
        // len encoding: (2 << 1) | 1 = 5
        assert_eq!(buf[6], 5);
    }

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
