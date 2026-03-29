/// BitWriter — LSB-first bit-level writer, faithful to mkgmap BitWriter.java

pub struct BitWriter {
    buf: Vec<u8>,
    bit_offset: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buf: vec![0u8; 20],
            bit_offset: 0,
        }
    }

    /// Put exactly one bit — mkgmap put1(int)
    pub fn put1(&mut self, b: bool) {
        self.ensure_size(self.bit_offset + 1);
        let off = self.bit_offset / 8;
        let rem = self.bit_offset % 8;
        if b {
            self.buf[off] |= 1 << rem;
        }
        self.bit_offset += 1;
    }

    /// Put n bits (n < 24) — mkgmap putn(int, int)
    pub fn putn(&mut self, bval: u32, nb: usize) {
        assert!(nb < 24, "putn: nb must be < 24");
        let mut val = bval & ((1u32 << nb) - 1);
        let mut n = nb;
        self.ensure_size(self.bit_offset + n);

        while n > 0 {
            let ind = self.bit_offset / 8;
            let rem = self.bit_offset % 8;

            self.buf[ind] |= ((val << rem) & 0xff) as u8;
            val >>= 8 - rem;

            let nput = (8 - rem).min(n);
            self.bit_offset += nput;
            n -= nput;
        }
    }

    /// Write signed value with overflow markers — mkgmap sputn(int, int)
    pub fn sputn(&mut self, bval: i32, nb: usize) {
        let top = 1u32 << (nb - 1);
        let mask = top - 1;
        let mut val = bval.unsigned_abs();

        while val > mask {
            self.putn(top, nb);
            val -= mask;
        }

        if bval < 0 {
            self.putn((top - val) | top, nb);
        } else {
            self.putn(val, nb);
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.buf[..self.get_length()]
    }

    pub fn get_length(&self) -> usize {
        (self.bit_offset + 7) / 8
    }

    pub fn bit_position(&self) -> usize {
        self.bit_offset
    }

    fn ensure_size(&mut self, new_bit_len: usize) {
        let needed_bytes = (new_bit_len + 7) / 8;
        if needed_bytes >= self.buf.len() {
            self.buf.resize(self.buf.len() + 50, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put1_true() {
        let mut bw = BitWriter::new();
        bw.put1(true);
        assert_eq!(bw.bytes()[0], 0x01);
        assert_eq!(bw.bit_position(), 1);
    }

    #[test]
    fn test_put1_false() {
        let mut bw = BitWriter::new();
        bw.put1(false);
        assert_eq!(bw.bytes()[0], 0x00);
    }

    #[test]
    fn test_putn_3bits() {
        let mut bw = BitWriter::new();
        bw.putn(0x05, 3); // 101 in binary
        assert_eq!(bw.bytes()[0], 0x05);
        assert_eq!(bw.bit_position(), 3);
    }

    #[test]
    fn test_putn_spanning_bytes() {
        let mut bw = BitWriter::new();
        bw.putn(0x0F, 4); // 1111
        bw.putn(0x0A, 4); // 1010
        assert_eq!(bw.bytes()[0], 0xAF); // LSB-first: low nibble=0xF, high nibble=0xA
    }

    #[test]
    fn test_sputn_positive() {
        let mut bw = BitWriter::new();
        bw.sputn(3, 4);
        // top=8, mask=7, val=3, 3<=7 so just putn(3, 4)
        assert_eq!(bw.bytes()[0] & 0x0F, 3);
    }

    #[test]
    fn test_sputn_negative() {
        let mut bw = BitWriter::new();
        bw.sputn(-3, 4);
        // top=8, mask=7, val=3, 3<=7, bval<0: putn((8-3)|8, 4) = putn(13, 4)
        assert_eq!(bw.bytes()[0] & 0x0F, 13);
    }

    #[test]
    fn test_sputn_overflow() {
        let mut bw = BitWriter::new();
        bw.sputn(10, 4);
        // top=8, mask=7, val=10
        // 10 > 7: putn(8, 4), val=10-7=3
        // 3 <= 7, bval>0: putn(3, 4)
        assert_eq!(bw.bit_position(), 8);
        let b = bw.bytes()[0];
        // First 4 bits: 8 = 0b1000, next 4 bits: 3 = 0b0011
        // Byte: 0b0011_1000 = 0x38
        assert_eq!(b, 0x38);
    }

    #[test]
    fn test_get_length() {
        let mut bw = BitWriter::new();
        bw.putn(0xFF, 8);
        assert_eq!(bw.get_length(), 1);
        bw.put1(true);
        assert_eq!(bw.get_length(), 2);
    }
}
