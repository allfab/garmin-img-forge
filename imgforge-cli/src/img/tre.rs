// TREFile — TRE subfile, faithful to mkgmap TREFile.java + TREHeader.java

use super::common_header::{self, CommonHeader};
use super::overview::{PointOverview, PolylineOverview, PolygonOverview};
use super::subdivision::Subdivision;
use super::zoom::Zoom;

pub const TRE_HEADER_LEN: u16 = 188;

/// TRE file writer
pub struct TreWriter {
    /// Map bounds in 24-bit map units
    pub north: i32,
    pub south: i32,
    pub east: i32,
    pub west: i32,
    /// Map levels (zoom definitions)
    pub levels: Vec<Zoom>,
    /// Subdivisions
    pub subdivisions: Vec<Subdivision>,
    /// Overviews
    pub polyline_overviews: Vec<PolylineOverview>,
    pub polygon_overviews: Vec<PolygonOverview>,
    pub point_overviews: Vec<PointOverview>,
    /// Display priority
    pub display_priority: u32,
    /// Copyright strings as LBL offsets
    pub copyright_offsets: Vec<u32>,
}

impl TreWriter {
    pub fn new() -> Self {
        Self {
            north: 0,
            south: 0,
            east: 0,
            west: 0,
            levels: Vec::new(),
            subdivisions: Vec::new(),
            polyline_overviews: Vec::new(),
            polygon_overviews: Vec::new(),
            point_overviews: Vec::new(),
            display_priority: 0x19,
            copyright_offsets: Vec::new(),
        }
    }

    pub fn set_bounds(&mut self, south: i32, west: i32, north: i32, east: i32) {
        self.south = south;
        self.west = west;
        self.north = north;
        self.east = east;
    }

    /// Build the complete TRE subfile bytes
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Build sections first to know their sizes
        let map_levels_data = self.build_map_levels();
        let subdivisions_data = self.build_subdivisions();
        let copyright_data = self.build_copyrights();
        let polyline_ov_data = self.build_polyline_overviews();
        let polygon_ov_data = self.build_polygon_overviews();
        let point_ov_data = self.build_point_overviews();

        // --- Header (188 bytes) ---
        let common = CommonHeader::new(TRE_HEADER_LEN, "GARMIN TRE");
        common.write(&mut buf);

        // Bounds: north(3) + east(3) + south(3) + west(3) = 12 bytes at offset 21
        common_header::write_i24(&mut buf, self.north);
        common_header::write_i24(&mut buf, self.east);
        common_header::write_i24(&mut buf, self.south);
        common_header::write_i24(&mut buf, self.west);

        // Map levels section: offset(4) + size(4) at offset 33
        let mut current_offset = TRE_HEADER_LEN as u32;
        common_header::write_section(&mut buf, current_offset, map_levels_data.len() as u32);
        current_offset += map_levels_data.len() as u32;

        // Subdivisions section at offset 41
        common_header::write_section(&mut buf, current_offset, subdivisions_data.len() as u32);
        current_offset += subdivisions_data.len() as u32;

        // Copyright section at offset 49
        common_header::write_section(&mut buf, current_offset, copyright_data.len() as u32);
        current_offset += copyright_data.len() as u32;

        // POI display flags
        buf.push(0x00);

        // Display priority (4 bytes)
        buf.extend_from_slice(&self.display_priority.to_le_bytes());

        // Polyline overview section at offset 74
        common_header::pad_to(&mut buf, 74);
        common_header::write_section(&mut buf, current_offset, polyline_ov_data.len() as u32);
        buf.extend_from_slice(&2u16.to_le_bytes());
        current_offset += polyline_ov_data.len() as u32;

        // Polygon overview section at offset 84
        common_header::pad_to(&mut buf, 84);
        common_header::write_section(&mut buf, current_offset, polygon_ov_data.len() as u32);
        buf.extend_from_slice(&2u16.to_le_bytes());
        current_offset += polygon_ov_data.len() as u32;

        // Point overview section at offset 94
        common_header::pad_to(&mut buf, 94);
        common_header::write_section(&mut buf, current_offset, point_ov_data.len() as u32);
        buf.extend_from_slice(&3u16.to_le_bytes());

        // Pad to exactly TRE_HEADER_LEN
        common_header::pad_to(&mut buf, TRE_HEADER_LEN as usize);

        // --- Section data ---
        buf.extend_from_slice(&map_levels_data);
        buf.extend_from_slice(&subdivisions_data);
        buf.extend_from_slice(&copyright_data);
        buf.extend_from_slice(&polyline_ov_data);
        buf.extend_from_slice(&polygon_ov_data);
        buf.extend_from_slice(&point_ov_data);

        buf
    }

    fn build_map_levels(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for level in &self.levels {
            let count = self.subdivisions.iter()
                .filter(|s| s.zoom_level == level.level)
                .count();
            // Use the level's own inherited flag (set by caller)
            let rec = level.write(count as u16);
            data.extend_from_slice(&rec);
        }
        data
    }

    fn build_subdivisions(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for subdiv in &self.subdivisions {
            data.extend_from_slice(&subdiv.write());
        }
        data
    }

    fn build_copyrights(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for &offset in &self.copyright_offsets {
            let b = offset.to_le_bytes();
            data.push(b[0]);
            data.push(b[1]);
            data.push(b[2]);
        }
        data
    }

    fn build_polyline_overviews(&self) -> Vec<u8> {
        let mut sorted = self.polyline_overviews.clone();
        sorted.sort();
        sorted.iter().flat_map(|o| o.write()).collect()
    }

    fn build_polygon_overviews(&self) -> Vec<u8> {
        let mut sorted = self.polygon_overviews.clone();
        sorted.sort();
        sorted.iter().flat_map(|o| o.write()).collect()
    }

    fn build_point_overviews(&self) -> Vec<u8> {
        let mut sorted = self.point_overviews.clone();
        sorted.sort();
        sorted.iter().flat_map(|o| o.write()).collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tre_header_size() {
        let tre = TreWriter::new();
        let data = tre.build();
        assert!(data.len() >= TRE_HEADER_LEN as usize);
    }

    #[test]
    fn test_tre_common_header() {
        let tre = TreWriter::new();
        let data = tre.build();
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, TRE_HEADER_LEN);
        assert_eq!(&data[2..12], b"GARMIN TRE");
    }

    #[test]
    fn test_tre_bounds() {
        let mut tre = TreWriter::new();
        tre.set_bounds(-100, -200, 300, 400);
        let data = tre.build();

        // Bounds at offset 21: north(3) + east(3) + south(3) + west(3)
        let north = i24_from_le(&data[21..24]);
        let east = i24_from_le(&data[24..27]);
        let south = i24_from_le(&data[27..30]);
        let west = i24_from_le(&data[30..33]);

        assert_eq!(north, 300);
        assert_eq!(east, 400);
        assert_eq!(south, -100);
        assert_eq!(west, -200);
    }

    #[test]
    fn test_tre_with_levels() {
        let mut tre = TreWriter::new();
        tre.levels.push(Zoom::new(0, 24));
        tre.levels.push(Zoom::new(1, 20));
        let data = tre.build();
        assert!(data.len() > TRE_HEADER_LEN as usize);
    }

    fn i24_from_le(bytes: &[u8]) -> i32 {
        let val = bytes[0] as i32 | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
        // Sign extend from 24-bit
        if val & 0x800000 != 0 {
            val | !0xFFFFFF
        } else {
            val
        }
    }
}
