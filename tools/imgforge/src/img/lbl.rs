// LBLFile — labels subfile, faithful to mkgmap LBLFile.java + LBLHeader.java

use std::collections::HashMap;
use super::common_header::{self, CommonHeader};
use super::labelenc::{self, LabelEncoding};

pub const LBL_HEADER_LEN: u16 = 196;

pub struct LblWriter {
    encoding: LabelEncoding,
    labels: Vec<u8>,
    label_cache: HashMap<String, u32>,
    codepage: u16,
    sort_id1: u16,
    sort_id2: u16,
}

impl LblWriter {
    pub fn new(encoding: LabelEncoding) -> Self {
        let codepage = match encoding {
            LabelEncoding::Format6 => 0,
            LabelEncoding::Format9(cp) => cp,
            LabelEncoding::Format10 => 65001,
        };
        Self {
            encoding,
            labels: vec![0], // offset 0 is reserved (empty label)
            label_cache: HashMap::new(),
            codepage,
            sort_id1: 0,
            sort_id2: 0,
        }
    }

    pub fn set_sort_ids(&mut self, id1: u16, id2: u16) {
        self.sort_id1 = id1;
        self.sort_id2 = id2;
    }

    /// Add a label and return its offset — mkgmap LBLFile.newLabel
    /// Deduplicates: if same text was already added, returns existing offset.
    pub fn add_label(&mut self, text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }

        if let Some(&offset) = self.label_cache.get(text) {
            return offset;
        }

        let offset = self.labels.len() as u32;
        let encoded = self.encode_label(text);
        self.labels.extend_from_slice(&encoded);
        self.label_cache.insert(text.to_string(), offset);
        offset
    }

    fn encode_label(&self, text: &str) -> Vec<u8> {
        match self.encoding {
            LabelEncoding::Format6 => labelenc::format6::encode(text),
            LabelEncoding::Format9(cp) => labelenc::format9::encode(text, cp),
            LabelEncoding::Format10 => labelenc::format10::encode(text),
        }
    }

    /// Build the complete LBL subfile bytes
    /// Layout faithful to mkgmap LBLHeader.writeFileHeader + PlacesHeader.writeFileHeader
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // CommonHeader (21 bytes)
        let common = CommonHeader::new(LBL_HEADER_LEN, "GARMIN LBL");
        common.write(&mut buf);

        let label_data_offset = LBL_HEADER_LEN as u32;
        let label_data_size = self.labels.len() as u32;
        // All empty PlacesHeader sections point to the end of label data
        let label_end = label_data_offset + label_data_size;

        // --- LBL1 section: label data offset + size (offset 21-28) ---
        common_header::write_section(&mut buf, label_data_offset, label_data_size);

        // Label offset multiplier (offset 29, 1 byte)
        buf.push(0x00);

        // Encoding format (offset 30, 1 byte)
        buf.push(self.encoding.format_id());

        // --- PlacesHeader (offset 31-169) — mkgmap PlacesHeader.writeFileHeader ---
        // All sections empty but with valid offsets pointing to label_end.
        // Each "standard" section: offset(4) + size(4) + recSize(2) + reserved(4) = 14 bytes

        // Country section (offset 31, rec_len=3)
        write_empty_section(&mut buf, label_end, 3);
        // Region section (offset 45, rec_len=5)
        write_empty_section(&mut buf, label_end, 5);
        // City section (offset 59, rec_len=5)
        write_empty_section(&mut buf, label_end, 5);
        // POI index section (offset 73, rec_len=4)
        write_empty_section(&mut buf, label_end, 4);

        // POI properties section (offset 87) — special layout: offset(4) + size(4) + mult(1) + flags(1) + reserved(2) + reserved(1) = 13 bytes
        buf.extend_from_slice(&label_end.to_le_bytes()); // offset
        buf.extend_from_slice(&0u32.to_le_bytes());      // size
        buf.push(0x00);                                   // offset multiplier
        buf.push(0x00);                                   // POI global flags
        buf.extend_from_slice(&0u16.to_le_bytes());       // reserved
        buf.push(0x00);                                   // reserved

        // POI type index section (offset 100, rec_len=4)
        write_empty_section(&mut buf, label_end, 4);
        // Zip section (offset 114, rec_len=3)
        write_empty_section(&mut buf, label_end, 3);
        // Highway section (offset 128, rec_len=6)
        write_empty_section(&mut buf, label_end, 6);
        // Exit facility section (offset 142, rec_len=5)
        write_empty_section(&mut buf, label_end, 5);
        // Highway data section (offset 156, rec_len=3)
        write_empty_section(&mut buf, label_end, 3);

        // --- Post-PlacesHeader fields (offset 170-195) ---
        assert_eq!(buf.len(), 170);

        // Codepage (offset 170, 2 bytes)
        buf.extend_from_slice(&self.codepage.to_le_bytes());

        // Sort ID1 (offset 172, 2 bytes)
        buf.extend_from_slice(&self.sort_id1.to_le_bytes());

        // Sort ID2 (offset 174, 2 bytes)
        buf.extend_from_slice(&self.sort_id2.to_le_bytes());

        // Sort description offset + length (offset 176-183)
        buf.extend_from_slice(&(LBL_HEADER_LEN as u32).to_le_bytes()); // offset = header_len (no sort desc)
        buf.extend_from_slice(&0u32.to_le_bytes());                      // length = 0

        // Last position (offset 184, 4 bytes) — mkgmap: highwayData end pos = label_end for empty
        buf.extend_from_slice(&label_end.to_le_bytes());

        // Reserved (offset 188-195, 8 bytes)
        buf.extend_from_slice(&0u32.to_le_bytes()); // reserved
        buf.extend_from_slice(&0u16.to_le_bytes()); // UNK3_REC_LEN = 0
        buf.extend_from_slice(&0u16.to_le_bytes()); // reserved

        assert_eq!(buf.len(), LBL_HEADER_LEN as usize);

        // --- Label data ---
        buf.extend_from_slice(&self.labels);

        buf
    }

    pub fn label_data_size(&self) -> usize {
        self.labels.len()
    }
}

