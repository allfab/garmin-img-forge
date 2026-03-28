//! IMG Garmin header — first 512-byte sector of every .img file.
//!
//! Standard Garmin IMG layout (MBR-style), compatible with QMapShack, gmt,
//! BaseCamp and mkgmap.
//!
//! Key signatures:
//! - "DSKIMG\0" at offset 0x010
//! - "GARMIN\0" at offset 0x041
//! - MBR partition entry at 0x1BE
//! - Boot signature 0x55 0xAA at 0x1FE

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

    /// Absolute year (e.g. 2026).
    pub fn year(&self) -> u16 {
        1900 + self.year_offset as u16
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

/// First 512-byte sector of a Garmin IMG file.
///
/// Standard Garmin layout with DSKIMG signature, MBR partition table,
/// and CHS geometry.
#[derive(Debug, Clone)]
pub struct ImgHeader {
    /// Map description (up to 20 bytes, ASCII, space-padded at 0x049).
    pub description: String,
    /// Block size exponent: `block_size = 1 << block_size_exponent`
    /// (e.g. 9 → 512 bytes, 14 → 16 384 bytes, 16 → 65 536 bytes).
    pub block_size_exponent: u8,
    /// Creation date/time.
    pub creation_date: ImgDate,
    /// Total number of 512-byte sectors in the file (for MBR partition entry).
    pub total_file_sectors: u32,
}

impl ImgHeader {
    /// Compute the block size in bytes.
    pub fn block_size(&self) -> u32 {
        1u32 << self.block_size_exponent
    }

    /// Compute CHS geometry (sectors_per_track, heads_per_cylinder) such
    /// that the number of cylinders stays below 1024.
    fn compute_geometry(&self) -> (u16, u16) {
        let ts = self.total_file_sectors;
        let spt = 4u32;
        let mut heads = 2u32;
        // Scale heads to keep cylinders < 1024, capped at 128 (stays valid as u8).
        while ts.div_ceil(heads * spt) >= 1024 && heads < 128 {
            heads *= 2;
        }
        (spt as u16, heads as u16)
    }

    /// Serialise the header into a fixed 512-byte buffer.
    ///
    /// Layout matches the standard Garmin IMG format (mkgmap reference).
    pub fn to_bytes(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        let (spt, heads) = self.compute_geometry();
        let year = self.creation_date.year();

        // ── MBR preamble (0x000–0x00F) ───────────────────────────────────
        buf[0x008] = 0x01;
        buf[0x00A] = self.creation_date.month;
        buf[0x00B] = self.creation_date.year_offset;
        buf[0x00E] = 0x01;

        // ── DSKIMG signature (0x010–0x017) ───────────────────────────────
        buf[0x010..0x017].copy_from_slice(b"DSKIMG\0");
        buf[0x017] = 0x02; // format version

        // ── CHS geometry (0x018–0x01D) ───────────────────────────────────
        buf[0x018..0x01A].copy_from_slice(&spt.to_le_bytes());
        buf[0x01A] = heads as u8;
        buf[0x01D] = 0x02; // unknown constant (matches reference)

        // ── Creation date — absolute (0x039–0x03F) ──────────────────────
        buf[0x039..0x03B].copy_from_slice(&year.to_le_bytes());
        buf[0x03B] = self.creation_date.month;
        buf[0x03C] = self.creation_date.day;
        buf[0x03D] = self.creation_date.hour;
        buf[0x03E] = self.creation_date.minute;
        buf[0x03F] = self.creation_date.second;

        // ── FAT flag + GARMIN signature (0x040–0x048) ───────────────────
        buf[0x040] = 0x02;
        buf[0x041..0x048].copy_from_slice(b"GARMIN\0");

        // ── Description (0x049–0x05C, 20 chars, space-padded) ───────────
        let mut desc = [0x20u8; 20];
        let db = self.description.as_bytes();
        let len = db.len().min(20);
        desc[..len].copy_from_slice(&db[..len]);
        buf[0x049..0x05D].copy_from_slice(&desc);

        // ── Block size exponent (0x05D) ─────────────────────────────────
        buf[0x05D] = self.block_size_exponent;

        // ── Unknown constants (0x05F) ───────────────────────────────────
        buf[0x05F] = 0x04;

        // ── Filesystem parameters (0x061–0x064) ────────────────────────
        // E1 (sector size exponent) and E2 (cluster factor): cluster_size = 2^(E1+E2).
        // E1+E2 must equal block_size_exponent so that gmt/QMapShack compute the
        // correct cluster_size matching our block allocation granularity.
        let e1: u8 = 9;
        let e2: u8 = self.block_size_exponent.saturating_sub(e1);
        buf[0x061] = e1;
        buf[0x062] = e2;
        // Total clusters (LE16): ceil(total_file_sectors / 2^E2).
        let cluster_divisor = 1u32 << e2;
        let total_clusters = self.total_file_sectors.div_ceil(cluster_divisor).min(0xFFFF) as u16;
        buf[0x063..0x065].copy_from_slice(&total_clusters.to_le_bytes());

        // ── Description 2 — date string (0x065–0x083, 31 bytes) ────────
        let date_str = format!(
            "{:04}-{:02}-{:02}",
            year, self.creation_date.month, self.creation_date.day
        );
        let mut desc2 = [0x20u8; 31];
        let d2b = date_str.as_bytes();
        let d2_len = d2b.len().min(30);
        desc2[..d2_len].copy_from_slice(&d2b[..d2_len]);
        desc2[30] = 0x00; // null terminator
        buf[0x065..0x084].copy_from_slice(&desc2);

        // ── MBR partition entry (0x1BE–0x1CD) ───────────────────────────
        let ts = self.total_file_sectors;

        // CHS start: (cylinder=0, head=0, sector=1)
        buf[0x1C0] = 0x01;

        // CHS end
        if ts > 0 {
            let last = ts - 1;
            let end_cyl = last / (heads as u32 * spt as u32);
            let rem = last % (heads as u32 * spt as u32);
            let end_head = rem / spt as u32;
            let end_sector = rem % spt as u32 + 1;

            buf[0x1C3] = end_head as u8;
            buf[0x1C4] =
                (end_sector as u8 & 0x3F) | (((end_cyl >> 8) as u8 & 0x03) << 6);
            buf[0x1C5] = (end_cyl & 0xFF) as u8;
        }

        // LBA start = 0 (already zeros)
        // Total sectors (LE32)
        buf[0x1CA..0x1CE].copy_from_slice(&ts.to_le_bytes());

        // ── Boot signature (0x1FE–0x1FF) ────────────────────────────────
        buf[0x1FE] = 0x55;
        buf[0x1FF] = 0xAA;

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
            total_file_sectors: 9,
        }
    }

    #[test]
    fn test_header_dskimg_signature() {
        let bytes = test_header().to_bytes();
        assert_eq!(&bytes[0x010..0x017], b"DSKIMG\0");
    }

    #[test]
    fn test_header_garmin_signature() {
        let bytes = test_header().to_bytes();
        assert_eq!(&bytes[0x041..0x048], b"GARMIN\0");
    }

    #[test]
    fn test_header_boot_signature() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x1FE], 0x55);
        assert_eq!(bytes[0x1FF], 0xAA);
    }

    #[test]
    fn test_header_date_absolute() {
        let bytes = test_header().to_bytes();
        let year = u16::from_le_bytes([bytes[0x039], bytes[0x03A]]);
        assert_eq!(year, 2026);
        assert_eq!(bytes[0x03B], 3); // month
        assert_eq!(bytes[0x03C], 26); // day
        assert_eq!(bytes[0x03D], 10); // hour
        assert_eq!(bytes[0x03E], 30); // minute
        assert_eq!(bytes[0x03F], 0); // second
    }

    #[test]
    fn test_header_date_preamble() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x00A], 3); // month
        assert_eq!(bytes[0x00B], 126); // year_offset
    }

    #[test]
    fn test_header_fat_flag() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x040], 0x02);
    }

    #[test]
    fn test_block_size() {
        let mut h = test_header();
        h.block_size_exponent = 9;
        assert_eq!(h.block_size(), 512);
        h.block_size_exponent = 14;
        assert_eq!(h.block_size(), 16384);
        h.block_size_exponent = 16;
        assert_eq!(h.block_size(), 65536);
    }

    #[test]
    fn test_header_length() {
        assert_eq!(test_header().to_bytes().len(), 512);
    }

    #[test]
    fn test_header_description() {
        let bytes = test_header().to_bytes();
        assert_eq!(&bytes[0x049..0x049 + 8], b"Test Map");
        // Remaining chars of the 20-byte field are space-padded.
        assert_eq!(bytes[0x049 + 8], 0x20);
    }

    #[test]
    fn test_header_description_truncation() {
        let h = ImgHeader {
            description: "A very long description that exceeds 20 chars".to_string(),
            block_size_exponent: 9,
            creation_date: ImgDate {
                year_offset: 126,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            total_file_sectors: 5,
        };
        let bytes = h.to_bytes();
        assert_eq!(&bytes[0x049..0x05D], b"A very long descript");
    }

    #[test]
    fn test_header_block_size_exponent() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x05D], 9);
    }

    #[test]
    fn test_header_partition_total_sectors() {
        let bytes = test_header().to_bytes();
        let ts = u32::from_le_bytes([
            bytes[0x1CA],
            bytes[0x1CB],
            bytes[0x1CC],
            bytes[0x1CD],
        ]);
        assert_eq!(ts, 9);
    }

    #[test]
    fn test_header_description2_date_string() {
        let bytes = test_header().to_bytes();
        // Date string starts at 0x065, 10 chars "2026-03-26"
        assert_eq!(&bytes[0x065..0x06F], b"2026-03-26");
        // Remaining bytes are space-padded
        assert_eq!(bytes[0x06F], 0x20);
        // Null terminator at end of 31-byte field
        assert_eq!(bytes[0x083], 0x00);
    }

    #[test]
    fn test_header_version() {
        let bytes = test_header().to_bytes();
        assert_eq!(bytes[0x017], 0x02);
    }

    #[test]
    fn test_header_geometry_small() {
        let h = test_header(); // 9 sectors
        let (spt, heads) = h.compute_geometry();
        assert_eq!(spt, 4);
        assert_eq!(heads, 2);
    }

    #[test]
    fn test_header_geometry_large() {
        let h = ImgHeader {
            description: "Test".to_string(),
            block_size_exponent: 16,
            creation_date: ImgDate {
                year_offset: 126,
                month: 3,
                day: 26,
                hour: 0,
                minute: 0,
                second: 0,
            },
            total_file_sectors: 25020, // ~12.2 MB
        };
        let (spt, heads) = h.compute_geometry();
        assert_eq!(spt, 4);
        assert!(heads >= 4, "heads must accommodate 25020 sectors");
        let cylinders = 25020u32.div_ceil(heads as u32 * spt as u32);
        assert!(cylinders < 1024, "cylinders must be < 1024");
    }

    #[test]
    fn test_secs_to_date_known_values() {
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
