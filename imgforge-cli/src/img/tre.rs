//! TRE subfile writer — geographic index and subdivisions for Garmin IMG.
//!
//! The TRE subfile provides:
//! - A bounding box covering all features in the tile
//! - A list of zoom levels with their resolutions
//! - One subdivision per level covering the full tile (stub `rgn_offset = 0`)
//!
//! Format: `[TRE Header — 148 B] [Map Levels — N×4 B] [Subdivisions — N×16 B]`

use crate::parser::mp_types::{MpFile, MpHeader};

/// Convert WGS84 degrees to Garmin 24-bit coordinate units.
///
/// Formula: `round(deg × 2^24 / 360.0)`
///
/// Examples:
/// - `45.0°` → `2_097_152`
/// - `90.0°` → `4_194_304`
/// - `-180.0°` → `-8_388_608`
pub fn to_garmin_units(deg: f64) -> i32 {
    (deg * (1u32 << 24) as f64 / 360.0).round() as i32
}

/// Append 3 little-endian bytes (signed LE24) to `buf`.
fn write_le24(buf: &mut Vec<u8>, val: i32) {
    buf.push((val & 0xFF) as u8);
    buf.push(((val >> 8) & 0xFF) as u8);
    buf.push(((val >> 16) & 0xFF) as u8);
}

// ── TreHeader ─────────────────────────────────────────────────────────────────

/// TRE file header — exactly 148 bytes, version 3.
///
/// Binary layout (matches mkgmap `TREHeader.java`):
/// ```text
/// 0x00  LE16   header_length = 148
/// 0x02  LE16   version = 3
/// 0x04  LE24s  max_lat  (Garmin 24-bit units, signed)
/// 0x07  LE24s  max_lon
/// 0x0A  LE24s  min_lat
/// 0x0D  LE24s  min_lon
/// 0x10  LE32   flags = 0 (reserved)
/// 0x14  LE32   levels_offset  (= 148)
/// 0x18  LE32   levels_size    (= n × 4)
/// 0x1C  LE32   subdivisions_offset
/// 0x20  LE32   subdivisions_size  (= n × 16)
/// 0x24  LE32   copyright_offset
/// 0x28  LE32   copyright_size = 0
/// 0x2C  104B   zero padding to reach byte 148
/// ```
pub struct TreHeader {
    /// Maximum latitude in Garmin 24-bit units (signed).
    pub max_lat: i32,
    /// Maximum longitude in Garmin 24-bit units (signed).
    pub max_lon: i32,
    /// Minimum latitude in Garmin 24-bit units (signed).
    pub min_lat: i32,
    /// Minimum longitude in Garmin 24-bit units (signed).
    pub min_lon: i32,
    /// Byte offset of the Map Levels section (= 148).
    pub levels_offset: u32,
    /// Size in bytes of the Map Levels section (= n_levels × 4).
    pub levels_size: u32,
    /// Byte offset of the Subdivisions section.
    pub subdivisions_offset: u32,
    /// Size in bytes of the Subdivisions section (= n_levels × 16).
    pub subdivisions_size: u32,
    /// Byte offset of the Copyright section (immediately after subdivisions).
    pub copyright_offset: u32,
    /// Size in bytes of the Copyright section (= 0, unused).
    pub copyright_size: u32,
}

impl TreHeader {
    /// Serialise into exactly 148 bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(148);
        // 0x00: header_length = 148 (LE16)
        buf.extend_from_slice(&148u16.to_le_bytes());
        // 0x02: version = 3 (LE16)
        buf.extend_from_slice(&3u16.to_le_bytes());
        // 0x04: max_lat (LE24s)
        write_le24(&mut buf, self.max_lat);
        // 0x07: max_lon (LE24s)
        write_le24(&mut buf, self.max_lon);
        // 0x0A: min_lat (LE24s)
        write_le24(&mut buf, self.min_lat);
        // 0x0D: min_lon (LE24s)
        write_le24(&mut buf, self.min_lon);
        // 0x10: flags = 0 (LE32, reserved)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x14: levels_offset (LE32)
        buf.extend_from_slice(&self.levels_offset.to_le_bytes());
        // 0x18: levels_size (LE32)
        buf.extend_from_slice(&self.levels_size.to_le_bytes());
        // 0x1C: subdivisions_offset (LE32)
        buf.extend_from_slice(&self.subdivisions_offset.to_le_bytes());
        // 0x20: subdivisions_size (LE32)
        buf.extend_from_slice(&self.subdivisions_size.to_le_bytes());
        // 0x24: copyright_offset (LE32)
        buf.extend_from_slice(&self.copyright_offset.to_le_bytes());
        // 0x28: copyright_size (LE32)
        buf.extend_from_slice(&self.copyright_size.to_le_bytes());
        // 0x2C–0x93: zero padding to reach 148 bytes total (148 - 44 = 104 bytes)
        buf.resize(148, 0u8);
        buf
    }
}

