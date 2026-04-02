// TREFile — TRE subfile, faithful to mkgmap TREFile.java + TREHeader.java

use super::common_header::{self, CommonHeader};
use super::overview::{
    PointOverview, PolylineOverview, PolygonOverview,
    ExtPointOverview, ExtPolylineOverview, ExtPolygonOverview,
};
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
    /// Transparent map (overlay)
    pub transparent: bool,
    /// Map ID (written at TRE header offset 116-119)
    pub map_id: u32,
    /// Copyright strings as LBL offsets
    pub copyright_offsets: Vec<u32>,
    /// Last RGN position (relative to body) — written as 4-byte terminator after subdivisions
    pub last_rgn_pos: u32,
    /// Extended overviews
    pub ext_point_overviews: Vec<ExtPointOverview>,
    pub ext_polyline_overviews: Vec<ExtPolylineOverview>,
    pub ext_polygon_overviews: Vec<ExtPolygonOverview>,
    /// Extended type offsets data (built externally)
    pub ext_type_offsets_data: Vec<u8>,
    /// Copyright text blob (written between header and section data, read by QMapShack for tooltip)
    /// Note: mkgmap writes BOTH this blob AND the LBL copyright records. The blob is used by
    /// QMapShack for the hover tooltip; the LBL records are the standard Garmin format for GPS devices.
    pub copyright_message: String,
    /// Codepage for encoding the copyright blob (0 or 65001 = UTF-8/ASCII)
    pub codepage: u16,
    /// Whether this map has routing data (NET/NOD subfiles present)
    pub has_routing: bool,
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
            transparent: false,
            map_id: 0,
            copyright_offsets: Vec::new(),
            last_rgn_pos: 0,
            ext_point_overviews: Vec::new(),
            ext_polyline_overviews: Vec::new(),
            ext_polygon_overviews: Vec::new(),
            ext_type_offsets_data: Vec::new(),
            copyright_message: String::new(),
            codepage: 0,
            has_routing: false,
        }
    }

    pub fn set_bounds(&mut self, south: i32, west: i32, north: i32, east: i32) {
        self.south = south;
        self.west = west;
        self.north = north;
        self.east = east;
    }

    /// Build the complete TRE subfile bytes
    /// Layout faithful to mkgmap TREHeader.java:writeFileHeader
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        let has_ext = !self.ext_type_offsets_data.is_empty();

        // Build sections first to know their sizes
        let map_levels_data = self.build_map_levels();
        let subdivisions_data = self.build_subdivisions();
        let copyright_data = self.build_copyrights();
        let polyline_ov_data = self.build_polyline_overviews();
        let polygon_ov_data = self.build_polygon_overviews();
        let point_ov_data = self.build_point_overviews();
        let ext_type_offsets_data = &self.ext_type_offsets_data;
        let ext_type_overviews_data = self.build_ext_type_overviews();

        // --- Header (188 bytes) — mkgmap TREHeader.java layout ---
        let common = CommonHeader::new(TRE_HEADER_LEN, "GARMIN TRE");
        common.write(&mut buf); // 21 bytes (0-20)

        // Bounds: north(3) + east(3) + south(3) + west(3) = 12 bytes @21-32
        common_header::write_i24(&mut buf, self.north);
        common_header::write_i24(&mut buf, self.east);
        common_header::write_i24(&mut buf, self.south);
        common_header::write_i24(&mut buf, self.west);

        // Map levels section: offset(4) + size(4) @33-40
        // Account for copyright text blob between header and section data
        let copyright_blob = self.encode_copyright_blob();
        let mut current_offset = TRE_HEADER_LEN as u32 + copyright_blob.len() as u32;
        common_header::write_section(&mut buf, current_offset, map_levels_data.len() as u32);
        current_offset += map_levels_data.len() as u32;

        // Subdivisions section: offset(4) + size(4) @41-48
        common_header::write_section(&mut buf, current_offset, subdivisions_data.len() as u32);
        current_offset += subdivisions_data.len() as u32;

        // Copyright section: offset(4) + size(4) + itemSize(2) @49-58
        common_header::write_section(&mut buf, current_offset, copyright_data.len() as u32);
        buf.extend_from_slice(&3u16.to_le_bytes()); // itemSize = 3 bytes per copyright entry
        current_offset += copyright_data.len() as u32;

        // Reserved @59-62
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Map properties @63 — mkgmap TREHeader.java
        // bit 0x20 = transparent map (overlay)
        // bit 0x01 = has routing (NET/NOD present)
        let mut map_props: u8 = 0;
        if self.transparent { map_props |= 0x20; }
        if self.has_routing { map_props |= 0x01; }
        buf.push(map_props);

        // Display priority @64-66 (3 bytes, mkgmap put3u)
        common_header::write_u24(&mut buf, self.display_priority);

        // Routing capability marker @67-70 — mkgmap: 0x00110301 for routable maps
        let marker: u32 = if self.has_routing { 0x00110301 } else { 0 };
        buf.extend_from_slice(&marker.to_le_bytes());

        // Reserved @71-72 (2 bytes, value 1 in mkgmap)
        buf.extend_from_slice(&1u16.to_le_bytes());

        // Reserved @73 (1 byte)
        buf.push(0x00);

        // Polyline overview: offset(4) + size(4) + itemSize(2) @74-83
        assert_eq!(buf.len(), 74);
        common_header::write_section(&mut buf, current_offset, polyline_ov_data.len() as u32);
        buf.extend_from_slice(&2u16.to_le_bytes());
        current_offset += polyline_ov_data.len() as u32;

        // Reserved @84-87
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Polygon overview: offset(4) + size(4) + itemSize(2) @88-97
        assert_eq!(buf.len(), 88);
        common_header::write_section(&mut buf, current_offset, polygon_ov_data.len() as u32);
        buf.extend_from_slice(&2u16.to_le_bytes());
        current_offset += polygon_ov_data.len() as u32;

        // Reserved @98-101
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Point overview: offset(4) + size(4) + itemSize(2) @102-111
        assert_eq!(buf.len(), 102);
        common_header::write_section(&mut buf, current_offset, point_ov_data.len() as u32);
        buf.extend_from_slice(&3u16.to_le_bytes());
        current_offset += point_ov_data.len() as u32;

        // Reserved @112-115
        buf.extend_from_slice(&0u32.to_le_bytes());

        // MapID @116-119
        buf.extend_from_slice(&self.map_id.to_le_bytes());

        // Reserved @120-123
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Extended type sections @124+
        if has_ext {
            // extTypeOffsets: offset(4) + size(4) + itemSize(2) @124-133
            assert_eq!(buf.len(), 124);
            common_header::write_section(&mut buf, current_offset, ext_type_offsets_data.len() as u32);
            buf.extend_from_slice(&13u16.to_le_bytes());
            current_offset += ext_type_offsets_data.len() as u32;

            // Magic 0x0607 @134-137 (4 bytes, mkgmap put4)
            buf.extend_from_slice(&0x0607u32.to_le_bytes());

            // extTypeOverviews: offset(4) + size(4) + itemSize(2) @138-147
            assert_eq!(buf.len(), 138);
            common_header::write_section(&mut buf, current_offset, ext_type_overviews_data.len() as u32);
            buf.extend_from_slice(&4u16.to_le_bytes());

            // NumExtType counts @148-153
            let num_ext_lines = self.ext_polyline_overviews.len() as u16;
            let num_ext_areas = self.ext_polygon_overviews.len() as u16;
            let num_ext_points = self.ext_point_overviews.len() as u16;
            buf.extend_from_slice(&num_ext_lines.to_le_bytes());
            buf.extend_from_slice(&num_ext_areas.to_le_bytes());
            buf.extend_from_slice(&num_ext_points.to_le_bytes());
        }

        // Pad to exactly TRE_HEADER_LEN
        common_header::pad_to(&mut buf, TRE_HEADER_LEN as usize);

        // --- Copyright text blob (between header and section data, for QMapShack tooltip) ---
        buf.extend_from_slice(&copyright_blob);

        // --- Section data ---
        buf.extend_from_slice(&map_levels_data);
        buf.extend_from_slice(&subdivisions_data);
        buf.extend_from_slice(&copyright_data);
        buf.extend_from_slice(&polyline_ov_data);
        buf.extend_from_slice(&polygon_ov_data);
        buf.extend_from_slice(&point_ov_data);

        if has_ext {
            buf.extend_from_slice(ext_type_offsets_data);
            buf.extend_from_slice(&ext_type_overviews_data);
        }

        buf
    }

    /// Encode copyright message blob as UTF-8 (QMapShack always reads this blob as UTF-8).
    /// Replaces literal `\n` escape sequences with real newlines (0x0A).
    /// Returns empty vec if no copyright message.
    fn encode_copyright_blob(&self) -> Vec<u8> {
        if self.copyright_message.is_empty() {
            return Vec::new();
        }
        let text = self.copyright_message.replace("\\n", "\n");
        let mut blob = text.into_bytes();
        blob.push(0x00);
        blob
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
        // mkgmap: 4-byte terminator = lastRgnPos (end of last subdivision's RGN data)
        data.extend_from_slice(&self.last_rgn_pos.to_le_bytes());
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

    fn build_ext_type_overviews(&self) -> Vec<u8> {
        let mut data = Vec::new();

        let mut sorted_points = self.ext_point_overviews.clone();
        sorted_points.sort();
        sorted_points.dedup();
        for o in &sorted_points {
            data.extend_from_slice(&o.write());
        }

        let mut sorted_lines = self.ext_polyline_overviews.clone();
        sorted_lines.sort();
        sorted_lines.dedup();
        for o in &sorted_lines {
            data.extend_from_slice(&o.write());
        }

        let mut sorted_polys = self.ext_polygon_overviews.clone();
        sorted_polys.sort();
        sorted_polys.dedup();
        for o in &sorted_polys {
            data.extend_from_slice(&o.write());
        }

        data
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

    #[test]
    fn test_tre_copyright_blob() {
        let mut tre = TreWriter::new();
        tre.copyright_message = "Map data (c) Test\nProgram released under GPL".to_string();
        let data = tre.build();

        // Copyright blob should appear right after the 188-byte header
        let blob_start = TRE_HEADER_LEN as usize;
        let blob = &data[blob_start..blob_start + tre.copyright_message.len()];
        assert_eq!(blob, tre.copyright_message.as_bytes());
        // Null terminator after the blob
        assert_eq!(data[blob_start + tre.copyright_message.len()], 0x00);

        // Map levels section offset (at header offset 33-36) should account for copyright blob
        let ml_offset = u32::from_le_bytes([data[33], data[34], data[35], data[36]]);
        let expected = TRE_HEADER_LEN as u32 + tre.copyright_message.len() as u32 + 1;
        assert_eq!(ml_offset, expected);
    }

    #[test]
    fn test_tre_copyright_blob_always_utf8() {
        let mut tre = TreWriter::new();
        tre.codepage = 1252; // even with CP1252, blob must be UTF-8
        tre.copyright_message = "Données IGN © 2026".to_string();
        let data = tre.build();

        let blob_start = TRE_HEADER_LEN as usize;
        let blob_end = {
            let mut i = blob_start;
            while i < data.len() && data[i] != 0x00 { i += 1; }
            i
        };
        let blob = &data[blob_start..blob_end];
        // Must contain UTF-8 é (c3 a9), NOT CP1252 single-byte
        assert!(blob.windows(2).any(|w| w == [0xc3, 0xa9]),
            "TRE copyright blob must be UTF-8 (é = c3 a9)");
        assert_eq!(std::str::from_utf8(blob).unwrap(), "Données IGN © 2026");
    }

    #[test]
    fn test_tre_copyright_blob_newline_escape() {
        let mut tre = TreWriter::new();
        tre.copyright_message = "Line 1\\nLine 2\\nLine 3".to_string();
        let data = tre.build();

        let blob_start = TRE_HEADER_LEN as usize;
        let blob_end = {
            let mut i = blob_start;
            while i < data.len() && data[i] != 0x00 { i += 1; }
            i
        };
        let blob = &data[blob_start..blob_end];
        assert_eq!(std::str::from_utf8(blob).unwrap(), "Line 1\nLine 2\nLine 3");
        // Should contain real 0x0A newlines, not literal backslash-n
        assert!(blob.contains(&0x0A), "blob should contain real newline bytes");
        assert!(!blob.windows(2).any(|w| w == [b'\\', b'n']),
            "blob should not contain literal \\n");
    }

    #[test]
    fn test_tre_no_copyright_blob() {
        let tre = TreWriter::new();
        let data = tre.build();
        // Map levels section offset should be TRE_HEADER_LEN (no copyright blob)
        let ml_offset = u32::from_le_bytes([data[33], data[34], data[35], data[36]]);
        assert_eq!(ml_offset, TRE_HEADER_LEN as u32);
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
