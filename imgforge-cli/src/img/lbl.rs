//! LBL subfile writer — labels & street names for Garmin IMG.
//!
//! The LBL subfile stores encoded label strings referenced by RGN records.
//!
//! Format: `[LBL Header — 28 B] [Label data section…]`
//!
//! Label data section layout:
//! ```text
//! Byte 0  : 0x00  (null initial — offset 0 = "no label")
//! Byte 1+ : label strings encoded in CP1252, each null-terminated
//! ```

use std::collections::HashMap;

use crate::img::common_header::{build_common_header, COMMON_HEADER_SIZE};
use crate::parser::mp_types::MpFile;

/// Standard Garmin LBL header size (196 bytes), compatible with QMapShack/BaseCamp.
/// Layout: 21-byte common header + 175-byte type-specific (section pointers for
/// labels, country, region, city, POI index, POI type, ZIP, highway, exit, etc.).
/// Empty sections use offset = header_size, size = 0.
const LBL_HEADER_SIZE: usize = 196;

// ── CP1252 encoding ─────────────────────────────────────────────────────────────

/// Map a Unicode codepoint in the CP1252 0x80–0x9F special range to its CP1252 byte.
///
/// Returns `None` for undefined CP1252 slots (0x81, 0x8D, 0x8F, 0x90, 0x9D) and for
/// codepoints outside this special range.
///
/// Implemented as a `match` so the compiler can generate an O(1) lookup (jump table
/// or binary search) rather than the O(n) linear scan of a flat array.
#[inline]
fn cp1252_special(cp: u32) -> Option<u8> {
    match cp {
        0x0152 => Some(0x8C), // Œ
        0x0153 => Some(0x9C), // œ
        0x0160 => Some(0x8A), // Š
        0x0161 => Some(0x9A), // š
        0x0178 => Some(0x9F), // Ÿ
        0x017D => Some(0x8E), // Ž
        0x017E => Some(0x9E), // ž
        0x0192 => Some(0x83), // ƒ
        0x02C6 => Some(0x88), // ˆ
        0x02DC => Some(0x98), // ˜
        0x2013 => Some(0x96), // –
        0x2014 => Some(0x97), // —
        0x2018 => Some(0x91), // '
        0x2019 => Some(0x92), // '
        0x201A => Some(0x82), // ‚
        0x201C => Some(0x93), // "
        0x201D => Some(0x94), // "
        0x201E => Some(0x84), // „
        0x2020 => Some(0x86), // †
        0x2021 => Some(0x87), // ‡
        0x2022 => Some(0x95), // •
        0x2026 => Some(0x85), // …
        0x2030 => Some(0x89), // ‰
        0x2039 => Some(0x8B), // ‹
        0x203A => Some(0x9B), // ›
        0x20AC => Some(0x80), // €
        0x2122 => Some(0x99), // ™
        _ => None,
    }
}

/// Attempt to parse a Garmin shield prefix `~[0xNN]` from `label`.
///
/// Returns `(shield_byte, rest_of_label)` if the prefix is found, else `None`.
fn parse_shield(label: &str) -> Option<(u8, &str)> {
    let rest = label.strip_prefix("~[0x")?;
    let bracket = rest.find(']')?;
    if bracket != 2 {
        return None;
    }
    let byte = u8::from_str_radix(&rest[..bracket], 16).ok()?;
    Some((byte, &rest[bracket + 1..]))
}

