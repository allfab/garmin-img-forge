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

    /// Write `count` zero bits followed by one 1-bit (unary coding)
    fn write_unary(&mut self, count: u32) {
        for _ in 0..count {
            self.add_bit(false);
        }
        self.add_bit(true);
    }

    /// Write `n_bits` of value, MSB first
    fn write_bits(&mut self, value: u32, n_bits: u32) {
        for i in (0..n_bits).rev() {
            self.add_bit((value >> i) & 1 != 0);
        }
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

    #[allow(dead_code)]
    fn bit_count(&self) -> usize {
        self.bits.len()
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
/// Faithful to mkgmap DEMTile.java encodeDeltas().
pub fn encode_dem_tile(heights: &[i16], width: u32, height: u32) -> DemTileResult {
    let total = (width * height) as usize;
    assert_eq!(heights.len(), total);

    // Check for all-UNDEF tile
    let has_undef = heights.iter().any(|&h| h == UNDEF);
    let valid_heights: Vec<i16> = heights.iter().copied().filter(|&h| h != UNDEF).collect();

    if valid_heights.is_empty() {
        // All NODATA
        return DemTileResult {
            base_height: 0,
            max_delta: 0,
            encoding_type: 0x02,
            bitstream: Vec::new(),
        };
    }

    let min_h = *valid_heights.iter().min().unwrap();
    let max_h = *valid_heights.iter().max().unwrap();
    let base_height = min_h;
    let max_delta = (max_h - min_h) as u16;

    // Normalize heights: replace UNDEF with max_delta, subtract base
    let mut normalized = vec![0u32; total];
    for i in 0..total {
        if heights[i] == UNDEF {
            normalized[i] = max_delta as u32;
        } else {
            normalized[i] = (heights[i] - base_height) as u32;
        }
    }

    // If all same height, minimal bitstream
    if max_delta == 0 {
        return DemTileResult {
            base_height,
            max_delta: 0,
            encoding_type: if has_undef { 0x02 } else { 0x00 },
            bitstream: Vec::new(),
        };
    }

    // Encode using delta prediction + hybrid/plateau compression
    let bitstream = encode_deltas(&normalized, width, height, max_delta);

    DemTileResult {
        base_height,
        max_delta,
        encoding_type: if has_undef { 0x02 } else { 0x00 },
        bitstream,
    }
}

/// Encode delta-compressed bitstream (faithful to mkgmap DEMTile.encodeDeltas)
fn encode_deltas(normalized: &[u32], width: u32, height: u32, max_delta: u16) -> Vec<u8> {
    let mut bw = DemBitWriter::new();
    let max_zero_bits = get_max_zero_bits(max_delta);
    let big_bin_bits = get_big_bin_bits(max_delta);
    let mut hunit = get_start_hunit(max_delta);

    // Windowed statistics for hunit adaptation (F5 fix)
    let mut window_sum: u64 = 0;
    let mut window_count: u32 = 0;

    let mut col = 0u32;
    let mut row = 0u32;

    while row < height {
        while col < width {
            let idx = (row * width + col) as usize;
            let val = normalized[idx];

            let h_upper = if row > 0 { normalized[((row - 1) * width + col) as usize] } else { 0 };
            let h_left = if col > 0 { normalized[idx - 1] } else { 0 };
            let h_up_left = if row > 0 && col > 0 { normalized[((row - 1) * width + col - 1) as usize] } else { 0 };

            if h_upper == h_left {
                // Plateau mode: count how many consecutive elements equal h_upper
                let plateau_len = count_plateau(normalized, row, col, width, h_upper);

                // Write the plateau run-length, then skip those elements
                write_plateau_len(&mut bw, plateau_len);

                // Advance past the plateau elements
                for i in 0..plateau_len {
                    let skip_idx = (row * width + col + i) as usize;
                    window_sum += normalized[skip_idx] as u64;
                    window_count += 1;

                    if window_count >= 64 {
                        hunit = adapt_hunit(window_sum, window_count, max_delta);
                        window_sum = 0;
                        window_count = 0;
                    }
                }
                col += plateau_len;

                // After plateau, if there are more elements in this row and the next
                // element differs from h_upper, encode it with standard prediction
                if col < width {
                    let next_idx = (row * width + col) as usize;
                    let next_val = normalized[next_idx];
                    let predict = clamp_predict(h_upper as i64 + h_upper as i64 - h_up_left as i64, max_delta);
                    let delta = wrap(next_val as i32 - predict as i32, max_delta);
                    write_val_hybrid(&mut bw, delta, hunit, max_zero_bits, big_bin_bits, max_delta);

                    window_sum += next_val as u64;
                    window_count += 1;
                    col += 1;

                    if window_count >= 64 {
                        hunit = adapt_hunit(window_sum, window_count, max_delta);
                        window_sum = 0;
                        window_count = 0;
                    }
                }
            } else {
                // Standard prediction mode
                let predict = clamp_predict(h_left as i64 + h_upper as i64 - h_up_left as i64, max_delta);
                let delta = wrap(val as i32 - predict as i32, max_delta);
                write_val_hybrid(&mut bw, delta, hunit, max_zero_bits, big_bin_bits, max_delta);

                window_sum += val as u64;
                window_count += 1;
                col += 1;

                if window_count >= 64 {
                    hunit = adapt_hunit(window_sum, window_count, max_delta);
                    window_sum = 0;
                    window_count = 0;
                }
            }
        }
        col = 0;
        row += 1;
    }

    bw.into_bytes()
}

/// Count how many consecutive elements from (row, col) equal `plateau_val`
fn count_plateau(normalized: &[u32], row: u32, col: u32, width: u32, plateau_val: u32) -> u32 {
    let mut count = 0;
    let mut c = col;
    while c < width {
        let idx = (row * width + c) as usize;
        if normalized[idx] != plateau_val {
            break;
        }
        count += 1;
        c += 1;
    }
    count
}

/// Write plateau length using the plateau unit/binbits tables
fn write_plateau_len(bw: &mut DemBitWriter, len: u32) {
    if len == 0 {
        bw.write_unary(0); // single 1-bit = zero length
        return;
    }

    let mut remaining = len;
    let mut table_idx = 0;

    while remaining > 0 && table_idx < PLATEAU_UNIT.len() {
        let unit = PLATEAU_UNIT[table_idx];
        let bin_bits = PLATEAU_BIN_BITS[table_idx];

        if remaining < unit * (1 << bin_bits) + unit {
            // This table entry can encode the remaining length
            let quotient = remaining / unit;
            let remainder = remaining % unit;
            bw.write_unary(quotient);
            if bin_bits > 0 {
                bw.write_bits(remainder, bin_bits);
            }
            return;
        }

        remaining -= unit * (1 << bin_bits);
        table_idx += 1;
    }

    // Fallback for very long plateaus
    bw.write_unary(remaining);
}

/// Hybrid value encoding (faithful to mkgmap writeValHybrid)
fn write_val_hybrid(
    bw: &mut DemBitWriter,
    val: u32,
    hunit: u32,
    max_zero_bits: u32,
    big_bin_bits: u32,
    max_delta: u16,
) {
    if val == 0 {
        bw.add_bit(true); // single 1-bit for zero
        return;
    }

    let abs_val = if val <= max_delta as u32 / 2 {
        val
    } else {
        max_delta as u32 + 1 - val
    };
    let sign = val > max_delta as u32 / 2;

    let len_part = (abs_val - 1) / hunit;
    let bin_part = (abs_val - 1) % hunit;
    let hunit_bits = log2_floor(hunit);

    if len_part <= max_zero_bits {
        // Normal hybrid: len_part zeros + 1 + bin_part + sign
        bw.write_unary(len_part);
        if hunit_bits > 0 {
            bw.write_bits(bin_part, hunit_bits);
        }
        bw.add_bit(sign);
    } else {
        // BigBin fallback
        for _ in 0..=max_zero_bits {
            bw.add_bit(false);
        }
        bw.write_bits(abs_val, big_bin_bits);
        bw.add_bit(!sign); // sign is inverted in bigbin
    }
}

/// Wrap signed delta into unsigned range for encoding
fn wrap(val: i32, max_delta: u16) -> u32 {
    let md = max_delta as i32 + 1;
    let mut v = val % md;
    if v < 0 {
        v += md;
    }
    v as u32
}

/// Clamp prediction to valid range [0, max_delta]
fn clamp_predict(val: i64, max_delta: u16) -> u32 {
    val.max(0).min(max_delta as i64) as u32
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

/// Get max zero bits for unary coding (mkgmap getMaxZeroBits)
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
    else if md < 512 { 23 }
    else if md < 1024 { 24 }
    else if md < 2048 { 25 }
    else if md < 4096 { 26 }
    else if md < 8192 { 27 }
    else { 43 }
}

/// Get big bin bits (mkgmap getBigBinBits)
fn get_big_bin_bits(max_delta: u16) -> u32 {
    if max_delta == 0 { return 0; }
    log2_floor(max_delta as u32) + 1
}

/// Floor of log2
fn log2_floor(val: u32) -> u32 {
    if val == 0 { return 0; }
    31 - val.leading_zeros()
}

/// Adapt hunit based on running statistics (mkgmap style)
fn adapt_hunit(sum_h: u64, elem_count: u32, max_delta: u16) -> u32 {
    if elem_count == 0 { return get_start_hunit(max_delta); }
    let avg = sum_h / elem_count as u64;
    let md = max_delta as u64;
    if md == 0 { return 1; }

    // Target hunit = power of 2 near average / 2
    let target = if avg > 0 { avg / 2 } else { 1 };
    let mut hunit = 1u32;
    while (hunit as u64) < target && hunit < 256 {
        hunit *= 2;
    }
    hunit.max(1).min(256)
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

        let dist_i = dist as i64;
        let dist_u = dist as u32;
        // Align bounds to DEM grid (use i64 throughout for negative coords — F2 fix)
        let left_i64 = (bounds.west / (dist as f64 * FACTOR)).floor() as i64 * dist_i * 256;
        let top_i64 = (bounds.north / (dist as f64 * FACTOR)).ceil() as i64 * dist_i * 256;
        let right_i64 = (bounds.east / (dist as f64 * FACTOR)).ceil() as i64 * dist_i * 256;
        let bottom_i64 = (bounds.south / (dist as f64 * FACTOR)).floor() as i64 * dist_i * 256;

        // Garmin DEM uses u32 wrapping for coordinates
        let left = left_i64 as u32;
        let top = top_i64 as u32;

        // Calculate total points using i64 to avoid underflow
        let total_points_lon = ((right_i64 - left_i64) / (dist_i * 256) + 1) as u32;
        let total_points_lat = ((top_i64 - bottom_i64) / (dist_i * 256) + 1) as u32;

        // Tile counts
        let tiles_lon = (total_points_lon + STD_DIM - 1) / STD_DIM;
        let tiles_lat = (total_points_lat + STD_DIM - 1) / STD_DIM;

        // Non-standard sizes for last row/column
        let non_std_width = total_points_lon % STD_DIM;
        let non_std_width = if non_std_width == 0 { STD_DIM } else { non_std_width };
        let non_std_height = total_points_lat % STD_DIM;
        let non_std_height = if non_std_height == 0 { STD_DIM } else { non_std_height };

        // Step size in degrees
        let step_lat = dist as f64 * FACTOR;
        let step_lon = dist as f64 * FACTOR;

        // Top-left corner in degrees (use i64 for negative coords)
        let top_lat = top_i64 as f64 * FACTOR / 256.0;
        let left_lon = left_i64 as f64 * FACTOR / 256.0;

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

// ─── DemWriter: complete DEM subfile ───

/// DEM subfile writer (like TreWriter, RgnWriter, etc.)
pub struct DemWriter {
    sections: Vec<DemSection>,
    adjusted_bounds: Option<GeoBounds>,
}

impl DemWriter {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
            adjusted_bounds: None,
        }
    }

    /// Calculate DEM sections for all zoom levels.
    /// Returns adjusted bounds if DEM grid alignment extends beyond tile bounds.
    pub fn calc(
        &mut self,
        bounds: &GeoBounds,
        dem_config: &DemConfig,
        converter: &DemConverter,
        levels: &[Zoom],
    ) -> GeoBounds {
        self.sections.clear();

        for (i, level) in levels.iter().enumerate() {
            let dist = if i < dem_config.dists.len() {
                dem_config.dists[i]
            } else if !dem_config.dists.is_empty() {
                // Scale from last provided distance
                let last = *dem_config.dists.last().unwrap();
                last * (1 << (i - dem_config.dists.len() + 1)) as i32
            } else {
                -1 // Auto
            };

            let section = DemSection::new(level.level as u8, bounds, dist, converter);
            self.sections.push(section);
        }

        // Calculate adjusted bounds from all sections
        let mut adj = bounds.clone();
        for section in &self.sections {
            let s_west = section.left as f64 * FACTOR / 256.0;
            let s_north = section.top as f64 * FACTOR / 256.0;
            let s_east = s_west + (section.tiles_lon * STD_DIM) as f64 * section.points_distance_lon as f64 * FACTOR;
            let s_south = s_north - (section.tiles_lat * STD_DIM) as f64 * section.points_distance_lat as f64 * FACTOR;

            if s_west < adj.west { adj.west = s_west; }
            if s_east > adj.east { adj.east = s_east; }
            if s_south < adj.south { adj.south = s_south; }
            if s_north > adj.north { adj.north = s_north; }
        }

        self.adjusted_bounds = Some(adj.clone());
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
    fn test_wrap() {
        assert_eq!(wrap(0, 100), 0);
        assert_eq!(wrap(5, 100), 5);
        assert_eq!(wrap(-5, 100), 96); // 101 - 5
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
        assert_eq!(get_max_zero_bits(10000), 43);
    }

    #[test]
    fn test_get_big_bin_bits() {
        assert_eq!(get_big_bin_bits(0), 0);
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