// ── MapLevel ──────────────────────────────────────────────────────────────────

/// One zoom-level entry in the TRE Map Levels section (4 bytes).
///
/// Level 0 = most detailed zoom level (highest `bits_per_coord`).
pub struct MapLevel {
    /// Resolution: bits per coordinate (e.g. 24, 21, 18).
    /// Bit 7 = 0 for a normal indexed level; bit 7 = 1 for inherited/overview.
    pub bits_per_coord: u8,
    /// Level number (0 = most detailed).
    pub level: u8,
    /// Number of subdivisions at this level.
    pub subdivision_count: u16,
}

impl MapLevel {
    /// Serialise into exactly 4 bytes.
    pub fn to_bytes(&self) -> [u8; 4] {
        [
            self.bits_per_coord,
            self.level,
            (self.subdivision_count & 0xFF) as u8,
            ((self.subdivision_count >> 8) & 0xFF) as u8,
        ]
    }
}

/// Build the list of map levels from a parsed MP header.
///
/// If `header.level_defs` is non-empty, one [`MapLevel`] per entry is created
/// (values > 24 are clamped to 24). Otherwise, three default levels
/// `[24, 21, 18]` are returned.
pub fn levels_from_mp(header: &MpHeader) -> Vec<MapLevel> {
    if !header.level_defs.is_empty() {
        header
            .level_defs
            .iter()
            .enumerate()
            .map(|(i, &bits)| MapLevel {
                bits_per_coord: bits.min(24),
                level: i as u8,
                subdivision_count: 0,
            })
            .collect()
    } else {
        vec![
            MapLevel {
                bits_per_coord: 24,
                level: 0,
                subdivision_count: 0,
            },
            MapLevel {
                bits_per_coord: 21,
                level: 1,
                subdivision_count: 0,
            },
            MapLevel {
                bits_per_coord: 18,
                level: 2,
                subdivision_count: 0,
            },
        ]
    }
}

// ── Subdivision ───────────────────────────────────────────────────────────────

/// One geographic subdivision in the TRE Subdivisions section (16 bytes).
///
/// Each subdivision covers a rectangular area and references a RGN data offset.
/// For Story 13.3, `rgn_offset` is always 0 (stub); Story 13.4 will populate it.
///
/// Binary layout (matches mkgmap `Subdivision.java`):
/// ```text
/// 0x00  3B    rgn_offset (LE24)
/// 0x03  u8    data_flags:
///               bit 0 (0x01) = has_indexed_points
///               bit 1 (0x02) = has_indexed_lines (routing — always 0 until Epic 14)
///               bit 2 (0x04) = has_polylines
///               bit 3 (0x08) = has_polygons
/// 0x04  LE16s lon_center >> 8 (bits 8–23 of 24-bit coordinate)
/// 0x06  LE16s lat_center >> 8
/// 0x08  LE16  half_width >> 8 ; bit 15 SET if last_in_level
/// 0x0A  LE16  half_height >> 8
/// 0x0C  LE16  next_level_first_subdiv
/// 0x0E  LE16  reserved = 0
/// ```
pub struct Subdivision {
    /// Byte offset into the RGN subfile (LE24). Stub = 0 until Story 13.4.
    pub rgn_offset: u32,
    /// True if this subdivision contains indexed points (POI).
    pub has_points: bool,
    /// True if this subdivision contains indexed polylines.
    pub has_polylines: bool,
    /// True if this subdivision contains polygons.
    pub has_polygons: bool,
    /// Longitude of the subdivision centre in Garmin 24-bit units.
    pub lon_center: i32,
    /// Latitude of the subdivision centre in Garmin 24-bit units.
    pub lat_center: i32,
    /// Half-width of the bounding box in Garmin 24-bit units.
    pub half_width: u32,
    /// Half-height of the bounding box in Garmin 24-bit units.
    pub half_height: u32,
    /// True if this is the last subdivision at its zoom level.
    pub last_in_level: bool,
    /// 0-based index of the first subdivision at the next more-detailed level.
    /// 0 when this is already the most-detailed level.
    pub next_level_first_subdiv: u16,
}

