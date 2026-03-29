// Zoom — zoom levels and resolutions, faithful to mkgmap Zoom.java

/// A zoom level definition for TRE
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zoom {
    pub level: u8,
    pub resolution: u8, // bits per coord, 1-24
    pub inherited: bool,
}

impl Zoom {
    pub fn new(level: u8, resolution: u8) -> Self {
        Self {
            level,
            resolution,
            inherited: false,
        }
    }

    /// Shift value = 24 - resolution — mkgmap Zoom.getShiftValue
    pub fn shift(&self) -> u8 {
        24 - self.resolution
    }

    /// Write 4-byte map level record — mkgmap TRE format
    /// Format: (level | inherited_flag) + resolution + subdiv_count(u16 LE)
    pub fn write(&self, subdiv_count: u16) -> [u8; 4] {
        let mut rec = [0u8; 4];
        rec[0] = self.level | if self.inherited { 0x80 } else { 0 };
        rec[1] = self.resolution;
        let cnt = subdiv_count.to_le_bytes();
        rec[2] = cnt[0];
        rec[3] = cnt[1];
        rec
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift() {
        assert_eq!(Zoom::new(0, 24).shift(), 0);
        assert_eq!(Zoom::new(1, 20).shift(), 4);
        assert_eq!(Zoom::new(2, 16).shift(), 8);
    }

    #[test]
    fn test_write_format() {
        let z = Zoom::new(2, 18);
        let rec = z.write(10);
        assert_eq!(rec[0], 2);
        assert_eq!(rec[1], 18);
        assert_eq!(u16::from_le_bytes([rec[2], rec[3]]), 10);
    }

    #[test]
    fn test_inherited_flag() {
        let mut z = Zoom::new(0, 24);
        z.inherited = true;
        let rec = z.write(1);
        assert_eq!(rec[0], 0x80);
    }
}
