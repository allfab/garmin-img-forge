//! IMG Garmin header — first 512-byte block of every .img file.
//!
//! Layout reference: mkgmap `ImgHeader.java`

/// Date/time of IMG file creation.
#[derive(Debug, Clone)]
pub struct ImgDate {
    /// `year - 1900` stored as u8 (e.g. 2026 → 126 = 0x7E)
    pub year_offset: u8,
    /// Month 1–12
    pub month: u8,
    /// Day 1–31
    pub day: u8,
    /// Hour 0–23
    pub hour: u8,
    /// Minute 0–59
    pub minute: u8,
    /// Second 0–59
    pub second: u8,
}

impl ImgDate {
    /// Build from wall-clock `SystemTime`, falling back to epoch on error.
    pub fn now() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Very simple calendar conversion (good enough for tests / non-critical metadata).
        // Uses the proleptic Gregorian algorithm.
        let (year, month, day, hour, minute, second) = secs_to_date(secs);
        Self {
            year_offset: year.saturating_sub(1900) as u8,
            month: month as u8,
            day: day as u8,
            hour: hour as u8,
            minute: minute as u8,
            second: second as u8,
        }
    }
}

/// Convert Unix timestamp (seconds since 1970-01-01T00:00:00Z) to
/// (year, month, day, hour, minute, second).
fn secs_to_date(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let second = secs % 60;
    let minutes = secs / 60;
    let minute = minutes % 60;
    let hours = minutes / 60;
    let hour = hours % 24;
    let days = hours / 24;

    // Days since 1970-01-01.  Civil date algorithm from
    // http://howardhinnant.github.io/date_algorithms.html#civil_from_days
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year, month, day, hour, minute, second)
}

/// First 512-byte block of a Garmin IMG file.
#[derive(Debug, Clone)]
pub struct ImgHeader {
    /// Map description (up to 49 bytes, ASCII, null-padded). Typically the map Name.
    pub description: String,
    /// Block size exponent: `block_size = 1 << block_size_exponent`
    /// (e.g. 9 → 512 bytes, 14 → 16 384 bytes).
    pub block_size_exponent: u8,
    /// Creation date/time.
    pub creation_date: ImgDate,
    /// Total number of blocks in the file (set by `ImgFilesystem` before serialisation).
    pub total_blocks: u32,
    /// Garmin family ID (LE16 at offset 0x054) — identifies the map family in BaseCamp.
    pub family_id: u16,
    /// Garmin product ID (LE16 at offset 0x056) — identifies the product in BaseCamp.
    pub product_id: u16,
}

impl ImgHeader {
    /// Compute the block size in bytes.
    pub fn block_size(&self) -> u32 {
        1u32 << self.block_size_exponent
    }

    /// Serialise the header into a fixed 512-byte buffer.
    ///
    /// The XOR byte at offset 0x000 is computed last so that
    /// `XOR(all 512 bytes) == 0x00`.
    pub fn to_bytes(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];

        // 0x001 — always 0x00 (already set by default).

        // 0x002–0x007 — "GARMIN" magic string.
        buf[0x002..0x008].copy_from_slice(b"GARMIN");

        // 0x008 — 0x00 (already set).
        // 0x009–0x00C — reserved (already 0x00).

        // 0x00D–0x012 — creation date/time.
        buf[0x00D] = self.creation_date.year_offset;
        buf[0x00E] = self.creation_date.month;
        buf[0x00F] = self.creation_date.day;
        buf[0x010] = self.creation_date.hour;
        buf[0x011] = self.creation_date.minute;
        buf[0x012] = self.creation_date.second;

        // 0x013–0x043 — description, 49 bytes, null-padded (already 0x00).
        // Truncate at a character boundary so we never write a partial multi-byte
        // codepoint into the field (Garmin expects 7-bit ASCII here).
        let desc_bytes = self.description.as_bytes();
        let safe_len = if desc_bytes.len() <= 49 {
            desc_bytes.len()
        } else {
            // Walk back from byte 49 to the nearest valid UTF-8 char boundary.
            let mut n = 49;
            while n > 0 && !self.description.is_char_boundary(n) {
                n -= 1;
            }
            n
        };
        buf[0x013..0x013 + safe_len].copy_from_slice(&desc_bytes[..safe_len]);
        // 0x044 — null terminator (already 0x00).

        // 0x045–0x048 — reserved (already 0x00).

        // 0x049 — heads = 1
        buf[0x049] = 1;
        // 0x04A — sectors per track = 63
        buf[0x04A] = 63;
        // 0x04B–0x04C — cylinders (le16): total_blocks / sectors_per_track / heads
        let cylinders = (self.total_blocks / 63).min(0xFFFF) as u16;
        buf[0x04B..0x04D].copy_from_slice(&cylinders.to_le_bytes());
        // 0x04D — FAT type flags = 0x02 (readable)
        buf[0x04D] = 0x02;
        // 0x04E — block size exponent
        buf[0x04E] = self.block_size_exponent;
        // 0x04F — blocks per cluster = 1
        buf[0x04F] = 1;
        // 0x050–0x053 — total blocks in file (le32)
        buf[0x050..0x054].copy_from_slice(&self.total_blocks.to_le_bytes());

        // 0x054–0x055 — family_id (LE16)
        buf[0x054..0x056].copy_from_slice(&self.family_id.to_le_bytes());
        // 0x056–0x057 — product_id (LE16)
        buf[0x056..0x058].copy_from_slice(&self.product_id.to_le_bytes());
        // 0x058–0x1FD — reserved (already 0x00).

        // 0x1FE–0x1FF — DOS partition signature
        buf[0x1FE] = 0x55;
        buf[0x1FF] = 0xAA;

