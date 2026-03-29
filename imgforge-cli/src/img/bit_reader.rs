/// BitReader — LSB-first bit-level reader, faithful to mkgmap BitReader.java

pub struct BitReader<'a> {
    buf: &'a [u8],
    bit_position: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            bit_position: 0,
        }
    }

    /// Read exactly one bit — mkgmap get1()
    pub fn get1(&mut self) -> bool {
        let off = self.bit_position % 8;
        let b = self.buf[self.bit_position / 8];
        self.bit_position += 1;
        ((b >> off) & 1) == 1
    }

    /// Read n bits unsigned — mkgmap get(int)
    pub fn get(&mut self, n: usize) -> u32 {
        let mut res: u32 = 0;
        let mut pos: usize = 0;

        while pos < n {
            let index = self.bit_position / 8;
            let off = self.bit_position % 8;

            let b = self.buf[index] >> off;
            let mut nbits = n - pos;
            if nbits > 8 - off {
                nbits = 8 - off;
            }

            let mask = (1u32 << nbits) - 1;
            res |= (b as u32 & mask) << pos;
            pos += nbits;
            self.bit_position += nbits;
        }

        res
    }

    /// Read signed value (sign bit is MSB of field) — mkgmap sget(int)
    pub fn sget(&mut self, n: usize) -> i32 {
        let res = self.get(n);
        let top = 1u32 << (n - 1);

        if (res & top) != 0 {
            let mask = top - 1;
            (!(mask) | res) as i32
        } else {
            res as i32
        }
    }

    /// Read signed value with extended range — mkgmap sget2(int)
    pub fn sget2(&mut self, n: usize) -> i32 {
        let top = 1u32 << (n - 1);
        let mask = top - 1;
        let mut base: u32 = 0;

        let mut res = self.get(n);
        while res == top {
            base += mask;
            res = self.get(n);
        }

        if (res & top) == 0 {
            (res + base) as i32
        } else {
            ((res | !mask) as i32) - base as i32
        }
    }

    pub fn bit_position(&self) -> usize {
        self.bit_position
    }

    pub fn remaining_bits(&self) -> usize {
        self.buf.len() * 8 - self.bit_position
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::img::bit_writer::BitWriter;

    #[test]
    fn test_get1_roundtrip() {
        let mut bw = BitWriter::new();
        bw.put1(true);
        bw.put1(false);
        bw.put1(true);

        let mut br = BitReader::new(bw.bytes());
        assert!(br.get1());
        assert!(!br.get1());
        assert!(br.get1());
    }

    #[test]
    fn test_get_roundtrip() {
        let mut bw = BitWriter::new();
        bw.putn(42, 8);
        bw.putn(7, 3);
        bw.putn(1023, 10);

        let mut br = BitReader::new(bw.bytes());
        assert_eq!(br.get(8), 42);
        assert_eq!(br.get(3), 7);
        assert_eq!(br.get(10), 1023);
    }

    #[test]
    fn test_sget_roundtrip() {
        let mut bw = BitWriter::new();
        // sget expects sign in MSB: for 4-bit field, -3 = 0b1101 (top bit set, value = 8-3=5? no)
        // Actually mkgmap sget: top = 1<<(n-1), if res&top != 0 => res = ~mask | res
        // So for n=4: top=8, mask=7
        // -3 in this encoding: we need res such that ~7 | res = -3
        // ~7 = 0xFFFFFFF8, ~7 | res = -3 = 0xFFFFFFFD => res = 0xFD & 0xF = 0xD = 13
        bw.putn(13, 4); // encodes -3 for sget(4)
        bw.putn(3, 4);  // encodes +3 for sget(4)

        let mut br = BitReader::new(bw.bytes());
        assert_eq!(br.sget(4), -3);
        assert_eq!(br.sget(4), 3);
    }

    #[test]
    fn test_sputn_sget2_roundtrip() {
        let mut bw = BitWriter::new();
        bw.sputn(3, 4);
        bw.sputn(-5, 4);
        bw.sputn(10, 4); // overflow case

        let mut br = BitReader::new(bw.bytes());
        assert_eq!(br.sget2(4), 3);
        assert_eq!(br.sget2(4), -5);
        assert_eq!(br.sget2(4), 10);
    }

    #[test]
    fn test_sputn_sget2_large_overflow() {
        let mut bw = BitWriter::new();
        bw.sputn(20, 4); // needs multiple overflow markers
        bw.sputn(-20, 4);

        let mut br = BitReader::new(bw.bytes());
        assert_eq!(br.sget2(4), 20);
        assert_eq!(br.sget2(4), -20);
    }

    #[test]
    fn test_remaining_bits() {
        let data = [0xFF, 0x00];
        let mut br = BitReader::new(&data);
        assert_eq!(br.remaining_bits(), 16);
        br.get(5);
        assert_eq!(br.remaining_bits(), 11);
    }
}
