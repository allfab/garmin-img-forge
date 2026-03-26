//! RGN subfile writer — geometry encoding for Garmin IMG.
//!
//! The RGN subfile stores the actual feature data (POIs, polylines, polygons)
//! as binary records referenced by TRE subdivisions.
//!
//! Format: `[RGN Header — 29 B] [Feature Records per level…]`

use crate::img::tre::{to_garmin_units, MapLevel, TreWriter};
use crate::parser::mp_types::{MpFile, MpPoint, MpPolygon, MpPolyline};

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
/// - `"0x10001"` → `{ base_type: 0x00, sub_type: 0x00, extended: true }` + warn
/// - any invalid string → `{ base_type: 0x00, sub_type: 0x00, extended: false }` + warn
pub fn parse_type_code(s: &str) -> RgnTypeCode {
    let trimmed = s.trim();
    let hex_str = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);

    match u32::from_str_radix(hex_str, 16) {
        Ok(v) if v > 0xFFFF => {
            tracing::warn!(
                "Extended type code '{}': not supported until Epic 14+, using 0x00",
                s
            );
            RgnTypeCode {
                base_type: 0x00,
                sub_type: 0x00,
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

/// RGN subfile header — exactly 29 bytes (0x1D).
///
/// Binary layout (matches mkgmap `RGNHeader.java`):
/// ```text
/// 0x00  LE16  header_length = 0x1D (29)
/// 0x02  LE16  version = 1
/// 0x04  LE32  data_offset = 0x1D
/// 0x08  LE32  data_size (total bytes of feature records)
/// 0x0C  LE32  point_overview_offset = data_offset + data_size
/// 0x10  LE32  point_overview_size = 0 (overview not required for basic display)
/// 0x14  LE32  polyline_overview_offset = data_offset + data_size
/// 0x18  LE32  polyline_overview_size = 0
/// 0x1C  u8    reserved = 0
/// ```
struct RgnHeader {
    /// Total byte count of all feature records following this header.
    data_size: u32,
}

impl RgnHeader {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(29);
        // 0x00: header_length = 29 (LE16)
        buf.extend_from_slice(&0x001Du16.to_le_bytes());
        // 0x02: version = 1 (LE16)
        buf.extend_from_slice(&1u16.to_le_bytes());
        // 0x04: data_offset = 29 (LE32)
        buf.extend_from_slice(&0x001Du32.to_le_bytes());
        // 0x08: data_size (LE32)
        buf.extend_from_slice(&self.data_size.to_le_bytes());
        // 0x0C: point_overview_offset = 29 + data_size (LE32)
        let overview_offset = 0x1Du32 + self.data_size;
        buf.extend_from_slice(&overview_offset.to_le_bytes());
        // 0x10: point_overview_size = 0 (LE32)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x14: polyline_overview_offset = same (LE32)
        buf.extend_from_slice(&overview_offset.to_le_bytes());
        // 0x18: polyline_overview_size = 0 (LE32)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x1C: reserved = 0 (u8)
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
/// [if has_label]: bytes 6-8 = label_offset (LE24) stub 0x000000 (LBL = Story 13.5)
/// ```
fn encode_point_record(
    poi: &MpPoint,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
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
        // label_offset stub 0x000000 — LBL writer is Story 13.5
        buf.push(0x00);
        buf.push(0x00);
        buf.push(0x00);
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
/// [if has_label]: 3 bytes label_offset stub 0x000000
/// ```
///
/// Note: 1-byte delta encoding is an optional optimisation deferred to Epic 14+.
fn encode_poly_record_inner(
    type_code: &str,
    label: &Option<String>,
    coords: &[(f64, f64)],
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
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
            buf.push(0x00);
            buf.push(0x00);
            buf.push(0x00);
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
        buf.push(0x00);
        buf.push(0x00);
        buf.push(0x00);
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
) -> Vec<u8> {
    encode_poly_record_inner(
        &line.type_code,
        &line.label,
        &line.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        last_in_group,
    )
}

/// Encode a polygon record — same format as polyline (outer ring only; inner rings = Epic 14+).
fn encode_polygon_record(
    poly: &MpPolygon,
    center_lat_g: i32,
    center_lon_g: i32,
    bits_per_coord: u8,
    last_in_group: bool,
) -> Vec<u8> {
    encode_poly_record_inner(
        &poly.type_code,
        &poly.label,
        &poly.coords,
        center_lat_g,
        center_lon_g,
        bits_per_coord,
        last_in_group,
    )
}

// ── RgnWriter ──────────────────────────────────────────────────────────────────

/// Result of `RgnWriter::build`: the complete RGN binary and per-level data offsets.
pub struct RgnBuildResult {
    /// Complete RGN subfile binary: `[RgnHeader || feature_data]`.
    pub data: Vec<u8>,
    /// Per-level byte offset into the feature data section (after the 29-byte header).
    ///
    /// `subdivision_offsets[0]` = 0 (first level starts at the beginning of data section).
    /// `subdivision_offsets[i]` = total bytes written for levels 0..i.
    pub subdivision_offsets: Vec<u32>,
}

/// Builds the RGN subfile binary from a parsed Polish Map.
pub struct RgnWriter;

impl RgnWriter {
    /// Build the complete RGN subfile binary and compute per-level subdivision offsets.
    ///
    /// For each level `i`, features are filtered by `end_level >= i` (i.e. a feature
    /// with `end_level = 0` appears only in level 0; `None` means all levels).
    ///
    /// The returned `subdivision_offsets[i]` is the byte offset from the start of the
    /// feature data section (not from the start of the RGN file) for level `i`.
    pub fn build(mp: &MpFile, levels: &[MapLevel]) -> RgnBuildResult {
        let n = levels.len();

        // Compute bounding box → subdivision centre (same formula as TreWriter).
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(mp);
        let min_lat_g = to_garmin_units(min_lat);
        let max_lat_g = to_garmin_units(max_lat);
        let min_lon_g = to_garmin_units(min_lon);
        let max_lon_g = to_garmin_units(max_lon);
        let center_lat_g = (max_lat_g + min_lat_g) / 2;
        let center_lon_g = (max_lon_g + min_lon_g) / 2;

        let mut feature_data: Vec<u8> = Vec::new();
        let mut subdivision_offsets = Vec::with_capacity(n);

        for (i, level) in levels.iter().enumerate() {
            // Record the offset into the feature data for this level.
            subdivision_offsets.push(feature_data.len() as u32);

            let bits = level.bits_per_coord;

            // Filter features: include if end_level >= level_index, or if end_level is unset.
            let points: Vec<_> = mp
                .points
                .iter()
                .filter(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .collect();
            let polylines: Vec<_> = mp
                .polylines
                .iter()
                .filter(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .collect();
            let polygons: Vec<_> = mp
                .polygons
                .iter()
                .filter(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8)
                .collect();

            // Write POI records (last_in_group on the final one).
            let poi_count = points.len();
            for (j, poi) in points.iter().enumerate() {
                let last = j + 1 == poi_count;
                let record =
                    encode_point_record(poi, center_lat_g, center_lon_g, bits, last);
                feature_data.extend_from_slice(&record);
            }

            // Write polyline records.
            let line_count = polylines.len();
            for (j, line) in polylines.iter().enumerate() {
                let last = j + 1 == line_count;
                let record =
                    encode_polyline_record(line, center_lat_g, center_lon_g, bits, last);
                feature_data.extend_from_slice(&record);
            }

            // Write polygon records.
            let poly_count = polygons.len();
            for (j, poly) in polygons.iter().enumerate() {
                let last = j + 1 == poly_count;
                let record =
                    encode_polygon_record(poly, center_lat_g, center_lon_g, bits, last);
                feature_data.extend_from_slice(&record);
            }
        }

        // Build the 29-byte header and prepend it.
        let header = RgnHeader {
            data_size: feature_data.len() as u32,
        };
        let mut data = header.to_bytes();
        data.extend_from_slice(&feature_data);

        RgnBuildResult {
            data,
            subdivision_offsets,
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
        assert_eq!(h.to_bytes().len(), 29, "RGN header must be exactly 29 bytes");
    }

    #[test]
    fn test_rgn_header_magic() {
        let h = RgnHeader { data_size: 0 };
        let bytes = h.to_bytes();
        // header_length=29 (LE16) + version=1 (LE16) → [0x1D, 0x00, 0x01, 0x00]
        assert_eq!(&bytes[0..4], &[0x1D, 0x00, 0x01, 0x00]);
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
        let rec = encode_point_record(&poi_no_label, center, center, 24, false);
        assert_eq!(rec.len(), 6, "POI record without label must be 6 bytes");
        // With label: 9 bytes
        let poi_with_label = make_poi(Some("Mairie"));
        let rec = encode_point_record(&poi_with_label, center, center, 24, false);
        assert_eq!(rec.len(), 9, "POI record with label must be 9 bytes");
    }

    #[test]
    fn test_encode_polyline_record_terminates() {
        let center = to_garmin_units(45.0);
        // Without label: terminator is the last 2 bytes.
        let line = make_polyline(3, None);
        let rec = encode_polyline_record(&line, center, center, 24, false);
        let len = rec.len();
        assert!(len >= 2, "polyline record must have at least 2 bytes");
        assert_eq!(rec[len - 2], 0x80, "terminator first byte must be 0x80");
        assert_eq!(rec[len - 1], 0x00, "terminator second byte must be 0x00 (two_byte_delta, mkgmap)");

        // With label: terminator is 3 bytes before end (3-byte label stub follows).
        let line_l = make_polyline(3, Some("Route"));
        let rec_l = encode_polyline_record(&line_l, center, center, 24, false);
        let len_l = rec_l.len();
        // Layout: …[0x80][0x00][0x00][0x00][0x00]  (terminator then label stub)
        assert_eq!(rec_l[len_l - 5], 0x80, "terminator first byte must be 0x80 (before label)");
        assert_eq!(rec_l[len_l - 4], 0x00, "terminator second byte must be 0x00 (before label)");
        assert_eq!(&rec_l[len_l - 3..], &[0x00, 0x00, 0x00], "label stub must be 0x000000");
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
        let rec = encode_polygon_record(&poly, center, center, 24, true);
        // byte 0 = base_type = 0x50
        assert_eq!(rec[0], 0x50, "polygon base_type must be 0x50");
        // byte 1 = flags: two_byte_delta (0x04) | last_in_group (0x80) = 0x84
        assert_eq!(rec[1], 0x84, "polygon flags: two_byte_delta | last_in_group");
        // must end with 0x80 0x00 terminator (mkgmap two_byte_delta terminator)
        let len = rec.len();
        assert_eq!(rec[len - 2], 0x80);
        assert_eq!(rec[len - 1], 0x00);
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
        // Data must be at least 29 (header) + 9 (POI with label) = 38 bytes
        assert!(
            result.data.len() >= 38,
            "RGN with single POI must be at least 38 bytes, got {}",
            result.data.len()
        );
        // Starts with RGN header magic
        assert_eq!(&result.data[0..4], &[0x1D, 0x00, 0x01, 0x00]);
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
        let total_feature_data = (result.data.len() as u32) - 29; // 29 = header
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
}
