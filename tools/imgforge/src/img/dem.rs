// DEM subfile writer — Garmin DEM format encoder
//
// Encodes elevation data into the Garmin DEM subfile format with:
// - Multi-level zoom sections (DEMSection, 64×64 tiles)
// - Compressed bitstream encoding (delta + hybrid + plateau run-length)
// - Faithful to mkgmap DEMFile.java / DEMSection.java / DEMTile.java

use crate::dem::{DemConfig, GeoBounds};
use crate::dem::converter::DemConverter;
use super::common_header::{CommonHeader, pad_to};
use super::zoom::Zoom;

/// Standard tile dimension (points per side)
const STD_DIM: u32 = 64;

/// Conversion factor: DEM units ↔ degrees
/// 45.0 / (1 << 29) degrees per DEM unit
const FACTOR: f64 = 45.0 / (1u64 << 29) as f64;

/// UNDEF elevation value
const UNDEF: i16 = -32768;

/// DEM header size (CommonHeader 21 + specific 20 = 41)
const DEM_HEADER_LEN: usize = 41;

// ─── Plateau encoding tables (from mkgmap DEMTile.java) ───

const PLATEAU_UNIT: [u32; 23] = [
    1, 1, 1, 1,  2, 2, 2, 2,  4, 4, 4, 4,
    8, 8, 8, 8,  16, 16,  32, 32,  64, 64,  128,
];

const PLATEAU_BIN_BITS: [u32; 23] = [
    0, 0, 0, 1,  1, 1, 1, 2,  2, 2, 2, 3,
    3, 3, 3, 4,  4, 5,  5, 6,  6, 7,  8,
];

// ─── MSB-first BitWriter for DEM bitstream ───

struct DemBitWriter {
    bits: Vec<bool>,
}

impl DemBitWriter {
    fn new() -> Self {
        Self { bits: Vec::with_capacity(4096) }
    }

    fn add_bit(&mut self, bit: bool) {
        self.bits.push(bit);
    }

