//! FAT-like directory entry (Dirent) for Garmin IMG subfiles.
//!
//! Each entry is exactly 32 bytes, matching the mkgmap `Dirent.java` layout:
//!
//! ```text
//! Offset  Len  Content
//! 0x00    8    Filename, space-padded (ASCII)
//! 0x08    3    Extension (e.g. "TRE", "RGN", "LBL")
//! 0x0B    1    Flag: 0x03 = active file
//! 0x0C    2    Block start index (le16)
//! 0x0E    4    Allocated size in bytes (le32)
//! 0x12    4    Used size in bytes (le32)
//! 0x16   10    Reserved (zeros)
//! ```

use crate::error::ImgError;

/// A single 32-byte FAT-like directory entry.
#[derive(Debug, Clone)]
pub struct Dirent {
    /// 8-byte filename, space-padded (0x20).
    pub name: [u8; 8],
    /// 3-byte extension (e.g. *b"TRE"*).
    pub ext: [u8; 3],
    /// File flag: 0x03 = active file.
    pub flag: u8,
    /// Index of the first block (little-endian 16-bit).
    pub block_start: u16,
    /// Allocated size in bytes (little-endian 32-bit).
    pub size_allocated: u32,
    /// Used size in bytes (little-endian 32-bit).
    pub size_used: u32,
}

impl Dirent {
    /// Create a new directory entry.
    ///
    /// # Errors
    /// Returns [`ImgError::InvalidMapId`] if `map_id` is empty, non-ASCII-digit, or
    /// longer than 8 characters.
    pub fn new(map_id: &str, ext: &str, block_start: u16, size: u32) -> Result<Self, ImgError> {
        // Validate map_id: non-empty, all ASCII digits, max 8 chars.
        if map_id.is_empty() || !map_id.chars().all(|c| c.is_ascii_digit()) || map_id.len() > 8 {
            return Err(ImgError::InvalidMapId {
                id: map_id.to_string(),
            });
        }

        // Build 8-byte name, space-padded on the right.
        let mut name = [0x20u8; 8];
        let id_bytes = map_id.as_bytes();
        name[..id_bytes.len()].copy_from_slice(id_bytes);

        // Build 3-byte extension (truncated / space-padded if needed).
        let mut ext_buf = [0x20u8; 3];
        let ext_bytes = ext.as_bytes();
        let len = ext_bytes.len().min(3);
        ext_buf[..len].copy_from_slice(&ext_bytes[..len]);

        Ok(Self {
            name,
            ext: ext_buf,
            flag: 0x03,
            block_start,
            size_allocated: size,
            size_used: size,
        })
    }

    /// Serialise this entry into a fixed 32-byte buffer.
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        // 0x00–0x07 : filename
        buf[0x00..0x08].copy_from_slice(&self.name);
        // 0x08–0x0A : extension
        buf[0x08..0x0B].copy_from_slice(&self.ext);
        // 0x0B : flag
        buf[0x0B] = self.flag;
        // 0x0C–0x0D : block_start (le16)
        buf[0x0C..0x0E].copy_from_slice(&self.block_start.to_le_bytes());
        // 0x0E–0x11 : size_allocated (le32)
        buf[0x0E..0x12].copy_from_slice(&self.size_allocated.to_le_bytes());
        // 0x12–0x15 : size_used (le32)
        buf[0x12..0x16].copy_from_slice(&self.size_used.to_le_bytes());
        // 0x16–0x1F : reserved (already 0x00)
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirent_name_padding() {
        let d = Dirent::new("12345", "TRE", 2, 512).unwrap();
        assert_eq!(d.name, [b'1', b'2', b'3', b'4', b'5', 0x20, 0x20, 0x20]);
    }

    #[test]
    fn test_dirent_name_no_padding_8chars() {
        let d = Dirent::new("63240001", "TRE", 2, 512).unwrap();
        assert_eq!(d.name, *b"63240001");
    }

    #[test]
    fn test_dirent_size_32_bytes() {
        let d = Dirent::new("63240001", "TRE", 2, 512).unwrap();
        assert_eq!(d.to_bytes().len(), 32);
    }

    #[test]
    fn test_dirent_offsets_le() {
        let d = Dirent::new("63240001", "TRE", 0x0102, 0x0304_0506).unwrap();
        let bytes = d.to_bytes();
        // block_start at 0x0C–0x0D (le16)
        assert_eq!(u16::from_le_bytes([bytes[0x0C], bytes[0x0D]]), 0x0102);
        // size_allocated at 0x0E–0x11 (le32)
        assert_eq!(
            u32::from_le_bytes([bytes[0x0E], bytes[0x0F], bytes[0x10], bytes[0x11]]),
            0x0304_0506
        );
    }

    #[test]
    fn test_dirent_extension() {
        let d = Dirent::new("63240001", "LBL", 4, 512).unwrap();
        assert_eq!(&d.ext, b"LBL");
        let bytes = d.to_bytes();
        assert_eq!(&bytes[0x08..0x0B], b"LBL");
    }

    #[test]
    fn test_dirent_flag() {
        let d = Dirent::new("63240001", "TRE", 2, 512).unwrap();
        assert_eq!(d.flag, 0x03);
        assert_eq!(d.to_bytes()[0x0B], 0x03);
    }

    #[test]
    fn test_dirent_invalid_map_id_empty() {
        let err = Dirent::new("", "TRE", 0, 0).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_dirent_invalid_map_id_non_digit() {
        let err = Dirent::new("NOTDIGIT", "TRE", 0, 0).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }

    #[test]
    fn test_dirent_invalid_map_id_too_long() {
        let err = Dirent::new("123456789", "TRE", 0, 0).unwrap_err();
        assert!(matches!(err, ImgError::InvalidMapId { .. }));
    }
}
