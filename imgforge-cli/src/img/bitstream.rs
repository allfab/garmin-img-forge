//! Garmin RGN bitstream encoder for coordinate delta compression.
//!
//! Implements the mkgmap-compatible bitstream format where coordinate deltas
//! are packed with variable-width encoding. The bitstream header contains
//! base values and sign flags, followed by packed deltas.

/// A bit-level writer for Garmin RGN coordinate bitstreams.
///
/// Writes values LSB-first into a byte buffer. The final byte is zero-padded.
pub struct BitWriter {
    buf: Vec<u8>,
    bits_written: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            bits_written: 0,
        }
    }

    /// Write an unsigned value using `bits` bits (LSB first).
    /// Equivalent to mkgmap's `putn(val, bits)`.
    pub fn putn(&mut self, value: u32, bits: u8) {
        if bits == 0 {
            return;
        }
        let mask = if bits >= 32 {
            u32::MAX
        } else {
            (1u32 << bits) - 1
        };
        let val = value & mask;

        for i in 0..bits as usize {
            if val & (1u32 << i) != 0 {
                let byte_idx = self.bits_written / 8;
                let bit_idx = self.bits_written % 8;
                while byte_idx >= self.buf.len() {
                    self.buf.push(0);
                }
                self.buf[byte_idx] |= 1u8 << bit_idx;
            }
            self.bits_written += 1;
        }
    }

    /// Write a single bit.
    pub fn put1(&mut self, value: bool) {
        self.putn(if value { 1 } else { 0 }, 1);
    }

    /// Write a signed value using mkgmap's sign-magnitude encoding.
    /// Equivalent to mkgmap's `sputn(val, bits)`.
    ///
    /// For values in range: MSB = sign (0=positive, 1=negative),
    /// lower bits = magnitude (inverted for negatives).
    /// For overflow: writes continuation markers (`top` value) then remainder.
    pub fn sputn(&mut self, bval: i32, nb: u8) {
        let top = 1i32 << (nb - 1);
        let mask = top - 1;
        let mut val = bval.unsigned_abs() as i32;
        while val > mask {
            self.putn(top as u32, nb);
            val -= mask;
        }
        if bval < 0 {
            self.putn(((top - val) | top) as u32, nb);
        } else {
            self.putn(val as u32, nb);
        }
    }

    /// Return the accumulated bytes (zero-padded to byte boundary).
    pub fn to_bytes(mut self) -> Vec<u8> {
        let needed = (self.bits_written + 7) / 8;
        self.buf.resize(needed, 0);
        self.buf
    }

    /// Number of complete bytes (same as mkgmap's `getLength()`).
    pub fn get_length(&self) -> usize {
        (self.bits_written + 7) / 8
    }

    /// Total number of bits written.
    pub fn bit_count(&self) -> usize {
        self.bits_written
    }
}

// ── Garmin base/bits conversion (mkgmap LinePreparer) ───────────────────────

/// Number of bits needed to represent the absolute value of `val`.
/// Returns 0 for val=0. Equivalent to mkgmap's `bitsNeeded(val)`.
pub fn bits_needed(val: i32) -> u8 {
    let n = val.unsigned_abs();
    if n == 0 {
        0
    } else {
        (32 - n.leading_zeros()) as u8
    }
}

/// Convert a base value (0-15, stored in 4-bit field) to actual magnitude bits.
/// Equivalent to mkgmap's `base2Bits(base)`.
pub fn base2bits(base: u8) -> u8 {
    if base < 10 {
        2 + base
    } else {
        2 + (2 * base) - 9
    }
}