/// Write an empty PlacesHeader section: offset(4) + size(4) + recSize(2) + reserved(4) = 14 bytes
/// Empty sections point to `label_end` with size 0.
fn write_empty_section(buf: &mut Vec<u8>, label_end: u32, rec_size: u16) {
    buf.extend_from_slice(&label_end.to_le_bytes()); // offset pointing past label data
    buf.extend_from_slice(&0u32.to_le_bytes());      // size = 0
    buf.extend_from_slice(&rec_size.to_le_bytes());  // record size
    buf.extend_from_slice(&0u32.to_le_bytes());      // reserved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lbl_header_size() {
        let lbl = LblWriter::new(LabelEncoding::Format6);
        let data = lbl.build();
        assert!(data.len() >= LBL_HEADER_LEN as usize);
        // Check CommonHeader type
        assert_eq!(&data[2..12], b"GARMIN LBL");
    }

    #[test]
    fn test_add_label_offset() {
        let mut lbl = LblWriter::new(LabelEncoding::Format10);
        let off1 = lbl.add_label("ROUTE");
        let off2 = lbl.add_label("HIGHWAY");
        assert_eq!(off1, 1); // offset 0 is reserved
        assert!(off2 > off1);
    }

    #[test]
    fn test_deduplication() {
        let mut lbl = LblWriter::new(LabelEncoding::Format10);
        let off1 = lbl.add_label("ROUTE");
        let off2 = lbl.add_label("ROUTE");
        assert_eq!(off1, off2);
    }

    #[test]
    fn test_empty_label_returns_zero() {
        let mut lbl = LblWriter::new(LabelEncoding::Format10);
        assert_eq!(lbl.add_label(""), 0);
    }

    #[test]
    fn test_header_length_field() {
        let lbl = LblWriter::new(LabelEncoding::Format6);
        let data = lbl.build();
        let header_len = u16::from_le_bytes([data[0], data[1]]);
        assert_eq!(header_len, LBL_HEADER_LEN);
    }

    #[test]
    fn test_encoding_format_in_header() {
        let lbl = LblWriter::new(LabelEncoding::Format6);
        let data = lbl.build();
        // Encoding format is at offset 21+8+1 = 30
        assert_eq!(data[30], 6);
    }
}