    fn into_bytes(self) -> Vec<u8> {
        let byte_len = (self.bits.len() + 7) / 8;
        let mut bytes = vec![0u8; byte_len];
        for (i, &bit) in self.bits.iter().enumerate() {
            if bit {
                // MSB-first packing: bit 0 of stream → bit 7 of byte 0
                bytes[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        bytes
    }

}

// ─── DemTile: single 64×64 tile compression ───

/// Result of encoding a single DEM tile
pub struct DemTileResult {
    pub base_height: i16,
    pub max_delta: u16,
    pub encoding_type: u8,
    pub bitstream: Vec<u8>,
}

/// Encode a single DEM tile's heights into a compressed bitstream.
/// Faithful to mkgmap DEMTile.java constructor + encodeDeltas().
pub fn encode_dem_tile(heights: &[i16], width: u32, height: u32) -> DemTileResult {
    let total = (width * height) as usize;
    assert_eq!(heights.len(), total);

    // Check for all-UNDEF and compute min/max
    let mut min_h = i32::MAX;
    let mut max_h = i32::MIN;
    let mut count_invalid = 0usize;
    for &h in heights {
        if h == UNDEF {
            count_invalid += 1;
        } else {
            let hi = h as i32;
            if hi > max_h { max_h = hi; }
            if hi < min_h { min_h = hi; }
        }
    }

    let (base_height, max_delta_height, encoding_type, has_data);
    if min_h == i32::MAX {
        // All values invalid
        has_data = false;
        encoding_type = 2u8;
        min_h = 0;
        max_h = 0;
        base_height = 0i16;
        max_delta_height = 0u16;
    } else if count_invalid > 0 {
        // Some values invalid — mkgmap: max++, encodingType=2
        has_data = true;
        encoding_type = 2;
        max_h += 1; // reserve highest value for UNDEF marker
        base_height = min_h as i16;
        max_delta_height = (max_h - min_h) as u16;
    } else {
        has_data = true;
        encoding_type = 0;
        base_height = min_h as i16;
        max_delta_height = (max_h - min_h) as u16;
    }

    if !has_data || min_h == max_h {
        return DemTileResult {
            base_height,
            max_delta: max_delta_height,
            encoding_type,
            bitstream: Vec::new(),
        };
    }

    // Normalize heights: UNDEF → maxDeltaHeight, else subtract base
    let md = max_delta_height as i32;
    let mut normalized = vec![0i32; total];
    for i in 0..total {
        if heights[i] == UNDEF {
            normalized[i] = md;
        } else {
            normalized[i] = heights[i] as i32 - min_h;
        }
    }

    let bitstream = encode_deltas(&normalized, width as usize, height as usize, md);

    DemTileResult {
        base_height,
        max_delta: max_delta_height,
        encoding_type,
        bitstream,
    }
}

// ─── Encoding types and wrap types (from mkgmap DEMTile.java) ───

#[derive(Clone, Copy, PartialEq)]
enum EncType { Hybrid, Len }

#[derive(Clone, Copy, PartialEq)]
enum WrapType { Wrap0, Wrap1, Wrap2 }

#[derive(Clone, Copy, PartialEq)]
enum CalcType { CalcPLen, CalcStd, CalcPlateauZero, CalcPlateauNonZero }

/// ValPredicter — keeps statistics about previously encoded values and predicts the next.
/// Faithful to mkgmap DEMTile.ValPredicter inner class.
struct ValPredicter {
    calc_type: CalcType,
    enc_type: EncType,
    wrap_type: WrapType,
    sum_h: i32,
    sum_l: i32,
    elem_count: i32,
    hunit: i32,
    unit_delta: i32,
    max_zero_bits: i32,
    max_delta_height: i32,
    // Wrap thresholds
    l0_wrap_down: i32, l0_wrap_up: i32,
    l1_wrap_down: i32, l1_wrap_up: i32,
    l2_wrap_down: i32, l2_wrap_up: i32,
    h_wrap_down: i32, h_wrap_up: i32,
}

impl ValPredicter {
    fn new(calc_type: CalcType, max_height: i32) -> Self {
        let num_zero_bits = get_max_zero_bits(max_height as u16) as i32;
        let max_zero_bits = if calc_type == CalcType::CalcPlateauNonZero
            || calc_type == CalcType::CalcPlateauZero {
            num_zero_bits - 1
        } else {
            num_zero_bits
        };

        let unit_delta = ((max_height - 0x5f).max(0)) / 0x40;
        let hunit = get_start_hunit(max_height as u16) as i32;

        let (l0_wrap_down, l0_wrap_up, l1_wrap_down, l1_wrap_up, l2_wrap_down, l2_wrap_up);
        if max_height % 2 == 0 {
            l0_wrap_down = max_height / 2;
            l0_wrap_up = -(max_height / 2);
            l1_wrap_down = (max_height + 2) / 2;
            l1_wrap_up = -(max_height / 2);
            l2_wrap_down = max_height / 2;
            l2_wrap_up = -(max_height / 2);
        } else {
            l0_wrap_down = (max_height + 1) / 2;
            l0_wrap_up = -((max_height - 1) / 2);
            l1_wrap_down = (max_height + 1) / 2;
            l1_wrap_up = -((max_height - 1) / 2);
            l2_wrap_down = (max_height - 1) / 2;
            l2_wrap_up = -((max_height + 1) / 2);
        }

        let h_wrap_down = (max_height + 1) / 2;
        let h_wrap_up = -((max_height - 1) / 2);

        Self {
            calc_type, enc_type: EncType::Hybrid, wrap_type: WrapType::Wrap0,
            sum_h: 0, sum_l: 0, elem_count: 0,
            hunit, unit_delta, max_zero_bits, max_delta_height: max_height,
            l0_wrap_down, l0_wrap_up, l1_wrap_down, l1_wrap_up,
            l2_wrap_down, l2_wrap_up, h_wrap_down, h_wrap_up,
        }
    }

    fn wrap(&self, data: i32) -> i32 {
        let mut v = data;
        let (down, up) = if self.enc_type == EncType::Hybrid {
            (self.h_wrap_down, self.h_wrap_up)
        } else {
            match self.wrap_type {
                WrapType::Wrap0 => (self.l0_wrap_down, self.l0_wrap_up),
                WrapType::Wrap1 => (self.l1_wrap_down, self.l1_wrap_up),
                WrapType::Wrap2 => (self.l2_wrap_down, self.l2_wrap_up),
            }
        };
        if v > down { v -= self.max_delta_height + 1; }
        if v < up { v += self.max_delta_height + 1; }
        v
    }

    fn write(&mut self, val: i32, bw: &mut DemBitWriter,
             curr_calc_type: CalcType, curr_plateau_table_pos: usize,
             d_diff: i32) {
        let wrapped = self.wrap(val);
        let mut delta1 = wrapped;

        if self.calc_type == CalcType::CalcPlateauZero {
            if delta1 <= 0 { delta1 += 1; }
        } else if self.calc_type == CalcType::CalcPlateauNonZero {
            if d_diff > 0 { delta1 = -delta1; }
        }

        let delta2 = match self.wrap_type {
            WrapType::Wrap0 => delta1,
            WrapType::Wrap1 => 1 - delta1,
            WrapType::Wrap2 => -delta1,
        };

        let mut written = false;
        let current_max = self.get_current_max_zero_bits(curr_calc_type, curr_plateau_table_pos);

        if self.enc_type == EncType::Hybrid {
            written = write_val_hybrid(bw, delta2, self.hunit, current_max);
        } else {
            // EncType::Len
            let n0 = if delta2 < 0 {
                -delta2 * 2
            } else if delta2 > 0 {
                (delta2 - 1) * 2 + 1
            } else {
                0
            };
            if n0 <= current_max {
                write_number_of_zero_bits(bw, n0);
                written = true;
            }
        }

        if !written {
            write_val_big_bin(bw, delta2, current_max, self.max_delta_height);
        }

        self.process_val(delta1);
    }

    fn get_current_max_zero_bits(&self, curr_calc_type: CalcType, curr_plateau_table_pos: usize) -> i32 {
        if curr_calc_type == CalcType::CalcPlateauNonZero
            || curr_calc_type == CalcType::CalcPlateauZero {
            self.max_zero_bits - PLATEAU_BIN_BITS[curr_plateau_table_pos] as i32
        } else {
            self.max_zero_bits
        }
    }

    fn process_val(&mut self, delta1: i32) {
        match self.calc_type {
            CalcType::CalcStd => {
                // sumH: hybrid threshold
                self.sum_h += delta1.abs();
                if self.sum_h + self.unit_delta + 1 >= 0xffff {
                    self.sum_h -= 0x10000;
                }

                // sumL: length encoding threshold
                let mut work_data = delta1;
                let mut eval_region: i32 = -1;
                if self.elem_count == 63 {
                    eval_region = get_evaluate_data_region(self.sum_l, self.elem_count, delta1);
                    let data_even = delta1 % 2 == 0;
                    let sum_l1 = (self.sum_l - 1) % 4 == 0;
                    match eval_region {
                        0 | 2 | 4 => {
                            if (sum_l1 && !data_even) || (!sum_l1 && data_even) {
                                work_data += 1;
                            }
                        }
                        1 => {
                            work_data += 1;
                            if (sum_l1 && !data_even) || (!sum_l1 && data_even) {
                                work_data += 1;
                            }
                        }
                        3 => {
                            if (sum_l1 && data_even) || (!sum_l1 && !data_even) {
                                work_data -= 1;
                            }
                        }
                        _ => {}
                    }
                }
                if eval_region < 0 {
                    eval_region = get_evaluate_data_region(self.sum_l, self.elem_count, work_data);
                }
                let eval = evaluate_data(self.sum_l, self.elem_count, work_data, eval_region);
                self.sum_l += eval;

                self.elem_count += 1;
                if self.elem_count == 64 {
                    self.elem_count = 32;
                    self.sum_h = ((self.sum_h - self.unit_delta) >> 1) - 1;
                    self.sum_l /= 2;
                }

                self.hunit = normalize_hunit((self.unit_delta + self.sum_h + 1) / (self.elem_count + 1));

                self.wrap_type = WrapType::Wrap0;
                if self.hunit > 0 {
                    self.enc_type = EncType::Hybrid;
                } else {
                    self.enc_type = EncType::Len;
                    if self.sum_l > 0 { self.wrap_type = WrapType::Wrap1; }
                }
            }
            CalcType::CalcPlateauZero => {
                self.sum_h += if delta1 > 0 { delta1 } else { 1 - delta1 };
                if self.sum_h + self.unit_delta + 1 >= 0xffff {
                    self.sum_h -= 0x10000;
                }
                self.sum_l += if delta1 <= 0 { -1 } else { 1 };
                self.elem_count += 1;
                if self.elem_count == 64 {
                    self.elem_count = 32;
                    self.sum_h = ((self.sum_h - self.unit_delta) >> 1) - 1;
                    self.sum_l /= 2;
                    if self.sum_l % 2 != 0 { self.sum_l += 1; }
                }
                self.hunit = normalize_hunit((self.unit_delta + self.sum_h + 1 - self.elem_count / 2) / (self.elem_count + 1));
                self.wrap_type = WrapType::Wrap0;
                if self.hunit > 0 {
                    self.enc_type = EncType::Hybrid;
                } else {
                    self.enc_type = EncType::Len;
                    if self.sum_l >= 0 { self.wrap_type = WrapType::Wrap1; }
                }
            }
            CalcType::CalcPlateauNonZero => {
                self.sum_h += delta1.abs();
                if self.sum_h + self.unit_delta + 1 >= 0xffff {
                    self.sum_h -= 0x10000;
                }
                self.sum_l += if delta1 <= 0 { -1 } else { 1 };
                self.elem_count += 1;
                if self.elem_count == 64 {
                    self.elem_count = 32;
                    self.sum_h = ((self.sum_h - self.unit_delta) >> 1) - 1;
                    self.sum_l /= 2;
                    if self.sum_l % 2 != 0 { self.sum_l -= 1; } // different from PlateauZero!
                }
                self.hunit = normalize_hunit((self.unit_delta + self.sum_h + 1) / (self.elem_count + 1));
                self.wrap_type = WrapType::Wrap0;
                if self.hunit > 0 {
                    self.enc_type = EncType::Hybrid;
                } else {
                    self.enc_type = EncType::Len;
                    if self.sum_l <= 0 { self.wrap_type = WrapType::Wrap2; }
                }
            }
            CalcType::CalcPLen => {} // never called with this type
        }
    }
}

/// Encode delta-compressed bitstream (faithful to mkgmap DEMTile.encodeDeltas)
#[allow(unused_assignments)]
fn encode_deltas(heights: &[i32], width: usize, _height: usize, max_delta_height: i32) -> Vec<u8> {
    let mut bw = DemBitWriter::new();

    let mut enc_standard = ValPredicter::new(CalcType::CalcStd, max_delta_height);
    let mut enc_plateau_f0 = ValPredicter::new(CalcType::CalcPlateauZero, max_delta_height);
    let mut enc_plateau_f1 = ValPredicter::new(CalcType::CalcPlateauNonZero, max_delta_height);

    let mut curr_calc_type = CalcType::CalcPLen; // initial dummy
    let mut curr_plateau_table_pos: usize = 0;
    let mut write_follower = false;

    let get_height = |col: i32, row: i32| -> i32 {
        if row < 0 { return 0; }
        if col < 0 {
            return if row == 0 { 0 } else { heights[(row as usize - 1) * width] };
        }
        heights[col as usize + row as usize * width]
    };

    let mut pos: usize = 0;
    while pos < heights.len() {
        let n = (pos % width) as i32; // col
        let m = (pos / width) as i32; // row

        let h_upper = get_height(n, m - 1);
        let h_left = get_height(n - 1, m);
        let d_diff = h_upper - h_left;

        if write_follower {
            let encoder = if d_diff == 0 { &mut enc_plateau_f0 } else { &mut enc_plateau_f1 };
            write_follower = false;

            curr_calc_type = encoder.calc_type;
            let h = get_height(n, m);
            // Plateau follower: predicted value is upper height
            let v = h - h_upper;

            encoder.write(v, &mut bw, curr_calc_type, curr_plateau_table_pos, d_diff);
            pos += 1;
        } else if d_diff == 0 {
            curr_calc_type = CalcType::CalcPLen;
            let p_len = calc_plateau_len(heights, n, m, width);
            write_plateau_len(&mut bw, p_len as u32, n as u32, width as u32, &mut curr_plateau_table_pos);
            pos += p_len;
            write_follower = (pos % width != 0) || p_len == 0;
        } else {
            curr_calc_type = CalcType::CalcStd;
            let h = get_height(n, m);

            // Standard prediction
            let h_up_left = get_height(n - 1, m - 1);
            let hdiff_up = h_upper - h_up_left;
            let predict = if hdiff_up >= max_delta_height - h_left {
                -1
            } else if hdiff_up <= -h_left {
                0
            } else {
                h_left + hdiff_up
            };

            let v = if d_diff > 0 { -h + predict } else { h - predict };

            enc_standard.write(v, &mut bw, curr_calc_type, curr_plateau_table_pos, d_diff);
            pos += 1;
        }
    }

    bw.into_bytes()
}

/// Calculate plateau length — counts elements from (col, row) equal to h_left.
/// Faithful to mkgmap DEMTile.calcPlateauLen: compares with getHeight(col-1, row).
fn calc_plateau_len(heights: &[i32], col: i32, row: i32, width: usize) -> usize {
    let h_left = if col > 0 {
        heights[col as usize - 1 + row as usize * width]
    } else if row == 0 {
        0
    } else {
        heights[(row as usize - 1) * width]
    };

    let mut len = 0usize;
    let mut c = col;
    while (c as usize) < width {
        if heights[c as usize + row as usize * width] != h_left {
            break;
        }
        len += 1;
        c += 1;
    }
    len
}

/// Write plateau length using the plateau unit/binbits tables.
/// Faithful to mkgmap DEMTile.writePlateauLen with persistent currPlateauTablePos.
#[allow(unused_assignments)]
fn write_plateau_len(
    bw: &mut DemBitWriter, p_len: u32, col: u32, width: u32,
    curr_plateau_table_pos: &mut usize,
) {
    let mut len = p_len as i32;
    let mut x = col as i32;

    if col as i32 + len >= width as i32 {
        // Row-end optimization
        while x < width as i32 {
            let unit = PLATEAU_UNIT[*curr_plateau_table_pos] as i32;
            *curr_plateau_table_pos += 1;
            len -= unit;
            x += unit;
            bw.add_bit(true);
        }
        if x != width as i32 {
            *curr_plateau_table_pos -= 1;
        }
    } else {
        loop {
            let unit = PLATEAU_UNIT[*curr_plateau_table_pos] as i32;
            if len < unit { break; }
            *curr_plateau_table_pos += 1;
            len -= unit;
            bw.add_bit(true);
            x += unit;
            if x > width as i32 { *curr_plateau_table_pos -= 1; }
            if x >= width as i32 { return; }
        }
        if *curr_plateau_table_pos > 0 {
            *curr_plateau_table_pos -= 1;
        }

        bw.add_bit(false); // separator bit
        let bin_bits = PLATEAU_BIN_BITS[*curr_plateau_table_pos] as u32;
        if bin_bits > 0 {
            write_val_as_bin(bw, len.unsigned_abs(), bin_bits);
        }
    }
}

/// Write unsigned binary value MSB first
fn write_val_as_bin(bw: &mut DemBitWriter, val: u32, num_bits: u32) {
    if num_bits == 0 && val == 0 { return; }
    let mut t = 1u32 << (num_bits - 1);
    while t > 0 {
        bw.add_bit((val & t) != 0);
        t >>= 1;
    }
}

/// Write length-encoded value: sequence of 0-bits followed by 1-bit
fn write_number_of_zero_bits(bw: &mut DemBitWriter, val: i32) {
    for _ in 0..val { bw.add_bit(false); }
    bw.add_bit(true);
}

/// Hybrid value encoding — returns true if value was written, false if BigBin needed.
/// Faithful to mkgmap DEMTile.writeValHybrid (signed values).
fn write_val_hybrid(bw: &mut DemBitWriter, val: i32, hunit: i32, max_zero_bits: i32) -> bool {
    if hunit <= 0 { return false; }
    let num_bits = (hunit as u32).trailing_zeros();
    let (bin_part, len_part) = if val > 0 {
        ((val - 1) % hunit, (val - 1 - (val - 1) % hunit) / hunit)
    } else {
        ((-val) % hunit, (-val - (-val) % hunit) / hunit)
    };

    if len_part <= max_zero_bits {
        write_number_of_zero_bits(bw, len_part);
        write_val_as_bin(bw, bin_part as u32, num_bits);
        bw.add_bit(val > 0); // sign bit: 1 = positive
        true
    } else {
        false
    }
}

/// BigBin fallback encoding.
/// Faithful to mkgmap DEMTile.writeValBigBin.
fn write_val_big_bin(bw: &mut DemBitWriter, val: i32, num_zero_bits: i32, max_delta_height: i32) {
    // Signal BigBin by writing an invalid number of zero bits
    write_number_of_zero_bits(bw, num_zero_bits + 1);
    let bits = get_big_bin_bits(max_delta_height as u16);
    if val < 0 {
        write_val_as_bin(bw, (-val - 1) as u32, bits - 1);
    } else {
        write_val_as_bin(bw, (val - 1) as u32, bits - 1);
    }
    bw.add_bit(val <= 0); // sign bit: 0 = positive
}

/// evaluateData — from mkgmap DEMTile.evaluateData
fn evaluate_data(oldsum: i32, elemcount: i32, newdata: i32, region: i32) -> i32 {
    match region {
        0 => -1 - oldsum - elemcount,
        1 => 2 * (newdata + elemcount) + 3,
        2 => 2 * newdata - 1,
        3 => 2 * (newdata - elemcount) - 5,
        _ => 1 - oldsum + elemcount,
    }
}

/// getEvaluateDataRegion — from mkgmap DEMTile.getEvaluateDataRegion
fn get_evaluate_data_region(oldsum: i32, elemcount: i32, newdata: i32) -> i32 {
    if elemcount < 63 {
        if newdata < -2 - ((oldsum + 3 * elemcount) >> 1) { 0 }
        else if newdata < -((oldsum + elemcount) >> 1) { 1 }
        else if newdata < 2 - ((oldsum - elemcount) >> 1) { 2 }
        else if newdata < 4 - ((oldsum - 3 * elemcount) >> 1) { 3 }
        else { 4 }
    } else {
        if newdata < -2 - ((oldsum + 3 * elemcount) >> 1) { 0 }
        else if newdata < -((oldsum + elemcount) >> 1) - 1 { 1 } // special case
        else if newdata < 2 - ((oldsum - elemcount) >> 1) { 2 }
        else if newdata < 4 - ((oldsum - 3 * elemcount) >> 1) { 3 }
        else { 4 }
    }
}

/// Normalize hunit to power of 2 (or 0).
/// Faithful to mkgmap DEMTile.normalizeHUnit.
fn normalize_hunit(hu: i32) -> i32 {
    if hu > 0 {
        1 << (31 - (hu as u32).leading_zeros())
    } else {
        0
    }
}

/// Get initial hunit based on max_delta (mkgmap getStartHUnit)
fn get_start_hunit(max_delta: u16) -> u32 {
    let md = max_delta as u32;
    if md < 0x9f { 1 }
    else if md < 0x11f { 2 }
    else if md < 0x21f { 4 }
    else if md < 0x41f { 8 }
    else if md < 0x81f { 16 }
    else if md < 0x101f { 32 }
    else if md < 0x201f { 64 }
    else if md < 0x401f { 128 }
    else { 256 }
}

/// Get max zero bits for unary coding (mkgmap getMaxLengthZeroBits — FIXED)
fn get_max_zero_bits(max_delta: u16) -> u32 {
    let md = max_delta as u32;
    if md < 2 { 15 }
    else if md < 4 { 16 }
    else if md < 8 { 17 }
    else if md < 16 { 18 }
    else if md < 32 { 19 }
    else if md < 64 { 20 }
    else if md < 128 { 21 }
    else if md < 256 { 22 }
    else if md < 512 { 25 }
    else if md < 1024 { 28 }
    else if md < 2048 { 31 }
    else if md < 4096 { 34 }
    else if md < 8192 { 37 }
    else if md < 16384 { 40 }
    else { 43 }
}

/// Get big bin bits (mkgmap getBigBinBits — using highestOneBit approach)
fn get_big_bin_bits(max_delta: u16) -> u32 {
    let md = max_delta as u32;
    if md == 0 { return 1; }
    if md < 16384 {
        let n = 1u32 << (31 - md.leading_zeros()); // highest_one_bit
        n.trailing_zeros() + 1
    } else {
        15
    }
}

// ─── DemSection: one zoom level ───

/// One DEM section (one zoom level)
pub struct DemSection {
    pub zoom_level: u8,
    pub points_per_lat: u32,
    pub points_per_lon: u32,
    pub non_std_height: u32,
    pub non_std_width: u32,
    pub tiles_lat: u32,
    pub tiles_lon: u32,
    pub left: u32,
    pub top: u32,
    pub points_distance_lat: u32,
    pub points_distance_lon: u32,
    pub min_height: i16,
    pub max_height: i16,
    pub tile_results: Vec<DemTileResult>,
    pub flags: u16,
    pub record_desc: u16,
    pub tile_desc_size: u16,
}

impl DemSection {
    /// Create a DEM section for a given zoom level and bounds.
    pub fn new(
        zoom_level: u8,
        bounds: &GeoBounds,
        distance: i32,
        converter: &DemConverter,
    ) -> Self {
        // Calculate points distance in DEM units
        let dist = if distance <= 0 {
            // Auto-calculate from highest resolution
            let res = converter.highest_resolution();
            let d = ((1u64 << 29) as f64 * res / 45.0) as i32;
            let aligned = align_distance(d);
            if aligned == 0 { 16 } else { aligned } // F10 fix: ensure non-zero
        } else {
            let aligned = align_distance(distance);
            if aligned == 0 { 16 } else { aligned } // F10 fix: ensure non-zero
        };

        let dist_i = dist as i32;
        let dist_u = dist as u32;

        // Convert bounds to Garmin map units (24-bit), then ×256 for DEM units
        // This matches mkgmap DEMFile.calc(): int top = treArea.getMaxLat() * 256;
        const MAP_UNIT_FACTOR: f64 = 360.0 / (1u64 << 24) as f64; // degrees per map unit
        let top_mu = (bounds.north / MAP_UNIT_FACTOR).ceil() as i32;
        let left_mu = (bounds.west / MAP_UNIT_FACTOR).floor() as i32;
        let bottom_mu = (bounds.south / MAP_UNIT_FACTOR).floor() as i32;
        let right_mu = (bounds.east / MAP_UNIT_FACTOR).ceil() as i32;

        // DEM units = map_units × 256 (fits in i32 for reasonable latitudes)
        let top_dem = top_mu.wrapping_mul(256);
        let left_dem = left_mu.wrapping_mul(256);
        let bottom_dem = bottom_mu.wrapping_mul(256);
        let right_dem = right_mu.wrapping_mul(256);

        // Align to distance grid (like mkgmap moveUp/moveLeft)
        let extra_dem = (0.1f64 / 45.0 * (1u64 << 29) as f64) as i32;
        let (x_top, x_left) = if dist_i < extra_dem {
            (move_up(top_dem, dist_i), move_left(left_dem, dist_i))
        } else {
            (top_dem, left_dem)
        };

        let left = x_left as u32; // written as i32 in LE, same bit pattern
        let top = x_top as u32;

        // Calculate area dimensions in DEM units (like mkgmap)
        let area_height = x_top - bottom_dem;
        let area_width = right_dem - x_left;

        // getTileInfo normalization (faithful to mkgmap DEMSection.getTileInfo)
        let (tiles_lat, non_std_height) = get_tile_info(area_height, dist_i);
        let (tiles_lon, non_std_width) = get_tile_info(area_width, dist_i);

        // Step size in degrees
        let step_lat = dist as f64 * FACTOR;
        let step_lon = dist as f64 * FACTOR;

        // Top-left corner in degrees (from aligned DEM units = map_units × 256)
        let top_lat = x_top as f64 * MAP_UNIT_FACTOR / 256.0;
        let left_lon = x_left as f64 * MAP_UNIT_FACTOR / 256.0;

        // Generate tiles
        let mut tile_results = Vec::with_capacity((tiles_lat * tiles_lon) as usize);
        let mut global_min = i16::MAX;
        let mut global_max = i16::MIN;

        for tile_row in 0..tiles_lat {
            for tile_col in 0..tiles_lon {
                let tile_h = if tile_row == tiles_lat - 1 { non_std_height } else { STD_DIM };
                let tile_w = if tile_col == tiles_lon - 1 { non_std_width } else { STD_DIM };

                let tile_top = top_lat - (tile_row * STD_DIM) as f64 * step_lat;
                let tile_left = left_lon + (tile_col * STD_DIM) as f64 * step_lon;

                let heights = converter.get_heights(
                    tile_top, tile_left,
                    tile_h, tile_w,
                    step_lat, step_lon,
                );

                // Track min/max
                for &h in &heights {
                    if h != UNDEF {
                        if h < global_min { global_min = h; }
                        if h > global_max { global_max = h; }
                    }
                }

                let result = encode_dem_tile(&heights, tile_w, tile_h);
                tile_results.push(result);
            }
        }

        if global_min > global_max {
            global_min = 0;
            global_max = 0;
        }

        // Calculate record_desc and tile_desc_size
        let (record_desc, tile_desc_size) = calc_record_desc(&tile_results);

        DemSection {
            zoom_level,
            points_per_lat: STD_DIM,
            points_per_lon: STD_DIM,
            non_std_height,
            non_std_width,
            tiles_lat,
            tiles_lon,
            left,
            top,
            points_distance_lat: dist_u,
            points_distance_lon: dist_u,
            min_height: global_min,
            max_height: global_max,
            tile_results,
            flags: 0,
            record_desc,
            tile_desc_size,
        }
    }

    /// Serialize the 60-byte section header
    pub fn build_header(&self, data_offset: u32, data_offset2: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(60);

        buf.push(0x00); // unknown
        buf.push(self.zoom_level);
        buf.extend_from_slice(&self.points_per_lat.to_le_bytes());
        buf.extend_from_slice(&self.points_per_lon.to_le_bytes());
        buf.extend_from_slice(&(self.non_std_height - 1).to_le_bytes());
        buf.extend_from_slice(&(self.non_std_width - 1).to_le_bytes());
        buf.extend_from_slice(&self.flags.to_le_bytes());
        buf.extend_from_slice(&(self.tiles_lon - 1).to_le_bytes());
        buf.extend_from_slice(&(self.tiles_lat - 1).to_le_bytes());
        buf.extend_from_slice(&self.record_desc.to_le_bytes());
        buf.extend_from_slice(&self.tile_desc_size.to_le_bytes());
        buf.extend_from_slice(&data_offset.to_le_bytes());
        buf.extend_from_slice(&data_offset2.to_le_bytes());
        buf.extend_from_slice(&self.left.to_le_bytes());
        buf.extend_from_slice(&self.top.to_le_bytes());
        buf.extend_from_slice(&self.points_distance_lat.to_le_bytes());
        buf.extend_from_slice(&self.points_distance_lon.to_le_bytes());
        buf.extend_from_slice(&self.min_height.to_le_bytes());
        buf.extend_from_slice(&self.max_height.to_le_bytes());

        pad_to(&mut buf, 60);
        buf
    }

    /// Build tile descriptor bytes
    pub fn build_tile_descriptors(&self) -> Vec<u8> {
        let offset_size = ((self.record_desc & 0x03) + 1) as usize;
        let base_size = if self.record_desc & 0x04 != 0 { 2 } else { 1 };
        let delta_size = if self.record_desc & 0x08 != 0 { 2 } else { 1 };
        let has_extra = self.record_desc & 0x10 != 0;

        let mut buf = Vec::new();
        let mut bitstream_offset: u32 = 0;

        for tile in &self.tile_results {
            // Offset
            write_le_n(&mut buf, bitstream_offset, offset_size);
            // Base height
            write_le_n_signed(&mut buf, tile.base_height, base_size);
            // Max delta
            write_le_n(&mut buf, tile.max_delta as u32, delta_size);
            // Extra byte (encoding type)
            if has_extra {
                buf.push(tile.encoding_type);
            }

            bitstream_offset += tile.bitstream.len() as u32;
        }

        buf
    }

    /// Build concatenated bitstream data
    pub fn build_bitstream_data(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for tile in &self.tile_results {
            buf.extend_from_slice(&tile.bitstream);
        }
        buf
    }
}

/// Calculate record_desc bits and tile descriptor size
fn calc_record_desc(tiles: &[DemTileResult]) -> (u16, u16) {
    let mut max_offset: u32 = 0;
    let mut needs_base_2 = false;
    let mut needs_delta_2 = false;
    let mut has_extra = false;

    let mut cumulative_offset: u32 = 0;
    for tile in tiles {
        cumulative_offset += tile.bitstream.len() as u32;
        if cumulative_offset > max_offset {
            max_offset = cumulative_offset;
        }
        if tile.base_height < -128 || tile.base_height > 127 {
            needs_base_2 = true;
        }
        if tile.max_delta > 255 {
            needs_delta_2 = true;
        }
        if tile.encoding_type != 0x00 {
            has_extra = true;
        }
    }

    let offset_size = if max_offset <= 0xFF { 1u16 }
        else if max_offset <= 0xFFFF { 2 }
        else if max_offset <= 0xFFFFFF { 3 }
        else { 4 };

    let mut desc: u16 = (offset_size - 1) & 0x03;
    if needs_base_2 { desc |= 0x04; }
    if needs_delta_2 { desc |= 0x08; }
    if has_extra { desc |= 0x10; }

    let base_size = if needs_base_2 { 2u16 } else { 1 };
    let delta_size = if needs_delta_2 { 2u16 } else { 1 };
    let extra_size = if has_extra { 1u16 } else { 0 };
    let tile_desc_size = offset_size + base_size + delta_size + extra_size;

    (desc, tile_desc_size)
}

/// Write N bytes of a u32 in little-endian
fn write_le_n(buf: &mut Vec<u8>, val: u32, n: usize) {
    let bytes = val.to_le_bytes();
    buf.extend_from_slice(&bytes[..n]);
}

/// Write N bytes of a signed i16 in little-endian
/// For n=1: the value must fit in [-128, 127] (enforced by calc_record_desc)
fn write_le_n_signed(buf: &mut Vec<u8>, val: i16, n: usize) {
    if n == 2 {
        buf.extend_from_slice(&val.to_le_bytes());
    } else {
        // n == 1: write as signed byte (Garmin decoder sign-extends)
        buf.push(val as i8 as u8);
    }
}

/// Align distance to 16-unit boundary
fn align_distance(d: i32) -> i32 {
    ((d + 8) / 16) * 16
}

/// Move latitude up to align to distance grid (mkgmap DEMFile.moveUp)
fn move_up(orig_lat: i32, distance: i32) -> i32 {
    let mut moved = orig_lat;
    if moved >= 0 {
        moved -= moved % distance;
        if moved < 0x3FFFFFFF - distance {
            moved += distance;
        }
    } else {
        moved -= moved % distance;
    }
    moved
}

/// Move longitude left to align to distance grid (mkgmap DEMFile.moveLeft)
fn move_left(orig_lon: i32, distance: i32) -> i32 {
    let mut moved = orig_lon;
    if moved >= 0 {
        moved -= moved % distance;
    } else {
        moved -= moved % distance;
        if moved > i32::MIN + distance {
            moved -= distance;
        }
    }
    moved
}

/// Calculate tile count and non-standard dimension.
/// Faithful to mkgmap DEMSection.getTileInfo — normalizes nonstd to 1..95 range.
fn get_tile_info(dem_points: i32, dem_dist: i32) -> (u32, u32) {
    let resolution = STD_DIM as i32 * dem_dist;
    let points = dem_points + dem_dist; // mkgmap: "Garmin seems to prefer large overlaps"
    let n_full = points / resolution;
    let rest = points - n_full * resolution;
    let mut num = n_full;
    let mut nonstd = rest / dem_dist;
    if rest % dem_dist != 0 {
        nonstd += 1;
    }
    // Normalize so nonstd is between 1..95 (as Garmin does)
    if nonstd >= STD_DIM as i32 / 2 {
        num += 1; // absorb into an extra full tile
    } else if num > 0 {
        nonstd += STD_DIM as i32; // make > 64 so header stores nonstd-1 > 63
    }
    if num == 0 {
        num = 1;
    }
    (num as u32, nonstd as u32)
}

// ─── DemWriter: complete DEM subfile ───

/// DEM subfile writer (like TreWriter, RgnWriter, etc.)
pub struct DemWriter {
    sections: Vec<DemSection>,
}

impl DemWriter {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Calculate DEM sections based on dem-dists configuration.
    /// Like mkgmap, the number of DEM sections is determined by dem_config.dists,
    /// NOT by the number of map levels. Default: 1 section with auto distance.
    /// Returns adjusted bounds if DEM grid alignment extends beyond tile bounds.
    pub fn calc(
        &mut self,
        bounds: &GeoBounds,
        dem_config: &DemConfig,
        converter: &DemConverter,
        _levels: &[Zoom],
    ) -> GeoBounds {
        self.sections.clear();

        // Build the list of distances (like mkgmap MapBuilder.parseDemDists)
        // Default: single entry [-1] = auto-detect from source resolution
        let dists = if dem_config.dists.is_empty() {
            vec![-1i32] // mkgmap default: 1 zoom level, auto distance
        } else {
            dem_config.dists.clone()
        };

        for (i, &dist) in dists.iter().enumerate() {
            let section = DemSection::new(i as u8, bounds, dist, converter);
            self.sections.push(section);
        }

        // Calculate adjusted bounds from all sections
        // left/top are in DEM units (map_units × 256), convert via MAP_UNIT_FACTOR/256
        const MAP_UNIT_FACTOR_ADJ: f64 = 360.0 / (1u64 << 24) as f64;
        let mut adj = bounds.clone();
        for section in &self.sections {
            let s_west = section.left as i32 as f64 * MAP_UNIT_FACTOR_ADJ / 256.0;
            let s_north = section.top as i32 as f64 * MAP_UNIT_FACTOR_ADJ / 256.0;
            let s_east = s_west + (section.tiles_lon * STD_DIM) as f64 * section.points_distance_lon as f64 * FACTOR;
            let s_south = s_north - (section.tiles_lat * STD_DIM) as f64 * section.points_distance_lat as f64 * FACTOR;

            if s_west < adj.west { adj.west = s_west; }
            if s_east > adj.east { adj.east = s_east; }
            if s_south < adj.south { adj.south = s_south; }
            if s_north > adj.north { adj.north = s_north; }
        }

        adj
    }

    /// Build the complete DEM subfile binary
    pub fn build(&self) -> Vec<u8> {
        let zoom_count = self.sections.len() as u16;

        // Pre-compute all section data
        let mut all_tile_descs = Vec::new();
        let mut all_bitstreams = Vec::new();
        for section in &self.sections {
            all_tile_descs.push(section.build_tile_descriptors());
            all_bitstreams.push(section.build_bitstream_data());
        }

        // Layout:
        // 1. CommonHeader (21 bytes) + DEM-specific header (20 bytes) = 41 bytes
        // 2. Tile descriptors for each section
        // 3. Bitstream data for each section
        // 4. Section headers (60 bytes each)

        let mut buf = Vec::with_capacity(4096);

        // 1. CommonHeader
        let ch = CommonHeader::new(DEM_HEADER_LEN as u16, "GARMIN DEM");
        ch.write(&mut buf);

        // DEM-specific header (20 bytes after CommonHeader)
        // [0-3]  u32 LE elevation_units (0 = metres)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // [4-5]  u16 LE zoom level count
        buf.extend_from_slice(&zoom_count.to_le_bytes());
        // [6-9]  u32 LE unknown (0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // [10-11] u16 LE record size (60)
        buf.extend_from_slice(&60u16.to_le_bytes());
        // [12-15] u32 LE sections_offset (placeholder, will be updated)
        let sections_offset_pos = buf.len();
        buf.extend_from_slice(&0u32.to_le_bytes());
        // [16-19] u32 LE unknown (1)
        buf.extend_from_slice(&1u32.to_le_bytes());

        pad_to(&mut buf, DEM_HEADER_LEN);

        // 2. Tile descriptors + 3. Bitstream data
        let mut section_desc_offsets = Vec::new();
        let mut section_data_offsets = Vec::new();

        for i in 0..self.sections.len() {
            let desc_offset = buf.len() as u32;
            section_desc_offsets.push(desc_offset);
            buf.extend_from_slice(&all_tile_descs[i]);

            let data_offset = buf.len() as u32;
            section_data_offsets.push(data_offset);
            buf.extend_from_slice(&all_bitstreams[i]);
        }

        // 4. Section headers (at end of file, like mkgmap)
        let sections_start = buf.len() as u32;

        // Update sections_offset in the DEM header
        let offset_bytes = sections_start.to_le_bytes();
        buf[sections_offset_pos] = offset_bytes[0];
        buf[sections_offset_pos + 1] = offset_bytes[1];
        buf[sections_offset_pos + 2] = offset_bytes[2];
        buf[sections_offset_pos + 3] = offset_bytes[3];

        // Write section headers
        for (i, section) in self.sections.iter().enumerate() {
            let header = section.build_header(
                section_desc_offsets[i],
                section_data_offsets[i],
            );
            buf.extend_from_slice(&header);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dem_bit_writer_msb() {
        let mut bw = DemBitWriter::new();
        // Write bits: 1,0,1,1,0,0,0,1 MSB-first → byte 0xB1
        bw.add_bit(true);
        bw.add_bit(false);
        bw.add_bit(true);
        bw.add_bit(true);
        bw.add_bit(false);
        bw.add_bit(false);
        bw.add_bit(false);
        bw.add_bit(true);
        let bytes = bw.into_bytes();
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0], 0xB1);
    }

    #[test]
    fn test_dem_tile_flat() {
        // All same height → minimal/empty bitstream
        let heights = vec![500i16; 16];
        let result = encode_dem_tile(&heights, 4, 4);
        assert_eq!(result.base_height, 500);
        assert_eq!(result.max_delta, 0);
        assert_eq!(result.encoding_type, 0x00);
        assert!(result.bitstream.is_empty());
    }

    #[test]
    fn test_dem_tile_gradient() {
        // Simple gradient
        let mut heights = Vec::new();
        for r in 0..4 {
            for c in 0..4 {
                heights.push((100 + r * 10 + c * 5) as i16);
            }
        }
        let result = encode_dem_tile(&heights, 4, 4);
        assert_eq!(result.base_height, 100);
        assert_eq!(result.max_delta, 45);
        assert_eq!(result.encoding_type, 0x00);
        assert!(!result.bitstream.is_empty());
    }

    #[test]
    fn test_dem_tile_with_nodata() {
        let mut heights = vec![500i16; 16];
        heights[5] = UNDEF;
        let result = encode_dem_tile(&heights, 4, 4);
        assert_eq!(result.encoding_type, 0x02);
    }

    #[test]
    fn test_dem_tile_all_nodata() {
        let heights = vec![UNDEF; 16];
        let result = encode_dem_tile(&heights, 4, 4);
        assert_eq!(result.encoding_type, 0x02);
        assert!(result.bitstream.is_empty());
    }

    #[test]
    fn test_dem_section_header_size() {
        let header = DemSection {
            zoom_level: 0,
            points_per_lat: 64,
            points_per_lon: 64,
            non_std_height: 64,
            non_std_width: 64,
            tiles_lat: 1,
            tiles_lon: 1,
            left: 0,
            top: 0,
            points_distance_lat: 3312,
            points_distance_lon: 3312,
            min_height: 0,
            max_height: 100,
            tile_results: Vec::new(),
            flags: 0,
            record_desc: 0,
            tile_desc_size: 3,
        }.build_header(0, 0);

        assert_eq!(header.len(), 60);
    }

    #[test]
    fn test_dem_writer_signature() {
        let writer = DemWriter::new();
        let data = writer.build();
        // CommonHeader: first 2 bytes = header_length (41 = 0x29, 0x00)
        assert_eq!(data[0], DEM_HEADER_LEN as u8);
        assert_eq!(data[1], 0x00);
        // Type string at offset 2
        assert_eq!(&data[2..12], b"GARMIN DEM");
    }

    #[test]
    fn test_val_predicter_wrap() {
        let vp = ValPredicter::new(CalcType::CalcStd, 100);
        assert_eq!(vp.wrap(0), 0);
        assert_eq!(vp.wrap(5), 5);
        // Wrapping: val > h_wrap_down (51) → val - 101
        assert_eq!(vp.wrap(60), 60 - 101); // -41
    }

    #[test]
    fn test_align_distance() {
        assert_eq!(align_distance(3314), 3312);
        assert_eq!(align_distance(9942), 9936);
        assert_eq!(align_distance(16), 16);
        assert_eq!(align_distance(0), 0);
    }

    #[test]
    fn test_get_start_hunit() {
        assert_eq!(get_start_hunit(50), 1);
        assert_eq!(get_start_hunit(200), 2);
        assert_eq!(get_start_hunit(1000), 8);
    }

    #[test]
    fn test_get_max_zero_bits() {
        assert_eq!(get_max_zero_bits(1), 15);
        assert_eq!(get_max_zero_bits(100), 21);
        assert_eq!(get_max_zero_bits(300), 25);   // was 23, now fixed per mkgmap
        assert_eq!(get_max_zero_bits(1000), 28);  // was 24, now fixed per mkgmap
        assert_eq!(get_max_zero_bits(10000), 40); // was 43, now correct for <16384
        assert_eq!(get_max_zero_bits(20000), 43);
    }

    #[test]
    fn test_get_big_bin_bits() {
        assert_eq!(get_big_bin_bits(0), 1);  // mkgmap: highestOneBit(0) special case
        assert_eq!(get_big_bin_bits(1), 1);
        assert_eq!(get_big_bin_bits(255), 8);
        assert_eq!(get_big_bin_bits(1000), 10);
    }

    #[test]
    fn test_calc_record_desc() {
        let tiles = vec![
            DemTileResult {
                base_height: 100,
                max_delta: 50,
                encoding_type: 0x00,
                bitstream: vec![0u8; 100],
            },
        ];
        let (desc, size) = calc_record_desc(&tiles);
        // offset fits in 1 byte, base fits in 1 byte, delta fits in 1 byte, no extra
        assert_eq!(desc & 0x03, 0); // offset_size - 1 = 0
        assert_eq!(desc & 0x04, 0); // base_size == 1
        assert_eq!(desc & 0x08, 0); // delta_size == 1
        assert_eq!(desc & 0x10, 0); // no extra
        assert_eq!(size, 3); // 1 + 1 + 1
    }
}
