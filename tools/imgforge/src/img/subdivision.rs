// Subdivision — hierarchical tree structure, faithful to mkgmap Subdivision.java

use super::coord::Coord;

// Content type flags — mkgmap Subdivision
pub const HAS_POINTS: u8 = 0x10;
pub const HAS_IND_POINTS: u8 = 0x20;
pub const HAS_POLYLINES: u8 = 0x40;
pub const HAS_POLYGONS: u8 = 0x80;

/// Subdivision record size: 14 bytes without children, 16 with (end pointer)
pub const SUBDIV_REC_SIZE: usize = 14;
pub const SUBDIV_REC_SIZE_WITH_END: usize = 16;

/// A subdivision in the TRE hierarchy
#[derive(Debug, Clone)]
pub struct Subdivision {
    /// 1-based subdivision number (for referencing)
    pub number: u16,
    /// Zoom level this subdivision belongs to
    pub zoom_level: u8,
    /// Resolution (bits per coord)
    pub resolution: u8,
    /// Center latitude in 24-bit map units
    pub center_lat: i32,
    /// Center longitude in 24-bit map units
    pub center_lon: i32,
    /// Width (in shifted units: actual_width >> (24 - resolution))
    pub width: u16,
    /// Height (in shifted units)
    pub height: u16,
    /// RGN data offset for this subdivision
    pub rgn_offset: u32,
    /// Content flags
    pub flags: u8,
    /// End offset in RGN (written as next_level pointer for subdivs with children)
    pub end_rgn_offset: u32,
    /// Whether this subdivision has children
    pub has_children: bool,
    /// Whether this is the last subdivision in its level
    pub is_last: bool,
    /// Parent subdivision number (0 if root)
    pub parent: u16,
    /// Children subdivision numbers
    pub children: Vec<u16>,
}

impl Subdivision {
    pub fn new(number: u16, zoom_level: u8, resolution: u8) -> Self {
        Self {
            number,
            zoom_level,
            resolution,
            center_lat: 0,
            center_lon: 0,
            width: 0,
            height: 0,
            rgn_offset: 0,
            flags: 0,
            end_rgn_offset: 0,
            has_children: false,
            is_last: false,
            parent: 0,
            children: Vec::new(),
        }
    }

    /// Set center from a Coord, aligned to 2^shift — mkgmap Subdivision
    pub fn set_center(&mut self, coord: &Coord) {
        let shift = (24 - self.resolution) as i32;
        self.center_lat = align_to_shift(coord.latitude(), shift);
        self.center_lon = align_to_shift(coord.longitude(), shift);
    }

    /// Set bounds from an Area, stored as shifted width/height
    pub fn set_bounds(&mut self, min_lat: i32, min_lon: i32, max_lat: i32, max_lon: i32) {
        let shift = 24 - self.resolution as i32;
        let w = ((max_lon - min_lon) >> shift) as u16;
        let h = ((max_lat - min_lat) >> shift) as u16;
        self.width = if w == 0 { 1 } else { w };
        self.height = if h == 0 { 1 } else { h };
    }

    /// Round a latitude to local shifted coords — mkgmap Subdivision.roundLatToLocalShifted
    pub fn round_lat_to_local_shifted(&self, lat: i32) -> i32 {
        let shift = 24 - self.resolution as i32;
        let val = lat - self.center_lat + ((1 << shift) / 2);
        val >> shift
    }

    /// Round a longitude to local shifted coords — mkgmap Subdivision.roundLonToLocalShifted
    pub fn round_lon_to_local_shifted(&self, lon: i32) -> i32 {
        let shift = 24 - self.resolution as i32;
        let val = lon - self.center_lon + ((1 << shift) / 2);
        val >> shift
    }

    /// Write subdivision record — mkgmap format
    /// 14 bytes without end pointer, 16 bytes with end pointer
    pub fn write(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(SUBDIV_REC_SIZE_WITH_END);

        // RGN offset (3 bytes LE)
        let off_bytes = self.rgn_offset.to_le_bytes();
        buf.push(off_bytes[0]);
        buf.push(off_bytes[1]);
        buf.push(off_bytes[2]);

        // Content flags (1 byte)
        buf.push(self.flags);

        // Center longitude (3 bytes LE, signed 24-bit)
        let lon_bytes = self.center_lon.to_le_bytes();
        buf.push(lon_bytes[0]);
        buf.push(lon_bytes[1]);
        buf.push(lon_bytes[2]);

        // Center latitude (3 bytes LE, signed 24-bit)
        let lat_bytes = self.center_lat.to_le_bytes();
        buf.push(lat_bytes[0]);
        buf.push(lat_bytes[1]);
        buf.push(lat_bytes[2]);

        // Width (2 bytes): MSB of first byte is "last subdivision" flag
        let mut w = self.width;
        if self.is_last {
            w |= 0x8000;
        }
        buf.extend_from_slice(&w.to_le_bytes());

        // Height (2 bytes)
        buf.extend_from_slice(&self.height.to_le_bytes());

        // Next-level start pointer (2 bytes = subdiv number, for subdivisions with children)
        if self.has_children {
            // This is the 1-based index of the first child subdivision, not RGN offset
            let first_child = self.children.first().copied().unwrap_or(0);
            buf.extend_from_slice(&first_child.to_le_bytes());
        }

        buf
    }

    pub fn record_size(&self) -> usize {
        if self.has_children {
            SUBDIV_REC_SIZE_WITH_END
        } else {
            SUBDIV_REC_SIZE
        }
    }
}

/// Align value to 2^shift boundary
fn align_to_shift(val: i32, shift: i32) -> i32 {
    if shift <= 0 {
        return val;
    }
    let half = 1 << (shift - 1);
    ((val + half) >> shift) << shift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_size() {
        let mut s = Subdivision::new(1, 0, 24);
        assert_eq!(s.record_size(), 14);
        s.has_children = true;
        assert_eq!(s.record_size(), 16);
    }

    #[test]
    fn test_write_14_bytes() {
        let s = Subdivision::new(1, 0, 24);
        let buf = s.write();
        assert_eq!(buf.len(), 14);
    }

    #[test]
    fn test_write_16_bytes_with_children() {
        let mut s = Subdivision::new(1, 0, 24);
        s.has_children = true;
        let buf = s.write();
        assert_eq!(buf.len(), 16);
    }

    #[test]
    fn test_last_flag_in_width() {
        let mut s = Subdivision::new(1, 0, 24);
        s.width = 100;
        s.is_last = true;
        let buf = s.write();
        let w = u16::from_le_bytes([buf[10], buf[11]]);
        assert!(w & 0x8000 != 0);
        assert_eq!(w & 0x7FFF, 100);
    }

    #[test]
    fn test_local_coords() {
        let mut s = Subdivision::new(1, 0, 20);
        s.center_lat = 2_000_000;
        s.center_lon = 1_000_000;

        // shift = 24 - 20 = 4, half = 8
        // (16 + 8) >> 4 = 1
        let local = s.round_lat_to_local_shifted(2_000_016);
        assert_eq!(local, 1);

        // Rounding bias edge case: without bias (9 >> 4 = 0), with bias (9+8 >> 4 = 1)
        let biased = s.round_lat_to_local_shifted(2_000_009);
        assert_eq!(biased, 1); // rounding bias rounds up
    }

    #[test]
    fn test_set_center_alignment() {
        let mut s = Subdivision::new(1, 0, 20);
        let c = Coord::new(12345, 67890);
        s.set_center(&c);
        // Should be aligned to 2^4 = 16
        assert_eq!(s.center_lat % 16, 0);
        assert_eq!(s.center_lon % 16, 0);
    }
}