        // 0x000 — XOR byte: ensure XOR of all 512 bytes == 0x00.
        let xor = buf[1..].iter().fold(0u8, |acc, &b| acc ^ b);
        buf[0x000] = xor;

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_header() -> ImgHeader {
        ImgHeader {
            description: "Test Map".to_string(),
            block_size_exponent: 9,
            creation_date: ImgDate {
                year_offset: 126, // 2026 - 1900
                month: 3,
                day: 26,
                hour: 10,
                minute: 30,
                second: 0,
            },
            total_blocks: 5,
            family_id: 0,
            product_id: 0,
        }
    }

    #[test]
    fn test_header_magic() {
        let bytes = test_header().to_bytes();
        assert_eq!(&bytes[0x002..0x008], b"GARMIN");
    }

    #[test]
    fn test_header_xor() {
        let bytes = test_header().to_bytes();
        let xor = bytes.iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(xor, 0x00, "XOR of all 512 header bytes must be 0x00");
    }

    #[test]
    fn test_header_signature() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
    }

    #[test]
    fn test_header_date() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x00D], 126); // 2026 - 1900
        assert_eq!(bytes[0x00E], 3); // month
        assert_eq!(bytes[0x00F], 26); // day
        assert_eq!(bytes[0x010], 10); // hour
        assert_eq!(bytes[0x011], 30); // minute
        assert_eq!(bytes[0x012], 0); // second
    }

    #[test]
    fn test_block_size() {
        let mut h = test_header();
        h.block_size_exponent = 9;
        assert_eq!(h.block_size(), 512);
        h.block_size_exponent = 14;
        assert_eq!(h.block_size(), 16384);
    }

    #[test]
    fn test_header_length() {
        assert_eq!(test_header().to_bytes().len(), 512);
    }

    #[test]
    fn test_header_description() {
        let bytes = test_header().to_bytes();
        assert_eq!(&bytes[0x013..0x013 + 8], b"Test Map");
        // Remaining bytes of the 49-byte field are null-padded.
        assert_eq!(bytes[0x013 + 8], 0x00);
    }

    #[test]
    fn test_header_description_utf8_boundary() {
        // U+2019 RIGHT SINGLE QUOTATION MARK = 3 bytes (0xE2 0x80 0x99).
        // Place it so that a naive byte truncation would cut inside the codepoint.
        // 47 ASCII chars + 3-byte char = bytes 47-49 straddle the 49-byte limit.
        let long_desc = "A".repeat(47) + "\u{2019}suffix";
        let h = ImgHeader {
            description: long_desc,
            block_size_exponent: 9,
            creation_date: ImgDate {
                year_offset: 126,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            total_blocks: 2,
            family_id: 0,
            product_id: 0,
        };
        let bytes = h.to_bytes();
        // The 49-byte field must never contain a partial codepoint.
        let field = &bytes[0x013..0x013 + 49];
        let non_zero_end = field
            .iter()
            .rposition(|&b| b != 0)
            .map(|i| i + 1)
            .unwrap_or(0);
        let written = &field[..non_zero_end];
        assert!(
            std::str::from_utf8(written).is_ok(),
            "description field must not contain a partial UTF-8 codepoint: {:?}",
            written
        );
        // XOR invariant must still hold.
        let xor = bytes.iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(xor, 0x00);
    }

    #[test]
    fn test_header_family_id_offset() {
        let h = ImgHeader {
            description: "Test".to_string(),
            block_size_exponent: 9,
            creation_date: ImgDate { year_offset: 126, month: 1, day: 1, hour: 0, minute: 0, second: 0 },
            total_blocks: 2,
            family_id: 6324,
            product_id: 0,
        };
        let bytes = h.to_bytes();
        let fid = u16::from_le_bytes([bytes[0x054], bytes[0x055]]);
        assert_eq!(fid, 6324, "family_id must be at offset 0x054 (LE16)");
    }

    #[test]
    fn test_header_product_id_offset() {
        let h = ImgHeader {
            description: "Test".to_string(),
            block_size_exponent: 9,
            creation_date: ImgDate { year_offset: 126, month: 1, day: 1, hour: 0, minute: 0, second: 0 },
            total_blocks: 2,
            family_id: 0,
            product_id: 1,
        };
        let bytes = h.to_bytes();
        assert_eq!(bytes[0x056], 0x01, "product_id low byte at 0x056");
        assert_eq!(bytes[0x057], 0x00, "product_id high byte at 0x057");
    }

    #[test]
    fn test_header_xor_with_family_product() {
        let h = ImgHeader {
            description: "Test".to_string(),
            block_size_exponent: 9,
            creation_date: ImgDate { year_offset: 126, month: 1, day: 1, hour: 0, minute: 0, second: 0 },
            total_blocks: 5,
            family_id: 6324,
            product_id: 1,
        };
        let bytes = h.to_bytes();
        let xor = bytes.iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(xor, 0x00, "XOR invariant must hold with family_id/product_id set");
    }

    #[test]
    fn test_secs_to_date_known_values() {
        // 2026-03-26 00:00:00 UTC = 1774483200 seconds since epoch.
        // Computed with: date -d "2026-03-26 00:00:00 UTC" +%s
        let (year, month, day, hour, minute, second) = secs_to_date(1_774_483_200);
        assert_eq!(year, 2026);
        assert_eq!(month, 3);
        assert_eq!(day, 26);
        assert_eq!(hour, 0);
        assert_eq!(minute, 0);
        assert_eq!(second, 0);
    }

    #[test]
    fn test_secs_to_date_epoch() {
        let (year, month, day, hour, minute, second) = secs_to_date(0);
        assert_eq!(year, 1970);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
        assert_eq!(hour, 0);
        assert_eq!(minute, 0);
        assert_eq!(second, 0);
    }
}
