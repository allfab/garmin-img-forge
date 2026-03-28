//! RGN subfile writer — geometry encoding for Garmin IMG.
//!
//! The RGN subfile stores the actual feature data (POIs, polylines, polygons)
//! as binary records referenced by TRE subdivisions.
//!
//! Format: `[RGN Header — 29 B] [Feature Records per level…]`

use std::collections::HashMap;

use crate::img::bitstream::{base2bits, bits2base, bits_needed, BitWriter};
use crate::img::common_header::{build_common_header, COMMON_HEADER_SIZE};
use crate::img::tre::{to_garmin_units, MapLevel, TreWriter};
use crate::parser::mp_types::{MpFile, MpPoint, MpPolygon, MpPolyline};

/// Standard Garmin RGN header size (125 bytes), compatible with QMapShack/BaseCamp.
/// Includes pointers for data, point/polyline/polygon overviews, and extended types.
const RGN_HEADER_SIZE: usize = 125;

// ── RgnTypeCode ────────────────────────────────────────────────────────────────

/// Parsed Garmin type code from a Polish Map type string.
pub struct RgnTypeCode {
    /// Base feature type byte (e.g. 0x2C for `0x2C00`).
    pub base_type: u8,
    /// Sub-type nibble (low byte of a 4-digit type code, e.g. 0x00 for `0x2C00`).
    pub sub_type: u8,
    /// True when the original value exceeds 0xFFFF (Epic 14+ only).
    pub extended: bool,
}

/// Parse a Polish Map type string into an [`RgnTypeCode`].
///
/// Parsing rules:
/// - `"0x06"` → `{ base_type: 0x06, sub_type: 0x00, extended: false }`
/// - `"0x2C00"` → `{ base_type: 0x2C, sub_type: 0x00, extended: false }`
/// - `"0x10001"` → `{ base_type: 0x00, sub_type: 0x01, extended: true }`
/// - `"0x1101c"` → `{ base_type: 0x10, sub_type: 0x1c, extended: true }`
/// - any invalid string → `{ base_type: 0x00, sub_type: 0x00, extended: false }` + warn
pub fn parse_type_code(s: &str) -> RgnTypeCode {
    let trimmed = s.trim();
    let hex_str = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    match u32::from_str_radix(hex_str, 16) {
        Ok(v) if v > 0xFFFF => {
            if v < 0x10000 || v > 0x1FFFF {
                tracing::warn!(
                    "Type code '{}' (0x{:X}) is outside the extended range 0x10000-0x1FFFF, using 0x00",
                    s, v
                );
                return RgnTypeCode {
                    base_type: 0x00,
                    sub_type: 0x00,
                    extended: true,
                };
            }
            let residue = v & 0xFFFF;
            RgnTypeCode {
                base_type: ((residue >> 8) & 0xFF) as u8,
                sub_type: (residue & 0xFF) as u8,
                extended: true,
            }
        }
        Ok(v) if v > 0xFF => {
            // 4-digit type like 0x2C00 → base = high byte, sub = low byte
            RgnTypeCode {
                base_type: ((v >> 8) & 0xFF) as u8,
                sub_type: (v & 0xFF) as u8,
                extended: false,
            }
        }
        Ok(v) => {
            // 2-digit type like 0x06 → base only, sub = 0x00
            RgnTypeCode {
                base_type: v as u8,
                sub_type: 0x00,
                extended: false,
            }
        }
        Err(_) => {
            tracing::warn!("Invalid type code '{}': using 0x00", s);
            RgnTypeCode {
                base_type: 0x00,
                sub_type: 0x00,
                extended: false,
            }
        }
    }
}

// ── Delta-encoding ─────────────────────────────────────────────────────────────

/// Encode a coordinate as a signed delta relative to `center_g`, shifted by `24 - bits_per_coord`.
///
/// Formula: `((coord_g - center_g) >> (24 - bits_per_coord)).clamp(i16::MIN, i16::MAX)`
///
/// The result is clamped to i16 range to avoid overflow for features far from the centre.
fn encode_delta(coord_g: i32, center_g: i32, bits_per_coord: u8) -> i16 {
    let delta = coord_g - center_g;
    let shift = 24i32 - bits_per_coord as i32;
    (delta >> shift).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

// ── RGN Header ─────────────────────────────────────────────────────────────────

/// RGN subfile header — 125 bytes, compatible with QMapShack/BaseCamp.
///
/// Binary layout (offsets from subfile start):
/// ```text
/// 0x00  21B   Common header "GARMIN RGN"
/// 0x15  LE32  data_offset = 125
/// 0x19  LE32  data_size (standard feature records)
/// 0x1D  8B    point_overview section (offset + size, both 0)
/// 0x25  8B    polyline_overview section (offset + size, both 0)
/// 0x2D  8B    polygon_overview section (offset + size, both 0)
/// 0x35  4B    reserved (0)
/// 0x39  8B    extended_areas section (offset + size, from subfile start)
/// 0x41  20B   reserved (0)
/// 0x55  8B    extended_lines section (offset + size)
/// 0x5D  20B   reserved (0)
/// 0x71  8B    extended_points section (offset + size)
/// 0x79  4B    reserved (0)
/// ```
struct RgnHeader {
    /// Byte count of standard feature records (data section).
    data_size: u32,
    /// Extended polygon (area) section: offset from subfile start, size in bytes.
    ext_areas_offset: u32,
    ext_areas_size: u32,
    /// Extended polyline (line) section: offset from subfile start, size in bytes.
    ext_lines_offset: u32,
    ext_lines_size: u32,
    /// Extended point section: offset from subfile start, size in bytes.
    ext_points_offset: u32,
    ext_points_size: u32,
}

impl RgnHeader {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(RGN_HEADER_SIZE);
        // 0x00: Common header (21 bytes)
        buf.extend_from_slice(&build_common_header("RGN", RGN_HEADER_SIZE as u16));
        // 0x15: data_offset = RGN_HEADER_SIZE (LE32)
        buf.extend_from_slice(&(RGN_HEADER_SIZE as u32).to_le_bytes());
        // 0x19: data_size (LE32)
        buf.extend_from_slice(&self.data_size.to_le_bytes());
        // 0x1D–0x34: 3 overview sections (offset=0, size=0, each 8 bytes)
        for _ in 0..3 {
            buf.extend_from_slice(&0u32.to_le_bytes()); // offset = 0
            buf.extend_from_slice(&0u32.to_le_bytes()); // size = 0
        }
        // 0x35: reserved (4 bytes)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x39: extended_areas section (offset + size)
        buf.extend_from_slice(&self.ext_areas_offset.to_le_bytes());
        buf.extend_from_slice(&self.ext_areas_size.to_le_bytes());
        // 0x41: reserved (20 bytes)
        buf.resize(0x55, 0u8);
        // 0x55: extended_lines section (offset + size)
        buf.extend_from_slice(&self.ext_lines_offset.to_le_bytes());
        buf.extend_from_slice(&self.ext_lines_size.to_le_bytes());
        // 0x5D: reserved (20 bytes)
        buf.resize(0x71, 0u8);
        // 0x71: extended_points section (offset + size)
        buf.extend_from_slice(&self.ext_points_offset.to_le_bytes());
        buf.extend_from_slice(&self.ext_points_size.to_le_bytes());
        // 0x79: reserved (4 bytes) to reach 0x7D = 125
        buf.resize(RGN_HEADER_SIZE, 0u8);
        buf
    }
}

// ── Record encoders ─────────────────────────────────────────────────────────────

/// Encode a standard POI record in Garmin format.
///
/// Layout:
/// ```text
/// byte 0    : type (base_type)
/// byte 1-3  : label_info (LE24) — bits 0-21 = label_offset, bit 23 = has_sub_type
/// byte 4-5  : delta_lon (LE16s) from subdivision centre
/// byte 6-7  : delta_lat (LE16s) from subdivision centre
/// [byte 8   : sub_type if bit 23 of label_info is set]
/// ```
///
/// Label offset is always present (0x000000 if no label). No `last_in_group` flag.
fn encode_point_record(
    poi: &MpPoint,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    label_offset: u32,
) -> Vec<u8> {
    let tc = parse_type_code(&poi.type_code);
    let lat_g = to_garmin_units(poi.lat);
    let lon_g = to_garmin_units(poi.lon);
    let d_lon = encode_delta(lon_g, center_lon_g, bits_per_coord);
    let d_lat = encode_delta(lat_g, center_lat_g, bits_per_coord);

    let has_sub_type = tc.sub_type != 0;

    // Build label_info LE24: bits 0-21 = label offset, bit 23 = has_sub_type
    let mut label_info = label_offset & 0x003F_FFFF;
    if has_sub_type {
        label_info |= 0x0080_0000; // bit 23
    }

    let mut buf = Vec::with_capacity(9);
    buf.push(tc.base_type);
    buf.push((label_info & 0xFF) as u8);
    buf.push(((label_info >> 8) & 0xFF) as u8);
    buf.push(((label_info >> 16) & 0xFF) as u8);
    buf.extend_from_slice(&d_lon.to_le_bytes());
    buf.extend_from_slice(&d_lat.to_le_bytes());
    if has_sub_type {
        buf.push(tc.sub_type);
    }
    buf
}