impl Subdivision {
    /// Serialise into exactly 16 bytes.
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];

        // 0x00–0x02: rgn_offset (LE24)
        buf[0] = (self.rgn_offset & 0xFF) as u8;
        buf[1] = ((self.rgn_offset >> 8) & 0xFF) as u8;
        buf[2] = ((self.rgn_offset >> 16) & 0xFF) as u8;

        // 0x03: data_flags — Garmin format (mkgmap TREFile.java):
        //   bit 0 (0x01) = indexed points
        //   bit 1 (0x02) = indexed lines / routing (always 0 until Epic 14 NET/NOD)
        //   bit 2 (0x04) = polylines
        //   bit 3 (0x08) = polygons
        buf[3] = (self.has_points as u8)
            | ((self.has_polylines as u8) << 2)
            | ((self.has_polygons as u8) << 3);

        // 0x04–0x05: lon_center stored as bits 8–23 of the 24-bit value (LE16s)
        let lon_stored = (self.lon_center >> 8) as i16;
        let lon_bytes = lon_stored.to_le_bytes();
        buf[4] = lon_bytes[0];
        buf[5] = lon_bytes[1];

        // 0x06–0x07: lat_center stored as bits 8–23 (LE16s)
        let lat_stored = (self.lat_center >> 8) as i16;
        let lat_bytes = lat_stored.to_le_bytes();
        buf[6] = lat_bytes[0];
        buf[7] = lat_bytes[1];

        // 0x08–0x09: half_width (bits 8–23), bit 15 SET if last_in_level
        let mut hw = (self.half_width >> 8) as u16;
        if self.last_in_level {
            hw |= 0x8000;
        }
        let hw_bytes = hw.to_le_bytes();
        buf[8] = hw_bytes[0];
        buf[9] = hw_bytes[1];

        // 0x0A–0x0B: half_height (bits 8–23)
        let hh = (self.half_height >> 8) as u16;
        let hh_bytes = hh.to_le_bytes();
        buf[10] = hh_bytes[0];
        buf[11] = hh_bytes[1];

        // 0x0C–0x0D: next_level_first_subdiv (LE16)
        let nls_bytes = self.next_level_first_subdiv.to_le_bytes();
        buf[12] = nls_bytes[0];
        buf[13] = nls_bytes[1];

        // 0x0E–0x0F: reserved = 0 (already zero-initialised)

        buf
    }

    /// Build a subdivision covering the given bounding box.
    ///
    /// `bounds_g = (min_lat_g, max_lat_g, min_lon_g, max_lon_g)` in Garmin 24-bit units.
    /// `last` = true if this is the last subdivision at its zoom level.
    /// `next_idx` = 0-based index of the first subdivision at the next more-detailed level.
    pub fn compute_subdivision(
        bounds_g: (i32, i32, i32, i32),
        mp: &MpFile,
        last: bool,
        next_idx: u16,
    ) -> Self {
        let (min_lat_g, max_lat_g, min_lon_g, max_lon_g) = bounds_g;

        let lat_center = (max_lat_g + min_lat_g) / 2;
        let lon_center = (max_lon_g + min_lon_g) / 2;
        // +1 ensures half_height/half_width >= 1 even for a single-point tile (max == min).
        // The `(max - min)` is always >= 0 because compute_bounds guarantees max >= min.
        let half_height = ((max_lat_g - min_lat_g) / 2 + 1) as u32;
        let half_width = ((max_lon_g - min_lon_g) / 2 + 1) as u32;

        Self {
            rgn_offset: 0,
            has_points: !mp.points.is_empty(),
            has_polylines: !mp.polylines.is_empty(),
            has_polygons: !mp.polygons.is_empty(),
            lon_center,
            lat_center,
            half_width,
            half_height,
            last_in_level: last,
            next_level_first_subdiv: next_idx,
        }
    }
}

