//! RGN subfile writer — geometry encoding for Garmin IMG.
//!
//! The RGN subfile stores the actual feature data (POIs, polylines, polygons)
//! as binary records referenced by TRE subdivisions.
//!
//! Format: `[RGN Header — 29 B] [Feature Records per level…]`

use std::collections::HashMap;

use crate::img::common_header::{build_common_header, COMMON_HEADER_SIZE};
use crate::img::tre::{to_garmin_units, MapLevel, TreWriter};
use crate::parser::mp_types::{MpFile, MpPoint, MpPolygon, MpPolyline};

/// Size of the RGN type-specific header (old 29 minus LE16 header_length and LE16 version).
const RGN_TYPE_SPECIFIC_SIZE: usize = 29 - 4; // 25 bytes
/// Total RGN header size with common header.
const RGN_HEADER_SIZE: usize = COMMON_HEADER_SIZE + RGN_TYPE_SPECIFIC_SIZE; // 46 bytes

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

/// RGN subfile header — 48 bytes (21-byte common header + 27-byte type-specific).
///
/// Binary layout:
/// ```text
/// 0x00  21B   Common header "GARMIN RGN"
/// 0x15  LE16  version = 1
/// 0x17  LE32  data_offset = 48 (0x30)
/// 0x1B  LE32  data_size (total bytes of feature records)
/// 0x1F  LE32  point_overview_offset = data_offset + data_size
/// 0x23  LE32  point_overview_size = 0
/// 0x27  LE32  polyline_overview_offset = data_offset + data_size
/// 0x2B  LE32  polyline_overview_size = 0
/// 0x2F  u8    reserved = 0
/// ```
struct RgnHeader {
    /// Total byte count of all feature records following this header.
    data_size: u32,
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
        // 0x1D: point_overview_offset = RGN_HEADER_SIZE + data_size (LE32)
        let overview_offset = RGN_HEADER_SIZE as u32 + self.data_size;
        buf.extend_from_slice(&overview_offset.to_le_bytes());
        // 0x21: point_overview_size = 0 (LE32)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x25: polyline_overview_offset = same (LE32)
        buf.extend_from_slice(&overview_offset.to_le_bytes());
        // 0x29: polyline_overview_size = 0 (LE32)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x2D: reserved = 0 (u8)
        buf.push(0u8);
        buf
    }
}

// ── Record encoders ─────────────────────────────────────────────────────────────

/// Encode a POI record into binary.
///
/// Layout:
/// ```text
/// byte 0    : base_type
/// byte 1-2  : delta_lon (LE16s) from subdivision centre
/// byte 3-4  : delta_lat (LE16s) from subdivision centre
/// byte 5    : flags — bit 7 = last_in_group, bit 3 = has_label, bits 2-0 = sub_type & 0x07
/// [if has_label]: bytes 6-8 = label_offset (LE24) from LBL data section start
/// ```
fn encode_point_record(
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

    let mut flags: u8 = tc.sub_type & 0x07;
    if has_label {
        flags |= 0x08;
    }
    if last_in_group {
        flags |= 0x80;
    }

    let mut buf = Vec::with_capacity(9);
    buf.push(tc.base_type);
    buf.extend_from_slice(&d_lon.to_le_bytes());
    buf.extend_from_slice(&d_lat.to_le_bytes());
    buf.push(flags);
    if has_label {
        let le = label_offset.to_le_bytes();
        buf.push(le[0]);
        buf.push(le[1]);
        buf.push(le[2]);
    }
    buf
}