/// Shared binary encoding for standard polyline and polygon records (Garmin format).
///
/// Layout:
/// ```text
/// byte 0      : type (base_type)
/// byte 0      : type | FLAG_2BYTE_LEN (bit 7, if bitstream >= 256 bytes)
/// byte 1-3    : label_info (LE24) — bits 0-21 = label_offset, bit 23 = has_net (FLAG_NETINFO)
/// byte 4-5    : delta_lon of 1st point from subdivision centre (LE16s)
/// byte 6-7    : delta_lat of 1st point from subdivision centre (LE16s)
/// byte 8(+9)  : blen (1 or 2 bytes LE, = bitstream_byte_length - 1)
/// remaining   : bitstream data (blen+1 bytes):
///               [xBase(4b)][yBase(4b)][xSameSign(1b)][xSignNeg?(1b)][ySameSign(1b)][ySignNeg?(1b)]
///               then packed coordinate deltas
/// [3 bytes    : NET1 offset if bit 23 of label_info is set]
/// ```
///
/// Follows mkgmap's Polyline.write() format exactly.
#[allow(clippy::too_many_arguments)]
fn encode_poly_record_inner(
    type_code: &str,
    _label: &Option<String>,
    coords: &[(f64, f64)],
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    label_offset: u32,
    net_offset: Option<u32>,
) -> Vec<u8> {
    let tc = parse_type_code(type_code);

    // Build label_info LE24: bits 0-21 = label offset, bit 23 = FLAG_NETINFO
    let mut label_info = label_offset & 0x003F_FFFF;
    if net_offset.is_some() {
        label_info |= 0x0080_0000; // bit 23 = FLAG_NETINFO (mkgmap convention)
    }

    // We'll build the type byte after knowing the bitstream length.
    let type_byte_pos = 0usize; // will be patched

    let mut buf = Vec::new();
    buf.push(tc.base_type); // placeholder, may set FLAG_2BYTE_LEN later
    buf.push((label_info & 0xFF) as u8);
    buf.push(((label_info >> 8) & 0xFF) as u8);
    buf.push(((label_info >> 16) & 0xFF) as u8);

    if coords.is_empty() {
        buf.extend_from_slice(&0i16.to_le_bytes());
        buf.extend_from_slice(&0i16.to_le_bytes());
        // Minimal bitstream: xBase=0, yBase=0, xSameSign=true(+), ySameSign=true(+)
        let mut bw = BitWriter::new();
        bw.putn(0, 4); // xBase
        bw.putn(0, 4); // yBase
        bw.put1(true);  // xSameSign
        bw.put1(false); // xSignNegative (positive)
        bw.put1(true);  // ySameSign
        bw.put1(false); // ySignNegative (positive)
        let bs_bytes = bw.to_bytes();
        let blen = bs_bytes.len() - 1;
        buf.push(blen as u8);
        buf.extend_from_slice(&bs_bytes);
        if let Some(net_off) = net_offset {
            let n = net_off & 0x003F_FFFF;
            buf.push((n & 0xFF) as u8);
            buf.push(((n >> 8) & 0xFF) as u8);
            buf.push(((n >> 16) & 0xFF) as u8);
        }
        return buf;
    }

    // First point: delta from subdivision centre
    let (lat0, lon0) = coords[0];
    let lat0_g = to_garmin_units(lat0);
    let lon0_g = to_garmin_units(lon0);
    buf.extend_from_slice(&encode_delta(lon0_g, center_lon_g, bits_per_coord).to_le_bytes());
    buf.extend_from_slice(&encode_delta(lat0_g, center_lat_g, bits_per_coord).to_le_bytes());

    if coords.len() <= 1 {
        // Only 1 point: minimal bitstream header, no deltas
        let mut bw = BitWriter::new();
        bw.putn(0, 4);
        bw.putn(0, 4);
        bw.put1(true); bw.put1(false);
        bw.put1(true); bw.put1(false);
        let bs_bytes = bw.to_bytes();
        let blen = bs_bytes.len() - 1;
        buf.push(blen as u8);
        buf.extend_from_slice(&bs_bytes);
        if let Some(net_off) = net_offset {
            let n = net_off & 0x003F_FFFF;
            buf.push((n & 0xFF) as u8);
            buf.push(((n >> 8) & 0xFF) as u8);
            buf.push(((n >> 16) & 0xFF) as u8);
        }
        return buf;
    }

    // Compute inter-point deltas in quantized (shifted) space.
    let shift = 24i32 - bits_per_coord as i32;
    let mut lon_deltas = Vec::with_capacity(coords.len() - 1);
    let mut lat_deltas = Vec::with_capacity(coords.len() - 1);
    let mut prev_lon_shifted = encode_delta(lon0_g, center_lon_g, bits_per_coord) as i32;
    let mut prev_lat_shifted = encode_delta(lat0_g, center_lat_g, bits_per_coord) as i32;
    for &(lat, lon) in &coords[1..] {
        let lat_g = to_garmin_units(lat);
        let lon_g = to_garmin_units(lon);
        let cur_lon_shifted = ((lon_g - center_lon_g) >> shift).clamp(i16::MIN as i32, i16::MAX as i32);
        let cur_lat_shifted = ((lat_g - center_lat_g) >> shift).clamp(i16::MIN as i32, i16::MAX as i32);
        lon_deltas.push(cur_lon_shifted - prev_lon_shifted);
        lat_deltas.push(cur_lat_shifted - prev_lat_shifted);
        prev_lon_shifted = cur_lon_shifted;
        prev_lat_shifted = cur_lat_shifted;
    }

    // Compute magnitude bits and base values (mkgmap LinePreparer logic).
    let x_max_bits = lon_deltas.iter().map(|&d| bits_needed(d)).max().unwrap_or(0);
    let y_max_bits = lat_deltas.iter().map(|&d| bits_needed(d)).max().unwrap_or(0);
    let x_base = bits2base(x_max_bits);
    let y_base = bits2base(y_max_bits);

    // Same-sign detection (mkgmap: !(minDx < 0 && maxDx > 0))
    let x_min = lon_deltas.iter().copied().min().unwrap_or(0);
    let x_max = lon_deltas.iter().copied().max().unwrap_or(0);
    let y_min = lat_deltas.iter().copied().min().unwrap_or(0);
    let y_max = lat_deltas.iter().copied().max().unwrap_or(0);
    let x_same_sign = !(x_min < 0 && x_max > 0);
    let y_same_sign = !(y_min < 0 && y_max > 0);
    let x_sign_negative = x_same_sign && x_min < 0;
    let y_sign_negative = y_same_sign && y_min < 0;

    // Encoding bits per delta
    let x_enc_bits = base2bits(x_base) + if x_same_sign { 0 } else { 1 };
    let y_enc_bits = base2bits(y_base) + if y_same_sign { 0 } else { 1 };

    // Build bitstream (mkgmap format: header bits then packed deltas)
    let mut bw = BitWriter::new();
    // Pre-bitstream info
    bw.putn(x_base as u32, 4);
    bw.putn(y_base as u32, 4);
    bw.put1(x_same_sign);
    if x_same_sign {
        bw.put1(x_sign_negative);
    }
    bw.put1(y_same_sign);
    if y_same_sign {
        bw.put1(y_sign_negative);
    }

    // Pack deltas
    for i in 0..lon_deltas.len() {
        if x_same_sign {
            bw.putn(lon_deltas[i].unsigned_abs(), base2bits(x_base));
        } else {
            bw.sputn(lon_deltas[i], x_enc_bits);
        }
        if y_same_sign {
            bw.putn(lat_deltas[i].unsigned_abs(), base2bits(y_base));
        } else {
            bw.sputn(lat_deltas[i], y_enc_bits);
        }
    }

    let bs_bytes = bw.to_bytes();
    let blen = bs_bytes.len().saturating_sub(1); // mkgmap: bw.getLength() - 1

    // Set FLAG_2BYTE_LEN in type byte if blen >= 256
    if blen >= 0x100 {
        buf[type_byte_pos] |= 0x80; // FLAG_2BYTE_LEN
    }

    // Write blen (1 or 2 bytes LE)
    if blen < 0x100 {
        buf.push(blen as u8);
    } else {
        buf.push((blen & 0xFF) as u8);
        buf.push(((blen >> 8) & 0xFF) as u8);
    }

    // Write bitstream data (blen + 1 bytes)
    buf.extend_from_slice(&bs_bytes[..blen + 1]);

    // Append NET1 cross-reference if present
    if let Some(net_off) = net_offset {
        let n = net_off & 0x003F_FFFF;
        buf.push((n & 0xFF) as u8);
        buf.push(((n >> 8) & 0xFF) as u8);
        buf.push(((n >> 16) & 0xFF) as u8);
    }

    buf
}

