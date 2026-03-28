//! Common header shared by all Garmin IMG subfiles (TRE, RGN, LBL, NET, NOD, SRT).
//!
//! Every subfile starts with a 21-byte common header:
//! ```text
//! 0x00  LE16  total header length (common + type-specific)
//! 0x02  10B   "GARMIN xxx" signature (space-padded to 10 bytes)
//! 0x0C  u8    version (0x01)
//! 0x0D  u8    lock flag (0x00)
//! 0x0E  LE16  creation year
//! 0x10  u8    month
//! 0x11  u8    day
//! 0x12  u8    hour
//! 0x13  u8    minute
//! 0x14  u8    second
//! ```

use crate::img::header::ImgDate;

/// Size in bytes of the Garmin subfile common header.
pub const COMMON_HEADER_SIZE: usize = 21;

/// Build the 21-byte common header for a Garmin IMG subfile.
///
/// # Arguments
/// - `subfile_type`: 3-character type string ("TRE", "RGN", "LBL", "NET", "NOD", "SRT")
/// - `total_header_length`: total header size in bytes (common + type-specific)
pub fn build_common_header(subfile_type: &str, total_header_length: u16) -> [u8; COMMON_HEADER_SIZE] {
    debug_assert_eq!(subfile_type.len(), 3, "subfile_type must be exactly 3 characters (e.g. \"TRE\")");
    let date = ImgDate::now();
    let mut buf = [0u8; COMMON_HEADER_SIZE];

    // 0x00: total_header_length (LE16)
    let len_bytes = total_header_length.to_le_bytes();
    buf[0] = len_bytes[0];
    buf[1] = len_bytes[1];

    // 0x02: "GARMIN xxx" — 6 chars "GARMIN" + space + 3 chars type = 10 bytes
    let sig = format!("GARMIN {}", subfile_type);
    let sig_bytes = sig.as_bytes();
    let copy_len = sig_bytes.len().min(10);
    buf[2..2 + copy_len].copy_from_slice(&sig_bytes[..copy_len]);

    // 0x0C: version = 0x01
    buf[0x0C] = 0x01;

    // 0x0D: lock = 0x00 (already zero)

    // 0x0E: creation year (LE16)
    let year_bytes = date.year().to_le_bytes();
    buf[0x0E] = year_bytes[0];
    buf[0x0F] = year_bytes[1];

    // 0x10–0x14: month, day, hour, minute, second
    buf[0x10] = date.month;
    buf[0x11] = date.day;
    buf[0x12] = date.hour;
    buf[0x13] = date.minute;
    buf[0x14] = date.second;

    buf
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_header_size() {
        let header = build_common_header("TRE", 165);
        assert_eq!(header.len(), 21);
    }

    #[test]
    fn test_common_header_length_field() {
        let header = build_common_header("TRE", 165);
        let len = u16::from_le_bytes([header[0], header[1]]);
        assert_eq!(len, 165);
    }

    #[test]
    fn test_common_header_signature_tre() {
        let header = build_common_header("TRE", 165);
        assert_eq!(&header[0x02..0x0C], b"GARMIN TRE");
    }

    #[test]
    fn test_common_header_signature_rgn() {
        let header = build_common_header("RGN", 46);
        assert_eq!(&header[0x02..0x0C], b"GARMIN RGN");
    }

    #[test]
    fn test_common_header_signature_lbl() {
        let header = build_common_header("LBL", 45);
        assert_eq!(&header[0x02..0x0C], b"GARMIN LBL");
    }

    #[test]
    fn test_common_header_signature_net() {
        let header = build_common_header("NET", 55);
        assert_eq!(&header[0x02..0x0C], b"GARMIN NET");
    }

    #[test]
    fn test_common_header_signature_nod() {
        let header = build_common_header("NOD", 48);
        assert_eq!(&header[0x02..0x0C], b"GARMIN NOD");
    }

    #[test]
    fn test_common_header_signature_srt() {
        let header = build_common_header("SRT", 44);
        assert_eq!(&header[0x02..0x0C], b"GARMIN SRT");
    }

    #[test]
    fn test_common_header_version() {
        let header = build_common_header("TRE", 165);
        assert_eq!(header[0x0C], 0x01, "version must be 0x01");
    }

    #[test]
    fn test_common_header_lock() {
        let header = build_common_header("TRE", 165);
        assert_eq!(header[0x0D], 0x00, "lock must be 0x00");
    }

    #[test]
    fn test_common_header_date_nonzero() {
        let header = build_common_header("TRE", 165);
        let year = u16::from_le_bytes([header[0x0E], header[0x0F]]);
        assert!(year >= 2020, "year must be >= 2020, got {}", year);
        assert!(header[0x10] >= 1 && header[0x10] <= 12, "month must be 1-12");
        assert!(header[0x11] >= 1 && header[0x11] <= 31, "day must be 1-31");
    }
}