/// Encode a label string into CP1252 bytes with a null terminator.
///
/// - Shield prefix `~[0xNN]` → raw byte NN prepended.
/// - ASCII (≤ 0x7F) → direct.
/// - Latin-1 (0xA0–0xFF) → direct (codepoint = byte value).
/// - CP1252 special (0x80–0x9F table) → mapped via lookup.
/// - Everything else → `tracing::warn!` + `?` (0x3F).
pub fn encode_label_cp1252(label: &str) -> Vec<u8> {
    let mut out = Vec::new();

    // Step 1 — shield prefix
    let text = if let Some((shield_byte, rest)) = parse_shield(label) {
        out.push(shield_byte);
        rest
    } else {
        label
    };

    // Step 2 — UTF-8 char → CP1252 byte
    for ch in text.chars() {
        let cp = ch as u32;
        let byte = if cp <= 0x7F {
            // ASCII direct
            cp as u8
        } else if (0x00A0..=0x00FF).contains(&cp) {
            // Latin-1 direct (coincides with CP1252 in this range)
            cp as u8
        } else {
            // Try special CP1252 lookup (0x80–0x9F range and a few higher codepoints)
            if let Some(b) = cp1252_special(cp) {
                b
            } else {
                tracing::warn!(
                    "Character U+{:04X} ('{}') has no CP1252 mapping, replaced with '?'",
                    cp,
                    ch
                );
                0x3F // '?'
            }
        };
        out.push(byte);
    }

    // Step 3 — null terminator
    out.push(0x00);
    out
}

// ── LBL Header ──────────────────────────────────────────────────────────────────

/// LBL subfile header — 196 bytes (standard Garmin format).
///
/// Binary layout (type-specific section starts at 0x15):
/// ```text
/// 0x15  LE32  label_offset (= 196)
/// 0x19  LE32  label_size
/// 0x1D  u8    addr_offset_multiplier = 0
/// 0x1E  u8    label_coding = 9 (8-bit CP1252)
/// 0x1F  u8    driving_side = 0 (right-hand traffic)
/// 0x20  LE32  country_def_offset      0x24  LE32  country_def_size
/// 0x28  LE16  country_def_record_size
/// 0x2A  LE32  region_def_offset       0x2E  LE32  region_def_size
/// 0x32  LE16  region_def_record_size
/// 0x34  LE32  city_def_offset         0x38  LE32  city_def_size
/// 0x3C  LE16  city_def_record_size
/// 0x3E  LE32  poi_index_offset        0x42  LE32  poi_index_size
/// ...   (remaining sections zero-padded to 196 bytes)
/// ```
struct LblHeader {
    /// Total byte count of the label data section (including the 0x00 initial byte).
    data_size: u32,
}

impl LblHeader {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(LBL_HEADER_SIZE);
        // 0x00: Common header (21 bytes)
        buf.extend_from_slice(&build_common_header("LBL", LBL_HEADER_SIZE as u16));
        // 0x15: label_offset = LBL_HEADER_SIZE (LE32)
        buf.extend_from_slice(&(LBL_HEADER_SIZE as u32).to_le_bytes());
        // 0x19: label_size (LE32)
        buf.extend_from_slice(&self.data_size.to_le_bytes());
        // 0x1D: addr_offset_multiplier = 0 (u8)
        buf.push(0x00);
        // 0x1E: label_coding = 9 (8-bit CP1252) (u8)
        buf.push(0x09);
        // 0x1F–0xC3: section pointers (country, region, city, POI index, POI props,
        //            POI type, ZIP, highway, exit, highway_data).
        // Each section: offset(LE32) + size(LE32) + record_size(LE16) = 10 bytes.
        // Empty sections point to end of label data with size=0.
        // NOTE: No "driving_side" byte — format goes directly from label_coding to
        // country_def_offset at 0x1F per standard Garmin LBL layout.
        let empty_off = LBL_HEADER_SIZE as u32 + self.data_size;
        for _ in 0..10 {
            buf.extend_from_slice(&empty_off.to_le_bytes()); // offset
            buf.extend_from_slice(&0u32.to_le_bytes());      // size = 0
            buf.extend_from_slice(&0u16.to_le_bytes());      // record_size = 0
        }
        // Zero-pad to reach LBL_HEADER_SIZE
        buf.resize(LBL_HEADER_SIZE, 0u8);
        buf
    }
}

// ── LblWriter ──────────────────────────────────────────────────────────────────

