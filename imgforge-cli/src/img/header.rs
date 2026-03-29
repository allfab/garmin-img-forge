// ImgHeader — 512-byte container header, faithful to mkgmap ImgHeader.java

const HEADER_SIZE: usize = 512;

// Key offsets from mkgmap ImgHeader.java
const OFF_XOR: usize = 0x00;
const OFF_UPDATE_MONTH: usize = 0x0A;
const OFF_UPDATE_YEAR: usize = 0x0B;
const OFF_SUPP: usize = 0x0E;
const OFF_CHECKSUM: usize = 0x0F;
const OFF_SIGNATURE: usize = 0x10; // "DSKIMG\0"
const OFF_UNK_1: usize = 0x17;
const OFF_SECTORS: usize = 0x18;
const OFF_HEADS: usize = 0x1A;
const OFF_CYLINDERS: usize = 0x1C;
const OFF_CREATION_DATE: usize = 0x39;
const OFF_DIRECTORY_START_BLOCK: usize = 0x40;
const OFF_MAP_FILE_IDENTIFIER: usize = 0x41; // "GARMIN\0"
const OFF_MAP_DESCRIPTION: usize = 0x49;
const OFF_HEADS2: usize = 0x5D;
const OFF_SECTORS2: usize = 0x5F;
const OFF_BLOCK_SIZE_EXPONENT1: usize = 0x61;
const OFF_BLOCK_SIZE_EXPONENT2: usize = 0x62;
const OFF_BLOCK_SIZE: usize = 0x63;
const OFF_MAP_NAME_CONT: usize = 0x65;

// Partition table offsets
const OFF_START_HEAD: usize = 0x1BF;
const OFF_START_SECTOR: usize = 0x1C0;
const OFF_START_CYLINDER: usize = 0x1C1;
const OFF_SYSTEM_TYPE: usize = 0x1C2;
const OFF_END_HEAD: usize = 0x1C3;
const OFF_END_SECTOR: usize = 0x1C4;
const OFF_END_CYLINDER: usize = 0x1C5;
const OFF_REL_SECTORS: usize = 0x1C6;
const OFF_NUMBER_OF_SECTORS: usize = 0x1CA;
const OFF_PARTITION_SIG: usize = 0x1FE;

const LEN_MAP_DESCRIPTION: usize = 20;
const LEN_MAP_NAME_CONT: usize = 30;

pub struct ImgHeader {
    pub block_size: u32,
    pub directory_start_entry: u8,
    pub description: String,
    pub is_gmapsupp: bool,
    pub num_blocks: u32,
}

impl ImgHeader {
    pub fn new(block_size: u32, description: &str) -> Self {
        Self {
            block_size,
            directory_start_entry: 2,
            description: description.to_string(),
            is_gmapsupp: false,
            num_blocks: 0,
        }
    }

    /// Write 512-byte header — mkgmap ImgHeader.sync()
    pub fn write(&self) -> Vec<u8> {
        let mut buf = vec![0u8; HEADER_SIZE];

        // XOR byte = 0
        buf[OFF_XOR] = 0x00;

        // Block size exponents: block_size = 2^(e1+e2), e1 always 9
        let exp = block_size_exponent(self.block_size);
        buf[OFF_BLOCK_SIZE_EXPONENT1] = 0x09;
        buf[OFF_BLOCK_SIZE_EXPONENT2] = (exp - 9) as u8;

        // Signatures
        let dskimg = b"DSKIMG\0";
        buf[OFF_SIGNATURE..OFF_SIGNATURE + 7].copy_from_slice(dskimg);

        let garmin = b"GARMIN\0";
        buf[OFF_MAP_FILE_IDENTIFIER..OFF_MAP_FILE_IDENTIFIER + 7].copy_from_slice(garmin);

        buf[OFF_UNK_1] = 0x02;

        // Directory start block
        buf[OFF_DIRECTORY_START_BLOCK] = self.directory_start_entry;

        // Creation date at 0x39 — 7 bytes
        write_creation_date(&mut buf[OFF_CREATION_DATE..OFF_CREATION_DATE + 7]);

        // Update date
        let now = current_year_month();
        buf[OFF_UPDATE_YEAR] = ((now.0 - 1900) & 0xFF) as u8;
        buf[OFF_UPDATE_MONTH] = now.1 as u8;

        // Description: split into 20 + 30 chars, space-padded
        write_description(&mut buf, &self.description);

        // gmapsupp flag
        if self.is_gmapsupp {
            buf[OFF_SUPP] = 0x00;
        }

        // Checksum
        buf[OFF_CHECKSUM] = 0x00;

        // CHS geometry and partition table
        self.write_size_values(&mut buf);

        buf
    }