// ── TreWriter ─────────────────────────────────────────────────────────────────

/// Builds the TRE (geographic index) subfile binary from a parsed Polish Map.
pub struct TreWriter;

impl TreWriter {
    /// Compute the bounding box of all features in `mp` (degrees WGS84).
    ///
    /// Returns `(min_lat, max_lat, min_lon, max_lon)`.
    /// If the file contains no features, logs a warning and returns `(0.0, 0.0, 0.0, 0.0)`.
    pub fn compute_bounds(mp: &MpFile) -> (f64, f64, f64, f64) {
        let mut min_lat = f64::MAX;
        let mut max_lat = f64::MIN;
        let mut min_lon = f64::MAX;
        let mut max_lon = f64::MIN;

        for p in &mp.points {
            min_lat = min_lat.min(p.lat);
            max_lat = max_lat.max(p.lat);
            min_lon = min_lon.min(p.lon);
            max_lon = max_lon.max(p.lon);
        }

        for line in &mp.polylines {
            for &(lat, lon) in &line.coords {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
        }

        for poly in &mp.polygons {
            for &(lat, lon) in &poly.coords {
                min_lat = min_lat.min(lat);
                max_lat = max_lat.max(lat);
                min_lon = min_lon.min(lon);
                max_lon = max_lon.max(lon);
            }
            for ring in &poly.holes {
                for &(lat, lon) in ring {
                    min_lat = min_lat.min(lat);
                    max_lat = max_lat.max(lat);
                    min_lon = min_lon.min(lon);
                    max_lon = max_lon.max(lon);
                }
            }
        }

        if min_lat == f64::MAX {
            tracing::warn!("TRE: empty MpFile — using zero bounding box");
            return (0.0, 0.0, 0.0, 0.0);
        }

        (min_lat, max_lat, min_lon, max_lon)
    }

    /// Build the complete TRE subfile binary from a parsed Polish Map.
    ///
    /// Output size: `148 + n_levels * 4 + n_levels * 16` bytes.
    pub fn build(mp: &MpFile) -> Vec<u8> {
        // Step 1: bounding box in degrees
        let (min_lat, max_lat, min_lon, max_lon) = Self::compute_bounds(mp);

        // Step 2: convert to Garmin 24-bit units
        let min_lat_g = to_garmin_units(min_lat);
        let max_lat_g = to_garmin_units(max_lat);
        let min_lon_g = to_garmin_units(min_lon);
        let max_lon_g = to_garmin_units(max_lon);

        // Step 3: build levels from header (or defaults)
        let mut levels = levels_from_mp(&mp.header);
        let n = levels.len();

        // Step 4: compute section byte offsets
        let levels_offset = 148u32;
        let levels_size = n as u32 * 4;
        let subdivisions_offset = levels_offset + levels_size;
        let subdivisions_size = n as u32 * 16;
        let copyright_offset = subdivisions_offset + subdivisions_size;
        let copyright_size = 0u32;

        // Step 5: create one subdivision per level, all covering the full bounding box
        let bounds_g = (min_lat_g, max_lat_g, min_lon_g, max_lon_g);
        let subdivisions: Vec<Subdivision> = (0..n)
            .map(|i| {
                // Level 0 is the most detailed — no finer child level (next_idx = 0).
                // Level i > 0 points to the subdivision of the more-detailed level i-1.
                let next_idx = if i == 0 { 0u16 } else { (i - 1) as u16 };
                Subdivision::compute_subdivision(bounds_g, mp, true, next_idx)
            })
            .collect();

        // Step 6: each level has exactly one subdivision
        for level in &mut levels {
            level.subdivision_count = 1;
        }

        // Step 7: assemble header
        let header = TreHeader {
            max_lat: max_lat_g,
            max_lon: max_lon_g,
            min_lat: min_lat_g,
            min_lon: min_lon_g,
            levels_offset,
            levels_size,
            subdivisions_offset,
            subdivisions_size,
            copyright_offset,
            copyright_size,
        };

        // Step 8: serialise header + levels + subdivisions
        let mut out = header.to_bytes();
        for level in &levels {
            out.extend_from_slice(&level.to_bytes());
        }
        for subdiv in &subdivisions {
            out.extend_from_slice(&subdiv.to_bytes());
        }

        out
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mp_types::{MpFile, MpHeader, MpPoint, MpPolygon};
    use std::collections::HashMap;

    fn empty_mp() -> MpFile {
        MpFile {
            header: MpHeader::default(),
            points: vec![],
            polylines: vec![],
            polygons: vec![],
        }
    }

    fn fixture_mp() -> MpFile {
        use crate::parser::MpParser;
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("minimal_for_img.mp");
        MpParser::parse_file(&path).unwrap()
    }

    // ── Task 1: to_garmin_units ───────────────────────────────────────────────

    #[test]
    fn test_garmin_units_zero() {
        assert_eq!(to_garmin_units(0.0), 0);
    }

    #[test]
    fn test_garmin_units_lat_45() {
        assert_eq!(to_garmin_units(45.0), 2_097_152);
    }

    #[test]
    fn test_garmin_units_lon_neg180() {
        assert_eq!(to_garmin_units(-180.0), -8_388_608);
    }

    #[test]
    fn test_garmin_units_lat_90() {
        assert_eq!(to_garmin_units(90.0), 4_194_304);
    }

    #[test]
    fn test_garmin_units_roundtrip() {
        let g = to_garmin_units(6.123);
        let back = g as f64 * 360.0 / (1u32 << 24) as f64;
        assert!(
            (back - 6.123).abs() < 0.00003,
            "roundtrip error too large: {:.8}°",
            (back - 6.123).abs()
        );
    }

    // ── Task 1: write_le24 ────────────────────────────────────────────────────

    #[test]
    fn test_write_le24_positive() {
        let mut buf = Vec::new();
        write_le24(&mut buf, 0x010203);
        assert_eq!(buf, vec![0x03, 0x02, 0x01]);
    }

    #[test]
    fn test_write_le24_negative() {
        let mut buf = Vec::new();
        write_le24(&mut buf, -1);
        assert_eq!(buf, vec![0xFF, 0xFF, 0xFF]);
    }

    // ── Task 2: TreHeader ─────────────────────────────────────────────────────

    fn default_header() -> TreHeader {
        TreHeader {
            max_lat: 0,
            max_lon: 0,
            min_lat: 0,
            min_lon: 0,
            levels_offset: 148,
            levels_size: 12,
            subdivisions_offset: 160,
            subdivisions_size: 48,
            copyright_offset: 208,
            copyright_size: 0,
        }
    }

    #[test]
    fn test_tre_header_size() {
        assert_eq!(default_header().to_bytes().len(), 148);
    }

    #[test]
    fn test_tre_header_version_magic() {
        let bytes = default_header().to_bytes();
        assert_eq!(&bytes[0..4], &[0x94, 0x00, 0x03, 0x00]);
    }

    #[test]
    fn test_tre_header_levels_offset() {
        let bytes = default_header().to_bytes();
        let offset = u32::from_le_bytes([bytes[0x14], bytes[0x15], bytes[0x16], bytes[0x17]]);
        assert_eq!(offset, 148);
    }

    #[test]
    fn test_tre_header_offsets_coherent() {
        let h = default_header();
        assert_eq!(h.subdivisions_offset, h.levels_offset + h.levels_size);
    }

    #[test]
    fn test_tre_header_padding() {
        let bytes = default_header().to_bytes();
        // All bytes from 0x2C (44) to end must be 0x00
        assert!(
            bytes[0x2C..].iter().all(|&b| b == 0x00),
            "header padding must be all zeros"
        );
    }

    // ── Task 3: MapLevel ──────────────────────────────────────────────────────

    #[test]
    fn test_map_level_bytes_basic() {
        let level = MapLevel {
            bits_per_coord: 24,
            level: 0,
            subdivision_count: 1,
        };
        assert_eq!(level.to_bytes(), [24, 0, 1, 0]);
    }

    #[test]
    fn test_map_level_bytes_count_gt255() {
        let level = MapLevel {
            bits_per_coord: 21,
            level: 1,
            subdivision_count: 300,
        };
        let bytes = level.to_bytes();
        let count = u16::from_le_bytes([bytes[2], bytes[3]]);
        assert_eq!(count, 300);
    }

    #[test]
    fn test_levels_from_mp_explicit() {
        let header = MpHeader {
            level_defs: vec![24, 21, 18],
            ..Default::default()
        };
        let levels = levels_from_mp(&header);
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0].bits_per_coord, 24);
        assert_eq!(levels[1].bits_per_coord, 21);
        assert_eq!(levels[2].bits_per_coord, 18);
        assert_eq!(levels[0].level, 0);
        assert_eq!(levels[1].level, 1);
        assert_eq!(levels[2].level, 2);
    }