/// Shared binary encoding for polyline and polygon records (same wire format).
///
/// Layout:
/// ```text
/// byte 0    : base_type
/// byte 1    : flags — bit 7 = last_in_group, bit 3 = has_label, bit 2 = two_byte_delta
/// byte 2-3  : delta_lon of 1st point from subdivision centre (LE16s)
/// byte 4-5  : delta_lat of 1st point from subdivision centre (LE16s)
/// for each subsequent point (two_byte_delta=1):
///   2 bytes delta_lon from previous point + 2 bytes delta_lat (LE16s)
/// terminator: 0x80 0x00  (mkgmap PolyRecord.java: 0x80 then 0x00 for two_byte_delta)
/// [if has_label]: 3 bytes label_offset (LE24) from LBL data section start
/// ```
///
/// Note: 1-byte delta encoding is an optional optimisation deferred to Epic 14+.
#[allow(clippy::too_many_arguments)]
fn encode_poly_record_inner(
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

    let mut flags: u8 = 0x04; // two_byte_delta = bit 2
    if has_label {
        flags |= 0x08;
    }
    if last_in_group {
        flags |= 0x80;
    }

    let mut buf = Vec::new();
    buf.push(tc.base_type);
    buf.push(flags);

    if coords.is_empty() {
        // End-of-coordinate-sequence terminator (mkgmap: 0x80 0x00 for two_byte_delta)
        buf.push(0x80);
        buf.push(0x00);
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

    // Subsequent points: delta from previous point
    let mut prev_lat_g = lat0_g;
    let mut prev_lon_g = lon0_g;
    for &(lat, lon) in &coords[1..] {
        let lat_g = to_garmin_units(lat);
        let lon_g = to_garmin_units(lon);
        buf.extend_from_slice(&encode_delta(lon_g, prev_lon_g, bits_per_coord).to_le_bytes());
        buf.extend_from_slice(&encode_delta(lat_g, prev_lat_g, bits_per_coord).to_le_bytes());
        prev_lat_g = lat_g;
        prev_lon_g = lon_g;
    }

    // End-of-coordinate-sequence terminator (mkgmap: 0x80 0x00 for two_byte_delta)
    buf.push(0x80);
    buf.push(0x00);

    if has_label {
        let le = label_offset.to_le_bytes();
        buf.push(le[0]);
        buf.push(le[1]);
        buf.push(le[2]);
    }
    buf
}

/// Encode a polyline record into binary (with `two_byte_delta = 1` always).
fn encode_polyline_record(
    line: &MpPolyline,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    encode_poly_record_inner(
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

/// Encode a polygon record — same format as polyline (outer ring only; inner rings = Epic 14+).
fn encode_polygon_record(
    poly: &MpPolygon,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
    label_offset: u32,
) -> Vec<u8> {
    encode_poly_record_inner(
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

/// Result of `RgnWriter::build`: the complete RGN binary and per-level data offsets.
pub struct RgnBuildResult {
    /// Complete RGN subfile binary: `[RgnHeader || feature_data]`.
    pub data: Vec<u8>,
    /// Per-level byte offset into the feature data section (after the RGN header).
    ///
    /// `subdivision_offsets[0]` = 0 (first level starts at the beginning of data section).
    /// `subdivision_offsets[i]` = total bytes written for levels 0..i.
    pub subdivision_offsets: Vec<u32>,
    /// Cross-references from RGN polylines to NET road definitions.
    /// Populated only when `build_with_net_offsets` is used.
    pub subdiv_road_refs: Vec<crate::img::net::SubdivRoadRef>,
    /// Per-subdivision flag: true if extended POIs are present.
    pub subdiv_has_extended_points: Vec<bool>,
    /// Per-subdivision flag: true if extended polylines are present.
    pub subdiv_has_extended_polylines: Vec<bool>,
    /// Per-subdivision flag: true if extended polygons are present.
    pub subdiv_has_extended_polygons: Vec<bool>,
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
    /// For each level `i`, features are filtered by `end_level >= i` (i.e. a feature
    /// with `end_level = 0` appears only in level 0; `None` means all levels).
    ///
    /// The returned `subdivision_offsets[i]` is the byte offset from the start of the
    /// feature data section (not from the start of the RGN file) for level `i`.
    ///
    /// `label_offsets`: map from label string → offset from the start of the LBL data
    /// section (as returned by [`LblWriter::build`]). An empty map produces stub
    /// 0x000000 offsets.
    ///
    /// `net_road_offsets`: for each routable polyline (by `polyline_idx` in RoadDef),
    /// the byte offset of its NET1 record. Empty slice = no NET cross-references.
    pub fn build_with_net_offsets(
        mp: &MpFile,
        levels: &[MapLevel],
        label_offsets: &HashMap<String, u32>,
        net_road_offsets: &[u32],
    ) -> RgnBuildResult {
        let n = levels.len();

        // Compute bounding box → subdivision centre (same formula as TreWriter).
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(mp);
        let min_lat_g = to_garmin_units(min_lat);
        let max_lat_g = to_garmin_units(max_lat);
        let min_lon_g = to_garmin_units(min_lon);
        let max_lon_g = to_garmin_units(max_lon);
        let center_lat_g = (max_lat_g + min_lat_g) / 2;
        let center_lon_g = (max_lon_g + min_lon_g) / 2;

        // Build mapping: polyline original index → NET1 offset (if routable).
        // net_road_offsets is indexed by road_def_idx; we need polyline_idx → net1_offset.
        // RoadDef.polyline_idx maps road_def → polyline. We invert this.
        let has_net = !net_road_offsets.is_empty();
        let mut polyline_net_offset: HashMap<usize, (u32, usize)> = HashMap::new();
        if has_net {
            // We need to find which polylines are routable. Since we don't have the
            // RoadNetwork here, we use the polyline's `routing` field directly.
            // net_road_offsets[road_def_idx] = NET1 offset. road_def_idx corresponds to
            // the order of routable polylines (those with routing.is_some()).
            let mut road_def_idx = 0usize;
            for (pi, pl) in mp.polylines.iter().enumerate() {
                if pl.routing.is_some() && road_def_idx < net_road_offsets.len() {
                    polyline_net_offset.insert(pi, (net_road_offsets[road_def_idx], road_def_idx));
                    road_def_idx += 1;
                }
            }
        }

        // Pre-compute extended flag per feature to avoid redundant parse_type_code calls.
        let point_extended: Vec<bool> = mp.points.iter().map(|p| parse_type_code(&p.type_code).extended).collect();
        let polyline_extended: Vec<bool> = mp.polylines.iter().map(|p| parse_type_code(&p.type_code).extended).collect();
        let polygon_extended: Vec<bool> = mp.polygons.iter().map(|p| parse_type_code(&p.type_code).extended).collect();

        let mut feature_data: Vec<u8> = Vec::new();
        let mut subdivision_offsets = Vec::with_capacity(n);
        let mut subdiv_road_refs: Vec<crate::img::net::SubdivRoadRef> = Vec::new();
        let mut subdiv_has_extended_points = Vec::with_capacity(n);
        let mut subdiv_has_extended_polylines = Vec::with_capacity(n);
        let mut subdiv_has_extended_polygons = Vec::with_capacity(n);

        for (i, level) in levels.iter().enumerate() {
            // Record the offset into the feature data for this level.
            subdivision_offsets.push(feature_data.len() as u32);

            let bits = level.bits_per_coord;
            let subdiv_number = (i + 1) as u16; // 1-based

            // Filter features by end_level and partition standard/extended using pre-computed flags.
            let (std_points, ext_points): (Vec<_>, Vec<_>) = mp
                .points
                .iter()
                .enumerate()
                .filter(|(_, f)| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .partition(|(idx, _)| !point_extended[*idx]);
            let std_points: Vec<_> = std_points.into_iter().map(|(_, p)| p).collect();
            let ext_points: Vec<_> = ext_points.into_iter().map(|(_, p)| p).collect();

            // Keep original index for polylines (needed for NET cross-reference).
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

            subdiv_has_extended_points.push(!ext_points.is_empty());
            subdiv_has_extended_polylines.push(!ext_polylines.is_empty());
            subdiv_has_extended_polygons.push(!ext_polygons.is_empty());

            // ── Standard sections ─────────────────────────────────────────

            // Write standard POI records.
            let poi_count = std_points.len();
            for (j, poi) in std_points.iter().enumerate() {
                let last = j + 1 == poi_count;
                let lbl_offset = poi
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record =
                    encode_point_record(poi, center_lat_g, center_lon_g, bits, last, lbl_offset);
                feature_data.extend_from_slice(&record);
            }

            // Write standard polyline records with optional NET1 cross-references.
            let line_count = std_polylines.len();
            let mut polyline_index_in_subdiv: u8 = 0;
            for (j, (orig_idx, line)) in std_polylines.iter().enumerate() {
                let last = j + 1 == line_count;
                let lbl_offset = line
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record =
                    encode_polyline_record(line, center_lat_g, center_lon_g, bits, last, lbl_offset);
                feature_data.extend_from_slice(&record);

                // Append NET1 offset (3 bytes) for routable polylines.
                if let Some(&(net1_offset, road_def_idx)) = polyline_net_offset.get(orig_idx) {
                    let net_ref: u32 = net1_offset & 0x003F_FFFF; // bits 0-21
                    feature_data.push((net_ref & 0xFF) as u8);
                    feature_data.push(((net_ref >> 8) & 0xFF) as u8);
                    feature_data.push(((net_ref >> 16) & 0xFF) as u8);

                    subdiv_road_refs.push(crate::img::net::SubdivRoadRef {
                        road_def_idx,
                        subdiv_number,
                        polyline_index: polyline_index_in_subdiv,
                    });
                }
                polyline_index_in_subdiv = polyline_index_in_subdiv.saturating_add(1);
            }

            // Write standard polygon records.
            let poly_count = std_polygons.len();
            for (j, poly) in std_polygons.iter().enumerate() {
                let last = j + 1 == poly_count;
                let lbl_offset = poly
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record =
                    encode_polygon_record(poly, center_lat_g, center_lon_g, bits, last, lbl_offset);
                feature_data.extend_from_slice(&record);
            }

            // ── Extended sections ─────────────────────────────────────────

            // Write extended POI records.
            let ext_poi_count = ext_points.len();
            for (j, poi) in ext_points.iter().enumerate() {
                let last = j + 1 == ext_poi_count;
                let lbl_offset = poi
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record = encode_extended_point_record(
                    poi, center_lat_g, center_lon_g, bits, last, lbl_offset,
                );
                feature_data.extend_from_slice(&record);
            }

            // Write extended polyline records (no NET cross-references for extended types).
            let ext_line_count = ext_polylines.len();
            for (j, (_orig_idx, line)) in ext_polylines.iter().enumerate() {
                let last = j + 1 == ext_line_count;
                let lbl_offset = line
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record = encode_extended_polyline_record(
                    line, center_lat_g, center_lon_g, bits, last, lbl_offset,
                );
                feature_data.extend_from_slice(&record);
            }

            // Write extended polygon records.
            let ext_poly_count = ext_polygons.len();
            for (j, poly) in ext_polygons.iter().enumerate() {
                let last = j + 1 == ext_poly_count;
                let lbl_offset = poly
                    .label
                    .as_ref()
                    .and_then(|l| label_offsets.get(l.as_str()).copied())
                    .unwrap_or(0);
                let record = encode_extended_polygon_record(
                    poly, center_lat_g, center_lon_g, bits, last, lbl_offset,
                );
                feature_data.extend_from_slice(&record);
            }
        }

        // Build the header and prepend it.
        let header = RgnHeader {
            data_size: feature_data.len() as u32,
        };
        let mut data = header.to_bytes();
        data.extend_from_slice(&feature_data);

        RgnBuildResult {
            data,
            subdivision_offsets,
            subdiv_road_refs,
            subdiv_has_extended_points,
            subdiv_has_extended_polylines,
            subdiv_has_extended_polygons,
        }
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
        let h = RgnHeader { data_size: 0 };
        assert_eq!(h.to_bytes().len(), RGN_HEADER_SIZE, "RGN header must be exactly 48 bytes");
    }

    #[test]
    fn test_rgn_header_common_header() {
        let h = RgnHeader { data_size: 0 };
        let bytes = h.to_bytes();
        // Common header: LE16(46) at 0x00, "GARMIN RGN" at 0x02
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), RGN_HEADER_SIZE as u16);
        assert_eq!(&bytes[0x02..0x0C], b"GARMIN RGN");
        // data_offset at 0x15 (no version field)
        let data_offset = u32::from_le_bytes([bytes[0x15], bytes[0x16], bytes[0x17], bytes[0x18]]);
        assert_eq!(data_offset, RGN_HEADER_SIZE as u32);
    }

    // ── Task 4: Record formats ────────────────────────────────────────────────

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
    fn test_encode_point_record_size() {
        let center = to_garmin_units(45.0);
        // Without label: 6 bytes
        let poi_no_label = make_poi(None);
        let rec = encode_point_record(&poi_no_label, center, center, 24, false, 0);
        assert_eq!(rec.len(), 6, "POI record without label must be 6 bytes");
        // With label: 9 bytes
        let poi_with_label = make_poi(Some("Mairie"));
        let rec = encode_point_record(&poi_with_label, center, center, 24, false, 0);
        assert_eq!(rec.len(), 9, "POI record with label must be 9 bytes");
    }

    #[test]
    fn test_encode_polyline_record_terminates() {
        let center = to_garmin_units(45.0);
        // Without label: terminator is the last 2 bytes.
        let line = make_polyline(3, None);
        let rec = encode_polyline_record(&line, center, center, 24, false, 0);
        let len = rec.len();
        assert!(len >= 2, "polyline record must have at least 2 bytes");
        assert_eq!(rec[len - 2], 0x80, "terminator first byte must be 0x80");
        assert_eq!(rec[len - 1], 0x00, "terminator second byte must be 0x00 (two_byte_delta, mkgmap)");

        // With label offset=0 (stub): terminator then 3 zero bytes.
        let line_l = make_polyline(3, Some("Route"));
        let rec_l = encode_polyline_record(&line_l, center, center, 24, false, 0);
        let len_l = rec_l.len();
        // Layout: …[0x80][0x00][0x00][0x00][0x00]  (terminator then label offset)
        assert_eq!(rec_l[len_l - 5], 0x80, "terminator first byte must be 0x80 (before label)");
        assert_eq!(rec_l[len_l - 4], 0x00, "terminator second byte must be 0x00 (before label)");
        assert_eq!(&rec_l[len_l - 3..], &[0x00, 0x00, 0x00], "label offset 0 must produce 0x000000");
    }

    #[test]
    fn test_encode_polygon_basic() {
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
        let rec = encode_polygon_record(&poly, center, center, 24, true, 0);
        // byte 0 = base_type = 0x50
        assert_eq!(rec[0], 0x50, "polygon base_type must be 0x50");
        // byte 1 = flags: two_byte_delta (0x04) | last_in_group (0x80) = 0x84
        assert_eq!(rec[1], 0x84, "polygon flags: two_byte_delta | last_in_group");
        // must end with 0x80 0x00 terminator (mkgmap two_byte_delta terminator)
        let len = rec.len();
        assert_eq!(rec[len - 2], 0x80);
        assert_eq!(rec[len - 1], 0x00);
    }

    // ── Task 4 (Story 13.5): LBL offset tests ─────────────────────────────────

    #[test]
    fn test_encode_point_record_real_lbl_offset() {
        // POI "Mairie" with offset=1 → bytes 6-8 must be [0x01, 0x00, 0x00]
        let center = to_garmin_units(45.0);
        let poi = make_poi(Some("Mairie"));
        let rec = encode_point_record(&poi, center, center, 24, false, 1);
        assert_eq!(rec.len(), 9);
        assert_eq!(&rec[6..9], &[0x01, 0x00, 0x00], "label_offset=1 must appear as LE24 [0x01,0x00,0x00]");
    }

    #[test]
    fn test_encode_polyline_record_real_lbl_offset() {
        // Polyline "D1075" with offset=8 → last 3 bytes before terminator (actually after) = [0x08, 0x00, 0x00]
        let center = to_garmin_units(45.0);
        let line = make_polyline(2, Some("D1075"));
        let rec = encode_polyline_record(&line, center, center, 24, false, 8);
        let len = rec.len();
        // Layout ends with: … [0x80][0x00] [0x08][0x00][0x00]
        assert_eq!(&rec[len - 3..], &[0x08, 0x00, 0x00], "label_offset=8 must appear as LE24 [0x08,0x00,0x00]");
    }

    #[test]
    fn test_build_stub_offsets_are_zero() {
        // Regression: RgnWriter::build() (without HashMap) → offsets remain 0x000000 (Story 13.4 behaviour).
        // NOTE: this also means `has_label=true` with `label_offset=0x000000` — offset 0 is the LBL null
        // sentinel. A GPS reading these records would find empty labels. build() is test-only; production
        // code must use build_with_lbl_offsets() with a populated LblBuildResult.
        let mp = make_mp_single_poi();
        let levels = levels_from_mp(&mp.header);
        let result = RgnWriter::build(&mp, &levels);
        // POI record: RGN_HEADER_SIZE + 6 = label offset bytes in first record
        let lbl_off_start = RGN_HEADER_SIZE + 6;
        assert_eq!(
            &result.data[lbl_off_start..lbl_off_start + 3],
            &[0x00, 0x00, 0x00],
            "build() without LBL offsets must produce stub 0x000000 in record"
        );
    }

    #[test]
    fn test_build_with_lbl_offsets_dedup_rgn_consistency() {
        // AC1 + AC4: two POIs sharing the same label must reference the SAME LBL offset in RGN.
        // This validates the RGN side of deduplication — LblWriter deduplicates and RgnWriter
        // must propagate the identical offset to every record sharing that label.
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

        // Feature data starts after the RGN header.
        // Both POIs have labels → each record is 9 bytes:
        //   byte 0: base_type, bytes 1-4: delta coords (LE16s × 2), byte 5: flags,
        //   bytes 6-8: label_offset (LE24)
        let data = &rgn.data[RGN_HEADER_SIZE..]; // feature data section
        let offset0 = u32::from_le_bytes([data[6], data[7], data[8], 0]);
        let offset1 = u32::from_le_bytes([data[15], data[16], data[17], 0]);

        assert_ne!(offset0, 0, "first 'Église' POI label_offset must be non-zero (real LBL offset)");
        assert_eq!(
            offset0, offset1,
            "duplicate 'Église' POIs must reference the same LBL offset in RGN (AC1 + AC4)"
        );
    }

    // ── Task 5: RgnWriter::build ──────────────────────────────────────────────

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
        // Data must be at least RGN_HEADER_SIZE (46) + 9 (POI with label) = 55 bytes
        assert!(
            result.data.len() >= RGN_HEADER_SIZE + 9,
            "RGN with single POI must be at least {} bytes, got {}",
            RGN_HEADER_SIZE + 9,
            result.data.len()
        );
        // Starts with common header "GARMIN RGN"
        assert_eq!(&result.data[0x02..0x0C], b"GARMIN RGN");
        // One level → one offset
        assert_eq!(result.subdivision_offsets.len(), 1);
        assert_eq!(result.subdivision_offsets[0], 0);
    }

    #[test]
    fn test_rgn_level_filter_excludes_low_endlevel() {
        // POI with EndLevel=0 → included in level 0, NOT in level 1
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
                    end_level: Some(0), // only in level 0
                    lat: 45.0,
                    lon: 5.0,
                    other_fields: HashMap::new(),
                },
                MpPoint {
                    type_code: "0x06".to_string(),
                    label: None,
                    end_level: Some(4), // in both levels
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

        // Level 0: 2 POIs. Level 1: 1 POI.
        // Data written for level 0 must be larger than for level 1.
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
        // All three levels include the POI (EndLevel=None → always included)
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24, 21, 18],
                ..Default::default()
            },
            points: vec![MpPoint {
                type_code: "0x2C00".to_string(),
                label: Some("Test".to_string()),
                end_level: None, // all levels
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

        assert!(result.subdiv_has_extended_points[0], "should have extended points");
        assert!(!result.subdiv_has_extended_polylines[0], "should not have extended polylines");
        assert!(result.subdiv_has_extended_polygons[0], "should have extended polygons");
    }
}
