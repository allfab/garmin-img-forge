//! TRE subfile writer — geographic index and subdivisions for Garmin IMG.
//!
//! The TRE subfile provides:
//! - A bounding box covering all features in the tile
//! - A list of zoom levels with their resolutions
//! - One subdivision per level covering the full tile (stub `rgn_offset = 0`)
//!
//! Format: `[TRE Header — 167 B] [Map Levels — N×4 B] [Subdivisions — N×16 B]`

use crate::img::common_header::{build_common_header, COMMON_HEADER_SIZE};
use crate::parser::mp_types::{MpFile, MpHeader};

/// Standard Garmin TRE header size (188 bytes), matching mkgmap/OTM reference.
/// Layout: 21-byte common header + 167-byte type-specific (bbox, levels, subdivisions,
/// copyright, and extended section pointers zero-padded).
/// No version field at 0x15 — bounding box starts directly after common header.
const TRE_HEADER_SIZE: usize = 188;

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

/// TRE file header — exactly 167 bytes (21-byte common header + 146-byte type-specific).
///
/// Binary layout:
/// ```text
/// ── Common header (21 bytes) ──
/// 0x00  LE16   header_length = 167
/// 0x02  10B    "GARMIN TRE" signature
/// 0x0C  u8     version = 1
/// 0x0D  u8     lock = 0
/// 0x0E  7B     creation date
/// ── Type-specific (146 bytes, starts at 0x15) ──
/// 0x15  LE24s  max_lat  (Garmin 24-bit units, signed)
/// 0x18  LE24s  max_lon
/// 0x1B  LE24s  min_lat
/// 0x1E  LE24s  min_lon
/// 0x21  LE32   levels_offset  (= 188)
/// 0x25  LE32   levels_size    (= n × 4)
/// 0x29  LE32   subdivisions_offset
/// 0x2D  LE32   subdivisions_size  (= n × 16)
/// 0x31  LE32   copyright_offset
/// 0x35  LE32   copyright_size = 0
/// 0x39  ..     zero padding to reach byte 188
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
    /// Byte offset of the Map Levels section (= 167).
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
    /// Serialise into exactly 188 bytes (21-byte common header + 167-byte type-specific).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TRE_HEADER_SIZE);
        // 0x00: Common header (21 bytes)
        buf.extend_from_slice(&build_common_header("TRE", TRE_HEADER_SIZE as u16));
        // 0x15: max_lat (LE24s) — bounding box starts directly after common header
        write_le24(&mut buf, self.max_lat);
        // 0x18: max_lon (LE24s)
        write_le24(&mut buf, self.max_lon);
        // 0x1B: min_lat (LE24s)
        write_le24(&mut buf, self.min_lat);
        // 0x1E: min_lon (LE24s)
        write_le24(&mut buf, self.min_lon);
        // 0x21: levels_offset (LE32)
        buf.extend_from_slice(&self.levels_offset.to_le_bytes());
        // 0x25: levels_size (LE32)
        buf.extend_from_slice(&self.levels_size.to_le_bytes());
        // 0x29: subdivisions_offset (LE32)
        buf.extend_from_slice(&self.subdivisions_offset.to_le_bytes());
        // 0x2D: subdivisions_size (LE32)
        buf.extend_from_slice(&self.subdivisions_size.to_le_bytes());
        // 0x31: copyright_offset (LE32)
        buf.extend_from_slice(&self.copyright_offset.to_le_bytes());
        // 0x35: copyright_size (LE32)
        buf.extend_from_slice(&self.copyright_size.to_le_bytes());
        // 0x39–0xBB: extended section pointers (zero-padded to 188 bytes)
        buf.resize(TRE_HEADER_SIZE, 0u8);
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
    /// True if this subdivision contains indexed/routable lines (NET routing).
    pub has_indexed_lines: bool,
    /// True if this subdivision contains indexed polylines.
    pub has_polylines: bool,
    /// True if this subdivision contains polygons.
    pub has_polygons: bool,
    /// True if this subdivision contains extended POI types (>0xFFFF).
    pub has_extended_points: bool,
    /// True if this subdivision contains extended polyline types (>0xFFFF).
    pub has_extended_polylines: bool,
    /// True if this subdivision contains extended polygon types (>0xFFFF).
    pub has_extended_polygons: bool,
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
    /// Serialise into 14 bytes (most detailed level) or 16 bytes (overview levels).
    ///
    /// In Garmin format, the most detailed level omits the `next_level_first_subdiv`
    /// and reserved fields (last 4 bytes). QMapShack requires this distinction.
    pub fn to_bytes_sized(&self, is_most_detailed_level: bool) -> Vec<u8> {
        let size = if is_most_detailed_level { 14 } else { 16 };
        let mut buf = vec![0u8; size];
        self.write_common(&mut buf);
        if !is_most_detailed_level {
            // 0x0C–0x0D: next_level_first_subdiv (LE16)
            let nls_bytes = self.next_level_first_subdiv.to_le_bytes();
            buf[12] = nls_bytes[0];
            buf[13] = nls_bytes[1];
            // 0x0E–0x0F: reserved = 0
        }
        buf
    }

    /// Serialise into exactly 16 bytes (legacy — used by tests).
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        self.write_common(&mut buf);
        let nls_bytes = self.next_level_first_subdiv.to_le_bytes();
        buf[12] = nls_bytes[0];
        buf[13] = nls_bytes[1];
        buf
    }

    /// Write the first 12 bytes common to both 14-byte and 16-byte subdivisions.
    fn write_common(&self, buf: &mut [u8]) {
        // 0x00–0x02: rgn_offset (LE24)
        buf[0] = (self.rgn_offset & 0xFF) as u8;
        buf[1] = ((self.rgn_offset >> 8) & 0xFF) as u8;
        buf[2] = ((self.rgn_offset >> 16) & 0xFF) as u8;

        // 0x03: data_flags
        buf[3] = (self.has_points as u8)
            | ((self.has_indexed_lines as u8) << 1)
            | ((self.has_polylines as u8) << 2)
            | ((self.has_polygons as u8) << 3)
            | ((self.has_extended_points as u8) << 4)
            | ((self.has_extended_polylines as u8) << 5)
            | ((self.has_extended_polygons as u8) << 6);

        // 0x04–0x05: lon_center (bits 8–23, LE16s)
        let lon_bytes = ((self.lon_center >> 8) as i16).to_le_bytes();
        buf[4] = lon_bytes[0];
        buf[5] = lon_bytes[1];

        // 0x06–0x07: lat_center (bits 8–23, LE16s)
        let lat_bytes = ((self.lat_center >> 8) as i16).to_le_bytes();
        buf[6] = lat_bytes[0];
        buf[7] = lat_bytes[1];

        // 0x08–0x09: half_width (bits 8–23), bit 15 = last_in_level
        let mut hw = (self.half_width >> 8) as u16;
        if self.last_in_level {
            hw |= 0x8000;
        }
        let hw_bytes = hw.to_le_bytes();
        buf[8] = hw_bytes[0];
        buf[9] = hw_bytes[1];

        // 0x0A–0x0B: half_height (bits 8–23)
        let hh_bytes = ((self.half_height >> 8) as u16).to_le_bytes();
        buf[10] = hh_bytes[0];
        buf[11] = hh_bytes[1];
    }

    /// Build a subdivision covering the given bounding box.
    ///
    /// `bounds_g = (min_lat_g, max_lat_g, min_lon_g, max_lon_g)` in Garmin 24-bit units.
    /// `has_points/has_polylines/has_polygons` = whether those feature types exist **at this level**
    /// (callers must filter by EndLevel before computing these flags).
    /// `last` = true if this is the last subdivision at its zoom level.
    /// `next_idx` = 0-based index of the first subdivision at the next more-detailed level.
    pub fn compute_subdivision(
        bounds_g: (i32, i32, i32, i32),
        has_points: bool,
        has_polylines: bool,
        has_polygons: bool,
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
            has_points,
            has_indexed_lines: false,
            has_polylines,
            has_polygons,
            has_extended_points: false,
            has_extended_polylines: false,
            has_extended_polygons: false,
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
    /// All subdivision `rgn_offset` fields are set to 0 (stub).
    /// Use [`TreWriter::build_with_rgn_offsets`] to inject real RGN offsets.
    ///
    /// Output size: `167 + n_levels * 4 + n_levels * 16` bytes.
    pub fn build(mp: &MpFile) -> Vec<u8> {
        Self::build_with_rgn_offsets(mp, &[])
    }

    /// Build the TRE subfile binary with pre-computed RGN subdivision offsets.
    ///
    /// `rgn_offsets[i]` is the byte offset of level `i`'s data within the RGN
    /// feature data section (i.e. relative to the start of the data section, not
    /// the start of the RGN file). If `rgn_offsets` is shorter than `n_levels`,
    /// remaining subdivisions keep `rgn_offset = 0`.
    ///
    /// Output size: `167 + n_levels * 4 + n_levels * 16` bytes.
    pub fn build_with_rgn_offsets(mp: &MpFile, rgn_offsets: &[u32]) -> Vec<u8> {
        Self::build_tre_inner(mp, rgn_offsets, false, None)
    }

    /// Build the TRE subfile with routing flag (bit 1 = has_indexed_lines) active.
    ///
    /// Same as [`build_with_rgn_offsets`] but sets the `has_indexed_lines` flag
    /// on subdivisions that contain routable polylines. This tells the GPS that
    /// NET cross-references are present in the RGN data.
    pub fn build_with_rgn_offsets_and_routing(mp: &MpFile, rgn_offsets: &[u32]) -> Vec<u8> {
        Self::build_tre_inner(mp, rgn_offsets, true, None)
    }

    /// Build the TRE subfile from a complete `RgnBuildResult`, propagating
    /// extended type flags (bits 4-6 in subdivision data_flags).
    ///
    /// `routing`: whether the map has routing data (NET/NOD subfiles). This must match
    /// the caller's `has_routing` flag — do not infer from `subdiv_road_refs` alone.
    pub fn build_with_rgn_result(
        mp: &MpFile,
        rgn_result: &crate::img::rgn::RgnBuildResult,
        routing: bool,
    ) -> Vec<u8> {
        Self::build_tre_inner(
            mp,
            &rgn_result.subdivision_offsets,
            routing,
            Some(rgn_result),
        )
    }

    /// Shared implementation for TRE building with optional routing and extended type support.
    fn build_tre_inner(
        mp: &MpFile,
        rgn_offsets: &[u32],
        routing: bool,
        rgn_result: Option<&crate::img::rgn::RgnBuildResult>,
    ) -> Vec<u8> {
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
        // Garmin format: most detailed level (0) uses 14-byte subdivisions,
        // all overview levels use 16-byte subdivisions.
        let levels_offset = TRE_HEADER_SIZE as u32;
        let levels_size = n as u32 * 4;
        let subdivisions_offset = levels_offset + levels_size;
        let subdivisions_size = if n > 0 {
            14u32 + (n as u32 - 1) * 16 // level 0: 14B, others: 16B each
        } else {
            0u32
        };
        let copyright_offset = subdivisions_offset + subdivisions_size;
        let copyright_size = 0u32;

        // Step 5: create one subdivision per level, all covering the full bounding box.
        // Feature-presence flags are computed per-level (filtered by EndLevel) so that the
        // GPS firmware does not attempt to read feature data that is absent at a given zoom.
        let bounds_g = (min_lat_g, max_lat_g, min_lon_g, max_lon_g);
        let subdivisions: Vec<Subdivision> = (0..n)
            .map(|i| {
                // Level 0 is the most detailed — no finer child level (next_idx = 0).
                // Level i > 0 points to the subdivision of level i-1 (1-based index).
                // With 1 subdivision per level: subdiv for level j is at 1-based index j+1.
                let next_idx = if i == 0 { 0u16 } else { i as u16 };
                // Level-aware flags: a feature is present at level i iff end_level >= i.
                let has_points = mp
                    .points
                    .iter()
                    .any(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8);
                let has_polylines = mp
                    .polylines
                    .iter()
                    .any(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8);
                let has_polygons = mp
                    .polygons
                    .iter()
                    .any(|f| f.end_level.unwrap_or(u8::MAX) >= i as u8);
                // Routing: set has_indexed_lines when routing is active and routable polylines
                // are visible at this level.
                let has_indexed_lines = routing
                    && mp
                        .polylines
                        .iter()
                        .any(|f| {
                            f.routing.is_some()
                                && f.end_level.unwrap_or(u8::MAX) >= i as u8
                        });
                let mut s = Subdivision::compute_subdivision(
                    bounds_g,
                    has_points,
                    has_polylines,
                    has_polygons,
                    true,
                    next_idx,
                );
                s.has_indexed_lines = has_indexed_lines;
                if let Some(rr) = rgn_result {
                    if i < rr.subdiv_has_extended_points.len() {
                        s.has_extended_points = rr.subdiv_has_extended_points[i];
                    }
                    if i < rr.subdiv_has_extended_polylines.len() {
                        s.has_extended_polylines = rr.subdiv_has_extended_polylines[i];
                    }
                    if i < rr.subdiv_has_extended_polygons.len() {
                        s.has_extended_polygons = rr.subdiv_has_extended_polygons[i];
                    }
                }
                if i < rgn_offsets.len() {
                    s.rgn_offset = rgn_offsets[i];
                }
                s
            })
            .collect();

        // Step 6: each level has exactly one subdivision.
        // Set INHERITED flag (bit 7) on the last level (overview) — required by QMapShack.
        for (i, level) in levels.iter_mut().enumerate() {
            level.subdivision_count = 1;
            if i == n - 1 {
                level.bits_per_coord |= 0x80; // INHERITED flag on overview level
            }
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
        for (i, subdiv) in subdivisions.iter().enumerate() {
            let is_most_detailed = i == 0; // level 0 = most detailed
            out.extend_from_slice(&subdiv.to_bytes_sized(is_most_detailed));
        }

        out
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::mp_types::{MpFile, MpHeader};
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
            levels_offset: TRE_HEADER_SIZE as u32,    // 167
            levels_size: 12,
            subdivisions_offset: TRE_HEADER_SIZE as u32 + 12, // 179
            subdivisions_size: 48,
            copyright_offset: TRE_HEADER_SIZE as u32 + 12 + 48, // 227
            copyright_size: 0,
        }
    }

    #[test]
    fn test_tre_header_size() {
        assert_eq!(default_header().to_bytes().len(), TRE_HEADER_SIZE);
    }

    #[test]
    fn test_tre_header_common_header_signature() {
        let bytes = default_header().to_bytes();
        // Common header: LE16(165) at 0x00, "GARMIN TRE" at 0x02
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), TRE_HEADER_SIZE as u16);
        assert_eq!(&bytes[0x02..0x0C], b"GARMIN TRE");
        // Bounding box starts at 0x15 (no version field)
        assert_eq!(bytes.len(), TRE_HEADER_SIZE);
    }

    #[test]
    fn test_tre_header_levels_offset() {
        let bytes = default_header().to_bytes();
        let offset = u32::from_le_bytes([bytes[0x21], bytes[0x22], bytes[0x23], bytes[0x24]]);
        assert_eq!(offset, TRE_HEADER_SIZE as u32);
    }

    #[test]
    fn test_tre_header_offsets_coherent() {
        let h = default_header();
        assert_eq!(h.subdivisions_offset, h.levels_offset + h.levels_size);
    }

    #[test]
    fn test_tre_header_padding() {
        let bytes = default_header().to_bytes();
        // All bytes from 0x39 (57 = 21 common + 12 bbox + 24 section ptrs)
        // to end must be 0x00
        assert!(
            bytes[0x39..].iter().all(|&b| b == 0x00),
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
        let s = Subdivision::compute_subdivision((0, 0, 0, 0), false, false, false, false, 0);
        assert_eq!(s.to_bytes().len(), 16);
    }

    #[test]
    fn test_subdivision_data_flags() {
        // has_points = true, has_polylines = false, has_polygons = true → flags = 0x09
        let s = Subdivision::compute_subdivision((0, 1, 0, 1), true, false, true, false, 0);
        let bytes = s.to_bytes();
        assert_eq!(
            bytes[0x03], 0x09,
            "data_flags: points=1, polylines=0, polygons=1 → 0x09 (bit0 | bit3)"
        );
    }

    #[test]
    fn test_subdivision_last_flag() {
        let s = Subdivision::compute_subdivision((0, 100, 0, 100), false, false, false, true, 0);
        let bytes = s.to_bytes();
        let hw = u16::from_le_bytes([bytes[8], bytes[9]]);
        assert!(
            hw & 0x8000 != 0,
            "last_in_level must set bit 15 of half_width field"
        );
    }

    #[test]
    fn test_subdivision_rgn_zero() {
        let s = Subdivision::compute_subdivision((0, 100, 0, 100), false, false, false, false, 0);
        let bytes = s.to_bytes();
        assert_eq!(&bytes[0..3], &[0, 0, 0], "stub rgn_offset must be zero");
    }

    #[test]
    fn test_subdivision_center_encoded() {
        // lat range 44°–46° → lat_center ≈ 45° (2_097_152 garmin units)
        // stored as (2097152 >> 8) as i16 = 8192
        let lat_lo = to_garmin_units(44.0); // 2_050_048
        let lat_hi = to_garmin_units(46.0); // 2_144_256
        let s = Subdivision::compute_subdivision((lat_lo, lat_hi, 0, 256), false, false, false, false, 0);
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
        // Size = 188 + 2×4 + (14 + 16) = 226 bytes
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        assert_eq!(data.len(), 226, "2-level TRE must be 226 bytes");
    }

    #[test]
    fn test_tre_build_header_signature() {
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        assert_eq!(&data[0x02..0x0C], b"GARMIN TRE");
    }

    #[test]
    fn test_tre_build_levels_offset() {
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        let offset = u32::from_le_bytes([data[0x21], data[0x22], data[0x23], data[0x24]]);
        assert_eq!(offset, TRE_HEADER_SIZE as u32);
    }

    #[test]
    fn test_tre_build_bounds_positive() {
        // minimal_for_img.mp has a POI at lat ≈ 45.19° → max_lat_g > 0
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        // max_lat at bytes[0x15..0x18] (LE24s, after common header)
        let raw = (data[0x15] as i32) | ((data[0x16] as i32) << 8) | ((data[0x17] as i32) << 16);
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
        // Must not panic; valid TRE: 188 + 3×4 + (14 + 2×16) = 246 bytes
        assert_eq!(data.len(), 246);
        assert_eq!(&data[0x02..0x0C], b"GARMIN TRE");
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
        // 188 + 3×4 + (14 + 2×16) = 246 bytes
        assert_eq!(data.len(), 246, "default 3-level TRE must be 246 bytes");
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

    // ── Task 6 (Story 13.4): build_with_rgn_offsets ───────────────────────────

    #[test]
    fn test_build_with_rgn_offsets_patches_subdivision_rgn_offset() {
        // minimal_for_img.mp: 2 levels → 2 subdivisions
        let mp = fixture_mp();
        let rgn_offsets = vec![0u32, 42u32];
        let tre_data = TreWriter::build_with_rgn_offsets(&mp, &rgn_offsets);

        // TRE layout for 2 levels: 188 (header) + 2×4 (levels) + 14 + 16 (subdivisions)
        // Subdivisions start at offset 196.
        let subdivs_offset = TRE_HEADER_SIZE + 2 * 4;

        // Subdivision 0 (level 0, 14 bytes): rgn_offset = 0
        let off0 = (tre_data[subdivs_offset] as u32)
            | ((tre_data[subdivs_offset + 1] as u32) << 8)
            | ((tre_data[subdivs_offset + 2] as u32) << 16);
        assert_eq!(off0, 0, "level 0 subdivision rgn_offset must be 0");

        // Subdivision 1 (level 1, 16 bytes): rgn_offset = 42 — starts at +14 (not +16)
        let subdiv1_start = subdivs_offset + 14;
        let off1 = (tre_data[subdiv1_start] as u32)
            | ((tre_data[subdiv1_start + 1] as u32) << 8)
            | ((tre_data[subdiv1_start + 2] as u32) << 16);
        assert_eq!(off1, 42, "level 1 subdivision rgn_offset must be 42");
    }

    #[test]
    fn test_build_delegates_to_build_with_rgn_offsets() {
        // build() must produce identical output to build_with_rgn_offsets(mp, &[])
        let mp = fixture_mp();
        let a = TreWriter::build(&mp);
        let b = TreWriter::build_with_rgn_offsets(&mp, &[]);
        assert_eq!(a, b, "build() must equal build_with_rgn_offsets(mp, &[])");
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

    // ── Extended data_flags tests ────────────────────────────────────────────

    #[test]
    fn test_subdivision_extended_data_flags() {
        let mut s = Subdivision::compute_subdivision((0, 1, 0, 1), true, false, true, false, 0);
        s.has_extended_points = true;
        s.has_extended_polylines = false;
        s.has_extended_polygons = true;
        let bytes = s.to_bytes();
        // Standard: points(0x01) | polygons(0x08) = 0x09
        // Extended: ext_points(0x10) | ext_polygons(0x40) = 0x50
        // Combined: 0x59
        assert_eq!(
            bytes[0x03], 0x59,
            "data_flags: points + polygons + ext_points + ext_polygons = 0x59"
        );
    }

    #[test]
    fn test_subdivision_mixed_flags() {
        let mut s = Subdivision::compute_subdivision((0, 1, 0, 1), true, true, true, false, 0);
        s.has_indexed_lines = true;
        s.has_extended_points = true;
        s.has_extended_polylines = true;
        s.has_extended_polygons = true;
        let bytes = s.to_bytes();
        // All standard: 0x01 | 0x02 | 0x04 | 0x08 = 0x0F
        // All extended: 0x10 | 0x20 | 0x40 = 0x70
        // Combined: 0x7F
        assert_eq!(
            bytes[0x03], 0x7F,
            "data_flags: all standard + all extended = 0x7F"
        );
    }
}
