// LBLFile — labels subfile, faithful to mkgmap LBLFile.java + LBLHeader.java

use std::collections::HashMap;
use super::common_header::{self, CommonHeader};
use super::labelenc::{self, LabelEncoding};

pub const LBL_HEADER_LEN: u16 = 196;
const OFF_CODEPAGE: usize = 170; // 0xAA — mkgmap LBLHeader codepage offset
const OFF_SORT_ID1: usize = 172; // 0xAC
const OFF_SORT_ID2: usize = 174; // 0xAE

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
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // CommonHeader (21 bytes)
        let common = CommonHeader::new(LBL_HEADER_LEN, "GARMIN LBL");
        common.write(&mut buf);

        // Pad to full header size
        let label_data_offset = LBL_HEADER_LEN as u32;
        let label_data_size = self.labels.len() as u32;

        // Label section offset + size (at offset 21)
        common_header::write_section(&mut buf, label_data_offset, label_data_size);

        // Label offset multiplier (1 byte)
        buf.push(0x00);

        // Encoding format (1 byte)
        buf.push(self.encoding.format_id());

        // Pad to full header, then place codepage and sort IDs at fixed offsets
        common_header::pad_to(&mut buf, LBL_HEADER_LEN as usize);

        // Codepage at fixed offset
        let cp = self.codepage.to_le_bytes();
        buf[OFF_CODEPAGE] = cp[0];
        buf[OFF_CODEPAGE + 1] = cp[1];

        // Sort IDs at fixed offsets
        let s1 = self.sort_id1.to_le_bytes();
        buf[OFF_SORT_ID1] = s1[0];
        buf[OFF_SORT_ID1 + 1] = s1[1];
        let s2 = self.sort_id2.to_le_bytes();
        buf[OFF_SORT_ID2] = s2[0];
        buf[OFF_SORT_ID2 + 1] = s2[1];

        // Label data
        buf.extend_from_slice(&self.labels);

        buf
    }

    pub fn label_data_size(&self) -> usize {
        self.labels.len()
    }
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
