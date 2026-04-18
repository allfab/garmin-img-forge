/// CommonHeader — shared 21-byte Garmin subfile header, faithful to mkgmap CommonHeader.java

#[cfg(test)]
const COMMON_HEADER_LEN: usize = 21;
const TYPE_LEN: usize = 10;

pub struct CommonHeader {
    pub header_length: u16,
    pub type_str: String,
}

impl CommonHeader {
    pub fn new(header_length: u16, type_str: &str) -> Self {
        Self {
            header_length,
            type_str: type_str.to_string(),
        }
    }

    /// Write exactly 21 bytes to buf — mkgmap CommonHeader.writeHeader
    /// Layout: header_len(2) + type(10) + unknown(1) + lock(1) + date(7)
    pub fn write(&self, buf: &mut Vec<u8>) {
        // header_length as u16 LE
        buf.extend_from_slice(&self.header_length.to_le_bytes());

        // type string padded to 10 bytes with 0x00
        let type_bytes = self.type_str.as_bytes();
        for i in 0..TYPE_LEN {
            if i < type_bytes.len() {
                buf.push(type_bytes[i]);
            } else {
                buf.push(0x00);
            }
        }

        // unknown byte (always 0x01 in mkgmap)
        buf.push(0x01);

        // lock flag (0x00 = unlocked)
        buf.push(0x00);

        // creation date — 7 bytes (mkgmap Utils.makeCreationTime)
        write_creation_time(buf);
    }
}

/// Returns "now" in seconds-since-epoch, or the value of `SOURCE_DATE_EPOCH`
/// if set (reproducible-builds.org standard).
///
/// Toute écriture de timestamp dans un sous-fichier IMG passe par ce helper :
/// fixer `SOURCE_DATE_EPOCH=<n>` produit un IMG bit-à-bit reproductible.
pub(crate) fn now_secs() -> u64 {
    if let Ok(val) = std::env::var("SOURCE_DATE_EPOCH") {
        if let Ok(n) = val.trim().parse::<u64>() {
            return n;
        }
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write 7-byte Garmin creation time (current UTC) — mkgmap Utils.fillBufFromTime
fn write_creation_time(buf: &mut Vec<u8>) {
    // Format: year(u16 LE) + month(u8) + day(u8) + hour(u8) + min(u8) + sec(u8)
    let secs = now_secs();

    // Simple UTC calendar calculation
    let (year, month, day, hour, min, sec) = unix_to_calendar(secs);

    buf.extend_from_slice(&(year as u16).to_le_bytes());
    buf.push(month as u8);
    buf.push(day as u8);
    buf.push(hour as u8);
    buf.push(min as u8);
    buf.push(sec as u8);
}

/// Write a specific date as 7-byte Garmin creation time
#[cfg(test)]
fn write_creation_time_fixed(buf: &mut Vec<u8>, year: u16, month: u8, day: u8, hour: u8, min: u8, sec: u8) {
    buf.extend_from_slice(&year.to_le_bytes());
    buf.push(month);
    buf.push(day);
    buf.push(hour);
    buf.push(min);
    buf.push(sec);
}

/// Write a section descriptor (offset u32 LE + size u32 LE) — shared across all subfile builders
pub fn write_section(buf: &mut Vec<u8>, offset: u32, size: u32) {
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&size.to_le_bytes());
}

/// Write a 24-bit unsigned LE value (3 bytes)
pub fn write_u24(buf: &mut Vec<u8>, val: u32) {
    let b = val.to_le_bytes();
    buf.push(b[0]);
    buf.push(b[1]);
    buf.push(b[2]);
}

/// Read a 24-bit signed LE value from 3 bytes, sign-extended to i32
pub fn read_i24(bytes: &[u8]) -> i32 {
    let val = bytes[0] as i32 | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
    if val & 0x800000 != 0 { val | !0xFFFFFF } else { val }
}

/// Extract map bounds (north, east, south, west) from raw TRE subfile bytes.
/// Bounds are stored as 24-bit signed LE at TRE header offsets 21-32.
pub fn read_tre_bounds(tre_data: &[u8]) -> (i32, i32, i32, i32) {
    let north = read_i24(&tre_data[21..24]);
    let east  = read_i24(&tre_data[24..27]);
    let south = read_i24(&tre_data[27..30]);
    let west  = read_i24(&tre_data[30..33]);
    (north, east, south, west)
}

/// Write a 24-bit signed LE value (3 bytes)
pub fn write_i24(buf: &mut Vec<u8>, val: i32) {
    let b = val.to_le_bytes();
    buf.push(b[0]);
    buf.push(b[1]);
    buf.push(b[2]);
}

/// Pad buffer to target length with zeros, or truncate if longer
pub fn pad_to(buf: &mut Vec<u8>, target: usize) {
    buf.resize(target, 0x00);
}

pub fn unix_to_calendar(secs: u64) -> (i32, i32, i32, i32, i32, i32) {
    let sec = (secs % 60) as i32;
    let min = ((secs / 60) % 60) as i32;
    let hour = ((secs / 3600) % 24) as i32;
    let mut days = (secs / 86400) as i32;

    // Days since 1970-01-01
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i32; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &dim in &days_in_months {
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }

    (year, month, days + 1, hour, min, sec)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_common_header_size() {
        let ch = CommonHeader::new(188, "GARMIN TRE");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        assert_eq!(buf.len(), COMMON_HEADER_LEN);
    }

    #[test]
    fn test_header_length_le() {
        let ch = CommonHeader::new(188, "GARMIN TRE");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        assert_eq!(buf[0], 188);
        assert_eq!(buf[1], 0); // 188 < 256, high byte = 0
    }

    #[test]
    fn test_type_string() {
        let ch = CommonHeader::new(188, "GARMIN TRE");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        let type_str = std::str::from_utf8(&buf[2..12]).unwrap();
        assert_eq!(type_str, "GARMIN TRE");
    }

    #[test]
    fn test_type_string_short_padded() {
        let ch = CommonHeader::new(125, "GARMIN RGN");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        assert_eq!(&buf[2..12], b"GARMIN RGN");
    }

    #[test]
    fn test_unknown_and_lock_bytes() {
        let ch = CommonHeader::new(188, "GARMIN TRE");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        assert_eq!(buf[12], 0x01); // unknown
        assert_eq!(buf[13], 0x00); // unlocked
    }

    #[test]
    fn test_date_is_7_bytes() {
        let ch = CommonHeader::new(188, "GARMIN TRE");
        let mut buf = Vec::new();
        ch.write(&mut buf);
        // Date starts at offset 14, 7 bytes → total = 21
        assert_eq!(buf.len(), 21);
        // Year should be reasonable (>= 2024)
        let year = u16::from_le_bytes([buf[14], buf[15]]);
        assert!(year >= 2024);
    }

    #[test]
    fn test_fixed_date() {
        let mut buf = Vec::new();
        write_creation_time_fixed(&mut buf, 2026, 3, 29, 12, 0, 0);
        assert_eq!(buf.len(), 7);
        let year = u16::from_le_bytes([buf[0], buf[1]]);
        assert_eq!(year, 2026);
        assert_eq!(buf[2], 3);
        assert_eq!(buf[3], 29);
    }
}
