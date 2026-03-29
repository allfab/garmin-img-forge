// RGNFile — RGN subfile, faithful to mkgmap RGNFile.java + RGNHeader.java

use super::common_header::{self, CommonHeader};

pub const RGN_HEADER_LEN: u16 = 125;

/// RGN file writer
pub struct RgnWriter {
    /// RGN data (per-subdivision records)
    data: Vec<u8>,
}

impl RgnWriter {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }

    /// Current write position (offset from start of data section)
    pub fn position(&self) -> u32 {
        self.data.len() as u32
    }

    /// Write subdivision data block
    /// Order: Points → IndPoints → Polylines → Polygons
    /// Optional 2-byte pointers at start for each section after the first present
    pub fn write_subdivision(
        &mut self,
        points_data: &[u8],
        ind_points_data: &[u8],
        polylines_data: &[u8],
        polygons_data: &[u8],
    ) -> u32 {
        let start_offset = self.data.len() as u32;

        // Determine which sections are present
        let has_points = !points_data.is_empty();
        let has_ind_points = !ind_points_data.is_empty();
        let has_polylines = !polylines_data.is_empty();
        let has_polygons = !polygons_data.is_empty();

        // Count pointer sections needed
        let mut num_pointers = 0u16;
        if has_ind_points && has_points { num_pointers += 1; }
        if has_polylines && (has_points || has_ind_points) { num_pointers += 1; }
        if has_polygons && (has_points || has_ind_points || has_polylines) { num_pointers += 1; }

        let pointers_size = num_pointers as usize * 2;

        // Calculate offsets for each section (relative to subdivision start in RGN data)
        let mut current = pointers_size;
        let points_off = current;
        current += points_data.len();
        let ind_points_off = current;
        current += ind_points_data.len();
        let polylines_off = current;
        current += polylines_data.len();
        let _polygons_off = current;

        // Write pointers (2-byte offsets relative to subdivision start)
        if has_ind_points && has_points {
            self.data.extend_from_slice(&(ind_points_off as u16).to_le_bytes());
        }
        if has_polylines && (has_points || has_ind_points) {
            self.data.extend_from_slice(&(polylines_off as u16).to_le_bytes());
        }
        if has_polygons && (has_points || has_ind_points || has_polylines) {
            let poly_off = polylines_off + polylines_data.len();
            self.data.extend_from_slice(&(poly_off as u16).to_le_bytes());
        }

        // Write section data
        if has_points {
            self.data.extend_from_slice(points_data);
        }
        if has_ind_points {
            self.data.extend_from_slice(ind_points_data);
        }
        if has_polylines {
            self.data.extend_from_slice(polylines_data);
        }
        if has_polygons {
            self.data.extend_from_slice(polygons_data);
        }

        start_offset
    }

    /// Build the complete RGN subfile bytes
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // CommonHeader (21 bytes)
        let common = CommonHeader::new(RGN_HEADER_LEN, "GARMIN RGN");
        common.write(&mut buf);

        // Data section: offset(4) + size(4)
        common_header::write_section(&mut buf, RGN_HEADER_LEN as u32, self.data.len() as u32);

        // Pad to 125 bytes
        common_header::pad_to(&mut buf, RGN_HEADER_LEN as usize);

        // Append RGN data
        buf.extend_from_slice(&self.data);

        buf
    }

    pub fn data_size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgn_header_size() {
        let rgn = RgnWriter::new();
        let data = rgn.build();
        assert!(data.len() >= RGN_HEADER_LEN as usize);
    }

    #[test]
    fn test_rgn_common_header() {
        let rgn = RgnWriter::new();
        let data = rgn.build();
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, RGN_HEADER_LEN);
        assert_eq!(&data[2..12], b"GARMIN RGN");
    }

    #[test]
    fn test_rgn_write_subdivision() {
        let mut rgn = RgnWriter::new();
        let points = vec![0x01, 0x02, 0x03];
        let offset = rgn.write_subdivision(&points, &[], &[], &[]);
        assert_eq!(offset, 0);
        assert_eq!(rgn.data_size(), 3);
    }

    #[test]
    fn test_rgn_multiple_sections() {
        let mut rgn = RgnWriter::new();
        let points = vec![0x01; 10];
        let polylines = vec![0x02; 20];
        let offset = rgn.write_subdivision(&points, &[], &polylines, &[]);
        assert_eq!(offset, 0);
        // 2 bytes pointer + 10 points + 20 polylines = 32
        assert_eq!(rgn.data_size(), 32);
    }

    #[test]
    fn test_rgn_data_section_offset() {
        let mut rgn = RgnWriter::new();
        rgn.write_subdivision(&[0xFF], &[], &[], &[]);
        let data = rgn.build();

        // Data section offset at byte 21
        let offset = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
        assert_eq!(offset, RGN_HEADER_LEN as u32);

        // Data section size
        let size = u32::from_le_bytes([data[25], data[26], data[27], data[28]]);
        assert_eq!(size, 1);

        // Actual data
        assert_eq!(data[RGN_HEADER_LEN as usize], 0xFF);
    }
}