/// Convert magnitude bits to a base value for 4-bit storage.
/// Equivalent to mkgmap's `bits2Base(bits)`.
pub fn bits2base(bits: u8) -> u8 {
    let base = bits.saturating_sub(2);
    if base > 10 {
        let base = if base % 2 == 0 { base + 1 } else { base };
        9 + (base - 9) / 2
    } else {
        base
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── BitWriter tests ─────────────────────────────────────────────────

    #[test]
    fn test_putn_basic() {
        let mut bw = BitWriter::new();
        bw.putn(3, 4); // 0b0011
        assert_eq!(bw.bit_count(), 4);
        assert_eq!(bw.to_bytes(), vec![0x03]);
    }

    #[test]
    fn test_putn_multiple() {
        let mut bw = BitWriter::new();
        bw.putn(3, 4);  // 0b0011
        bw.putn(14, 4); // 0b1110
        // Packed: bits 0-3 = 0011, bits 4-7 = 1110 → 0xE3
        assert_eq!(bw.to_bytes(), vec![0xE3]);
    }

    #[test]
    fn test_sputn_positive() {
        let mut bw = BitWriter::new();
        bw.sputn(3, 4); // positive: putn(3, 4) = 0b0011
        assert_eq!(bw.to_bytes(), vec![0x03]);
    }

    #[test]
    fn test_sputn_negative() {
        let mut bw = BitWriter::new();
        bw.sputn(-3, 4);
        // top=8, mask=7, val=3: putn((8-3)|8, 4) = putn(13, 4) = 0b1101
        assert_eq!(bw.to_bytes(), vec![0x0D]);
    }

    #[test]
    fn test_sputn_matches_twos_complement_for_in_range() {
        // For values in range [-7, +7] with 4 bits, sputn equals 2's complement
        for val in -7..=7i32 {
            let mut bw = BitWriter::new();
            bw.sputn(val, 4);
            let bytes = bw.to_bytes();
            let encoded = bytes[0] & 0x0F;

            let twos_comp = (val as u32) & 0x0F;
            assert_eq!(
                encoded, twos_comp as u8,
                "sputn({val}, 4) = {encoded:#04b} must equal 2's complement {twos_comp:#04b}"
            );
        }
    }

    #[test]
    fn test_put1() {
        let mut bw = BitWriter::new();
        bw.put1(true);
        bw.put1(false);
        bw.put1(true);
        assert_eq!(bw.bit_count(), 3);
        assert_eq!(bw.to_bytes(), vec![0x05]); // 0b101
    }

    #[test]
    fn test_bitwriter_empty() {
        let bw = BitWriter::new();
        assert_eq!(bw.bit_count(), 0);
        assert_eq!(bw.get_length(), 0);
        assert_eq!(bw.to_bytes(), Vec::<u8>::new());
    }

    #[test]
    fn test_get_length() {
        let mut bw = BitWriter::new();
        bw.putn(0xFF, 8);
        assert_eq!(bw.get_length(), 1);
        bw.putn(1, 1);
        assert_eq!(bw.get_length(), 2);
    }

    // ── base/bits conversion tests ──────────────────────────────────────

    #[test]
    fn test_bits_needed() {
        assert_eq!(bits_needed(0), 0);
        assert_eq!(bits_needed(1), 1);
        assert_eq!(bits_needed(-1), 1);
        assert_eq!(bits_needed(3), 2);
        assert_eq!(bits_needed(7), 3);
        assert_eq!(bits_needed(8), 4);
        assert_eq!(bits_needed(-100), 7);
    }

    #[test]
    fn test_base2bits() {
        assert_eq!(base2bits(0), 2);
        assert_eq!(base2bits(1), 3);
        assert_eq!(base2bits(5), 7);
        assert_eq!(base2bits(9), 11);
        assert_eq!(base2bits(10), 13);
        assert_eq!(base2bits(11), 15);
    }

    #[test]
    fn test_bits2base() {
        assert_eq!(bits2base(0), 0);
        assert_eq!(bits2base(2), 0);
        assert_eq!(bits2base(3), 1);
        assert_eq!(bits2base(7), 5);
        assert_eq!(bits2base(11), 9);
    }

    #[test]
    fn test_base_roundtrip_covers_bits() {
        // base2bits(bits2base(bits)) >= bits for all practical values
        for bits in 0..=16u8 {
            let base = bits2base(bits);
            let recovered = base2bits(base);
            assert!(
                recovered >= bits,
                "base2bits(bits2base({bits})) = {recovered} must be >= {bits}"
            );
        }
    }

    // ── Bitstream header format test (mkgmap compatible) ────────────────

    #[test]
    fn test_bitstream_header_format() {
        // Simulate mkgmap's bitstream header writing:
        // xBase=3, yBase=2, xSameSign=false, ySameSign=true, ySignNegative=false
        let mut bw = BitWriter::new();
        bw.putn(3, 4); // xBase
        bw.putn(2, 4); // yBase
        bw.put1(false); // xSameSign = false
        bw.put1(true);  // ySameSign = true
        bw.put1(false); // ySignNegative = false (all positive)
        assert_eq!(bw.bit_count(), 11);

        let bytes = bw.to_bytes();
        // First byte: bits 0-3 = xBase=3 (0b0011), bits 4-7 = yBase=2 (0b0010) → 0x23
        assert_eq!(bytes[0], 0x23);
        // Second byte: bit 0 = xSameSign=0, bit 1 = ySameSign=1, bit 2 = ySignNeg=0
        assert_eq!(bytes[1] & 0x07, 0x02);
    }
}