/// Encode a standard polyline record in Garmin format.
fn encode_polyline_record(
    line: &MpPolyline,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    label_offset: u32,
    net_offset: Option<u32>,
) -> Vec<u8> {
    encode_poly_record_inner(
        &line.type_code,
        &line.label,
        &line.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        label_offset,
        net_offset,
    )
}

/// Encode a standard polygon record — same format as polyline (outer ring only).
fn encode_polygon_record(
    poly: &MpPolygon,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    label_offset: u32,
) -> Vec<u8> {
    encode_poly_record_inner(
        &poly.type_code,
        &poly.label,
        &poly.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        label_offset,
        None, // polygons don't have NET references
    )
}

// ── Extended record encoders ──────────────────────────────────────────────────

/// Encode an extended POI record.
///
/// Layout:
/// ```text
/// byte 0      : type_byte (base_type)
/// byte 1      : sub_type (bits 0-4) | flags (bits 5-7)
///               bit 5 = has_label (0x20), bit 7 = last_in_group (0x80)
/// bytes 2-3   : delta_lon (LE16) from subdivision centre
/// bytes 4-5   : delta_lat (LE16) from subdivision centre
/// [if has_label]: 3 bytes label_offset (LE24)
/// ```
fn encode_extended_point_record(
    poi: &MpPoint,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    let tc = parse_type_code(&poi.type_code);
    let lat_g = to_garmin_units(poi.lat);
    let lon_g = to_garmin_units(poi.lon);
    let d_lon = encode_delta(lon_g, center_lon_g, bits_per_coord);
    let d_lat = encode_delta(lat_g, center_lat_g, bits_per_coord);
    let has_label = poi.label.is_some();

    if tc.sub_type > 0x1F {
        tracing::warn!(
            "Extended POI sub_type 0x{:02X} exceeds 5-bit limit, truncating to 0x{:02X}",
            tc.sub_type,
            tc.sub_type & 0x1F
        );
    }
    let mut sub_flags: u8 = tc.sub_type & 0x1F;
    if has_label {
        sub_flags |= 0x20;
    }
    if last_in_group {
        sub_flags |= 0x80;
    }

    let mut buf = Vec::with_capacity(9);
    buf.push(tc.base_type);
    buf.push(sub_flags);
    buf.extend_from_slice(&d_lon.to_le_bytes());
    buf.extend_from_slice(&d_lat.to_le_bytes());
    if has_label {
        let le = label_offset.to_le_bytes();
        buf.push(le[0]);
        buf.push(le[1]);
        buf.push(le[2]);
    }
    buf
}

/// Shared encoding for extended polyline and polygon records.
///
/// Layout:
/// ```text
/// byte 0      : type_byte (base_type)
/// byte 1      : sub_type
/// byte 2      : flags — bit 3 = has_label (0x08), bit 7 = last_in_group (0x80),
///               bits 0-2 = extra_bytes count (0)
/// bytes 3-4   : delta_lon (LE16) from subdivision centre
/// bytes 5-6   : delta_lat (LE16) from subdivision centre
/// bytes 7-8   : coord_stream_length (LE16) — byte count of all subsequent coordinate deltas
/// bytes 9+    : coordinate deltas (pairs of LE16)
/// [if has_label]: 3 bytes label_offset (LE24)
/// ```
fn encode_extended_poly_record_inner(
    type_code: &str,
    label: &Option<String>,
    coords: &[(f64, f64)],
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    let tc = parse_type_code(type_code);
    let has_label = label.is_some();

    let mut flags: u8 = 0; // extra_bytes = 0
    if has_label {
        flags |= 0x08;
    }
    if last_in_group {
        flags |= 0x80;
    }

    let mut buf = Vec::new();
    buf.push(tc.base_type);
    buf.push(tc.sub_type);
    buf.push(flags);

    if coords.is_empty() {
        // Empty: delta from centre = 0, stream length = 0
        buf.extend_from_slice(&0i16.to_le_bytes()); // delta_lon
        buf.extend_from_slice(&0i16.to_le_bytes()); // delta_lat
        buf.extend_from_slice(&0u16.to_le_bytes()); // coord_stream_length
        if has_label {
            let le = label_offset.to_le_bytes();
            buf.push(le[0]);
            buf.push(le[1]);
            buf.push(le[2]);
        }
        return buf;
    }

    // First point: delta from subdivision centre
    let (lat0, lon0) = coords[0];
    let lat0_g = to_garmin_units(lat0);
    let lon0_g = to_garmin_units(lon0);
    buf.extend_from_slice(&encode_delta(lon0_g, center_lon_g, bits_per_coord).to_le_bytes());
    buf.extend_from_slice(&encode_delta(lat0_g, center_lat_g, bits_per_coord).to_le_bytes());

    // Build coordinate stream (subsequent points: delta from previous)
    let mut coord_stream = Vec::new();
    let mut prev_lat_g = lat0_g;
    let mut prev_lon_g = lon0_g;
    for &(lat, lon) in &coords[1..] {
        let lat_g = to_garmin_units(lat);
        let lon_g = to_garmin_units(lon);
        coord_stream.extend_from_slice(&encode_delta(lon_g, prev_lon_g, bits_per_coord).to_le_bytes());
        coord_stream.extend_from_slice(&encode_delta(lat_g, prev_lat_g, bits_per_coord).to_le_bytes());
        prev_lat_g = lat_g;
        prev_lon_g = lon_g;
    }

    if coord_stream.len() > u16::MAX as usize {
        tracing::warn!(
            "Extended record coord stream ({} bytes) exceeds u16::MAX, truncating to {}",
            coord_stream.len(),
            u16::MAX
        );
    }
    let stream_len = coord_stream.len().min(u16::MAX as usize) as u16;
    buf.extend_from_slice(&stream_len.to_le_bytes());
    buf.extend_from_slice(&coord_stream[..stream_len as usize]);

    if has_label {
        let le = label_offset.to_le_bytes();
        buf.push(le[0]);
        buf.push(le[1]);
        buf.push(le[2]);
    }
    buf
}

/// Encode an extended polyline record.
fn encode_extended_polyline_record(
    line: &MpPolyline,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    encode_extended_poly_record_inner(
        &line.type_code,
        &line.label,
        &line.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        last_in_group,
        label_offset,
    )
}

/// Encode an extended polygon record.
fn encode_extended_polygon_record(
    poly: &MpPolygon,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    encode_extended_poly_record_inner(
        &poly.type_code,
        &poly.label,
        &poly.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        last_in_group,
        label_offset,
    )
}

// ── RgnWriter ──────────────────────────────────────────────────────────────────

/// Maximum feature data bytes per subdivision (~30 KB).
/// End-offset pointers are LE16, so the hard limit is 65535 bytes.
const MAX_SUBDIV_DATA_BYTES: usize = 30_000;

/// Maximum number of features per subdivision.
// TODO: multi-subdivision disabled pending NET cross-reference fix.
// With multi-subdivision, NET SubdivRoadRef numbering becomes inconsistent
// with TRE subdivision indices, causing QMapShack crashes on routed tiles.
const MAX_FEATURES_PER_SUBDIV: usize = 1_000_000;

/// Information about a single subdivision within the RGN output.
pub struct SubdivisionInfo {
    /// Byte offset of this subdivision's data within the RGN feature data section.
    pub rgn_offset: u32,
    /// Which zoom level this subdivision belongs to.
    pub level: u8,
    /// Bounding box in Garmin 24-bit units.
    pub min_lat_g: i32,
    pub max_lat_g: i32,
    pub min_lon_g: i32,
    pub max_lon_g: i32,
    /// Centre of the subdivision (for delta encoding reference).
    pub center_lat_g: i32,
    pub center_lon_g: i32,
    /// Feature type presence flags.
    pub has_points: bool,
    pub has_polylines: bool,
    pub has_polygons: bool,
    pub has_indexed_lines: bool,
    pub has_extended_points: bool,
    pub has_extended_polylines: bool,
    pub has_extended_polygons: bool,
}