    fn write_size_values(&self, buf: &mut [u8]) {
        let end_sector = ((self.num_blocks as u64 + 1) * self.block_size as u64 + 511) / 512;
        let end_sector = end_sector as u32;

        // Find valid CHS geometry
        let (h, s, c) = find_chs_geometry(end_sector);

        // Write sectors, heads, cylinders
        put_u16_le(buf, OFF_SECTORS, s as u16);
        put_u16_le(buf, OFF_SECTORS2, s as u16);
        put_u16_le(buf, OFF_HEADS, h as u16);
        put_u16_le(buf, OFF_HEADS2, h as u16);
        put_u16_le(buf, OFF_CYLINDERS, c as u16);

        // Block count (may overflow to 0xFFFF)
        let blocks = (end_sector as u64 * 512 / self.block_size as u64) as u32;
        let short_blocks = if blocks > 0xFFFF { 0xFFFF } else { blocks as u16 };
        put_u16_le(buf, OFF_BLOCK_SIZE, short_blocks);

        // Partition signature 0x55AA
        buf[OFF_PARTITION_SIG] = 0x55;
        buf[OFF_PARTITION_SIG + 1] = 0xAA;

        // Partition start: CHS 0,0,1
        buf[OFF_START_HEAD] = 0;
        buf[OFF_START_SECTOR] = 1;
        buf[OFF_START_CYLINDER] = 0;
        buf[OFF_SYSTEM_TYPE] = 0;

        // Partition end in CHS
        if end_sector > 0 {
            let lba = end_sector - 1;
            let ch = (lba / s) % h;
            let cs = (lba % s) + 1;
            let cc = lba / (s * h);
            buf[OFF_END_HEAD] = ch as u8;
            buf[OFF_END_SECTOR] = (cs as u8) | (((cc >> 2) & 0xC0) as u8);
            buf[OFF_END_CYLINDER] = (cc & 0xFF) as u8;
        }

        // LBA addresses
        put_u32_le(buf, OFF_REL_SECTORS, 0);
        put_u32_le(buf, OFF_NUMBER_OF_SECTORS, end_sector);
    }
}

/// Find CHS geometry — mkgmap ImgHeader.writeSizeValues
fn find_chs_geometry(end_sector: u32) -> (u32, u32, u32) {
    let heads_list = [16, 32, 64, 128, 256];
    let sectors_list = [4, 8, 16, 32];
    let cyls_list = [0x20, 0x40, 0x80, 0x100, 0x200, 0x3FF];

    for &h in &heads_list {
        for &s in &sectors_list {
            for &c in &cyls_list {
                if s * h * c > end_sector {
                    return (h, s, c);
                }
            }
        }
    }
    (256, 32, 0x3FF)
}

fn block_size_exponent(block_size: u32) -> u32 {
    let mut bs = block_size;
    let mut exp = 0;
    while bs > 1 {
        bs >>= 1;
        exp += 1;
    }
    exp
}

fn write_description(buf: &mut [u8], desc: &str) {
    let bytes = desc.as_bytes();
    let len = bytes.len().min(50);

    // Part 1: 20 bytes at OFF_MAP_DESCRIPTION, space-padded
    for i in 0..LEN_MAP_DESCRIPTION {
        buf[OFF_MAP_DESCRIPTION + i] = if i < len { bytes[i] } else { b' ' };
    }

    // Part 2: 30 bytes at OFF_MAP_NAME_CONT, space-padded
    for i in 0..LEN_MAP_NAME_CONT {
        let src_idx = LEN_MAP_DESCRIPTION + i;
        buf[OFF_MAP_NAME_CONT + i] = if src_idx < len { bytes[src_idx] } else { b' ' };
    }

    // Null terminator after part 2
    buf[OFF_MAP_NAME_CONT + LEN_MAP_NAME_CONT] = 0x00;
}

fn write_creation_date(buf: &mut [u8]) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let (year, month, day, hour, min, sec) = super::common_header::unix_to_calendar(secs);
    let yb = (year as u16).to_le_bytes();
    buf[0] = yb[0];
    buf[1] = yb[1];
    buf[2] = month as u8;
    buf[3] = day as u8;
    buf[4] = hour as u8;
    buf[5] = min as u8;
    buf[6] = sec as u8;
}

fn current_year_month() -> (i32, i32) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let (year, month, _, _, _, _) = super::common_header::unix_to_calendar(secs);
    (year, month)
}

fn put_u16_le(buf: &mut [u8], offset: usize, val: u16) {
    let b = val.to_le_bytes();
    buf[offset] = b[0];
    buf[offset + 1] = b[1];
}

fn put_u32_le(buf: &mut [u8], offset: usize, val: u32) {
    let b = val.to_le_bytes();
    buf[offset..offset + 4].copy_from_slice(&b);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_is_512_bytes() {
        let h = ImgHeader::new(512, "Test Map");
        let buf = h.write();
        assert_eq!(buf.len(), 512);
    }

    #[test]
    fn test_dskimg_signature() {
        let h = ImgHeader::new(512, "Test");
        let buf = h.write();
        assert_eq!(&buf[OFF_SIGNATURE..OFF_SIGNATURE + 7], b"DSKIMG\0");
    }

    #[test]
    fn test_garmin_signature() {
        let h = ImgHeader::new(512, "Test");
        let buf = h.write();
        assert_eq!(&buf[OFF_MAP_FILE_IDENTIFIER..OFF_MAP_FILE_IDENTIFIER + 7], b"GARMIN\0");
    }

    #[test]
    fn test_partition_signature() {
        let mut h = ImgHeader::new(512, "Test");
        h.num_blocks = 100;
        let buf = h.write();
        assert_eq!(buf[0x1FE], 0x55);
        assert_eq!(buf[0x1FF], 0xAA);
    }

    #[test]
    fn test_block_size_exponent() {
        // 512 = 2^9 → e1=9, e2=0
        let h = ImgHeader::new(512, "");
        let buf = h.write();
        assert_eq!(buf[OFF_BLOCK_SIZE_EXPONENT1], 0x09);
        assert_eq!(buf[OFF_BLOCK_SIZE_EXPONENT2], 0x00);
    }

    #[test]
    fn test_block_size_4096() {
        // 4096 = 2^12 → e1=9, e2=3
        let h = ImgHeader::new(4096, "");
        let buf = h.write();
        assert_eq!(buf[OFF_BLOCK_SIZE_EXPONENT1], 0x09);
        assert_eq!(buf[OFF_BLOCK_SIZE_EXPONENT2], 0x03);
    }

    #[test]
    fn test_directory_start_block() {
        let h = ImgHeader::new(512, "");
        let buf = h.write();
        assert_eq!(buf[OFF_DIRECTORY_START_BLOCK], 2);
    }
}