/// Result of [`LblWriter::build`]: complete LBL binary and per-label offsets.
///
/// ## Offset semantics
///
/// `label_offsets` values are **data-section-relative** (i.e. relative to `data_offset`).
/// Byte 0 of the data section is the null sentinel (meaning "no label").
/// The first real label starts at data-section offset 1.
///
/// In the `data` Vec, the data section starts at byte 47 (the header size):
/// `data[47]` = 0x00 sentinel, `data[48]` = first byte of the first label, etc.
pub struct LblBuildResult {
    /// Complete LBL subfile binary: `[LblHeader (28 B) || label_data_section]`.
    pub data: Vec<u8>,
    /// Map from label string → byte offset **into the data section** (relative to `data_offset`).
    ///
    /// Offset 0 is reserved for "no label" (the initial 0x00 byte). The first real label
    /// starts at offset 1.
    pub label_offsets: HashMap<String, u32>,
}

/// Builds the LBL subfile binary from a parsed Polish Map.
pub struct LblWriter;

impl LblWriter {
    /// Build the complete LBL subfile binary and compute per-label offsets.
    ///
    /// The data section begins with a 0x00 sentinel (offset 0 = "no label").
    /// Each unique label is then appended as a CP1252-encoded, null-terminated string.
    /// Duplicate labels share the same offset (deduplication).
    ///
    /// Features are processed in order: points, polylines, polygons.
    pub fn build(mp: &MpFile) -> LblBuildResult {
        let mut data_section: Vec<u8> = vec![0x00]; // offset 0 = "no label"
        let mut label_offsets: HashMap<String, u32> = HashMap::new();

        // Iterate: points, polylines, polygons — in that order
        let all_labels: Vec<Option<&String>> = mp
            .points
            .iter()
            .map(|f| f.label.as_ref())
            .chain(mp.polylines.iter().map(|f| f.label.as_ref()))
            .chain(mp.polygons.iter().map(|f| f.label.as_ref()))
            .collect();

        for label in all_labels.into_iter().flatten() {
            if !label_offsets.contains_key(label.as_str()) {
                let offset = data_section.len() as u32;
                data_section.extend_from_slice(&encode_label_cp1252(label));
                label_offsets.insert(label.clone(), offset);
            }
        }

        let header = LblHeader {
            data_size: data_section.len() as u32,
        };
        let mut data = header.to_bytes();
        data.extend_from_slice(&data_section);

        LblBuildResult { data, label_offsets }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mp_types::{MpFile, MpHeader, MpPoint, MpPolygon, MpPolyline};
    use std::collections::HashMap;

    // ── Task 1: LBL Header ─────────────────────────────────────────────────────

    #[test]
    fn test_lbl_header_size() {
        let h = LblHeader { data_size: 0 };
        assert_eq!(h.to_bytes().len(), LBL_HEADER_SIZE, "LBL header must be exactly 47 bytes");
    }

    #[test]
    fn test_lbl_header_common_header() {
        let h = LblHeader { data_size: 42 };
        let bytes = h.to_bytes();
        // Common header: LE16(45) at 0x00, "GARMIN LBL" at 0x02
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), LBL_HEADER_SIZE as u16);
        assert_eq!(&bytes[0x02..0x0C], b"GARMIN LBL");
        // data_offset at 0x15
        assert_eq!(u32::from_le_bytes([bytes[0x15], bytes[0x16], bytes[0x17], bytes[0x18]]),
                   LBL_HEADER_SIZE as u32);
        // data_size = 42 at 0x19
        assert_eq!(u32::from_le_bytes([bytes[0x19], bytes[0x1A], bytes[0x1B], bytes[0x1C]]),
                   42, "data_size must be 42");
        // addr_offset_multiplier = 0 at 0x1D
        assert_eq!(bytes[0x1D], 0x00);
        // label_coding = 9 (8-bit CP1252) at 0x1E
        assert_eq!(bytes[0x1E], 0x09);
        // 0x1F: first section pointer (country_def_offset) — should point to end of label data
        let country_off = u32::from_le_bytes([bytes[0x1F], bytes[0x20], bytes[0x21], bytes[0x22]]);
        assert_eq!(country_off, LBL_HEADER_SIZE as u32 + 42, "country_def_offset = end of label data");
    }

    // ── Task 2: encode_label_cp1252 ────────────────────────────────────────────

    #[test]
    fn test_encode_ascii() {
        let encoded = encode_label_cp1252("Mairie");
        assert_eq!(
            encoded,
            &[0x4D, 0x61, 0x69, 0x72, 0x69, 0x65, 0x00],
            "ASCII 'Mairie' must encode directly with null terminator"
        );
    }

    #[test]
    fn test_encode_accented_eglise() {
        // É = U+00C9 → 0xC9 (Latin-1 direct)
        let encoded = encode_label_cp1252("Église");
        assert_eq!(
            encoded,
            &[0xC9, 0x67, 0x6C, 0x69, 0x73, 0x65, 0x00],
            "'Église' must encode É as 0xC9"
        );
    }

    #[test]
    fn test_encode_accented_foret() {
        // ê = U+00EA → 0xEA (Latin-1 direct)
        let encoded = encode_label_cp1252("Forêt");
        assert_eq!(
            encoded,
            &[0x46, 0x6F, 0x72, 0xEA, 0x74, 0x00],
            "'Forêt' must encode ê as 0xEA"
        );
    }

    #[test]
    fn test_encode_shield_code() {
        // "~[0x04]D1075" → [0x04, 0x44, 0x31, 0x30, 0x37, 0x35, 0x00]
        let encoded = encode_label_cp1252("~[0x04]D1075");
        assert_eq!(
            encoded,
            &[0x04, 0x44, 0x31, 0x30, 0x37, 0x35, 0x00],
            "shield prefix ~[0x04] must produce raw byte 0x04"
        );
    }

    // ── Task 3: LblWriter ──────────────────────────────────────────────────────

    fn make_mp_with_labels(labels: &[Option<&str>]) -> MpFile {
        let points = labels
            .iter()
            .enumerate()
            .map(|(i, lbl)| MpPoint {
                type_code: "0x2C00".to_string(),
                label: lbl.map(|s| s.to_string()),
                end_level: None,
                lat: 45.0 + i as f64 * 0.001,
                lon: 5.0,
                other_fields: HashMap::new(),
            })
            .collect();
        MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points,
            polylines: vec![],
            polygons: vec![],
        }
    }

    #[test]
    fn test_lbl_data_starts_with_null() {
        let mp = make_mp_with_labels(&[Some("Mairie")]);
        let result = LblWriter::build(&mp);
        // data_section starts at offset 28 (header size)
        assert_eq!(
            result.data[LBL_HEADER_SIZE],
            0x00,
            "first byte of label data section must be 0x00 (no-label sentinel)"
        );
    }

    #[test]
    fn test_lbl_first_label_offset_is_1() {
        let mp = make_mp_with_labels(&[Some("Mairie")]);
        let result = LblWriter::build(&mp);
        assert_eq!(
            result.label_offsets.get("Mairie").copied(),
            Some(1),
            "'Mairie' must be at data section offset 1"
        );
    }

    #[test]
    fn test_lbl_deduplication() {
        // Two POIs with the same label → same offset
        let mp = make_mp_with_labels(&[Some("D1075"), Some("D1075")]);
        let result = LblWriter::build(&mp);
        // Only one entry in the map
        assert_eq!(
            result.label_offsets.len(),
            1,
            "duplicate labels must produce a single entry"
        );
        let offset = result.label_offsets.get("D1075").copied().unwrap();
        assert_eq!(offset, 1, "deduplicated label offset must be 1");
    }

    #[test]
    fn test_lbl_offsets_sequential() {
        // "Mairie" (7 bytes with null) → offset 1; "D1075" (6 bytes) → offset 8
        let mp = make_mp_with_labels(&[Some("Mairie"), Some("D1075")]);
        let result = LblWriter::build(&mp);
        assert_eq!(result.label_offsets.get("Mairie").copied(), Some(1));
        assert_eq!(result.label_offsets.get("D1075").copied(), Some(8));
    }

    // ── parse_shield edge cases ────────────────────────────────────────────────

    #[test]
    fn test_parse_shield_valid() {
        // Standard 2-digit shield — must be accepted
        let result = parse_shield("~[0x04]Route");
        assert_eq!(result, Some((0x04u8, "Route")));
    }

    #[test]
    fn test_parse_shield_single_digit_rejected() {
        // Single hex digit ~[0x4] is not valid per spec (must be exactly 2 digits NN)
        let result = parse_shield("~[0x4]D1075");
        assert!(result.is_none(), "single-digit shield ~[0x4] must be rejected (spec requires 2 digits)");
    }

    #[test]
    fn test_parse_shield_no_prefix() {
        // No shield prefix → None
        assert!(parse_shield("D1075").is_none());
    }

    // ── Features with None labels ──────────────────────────────────────────────

    #[test]
    fn test_lbl_no_label_features_produce_empty_map() {
        // Features with label=None must not add anything to label_offsets or the data section.
        let mp = make_mp_with_labels(&[None, None]);
        let result = LblWriter::build(&mp);
        assert!(
            result.label_offsets.is_empty(),
            "features with None labels must not add entries to the offset map"
        );
        // Data section has only the null sentinel byte (1 byte) → total = header + 1
        assert_eq!(
            result.data.len(),
            LBL_HEADER_SIZE + 1,
            "LBL with no labels must be header + 1 null sentinel byte"
        );
        assert_eq!(result.data[LBL_HEADER_SIZE], 0x00, "null sentinel must be present at data section byte 0");
    }

    // ── CP1252 special range ────────────────────────────────────────────────────

    #[test]
    fn test_encode_oe_ligature() {
        // Œ = U+0152 → 0x8C (CP1252 special)
        let encoded = encode_label_cp1252("Œuvre");
        assert_eq!(encoded[0], 0x8C, "Œ must encode to CP1252 0x8C");
    }

    #[test]
    fn test_encode_oe_lowercase_ligature() {
        // œ = U+0153 → 0x9C (CP1252 special)
        let encoded = encode_label_cp1252("cœur");
        assert_eq!(encoded[1], 0x9C, "œ must encode to CP1252 0x9C");
    }

    // ── LblWriter with polylines and polygons ───────────────────────────────────

    #[test]
    fn test_lbl_build_multitype() {
        // Ensure polyline and polygon labels are also indexed
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                level_defs: vec![24],
                ..Default::default()
            },
            points: vec![],
            polylines: vec![MpPolyline {
                type_code: "0x06".to_string(),
                label: Some("D1075".to_string()),
                end_level: None,
                coords: vec![(45.0, 5.0), (45.1, 5.0)],
                routing: None,
                other_fields: HashMap::new(),
            }],
            polygons: vec![MpPolygon {
                type_code: "0x50".to_string(),
                label: Some("Foret".to_string()),
                end_level: None,
                coords: vec![(45.0, 5.0), (45.1, 5.0), (45.1, 5.1), (45.0, 5.0)],
                holes: vec![],
                other_fields: HashMap::new(),
            }],
        };
        let result = LblWriter::build(&mp);
        assert!(result.label_offsets.contains_key("D1075"));
        assert!(result.label_offsets.contains_key("Foret"));
        // D1075 offset = 1 (first label); Foret offset = 1 + 6 = 7
        assert_eq!(result.label_offsets.get("D1075").copied(), Some(1));
        assert_eq!(result.label_offsets.get("Foret").copied(), Some(7));
    }
}