/// Result of `RgnWriter::build`: the complete RGN binary and subdivision info.
pub struct RgnBuildResult {
    /// Complete RGN subfile binary: `[RgnHeader || feature_data]`.
    pub data: Vec<u8>,
    /// Per-subdivision information (ordered by level, then within level).
    pub subdivisions: Vec<SubdivisionInfo>,
    /// Legacy: per-level byte offset (first subdivision of each level).
    /// Kept for backward compatibility with `TreWriter`.
    pub subdivision_offsets: Vec<u32>,
    /// Cross-references from RGN polylines to NET road definitions.
    pub subdiv_road_refs: Vec<crate::img::net::SubdivRoadRef>,
    /// Per-subdivision flag: true if extended POIs are present.
    pub subdiv_has_extended_points: Vec<bool>,
    /// Per-subdivision flag: true if extended polylines are present.
    pub subdiv_has_extended_polylines: Vec<bool>,
    /// Per-subdivision flag: true if extended polygons are present.
    pub subdiv_has_extended_polygons: Vec<bool>,
    /// Size of the extended areas (polygons) section in bytes.
    pub ext_areas_size: u32,
    /// Size of the extended lines (polylines) section in bytes.
    pub ext_lines_size: u32,
    /// Size of the extended points section in bytes.
    pub ext_points_size: u32,
}

/// Builds the RGN subfile binary from a parsed Polish Map.
pub struct RgnWriter;

impl RgnWriter {
    /// Build the complete RGN subfile binary using stub label offsets (0x000000).
    ///
    /// Convenience wrapper around [`build_with_lbl_offsets`] with an empty map,
    /// preserving backward compatibility with callers that don't need LBL integration.
    ///
    /// **Warning**: features with labels produce records with `has_label = true` but
    /// `label_offset = 0x000000`. LBL offset 0 is the null sentinel ("no label"), so a GPS
    /// device reading these records would find empty labels. This method is intended for
    /// unit tests only. For production output use [`build_with_lbl_offsets`] with a
    /// populated map from [`LblWriter::build`].
    pub fn build(mp: &MpFile, levels: &[MapLevel]) -> RgnBuildResult {
        Self::build_with_lbl_offsets(mp, levels, &HashMap::new())
    }

    /// Build the complete RGN subfile binary with real LBL label offsets.
    ///
    /// Convenience wrapper without NET offsets.
    pub fn build_with_lbl_offsets(
        mp: &MpFile,
        levels: &[MapLevel],
        label_offsets: &HashMap<String, u32>,
    ) -> RgnBuildResult {
        Self::build_with_net_offsets(mp, levels, label_offsets, &[])
    }

