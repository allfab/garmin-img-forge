// LinePreparer — CRITICAL bitstream encoder, faithful to mkgmap LinePreparer.java

use super::bit_writer::BitWriter;

/// Prepare a coordinate delta bitstream for polylines/polygons
/// Returns the encoded BitWriter bytes
pub fn prepare_line(
    deltas: &[(i32, i32)],
    extra_bit: bool,
    node_flags: Option<&[bool]>,
    ext_type: bool,
) -> Option<Vec<u8>> {
    if deltas.is_empty() {
        return None;
    }

    // Analyze deltas to find sign patterns and bit requirements
    let mut min_dx = i32::MAX;
    let mut max_dx = i32::MIN;
    let mut min_dy = i32::MAX;
    let mut max_dy = i32::MIN;

    for &(dx, dy) in deltas {
        if dx < min_dx { min_dx = dx; }
        if dx > max_dx { max_dx = dx; }
        if dy < min_dy { min_dy = dy; }
        if dy > max_dy { max_dy = dy; }
    }

    let x_bits = bits_needed(min_dx).max(bits_needed(max_dx));
    let y_bits = bits_needed(min_dy).max(bits_needed(max_dy));

    let x_base_initial = bits2base(x_bits);
    let y_base_initial = bits2base(y_bits);

    let x_same_sign_initial = !(min_dx < 0 && max_dx > 0);
    let y_same_sign_initial = !(min_dy < 0 && max_dy > 0);
    let x_sign_negative = min_dx < 0;
    let y_sign_negative = min_dy < 0;

    // Try optimization: reduce base and disable same_sign to find shortest bitstream
    let bs_simple = make_bit_stream(
        deltas, x_base_initial, y_base_initial,
        x_same_sign_initial, x_sign_negative,
        y_same_sign_initial, y_sign_negative,
        extra_bit, node_flags, ext_type,
    )?;

    let mut best = bs_simple.clone();
    let mut x_best_base = x_base_initial;

    // Optimize x base
    if x_base_initial > 0 {
        let mut not_better = 0;
        for x_test in (0..x_base_initial).rev() {
            if let Some(test) = make_bit_stream(
                deltas, x_test, y_base_initial,
                false, x_sign_negative,
                y_same_sign_initial, y_sign_negative,
                extra_bit, node_flags, ext_type,
            ) {
                if test.len() >= best.len() {
                    not_better += 1;
                    if not_better >= 2 { break; }
                } else {
                    x_best_base = x_test;
                    best = test;
                }
            }
        }
    }

    // Optimize y base
    if y_base_initial > 0 {
        let mut not_better = 0;
        for y_test in (0..y_base_initial).rev() {
            if let Some(test) = make_bit_stream(
                deltas, x_best_base, y_test,
                false, x_sign_negative,
                false, y_sign_negative,
                extra_bit, node_flags, ext_type,
            ) {
                if test.len() >= best.len() {
                    not_better += 1;
                    if not_better >= 2 { break; }
                } else {
                    best = test;
                }
            }
        }
    }

    // If byte length wasn't improved, prefer the simple version
    if bs_simple.len() == best.len() {
        Some(bs_simple)
    } else {
        Some(best)
    }
}