    #[test]
    fn test_levels_from_mp_default() {
        let header = MpHeader::default();
        let levels = levels_from_mp(&header);
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0].bits_per_coord, 24);
        assert_eq!(levels[1].bits_per_coord, 21);
        assert_eq!(levels[2].bits_per_coord, 18);
    }

    #[test]
    fn test_levels_from_mp_single() {
        let header = MpHeader {
            level_defs: vec![24],
            ..Default::default()
        };
        let levels = levels_from_mp(&header);
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].bits_per_coord, 24);
        assert_eq!(levels[0].level, 0);
    }

    // ── Task 4: Subdivision ───────────────────────────────────────────────────

    #[test]
    fn test_subdivision_size() {
        let mp = empty_mp();
        let s = Subdivision::compute_subdivision((0, 0, 0, 0), &mp, false, 0);
        assert_eq!(s.to_bytes().len(), 16);
    }

    #[test]
    fn test_subdivision_data_flags() {
        let mut mp = empty_mp();
        // has_points = true, has_polylines = false, has_polygons = true
        mp.points.push(MpPoint {
            type_code: "0x2C00".to_string(),
            label: None,
            end_level: None,
            lat: 45.0,
            lon: 5.0,
            other_fields: HashMap::new(),
        });
        mp.polygons.push(MpPolygon {
            type_code: "0x50".to_string(),
            label: None,
            end_level: None,
            coords: vec![
                (45.0, 5.0),
                (45.1, 5.0),
                (45.1, 5.1),
                (45.0, 5.1),
                (45.0, 5.0),
            ],
            holes: vec![],
            other_fields: HashMap::new(),
        });
        let s = Subdivision::compute_subdivision((0, 1, 0, 1), &mp, false, 0);
        let bytes = s.to_bytes();
        // has_points=true → bit 0 (0x01); has_polygons=true → bit 3 (0x08) → 0x09
        assert_eq!(
            bytes[0x03], 0x09,
            "data_flags: points=1, polylines=0, polygons=1 → 0x09 (bit0 | bit3)"
        );
    }

    #[test]
    fn test_subdivision_last_flag() {
        let mp = empty_mp();
        let s = Subdivision::compute_subdivision((0, 100, 0, 100), &mp, true, 0);
        let bytes = s.to_bytes();
        let hw = u16::from_le_bytes([bytes[8], bytes[9]]);
        assert!(
            hw & 0x8000 != 0,
            "last_in_level must set bit 15 of half_width field"
        );
    }

    #[test]
    fn test_subdivision_rgn_zero() {
        let mp = empty_mp();
        let s = Subdivision::compute_subdivision((0, 100, 0, 100), &mp, false, 0);
        let bytes = s.to_bytes();
        assert_eq!(&bytes[0..3], &[0, 0, 0], "stub rgn_offset must be zero");
    }

    #[test]
    fn test_subdivision_center_encoded() {
        // lat range 44°–46° → lat_center ≈ 45° (2_097_152 garmin units)
        // stored as (2097152 >> 8) as i16 = 8192
        let mp = empty_mp();
        let lat_lo = to_garmin_units(44.0); // 2_050_048
        let lat_hi = to_garmin_units(46.0); // 2_144_256
        let s = Subdivision::compute_subdivision((lat_lo, lat_hi, 0, 256), &mp, false, 0);
        let bytes = s.to_bytes();
        let lat_center_g = (lat_hi + lat_lo) / 2;
        let expected = (lat_center_g >> 8) as i16;
        let lat_stored = i16::from_le_bytes([bytes[6], bytes[7]]);
        assert_eq!(lat_stored, expected);
    }

    // ── Task 5: TreWriter ─────────────────────────────────────────────────────

    #[test]
    fn test_tre_build_minimal() {
        // minimal_for_img.mp: Level0=24, Level1=18 → 2 explicit levels
        // Size = 148 + 2×4 + 2×16 = 188 bytes
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        assert_eq!(data.len(), 188, "2-level TRE must be 188 bytes");
    }

    #[test]
    fn test_tre_build_header_magic() {
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        assert_eq!(&data[0..4], &[0x94, 0x00, 0x03, 0x00]);
    }

    #[test]
    fn test_tre_build_levels_offset() {
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        let offset = u32::from_le_bytes([data[0x14], data[0x15], data[0x16], data[0x17]]);
        assert_eq!(offset, 148);
    }

    #[test]
    fn test_tre_build_bounds_positive() {
        // minimal_for_img.mp has a POI at lat ≈ 45.19° → max_lat_g > 0
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        // max_lat at bytes[0x04..0x07] (LE24s)
        let raw = (data[0x04] as i32) | ((data[0x05] as i32) << 8) | ((data[0x06] as i32) << 16);
        let max_lat_g = if raw & 0x80_0000 != 0 {
            raw | !0xFF_FFFF
        } else {
            raw
        };
        assert!(
            max_lat_g > 0,
            "France tile max_lat garmin must be > 0, got {}",
            max_lat_g
        );
    }

    #[test]
    fn test_tre_build_empty_file() {
        // Empty MpFile → no features, zero bbox, 3 default levels
        let mp = empty_mp();
        let data = TreWriter::build(&mp);
        // Must not panic; valid TRE: 148 + 3×4 + 3×16 = 208 bytes
        assert_eq!(data.len(), 208);
        assert_eq!(&data[0..4], &[0x94, 0x00, 0x03, 0x00]);
    }

    #[test]
    fn test_tre_build_three_levels_default() {
        // MpFile with no level_defs → 3 default levels [24, 21, 18]
        let mp = MpFile {
            header: MpHeader {
                id: "63240001".to_string(),
                ..Default::default()
            },
            points: vec![],
            polylines: vec![],
            polygons: vec![],
        };
        let data = TreWriter::build(&mp);
        // 148 + 3×4 + 3×16 = 208 bytes
        assert_eq!(data.len(), 208, "default 3-level TRE must be 208 bytes");
    }

    // ── Task 3 (M2): clamping bits_per_coord > 24 ────────────────────────────

    #[test]
    fn test_levels_from_mp_clamp_gt24() {
        // level_defs = [25, 24] → bits_per_coord must be clamped to 24
        let header = MpHeader {
            level_defs: vec![25, 24],
            ..Default::default()
        };
        let levels = levels_from_mp(&header);
        assert_eq!(levels.len(), 2);
        assert_eq!(
            levels[0].bits_per_coord, 24,
            "bits_per_coord > 24 must be clamped to 24"
        );
        assert_eq!(levels[1].bits_per_coord, 24);
    }

    // ── Task 5 (L1): compute_bounds with polylines only ───────────────────────

    #[test]
    fn test_compute_bounds_polylines_only() {
        use crate::parser::mp_types::MpPolyline;
        let mut mp = empty_mp();
        mp.polylines.push(MpPolyline {
            type_code: "0x01".to_string(),
            label: None,
            end_level: None,
            coords: vec![(44.0, 5.0), (46.0, 7.0)],
            routing: None,
            other_fields: HashMap::new(),
        });
        let (min_lat, max_lat, min_lon, max_lon) = TreWriter::compute_bounds(&mp);
        assert!((min_lat - 44.0).abs() < 1e-9, "min_lat from polyline");
        assert!((max_lat - 46.0).abs() < 1e-9, "max_lat from polyline");
        assert!((min_lon - 5.0).abs() < 1e-9, "min_lon from polyline");
        assert!((max_lon - 7.0).abs() < 1e-9, "max_lon from polyline");
    }
}