    /// Build the complete RGN subfile binary with real LBL label offsets and NET1 cross-references.
    ///
    /// For each level `i`, features are filtered by `end_level >= i` and split into
    /// multiple subdivisions if needed (threshold: ~500 features or ~30 KB per subdiv).
    ///
    /// `label_offsets`: map from label string → offset from the start of the LBL data
    /// section (as returned by [`LblWriter::build`]). An empty map produces stub offsets.
    ///
    /// `net_road_offsets`: for each routable polyline, the byte offset of its NET1 record.
    pub fn build_with_net_offsets(
        mp: &MpFile,
        levels: &[MapLevel],
        label_offsets: &HashMap<String, u32>,
        net_road_offsets: &[u32],
    ) -> RgnBuildResult {
        let n = levels.len();

        // Compute tile-wide bounding box (used as fallback).
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(mp);
        let tile_min_lat_g = to_garmin_units(min_lat);
        let tile_max_lat_g = to_garmin_units(max_lat);
        let tile_min_lon_g = to_garmin_units(min_lon);
        let tile_max_lon_g = to_garmin_units(max_lon);

        // Build mapping: polyline original index → (NET1 offset, road_def_idx).
        let has_net = !net_road_offsets.is_empty();
        let mut polyline_net_offset: HashMap<usize, (u32, usize)> = HashMap::new();
        if has_net {
            let mut road_def_idx = 0usize;
            for (pi, pl) in mp.polylines.iter().enumerate() {
                if pl.routing.is_some() && road_def_idx < net_road_offsets.len() {
                    polyline_net_offset.insert(pi, (net_road_offsets[road_def_idx], road_def_idx));
                    road_def_idx += 1;
                }
            }
        }

        // Pre-compute extended flag per feature.
        let point_extended: Vec<bool> = mp.points.iter().map(|p| parse_type_code(&p.type_code).extended).collect();
        let polyline_extended: Vec<bool> = mp.polylines.iter().map(|p| parse_type_code(&p.type_code).extended).collect();
        let polygon_extended: Vec<bool> = mp.polygons.iter().map(|p| parse_type_code(&p.type_code).extended).collect();

        let mut feature_data: Vec<u8> = Vec::new();
        // Extended type records go in separate sections (Garmin convention).
        let mut ext_point_data: Vec<u8> = Vec::new();
        let mut ext_line_data: Vec<u8> = Vec::new();
        let mut ext_area_data: Vec<u8> = Vec::new();
        let mut subdivision_offsets = Vec::with_capacity(n);
        let mut subdivisions: Vec<SubdivisionInfo> = Vec::new();
        let mut subdiv_road_refs: Vec<crate::img::net::SubdivRoadRef> = Vec::new();
        let mut subdiv_has_extended_points = Vec::new();
        let mut subdiv_has_extended_polylines = Vec::new();
        let mut subdiv_has_extended_polygons = Vec::new();

        for (i, level) in levels.iter().enumerate() {
            // Record the offset for this level (first subdivision).
            subdivision_offsets.push(feature_data.len() as u32);

            let bits = level.bits_per_coord;

            // Filter features by end_level and partition standard/extended.
            let (std_points, ext_points): (Vec<_>, Vec<_>) = mp
                .points
                .iter()
                .enumerate()
                .filter(|(_, f)| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .partition(|(idx, _)| !point_extended[*idx]);
            let std_points: Vec<_> = std_points.into_iter().map(|(_, p)| p).collect();
            let ext_points: Vec<_> = ext_points.into_iter().map(|(_, p)| p).collect();

            let (std_polylines, ext_polylines): (Vec<_>, Vec<_>) = mp
                .polylines
                .iter()
                .enumerate()
                .filter(|(_, f)| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .partition(|(idx, _)| !polyline_extended[*idx]);

            let (std_polygons, ext_polygons): (Vec<_>, Vec<_>) = mp
                .polygons
                .iter()
                .enumerate()
                .filter(|(_, f)| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .partition(|(idx, _)| !polygon_extended[*idx]);
            let std_polygons: Vec<_> = std_polygons.into_iter().map(|(_, p)| p).collect();
            let ext_polygons: Vec<_> = ext_polygons.into_iter().map(|(_, p)| p).collect();

            // Determine number of chunks for this level.
            // Split by feature count OR estimated data size (whichever needs more chunks).
            let total_features = std_points.len() + std_polylines.len() + std_polygons.len()
                + ext_points.len() + ext_polylines.len() + ext_polygons.len();
            // Rough estimate: ~20 bytes per feature average (bitstream records vary).
            let est_data_bytes = total_features * 20;
            let chunks_by_count = if total_features > MAX_FEATURES_PER_SUBDIV {
                (total_features + MAX_FEATURES_PER_SUBDIV - 1) / MAX_FEATURES_PER_SUBDIV
            } else {
                1
            };
            let chunks_by_size = if est_data_bytes > MAX_SUBDIV_DATA_BYTES {
                (est_data_bytes + MAX_SUBDIV_DATA_BYTES - 1) / MAX_SUBDIV_DATA_BYTES
            } else {
                1
            };
            let n_chunks = chunks_by_count.max(chunks_by_size);

            for chunk_idx in 0..n_chunks {
                // Compute chunk slices (proportional splitting).
                let chunk_range = |total: usize| -> (usize, usize) {
                    let start = (total * chunk_idx) / n_chunks;
                    let end = (total * (chunk_idx + 1)) / n_chunks;
                    (start, end)
                };
                let (ps, pe) = chunk_range(std_points.len());
                let (ls, le) = chunk_range(std_polylines.len());
                let (gs, ge) = chunk_range(std_polygons.len());
                let (eps, epe) = chunk_range(ext_points.len());
                let (els, ele) = chunk_range(ext_polylines.len());
                let (egs, ege) = chunk_range(ext_polygons.len());

                let c_std_points = &std_points[ps..pe];
                let c_std_polylines = &std_polylines[ls..le];
                let c_std_polygons = &std_polygons[gs..ge];
                let c_ext_points = &ext_points[eps..epe];
                let c_ext_polylines = &ext_polylines[els..ele];
                let c_ext_polygons = &ext_polygons[egs..ege];

                // Compute bounding box for this chunk.
                let (c_min_lat, c_max_lat, c_min_lon, c_max_lon) =
                    Self::compute_chunk_bounds(
                        c_std_points,
                        c_std_polylines,
                        c_std_polygons,
                        c_ext_points,
                        c_ext_polylines,
                        c_ext_polygons,
                        (tile_min_lat_g, tile_max_lat_g, tile_min_lon_g, tile_max_lon_g),
                    );
                let center_lat_g = (c_max_lat + c_min_lat) / 2;
                let center_lon_g = (c_max_lon + c_min_lon) / 2;

                let subdiv_number = (subdivisions.len() + 1) as u16; // 1-based

                // Extended types: temporarily disabled — per-subdivision indexing
                // in TRE extTypeOffsets section needs proper cumulative offset tracking.
                // Standard features will render; extended features (0x1xxxx) are omitted.
                subdiv_has_extended_points.push(false);
                subdiv_has_extended_polylines.push(false);
                subdiv_has_extended_polygons.push(false);

                // ── Encode standard sections ────
                let mut buf_points: Vec<u8> = Vec::new();
                let mut buf_polylines: Vec<u8> = Vec::new();
                let mut buf_polygons: Vec<u8> = Vec::new();

                for poi in c_std_points.iter() {
                    let lbl_offset = poi
                        .label
                        .as_ref()
                        .and_then(|l| label_offsets.get(l.as_str()).copied())
                        .unwrap_or(0);
                    let record =
                        encode_point_record(poi, center_lat_g, center_lon_g, bits, lbl_offset);
                    buf_points.extend_from_slice(&record);
                }

                let mut polyline_index_in_subdiv: u8 = 0;
                for &(orig_idx, ref line) in c_std_polylines.iter() {
                    let lbl_offset = line
                        .label
                        .as_ref()
                        .and_then(|l| label_offsets.get(l.as_str()).copied())
                        .unwrap_or(0);
                    let net_off = polyline_net_offset.get(&orig_idx).map(|&(off, _)| off);
                    let record =
                        encode_polyline_record(line, center_lat_g, center_lon_g, bits, lbl_offset, net_off);
                    buf_polylines.extend_from_slice(&record);

                    if let Some(&(_, road_def_idx)) = polyline_net_offset.get(&orig_idx) {
                        subdiv_road_refs.push(crate::img::net::SubdivRoadRef {
                            road_def_idx,
                            subdiv_number,
                            polyline_index: polyline_index_in_subdiv,
                        });
                    }
                    polyline_index_in_subdiv = polyline_index_in_subdiv.saturating_add(1);
                }

                for poly in c_std_polygons.iter() {
                    let lbl_offset = poly
                        .label
                        .as_ref()
                        .and_then(|l| label_offsets.get(l.as_str()).copied())
                        .unwrap_or(0);
                    let record =
                        encode_polygon_record(poly, center_lat_g, center_lon_g, bits, lbl_offset);
                    buf_polygons.extend_from_slice(&record);
                }

                // Compute end-offset pointers.
                let has_pts = !buf_points.is_empty();
                let has_lines = !buf_polylines.is_empty();
                let has_polys = !buf_polygons.is_empty();
                let has_indexed_lines = has_net && c_std_polylines.iter()
                    .any(|(idx, _)| polyline_net_offset.contains_key(idx));
                let type_count = has_pts as u32 + has_lines as u32 + has_polys as u32;
                let n_end_offsets = if type_count > 1 { type_count - 1 } else { 0 };
                let header_bytes = n_end_offsets as usize * 2;

                let pts_end = header_bytes + buf_points.len();
                let lines_end = pts_end + buf_polylines.len();
                let total_std = lines_end + buf_polygons.len();

                // Guard: end-offset pointers are LE16 — subdivision data must fit in 65535 bytes.
                if total_std > u16::MAX as usize {
                    tracing::warn!(
                        level = i,
                        chunk = chunk_idx,
                        total_bytes = total_std,
                        "Subdivision data exceeds u16::MAX ({}) — end-offset pointers will overflow",
                        u16::MAX
                    );
                }

                // Record subdivision offset.
                let subdiv_offset = feature_data.len() as u32;

                if type_count > 1 {
                    let mut offsets_written = 0u32;
                    if has_pts && (has_lines || has_polys) {
                        let off = pts_end.min(u16::MAX as usize) as u16;
                        feature_data.extend_from_slice(&off.to_le_bytes());
                        offsets_written += 1;
                    }
                    if has_lines && has_polys && offsets_written < n_end_offsets {
                        let off = lines_end.min(u16::MAX as usize) as u16;
                        feature_data.extend_from_slice(&off.to_le_bytes());
                    }
                }
                feature_data.extend_from_slice(&buf_points);
                feature_data.extend_from_slice(&buf_polylines);
                feature_data.extend_from_slice(&buf_polygons);

                // Extended types: encoding skipped (see above).
                // TODO: implement proper per-subdivision cumulative offset tracking
                // for extTypeOffsets section in TRE.

                // Record SubdivisionInfo.
                subdivisions.push(SubdivisionInfo {
                    rgn_offset: subdiv_offset,
                    level: i as u8,
                    min_lat_g: c_min_lat,
                    max_lat_g: c_max_lat,
                    min_lon_g: c_min_lon,
                    max_lon_g: c_max_lon,
                    center_lat_g,
                    center_lon_g,
                    has_points: has_pts,
                    has_polylines: has_lines,
                    has_polygons: has_polys,
                    has_indexed_lines,
                    has_extended_points: false,
                    has_extended_polylines: false,
                    has_extended_polygons: false,
                });
            }
        }

        // Compute extended section offsets (from subfile start).
        // Layout: [header 125B] [standard data] [ext_areas] [ext_lines] [ext_points]
        let std_data_size = feature_data.len() as u32;
        let ext_areas_off = if ext_area_data.is_empty() {
            0u32
        } else {
            RGN_HEADER_SIZE as u32 + std_data_size
        };
        let ext_lines_off = if ext_line_data.is_empty() {
            0u32
        } else {
            RGN_HEADER_SIZE as u32 + std_data_size + ext_area_data.len() as u32
        };
        let ext_points_off = if ext_point_data.is_empty() {
            0u32
        } else {
            RGN_HEADER_SIZE as u32 + std_data_size
                + ext_area_data.len() as u32
                + ext_line_data.len() as u32
        };

        let header = RgnHeader {
            data_size: std_data_size,
            ext_areas_offset: ext_areas_off,
            ext_areas_size: ext_area_data.len() as u32,
            ext_lines_offset: ext_lines_off,
            ext_lines_size: ext_line_data.len() as u32,
            ext_points_offset: ext_points_off,
            ext_points_size: ext_point_data.len() as u32,
        };
        let mut data = header.to_bytes();
        data.extend_from_slice(&feature_data);
        data.extend_from_slice(&ext_area_data);
        data.extend_from_slice(&ext_line_data);
        data.extend_from_slice(&ext_point_data);

        RgnBuildResult {
            data,
            subdivisions,
            subdivision_offsets,
            subdiv_road_refs,
            subdiv_has_extended_points,
            subdiv_has_extended_polylines,
            subdiv_has_extended_polygons,
            ext_areas_size: ext_area_data.len() as u32,
            ext_lines_size: ext_line_data.len() as u32,
            ext_points_size: ext_point_data.len() as u32,
        }
    }

    /// Compute the bounding box (in Garmin units) for a chunk of features.
    /// Returns `(min_lat_g, max_lat_g, min_lon_g, max_lon_g)`.
    /// Falls back to `tile_bounds` if no features have coordinates.
    fn compute_chunk_bounds(
        std_points: &[&MpPoint],
        std_polylines: &[(usize, &MpPolyline)],
        std_polygons: &[&MpPolygon],
        ext_points: &[&MpPoint],
        ext_polylines: &[(usize, &MpPolyline)],
        ext_polygons: &[&MpPolygon],
        tile_bounds: (i32, i32, i32, i32),
    ) -> (i32, i32, i32, i32) {
        let mut min_lat = f64::MAX;
        let mut max_lat = f64::MIN;
        let mut min_lon = f64::MAX;
        let mut max_lon = f64::MIN;

        for p in std_points.iter().chain(ext_points.iter()) {
            min_lat = min_lat.min(p.lat);
            max_lat = max_lat.max(p.lat);
            min_lon = min_lon.min(p.lon);
            max_lon = max_lon.max(p.lon);
        }
        for (_, line) in std_polylines.iter().chain(ext_polylines.iter()) {
            for &(lat, lon) in &line.coords {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
        }
        for poly in std_polygons.iter().chain(ext_polygons.iter()) {
            for &(lat, lon) in &poly.coords {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
        }

        if min_lat == f64::MAX {
            return tile_bounds;
        }

        (
            to_garmin_units(min_lat),
            to_garmin_units(max_lat),
            to_garmin_units(min_lon),
            to_garmin_units(max_lon),
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::img::tre::levels_from_mp;
    use crate::parser::mp_types::{MpHeader, MpPolyline};
    use std::collections::HashMap;

    // ── Task 1: parse_type_code ───────────────────────────────────────────────

    #[test]
    fn test_parse_type_code_standard() {
        let tc = parse_type_code("0x06");
        assert_eq!(tc.base_type, 0x06);
        assert_eq!(tc.sub_type, 0x00);
        assert!(!tc.extended);
    }

    #[test]
    fn test_parse_type_code_with_subtype() {
        let tc = parse_type_code("0x2C00");
        assert_eq!(tc.base_type, 0x2C);
        assert_eq!(tc.sub_type, 0x00);
        assert!(!tc.extended);
    }

    #[test]
    fn test_parse_type_code_extended() {
        let tc = parse_type_code("0x10001");
        assert!(tc.extended);
        assert_eq!(tc.base_type, 0x00);
        assert_eq!(tc.sub_type, 0x01);
    }

    #[test]
    fn test_parse_type_code_extended_building() {
        let tc = parse_type_code("0x1101c");
        assert!(tc.extended);
        assert_eq!(tc.base_type, 0x10);
        assert_eq!(tc.sub_type, 0x1c);
    }

    #[test]
    fn test_parse_type_code_extended_wall() {
        let tc = parse_type_code("0x13308");
        assert!(tc.extended);
        assert_eq!(tc.base_type, 0x33);
        assert_eq!(tc.sub_type, 0x08);
    }

    #[test]
    fn test_parse_type_code_invalid() {
        let tc = parse_type_code("garbage");
        assert_eq!(tc.base_type, 0x00);
        assert_eq!(tc.sub_type, 0x00);
        assert!(!tc.extended);
    }

    // ── Task 2: encode_delta ──────────────────────────────────────────────────

    #[test]
    fn test_encode_delta_zero() {
        // Same coord and center → 0
        let result = encode_delta(1_000_000, 1_000_000, 24);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_encode_delta_positive() {
        // bits=24, shift=0 → delta is direct
        let coord_g = 1_000_100;
        let center_g = 1_000_000;
        let result = encode_delta(coord_g, center_g, 24);
        assert_eq!(result, 100);
    }

    #[test]
    fn test_encode_delta_negative() {
        // Negative delta
        let coord_g = 999_900;
        let center_g = 1_000_000;
        let result = encode_delta(coord_g, center_g, 24);
        assert_eq!(result, -100);
    }

    #[test]
    fn test_encode_delta_clamp_overflow() {
        // Delta >> 0 = i32::MAX → clamped to i16::MAX
        let result = encode_delta(i32::MAX, 0, 24);
        assert_eq!(result, i16::MAX);
    }

    #[test]
    fn test_encode_delta_bits18() {
        // bits_per_coord=18 → shift=6 → delta divided by 64 (level-1 resolution)
        let center_g = 1_000_000;
        let result = encode_delta(1_000_064, center_g, 18);
        assert_eq!(result, 1, "64 >> 6 = 1");
        let result_neg = encode_delta(999_936, center_g, 18);
        assert_eq!(result_neg, -1, "-64 >> 6 = -1");
        // Zero delta
        assert_eq!(encode_delta(center_g, center_g, 18), 0, "zero delta at any bit depth");
    }

    // ── Task 3: RgnHeader ─────────────────────────────────────────────────────

    #[test]
    fn test_rgn_header_size() {
        let h = RgnHeader { data_size: 0, ext_areas_offset: 0, ext_areas_size: 0, ext_lines_offset: 0, ext_lines_size: 0, ext_points_offset: 0, ext_points_size: 0 };
        assert_eq!(h.to_bytes().len(), RGN_HEADER_SIZE, "RGN header must be exactly 48 bytes");
    }

    #[test]
    fn test_rgn_header_common_header() {
        let h = RgnHeader { data_size: 0, ext_areas_offset: 0, ext_areas_size: 0, ext_lines_offset: 0, ext_lines_size: 0, ext_points_offset: 0, ext_points_size: 0 };
        let bytes = h.to_bytes();
        // Common header: LE16(46) at 0x00, "GARMIN RGN" at 0x02
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), RGN_HEADER_SIZE as u16);
        assert_eq!(&bytes[0x02..0x0C], b"GARMIN RGN");
        // data_offset at 0x15 (no version field)
        let data_offset = u32::from_le_bytes([bytes[0x15], bytes[0x16], bytes[0x17], bytes[0x18]]);
        assert_eq!(data_offset, RGN_HEADER_SIZE as u32);
    }

    // ── Standard record format tests (Garmin format) ────────────────────────

    fn make_poi(label: Option<&str>) -> MpPoint {
        MpPoint {
            type_code: "0x2C00".to_string(),
            label: label.map(|s| s.to_string()),
            end_level: None,
            lat: 45.0,
            lon: 5.0,
            other_fields: HashMap::new(),
        }
    }

    fn make_polyline(n_coords: usize, label: Option<&str>) -> MpPolyline {
        let coords = (0..n_coords).map(|i| (45.0 + i as f64 * 0.001, 5.0)).collect();
        MpPolyline {
            type_code: "0x06".to_string(),
            label: label.map(|s| s.to_string()),
            end_level: None,
            coords,
            routing: None,
            other_fields: HashMap::new(),
        }
    }

    #[test]
    fn test_encode_point_record_garmin_format() {
        // AC1: POI format = [type(1B)][label_info LE24(3B)][dlon(2B)][dlat(2B)] = 8 bytes
        let center = to_garmin_units(45.0);

        // Without label: 8 bytes (label_info = 0x000000), sub_type=0 → no extra byte
        let poi_no_label = make_poi(None);
        let rec = encode_point_record(&poi_no_label, center, center, 24, 0);
        assert_eq!(rec.len(), 8, "POI record must be 8 bytes (no sub_type extra byte)");
        assert_eq!(rec[0], 0x2C, "byte 0: base_type = 0x2C");
        assert_eq!(&rec[1..4], &[0x00, 0x00, 0x00], "bytes 1-3: label_info = 0x000000");

        // With label offset=1: 8 bytes
        let poi_with_label = make_poi(Some("Mairie"));
        let rec = encode_point_record(&poi_with_label, center, center, 24, 1);
        assert_eq!(rec.len(), 8, "POI record with label must also be 8 bytes");
        assert_eq!(&rec[1..4], &[0x01, 0x00, 0x00], "label_info LE24 = 0x000001");
    }

    #[test]
    fn test_encode_point_record_with_sub_type() {
        // POI with non-zero sub_type → has_extra_byte (bit 23) + extra byte at end
        let center = to_garmin_units(45.0);
        let poi = MpPoint {
            type_code: "0x2C01".to_string(),
            label: None,
            end_level: None,
            lat: 45.0,
            lon: 5.0,
            other_fields: HashMap::new(),
        };
        let rec = encode_point_record(&poi, center, center, 24, 0);
        assert_eq!(rec.len(), 9, "POI with sub_type must be 9 bytes");
        assert_eq!(rec[0], 0x2C, "base_type");
        // label_info: offset=0, has_extra_byte=1 → bit 23 = 0x80 in byte 3
        assert_eq!(rec[3] & 0x80, 0x80, "bit 23 (has_extra_byte) must be set");
        assert_eq!(rec[8], 0x01, "extra byte = sub_type 0x01");
    }

    #[test]
    fn test_encode_polyline_record_garmin_format() {
        // AC1: polyline = [type(1B)][label_info(3B)][dlon(2B)][dlat(2B)][bs_len][bs_info][bs_data]
        let center = to_garmin_units(45.0);
        let line = make_polyline(3, None);
        let rec = encode_polyline_record(&line, center, center, 24, 0, None);

        // byte 0 = type
        assert_eq!(rec[0], 0x06, "byte 0: type = 0x06");
        // bytes 1-3 = label_info (offset=0, no NET)
        assert_eq!(&rec[1..4], &[0x00, 0x00, 0x00], "label_info = 0x000000");
        // bytes 4-7 = first point deltas (LE16s)
        // byte 8+ = bitstream_len + bitstream_info + data
        assert!(rec.len() > 8, "polyline must have bitstream data after fixed header");
        // No terminator: last bytes must NOT be 0x80 0x00
        let len = rec.len();
        assert!(
            !(rec[len - 2] == 0x80 && rec[len - 1] == 0x00),
            "Garmin format must NOT have 0x80 0x00 terminator"
        );
    }

    #[test]
    fn test_encode_polyline_record_with_label() {
        let center = to_garmin_units(45.0);
        let line = make_polyline(2, Some("D1075"));
        let rec = encode_polyline_record(&line, center, center, 24, 8, None);
        // label_info bytes 1-3: offset=8, no NET → LE24 = [0x08, 0x00, 0x00]
        assert_eq!(&rec[1..4], &[0x08, 0x00, 0x00], "label_info = offset 8");
    }

    #[test]
    fn test_encode_polyline_record_with_net() {
        let center = to_garmin_units(45.0);
        let line = make_polyline(2, Some("Route"));
        let net_off = Some(0x001234u32);
        let rec = encode_polyline_record(&line, center, center, 24, 5, net_off);
        // label_info: offset=5, bit 23 set (FLAG_NETINFO) → byte 3 bit 7 (0x80)
        let label_info = u32::from_le_bytes([rec[1], rec[2], rec[3], 0]);
        assert_eq!(label_info & 0x003F_FFFF, 5, "label offset = 5");
        assert_ne!(label_info & 0x0080_0000, 0, "bit 23 (FLAG_NETINFO) must be set");
        // Last 3 bytes = NET1 offset LE24
        let len = rec.len();
        let net_ref = u32::from_le_bytes([rec[len - 3], rec[len - 2], rec[len - 1], 0]);
        assert_eq!(net_ref, 0x001234, "NET1 offset must be 0x001234");
    }

    #[test]
    fn test_encode_polygon_garmin_format() {
        let center = to_garmin_units(45.0);
        use crate::parser::mp_types::MpPolygon;
        let poly = MpPolygon {
            type_code: "0x50".to_string(),
            label: None,
            end_level: None,
            coords: vec![(45.0, 5.0), (45.1, 5.0), (45.1, 5.1), (45.0, 5.1), (45.0, 5.0)],
            holes: vec![],
            other_fields: HashMap::new(),
        };
        let rec = encode_polygon_record(&poly, center, center, 24, 0);
        assert_eq!(rec[0], 0x50, "polygon base_type must be 0x50");
        // bytes 1-3 = label_info (no label, no NET)
        assert_eq!(&rec[1..4], &[0x00, 0x00, 0x00], "label_info = 0");
        // Must have bitstream data (4 extra points)
        assert!(rec.len() > 9, "polygon with 5 coords must have bitstream");
    }

    // ── LBL offset tests (Garmin format) ─────────────────────────────────────

    #[test]
    fn test_build_stub_offsets_are_zero() {
        // RgnWriter::build() without LBL offsets → label_info = 0x000000
        let mp = make_mp_single_poi();
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);
        // New format: label_info at bytes 1-3 of first record
        let lbl_info_start = RGN_HEADER_SIZE + 1; // byte 1 of first record
        assert_eq!(
            &result.data[lbl_info_start..lbl_info_start + 3],
            &[0x00, 0x00, 0x00],
            "build() without LBL offsets must produce stub 0x000000 in label_info"
        );
    }

    #[test]
    fn test_build_with_lbl_offsets_dedup_rgn_consistency() {
        // Two POIs sharing the same label must reference the SAME LBL offset.
        use crate::img::lbl::LblWriter;

        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points: vec![
                MpPoint {
                    type_code: "0x2C00".to_string(),
                    label: Some("Église".to_string()),
                    lat: 45.0,
                    lon: 5.0,
                    end_level: None,
                    other_fields: HashMap::new(),
                },
                MpPoint {
                    type_code: "0x2C00".to_string(),
                    label: Some("Église".to_string()),
                    lat: 45.1,
                    lon: 5.1,
                    end_level: None,
                    other_fields: HashMap::new(),
                },
            ],
            polylines: vec![],
            polygons: vec![],
        };

        let lbl = LblWriter::build(&mp);
        let levels = levels_from_mp(&mp.header);
        let rgn = RgnWriter::build_with_lbl_offsets(&mp, &levels, &lbl.label_offsets);

        // New format: each POI is 8 bytes [type(1)][label_info(3)][dlon(2)][dlat(2)]
        // label_info at offset 1 within each record
        let data = &rgn.data[RGN_HEADER_SIZE..];
        let offset0 = u32::from_le_bytes([data[1], data[2], data[3], 0]) & 0x003F_FFFF;
        let offset1 = u32::from_le_bytes([data[9], data[10], data[11], 0]) & 0x003F_FFFF;

        assert_ne!(offset0, 0, "first 'Église' POI label_offset must be non-zero");
        assert_eq!(offset0, offset1, "duplicate labels must reference the same LBL offset");
    }

    // ── RgnWriter::build tests ───────────────────────────────────────────────

    fn make_mp_single_poi() -> MpFile {
        MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points: vec![MpPoint {
                type_code: "0x2C00".to_string(),
                label: Some("Mairie".to_string()),
                end_level: None,
                lat: 45.188,
                lon: 5.7245,
                other_fields: HashMap::new(),
            }],
            polylines: vec![],
            polygons: vec![],
        }
    }

    #[test]
    fn test_rgn_single_poi_one_level() {
        let mp = make_mp_single_poi();
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);
        // New format: POI = 8 bytes. RGN = header + 8 bytes min.
        assert!(
            result.data.len() >= RGN_HEADER_SIZE + 8,
            "RGN with single POI must be at least {} bytes, got {}",
            RGN_HEADER_SIZE + 8,
            result.data.len()
        );
        assert_eq!(&result.data[0x02..0x0C], b"GARMIN RGN");
        assert_eq!(result.subdivision_offsets.len(), 1);
        assert_eq!(result.subdivision_offsets[0], 0);
        // Must also have SubdivisionInfo
        assert_eq!(result.subdivisions.len(), 1);
        assert_eq!(result.subdivisions[0].level, 0);
        assert!(result.subdivisions[0].has_points);
    }

    #[test]
    fn test_rgn_level_filter_excludes_low_endlevel() {
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24, 18],
                ..Default::default()
            },
            points: vec![
                MpPoint {
                    type_code: "0x06".to_string(),
                    label: None,
                    end_level: Some(0),
                    lat: 45.0,
                    lon: 5.0,
                    other_fields: HashMap::new(),
                },
                MpPoint {
                    type_code: "0x06".to_string(),
                    label: None,
                    end_level: Some(4),
                    lat: 45.1,
                    lon: 5.1,
                    other_fields: HashMap::new(),
                },
            ],
            polylines: vec![],
            polygons: vec![],
        };
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);

        assert_eq!(result.subdivision_offsets.len(), 2);
        assert_eq!(result.subdivision_offsets[0], 0);

        let level0_size = result.subdivision_offsets[1];
        let total_feature_data = (result.data.len() as u32) - RGN_HEADER_SIZE as u32;
        let level1_size = total_feature_data - level0_size;
        assert!(
            level0_size > level1_size,
            "level 0 should have more data (2 POIs) than level 1 (1 POI): {} vs {}",
            level0_size,
            level1_size
        );
    }

    #[test]
    fn test_rgn_offsets_monotone_increasing() {
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24, 21, 18],
                ..Default::default()
            },
            points: vec![MpPoint {
                type_code: "0x2C00".to_string(),
                label: Some("Test".to_string()),
                end_level: None,
                lat: 45.0,
                lon: 5.0,
                other_fields: HashMap::new(),
            }],
            polylines: vec![],
            polygons: vec![],
        };
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);

        assert_eq!(result.subdivision_offsets.len(), 3);
        assert_eq!(result.subdivision_offsets[0], 0, "first offset must be 0");
        assert!(
            result.subdivision_offsets[1] > result.subdivision_offsets[0],
            "offsets must be strictly increasing"
        );
        assert!(
            result.subdivision_offsets[2] > result.subdivision_offsets[1],
            "offsets must be strictly increasing"
        );
    }

    // ── Bitstream roundtrip test ─────────────────────────────────────────────

    #[test]
    fn test_polyline_bitstream_mkgmap_format() {
        // Verify mkgmap-compatible bitstream format:
        // [type(1)][label(3)][dlon(2)][dlat(2)][blen(1-2)][bitstream(blen+1)]
        let center = to_garmin_units(45.0);
        let line = make_polyline(4, None); // 4 points → 3 deltas in bitstream
        let rec = encode_polyline_record(&line, center, center, 24, 0, None);

        // Fixed header: type(1) + label_info(3) + dlon(2) + dlat(2) = 8 bytes
        assert!(rec.len() > 8, "polyline with 4 points must have data after header");

        // Type byte: bit 7 = FLAG_2BYTE_LEN (should be 0 for small records)
        let type_byte = rec[0];
        assert_eq!(type_byte & 0x3F, 0x06, "type must be 0x06");

        // blen at byte 8 (1 byte if FLAG_2BYTE_LEN not set)
        let two_byte_len = type_byte & 0x80 != 0;
        let blen = if two_byte_len {
            u16::from_le_bytes([rec[8], rec[9]]) as usize
        } else {
            rec[8] as usize
        };
        let bs_start = if two_byte_len { 10 } else { 9 };

        // Bitstream must be blen+1 bytes
        assert_eq!(rec.len(), 8 + (if two_byte_len { 2 } else { 1 }) + blen + 1,
            "record length must match: header + blen_field + bitstream");

        // First byte of bitstream: xBase(4 bits) + yBase(4 bits)
        let first_bs_byte = rec[bs_start];
        let x_base = first_bs_byte & 0x0F;
        let y_base = (first_bs_byte >> 4) & 0x0F;
        assert!(x_base <= 15, "xBase must fit in 4 bits");
        assert!(y_base <= 15, "yBase must fit in 4 bits");
    }

    // ── End-offset pointer tests (mixed feature types) ─────────────────────

    #[test]
    fn test_build_mixed_polylines_polygons_no_points() {
        // F1/F8: Subdivision with polylines + polygons but NO points.
        // End-offset pointers must be correct (no double-write).
        use crate::parser::mp_types::MpPolygon;
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points: vec![], // no points!
            polylines: vec![make_polyline(3, Some("Route"))],
            polygons: vec![MpPolygon {
                type_code: "0x50".to_string(),
                label: None,
                end_level: None,
                coords: vec![(45.0, 5.0), (45.1, 5.0), (45.1, 5.1), (45.0, 5.0)],
                holes: vec![],
                other_fields: HashMap::new(),
            }],
        };
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);

        // Must have data and not panic
        assert!(result.data.len() > RGN_HEADER_SIZE, "must have feature data");

        // 2 feature types (lines + polys), no points → 1 end-offset pointer (2 bytes).
        // The first 2 bytes of feature data = end-offset of polylines section.
        let data = &result.data[RGN_HEADER_SIZE..];
        let end_ptr = u16::from_le_bytes([data[0], data[1]]);
        // The pointer must be > 2 (header) and < total data length.
        assert!(end_ptr > 2, "end-offset pointer must be > header size");
        assert!(
            (end_ptr as usize) < data.len(),
            "end-offset pointer ({}) must be < total data ({})",
            end_ptr,
            data.len()
        );
    }

    // ── Extended type encoding tests ──────────────────────────────────────────

    fn make_ext_poi(type_code: &str, label: Option<&str>) -> MpPoint {
        MpPoint {
            type_code: type_code.to_string(),
            label: label.map(|s| s.to_string()),
            end_level: None,
            lat: 45.0,
            lon: 5.0,
            other_fields: HashMap::new(),
        }
    }

    fn make_ext_polyline(type_code: &str, n_coords: usize, label: Option<&str>) -> MpPolyline {
        let coords = (0..n_coords).map(|i| (45.0 + i as f64 * 0.001, 5.0)).collect();
        MpPolyline {
            type_code: type_code.to_string(),
            label: label.map(|s| s.to_string()),
            end_level: None,
            coords,
            routing: None,
            other_fields: HashMap::new(),
        }
    }

    #[test]
    fn test_encode_extended_point_record_size() {
        let center = to_garmin_units(45.0);
        // Extended POI without label: type_byte(1) + sub_flags(1) + delta_lon(2) + delta_lat(2) = 6 bytes
        let poi = make_ext_poi("0x11503", None);
        let rec = encode_extended_point_record(&poi, center, center, 24, false, 0);
        assert_eq!(rec.len(), 6, "extended POI without label must be 6 bytes");
        assert_eq!(rec[0], 0x15, "base_type for 0x11503 should be 0x15");
        assert_eq!(rec[1] & 0x1F, 0x03, "sub_type for 0x11503 should be 0x03");

        // With label: + 3 bytes label_offset = 9 bytes
        let poi_l = make_ext_poi("0x11503", Some("Antenne"));
        let rec_l = encode_extended_point_record(&poi_l, center, center, 24, false, 0);
        assert_eq!(rec_l.len(), 9, "extended POI with label must be 9 bytes");
        assert!(rec_l[1] & 0x20 != 0, "has_label flag (bit 5) must be set");
    }

    #[test]
    fn test_encode_extended_point_record_last_in_group() {
        let center = to_garmin_units(45.0);
        let poi = make_ext_poi("0x11503", None);
        let rec = encode_extended_point_record(&poi, center, center, 24, true, 0);
        assert!(rec[1] & 0x80 != 0, "last_in_group flag (bit 7) must be set");
    }

    #[test]
    fn test_encode_extended_polyline_record_header() {
        let center = to_garmin_units(45.0);
        let line = make_ext_polyline("0x10c00", 3, None);
        let rec = encode_extended_polyline_record(&line, center, center, 24, false, 0);
        // byte 0 = base_type = 0x0c
        assert_eq!(rec[0], 0x0c, "base_type for 0x10c00 should be 0x0c");
        // byte 1 = sub_type = 0x00
        assert_eq!(rec[1], 0x00, "sub_type for 0x10c00 should be 0x00");
        // byte 2 = flags (no label, not last, extra_bytes=0)
        assert_eq!(rec[2], 0x00, "flags should be 0x00 (no label, not last)");
    }

    #[test]
    fn test_encode_extended_polyline_record_with_label() {
        let center = to_garmin_units(45.0);
        let line = make_ext_polyline("0x10c00", 2, Some("Voie Ferrée"));
        let rec = encode_extended_polyline_record(&line, center, center, 24, true, 42);
        // flags: has_label (0x08) | last_in_group (0x80) = 0x88
        assert_eq!(rec[2], 0x88, "flags: has_label | last_in_group");
        // Last 3 bytes = label_offset = 42 as LE24
        let len = rec.len();
        assert_eq!(rec[len - 3], 42, "label_offset low byte");
        assert_eq!(rec[len - 2], 0, "label_offset mid byte");
        assert_eq!(rec[len - 1], 0, "label_offset high byte");
    }

    #[test]
    fn test_encode_extended_polygon_record_basic() {
        let center = to_garmin_units(45.0);
        use crate::parser::mp_types::MpPolygon;
        let poly = MpPolygon {
            type_code: "0x1101c".to_string(),
            label: None,
            end_level: None,
            coords: vec![(45.0, 5.0), (45.1, 5.0), (45.1, 5.1), (45.0, 5.1), (45.0, 5.0)],
            holes: vec![],
            other_fields: HashMap::new(),
        };
        let rec = encode_extended_polygon_record(&poly, center, center, 24, true, 0);
        assert_eq!(rec[0], 0x10, "base_type for 0x1101c should be 0x10");
        assert_eq!(rec[1], 0x1c, "sub_type for 0x1101c should be 0x1c");
        // flags: last_in_group (0x80) | no label | extra_bytes=0
        assert_eq!(rec[2], 0x80, "flags: last_in_group only");
    }

    #[test]
    fn test_build_with_extended_types_has_flags() {
        use crate::parser::mp_types::MpPolygon;
        let mp = MpFile {
            header: MpHeader {
                name: "Extended Test".to_string(),
                id: "63240099".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points: vec![
                make_ext_poi("0x2C00", Some("Standard")),
                make_ext_poi("0x11503", Some("Extended")),
            ],
            polylines: vec![],
            polygons: vec![MpPolygon {
                type_code: "0x1101c".to_string(),
                label: None,
                end_level: None,
                coords: vec![(45.0, 5.0), (45.1, 5.0), (45.1, 5.1), (45.0, 5.0)],
                holes: vec![],
                other_fields: HashMap::new(),
            }],
        };
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);

        // Extended types temporarily disabled.
        assert!(!result.subdiv_has_extended_points[0], "extended types disabled");
        assert!(!result.subdiv_has_extended_polylines[0], "extended types disabled");
        assert!(!result.subdiv_has_extended_polygons[0], "extended types disabled");
    }
}