/// Build the bitstream for given base values — mkgmap LinePreparer.makeBitStream
fn make_bit_stream(
    deltas: &[(i32, i32)],
    x_base: i32,
    y_base: i32,
    x_same_sign: bool,
    x_sign_negative: bool,
    y_same_sign: bool,
    y_sign_negative: bool,
    extra_bit: bool,
    node_flags: Option<&[bool]>,
    ext_type: bool,
) -> Option<Vec<u8>> {
    let mut x_bits = base2bits(x_base) as usize;
    if !x_same_sign { x_bits += 1; }
    let mut y_bits = base2bits(y_base) as usize;
    if !y_same_sign { y_bits += 1; }

    let mut bw = BitWriter::with_capacity(deltas.len() * 4 + 8);

    // Header: xBase(4) + yBase(4)
    bw.putn(x_base as u32, 4);
    bw.putn(y_base as u32, 4);

    // Sign info
    bw.put1(x_same_sign);
    if x_same_sign {
        bw.put1(x_sign_negative);
    }

    bw.put1(y_same_sign);
    if y_same_sign {
        bw.put1(y_sign_negative);
    }

    // Extended type extra bit flag
    if ext_type {
        bw.put1(false);
    }

    // First extra bit (always false)
    if extra_bit {
        bw.put1(false);
    }

    // Encode deltas
    let mut num_points = 1; // start point counts
    for (i, &(dx, dy)) in deltas.iter().enumerate() {
        if dx == 0 && dy == 0 {
            if extra_bit {
                if let Some(flags) = node_flags {
                    if !flags[i + 1] && i + 1 != deltas.len() {
                        continue; // skip non-node zero delta
                    }
                }
            }
        }
        num_points += 1;

        if x_same_sign {
            bw.putn(dx.unsigned_abs(), x_bits);
        } else {
            bw.sputn(dx, x_bits);
        }

        if y_same_sign {
            bw.putn(dy.unsigned_abs(), y_bits);
        } else {
            bw.sputn(dy, y_bits);
        }

        if extra_bit {
            if let Some(flags) = node_flags {
                bw.put1(flags[i + 1]);
            }
        }
    }

    if num_points < 2 {
        return None;
    }

    Some(bw.bytes().to_vec())
}

/// Number of bits needed to hold |val| — mkgmap LinePreparer.bitsNeeded
pub fn bits_needed(val: i32) -> i32 {
    let n = val.unsigned_abs();
    if n == 0 { 0 } else { (32 - n.leading_zeros()) as i32 }
}

/// Convert base to actual bits — mkgmap LinePreparer.base2Bits
pub fn base2bits(base: i32) -> i32 {
    let bits = 2;
    if base < 10 {
        bits + base
    } else {
        bits + (2 * base) - 9
    }
}

/// Convert actual bits to base — mkgmap LinePreparer.bits2Base
pub fn bits2base(bits: i32) -> i32 {
    let mut base = (bits - 2).max(0);
    if base > 10 {
        if (base & 1) == 0 {
            base += 1;
        }
        base = 9 + (base - 9) / 2;
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_needed() {
        assert_eq!(bits_needed(0), 0);
        assert_eq!(bits_needed(1), 1);
        assert_eq!(bits_needed(-1), 1);
        assert_eq!(bits_needed(3), 2);
        assert_eq!(bits_needed(7), 3);
        assert_eq!(bits_needed(8), 4);
        assert_eq!(bits_needed(-128), 8); // 128 needs 8 bits
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
    }

    #[test]
    fn test_bits_base_roundtrip() {
        for base in 0..15 {
            let bits = base2bits(base);
            let back = bits2base(bits);
            assert_eq!(back, base, "roundtrip failed for base {base}");
        }
    }

    #[test]
    fn test_prepare_simple_line() {
        // Simple horizontal line: deltas all positive
        let deltas = vec![(10, 0), (10, 0), (10, 0)];
        let result = prepare_line(&deltas, false, None, false);
        assert!(result.is_some());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_prepare_diagonal_line() {
        let deltas = vec![(5, 5), (-3, 2), (1, -4)];
        let result = prepare_line(&deltas, false, None, false);
        assert!(result.is_some());
    }

    #[test]
    fn test_prepare_with_zero_deltas() {
        let deltas = vec![(0, 0), (5, 5)];
        let result = prepare_line(&deltas, false, None, false);
        assert!(result.is_some());
    }

    #[test]
    fn test_prepare_single_delta() {
        let deltas = vec![(1, 1)];
        let result = prepare_line(&deltas, false, None, false);
        assert!(result.is_some());
    }

    #[test]
    fn test_prepare_empty() {
        let result = prepare_line(&[], false, None, false);
        assert!(result.is_none());
    }
}
