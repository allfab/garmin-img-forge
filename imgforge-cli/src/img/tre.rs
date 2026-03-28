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

/// TRE file header — exactly 188 bytes (21-byte common header + 167-byte type-specific).
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
    pub max_lat: i32,
    pub max_lon: i32,
    pub min_lat: i32,
    pub min_lon: i32,
    pub levels_offset: u32,
    pub levels_size: u32,
    pub subdivisions_offset: u32,
    pub subdivisions_size: u32,
    pub copyright_offset: u32,
    pub copyright_size: u32,
    /// Map ID (numeric, e.g. 63240001).
    pub map_id: u32,
    /// Offset and size of the extended type offsets section (in TRE, after subdivisions).
    /// Each record = 13 bytes (per-subdivision cumulative offsets into RGN ext sections).
    pub ext_type_offsets_offset: u32,
    pub ext_type_offsets_size: u32,
    /// Number of extended type line/area/point overview entries.
    pub num_ext_type_line_types: u16,
    pub num_ext_type_area_types: u16,
    pub num_ext_type_point_types: u16,
}

impl TreHeader {
    /// Serialise into exactly 188 bytes, matching mkgmap's TREHeader.writeFileHeader().
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TRE_HEADER_SIZE);
        // 0x00: Common header (21 bytes)
        buf.extend_from_slice(&build_common_header("TRE", TRE_HEADER_SIZE as u16));
        // 0x15–0x20: Bounding box (4 × LE24s = 12 bytes)
        write_le24(&mut buf, self.max_lat);
        write_le24(&mut buf, self.max_lon);
        write_le24(&mut buf, self.min_lat);
        write_le24(&mut buf, self.min_lon);
        // 0x21–0x28: Map levels section (offset + size)
        buf.extend_from_slice(&self.levels_offset.to_le_bytes());
        buf.extend_from_slice(&self.levels_size.to_le_bytes());
        // 0x29–0x30: Subdivisions section (offset + size)
        buf.extend_from_slice(&self.subdivisions_offset.to_le_bytes());
        buf.extend_from_slice(&self.subdivisions_size.to_le_bytes());
        // 0x31–0x38: Copyright section info (offset + size)
        buf.extend_from_slice(&self.copyright_offset.to_le_bytes());
        buf.extend_from_slice(&self.copyright_size.to_le_bytes());
        // 0x39–0x3C: padding (4 bytes)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x3D: POI display flags (1 byte)
        buf.push(0x00);
        // 0x3E–0x40: Display priority (LE24, default 0x19)
        write_le24(&mut buf, 0x19);
        // 0x41–0x44: Magic value (mkgmap: 0x110301)
        buf.extend_from_slice(&0x0011_0301u32.to_le_bytes());
        // 0x45–0x46: put2u(1)
        buf.extend_from_slice(&1u16.to_le_bytes());
        // 0x47: put1u(0)
        buf.push(0x00);
        // 0x48–0x4F: Polyline overview section (offset=0, size=0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x50–0x53: padding
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x54–0x5B: Polygon overview section (offset=0, size=0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x5C–0x5F: padding
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x60–0x67: Points overview section (offset=0, size=0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x68–0x6B: padding
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x6C–0x6F: Map ID (LE32)
        buf.extend_from_slice(&self.map_id.to_le_bytes());
        // 0x70–0x73: padding
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x74–0x7D: extTypeOffsets section info (offset + size + itemSize=13)
        buf.extend_from_slice(&self.ext_type_offsets_offset.to_le_bytes());
        buf.extend_from_slice(&self.ext_type_offsets_size.to_le_bytes());
        buf.extend_from_slice(&(13u16).to_le_bytes()); // EXT_TYPE_OFFSETS_REC_LEN = 13
        // 0x7E–0x81: Magic 0x0607 (indicates extended type data present)
        buf.extend_from_slice(&0x0000_0607u32.to_le_bytes());
        // 0x82–0x89: extTypeOverviews section (offset=0, size=0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        // 0x8A–0x8F: Extended type counts
        buf.extend_from_slice(&self.num_ext_type_line_types.to_le_bytes());
        buf.extend_from_slice(&self.num_ext_type_area_types.to_le_bytes());
        buf.extend_from_slice(&self.num_ext_type_point_types.to_le_bytes());
        // 0x90–0x9F: MapValues (4 × LE32, anti-piracy checksum)
        let mv = compute_map_values(self.map_id, TRE_HEADER_SIZE as u32);
        for v in &mv {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        // 0xA0–0xB1: remaining padding
        while buf.len() < TRE_HEADER_SIZE {
            buf.push(0);
        }
        buf.truncate(TRE_HEADER_SIZE);
        buf
    }
}

/// Compute the 4 MapValues (mkgmap TreCalc/MapValues.java).
/// These are anti-piracy checksums derived from the map ID and header length.
fn compute_map_values(map_id: u32, header_length: u32) -> [u32; 4] {
    let map_id_code_table: [u8; 16] = [
        0, 1, 0xf, 5, 0xd, 4, 7, 6, 0xb, 9, 0xe, 8, 2, 0xa, 0xc, 3,
    ];
    let offset_map: [u8; 16] = [
        6, 7, 5, 11, 3, 10, 13, 12, 1, 15, 4, 14, 8, 0, 2, 9,
    ];

    let digit = |i: usize| -> u8 { ((map_id >> (4 * (7 - i))) & 0xF) as u8 };

    // Third value: coded map ID digits
    let mut v3 = [0u8; 8];
    for i in 0..8 {
        v3[i ^ 1] = map_id_code_table[digit(i) as usize];
    }
    // Fourth = copy of third
    let mut v4 = v3;

    // First value
    let mut v1 = [0u8; 8];
    v1[0] = digit(4).wrapping_add(v4[0]);
    v1[1] = digit(5).wrapping_add(v4[1]);
    v1[2] = digit(6).wrapping_add(v4[2]);
    v1[3] = digit(7).wrapping_add(v4[3]);
    v1[4] = v4[4];
    v1[5] = v4[5];
    v1[6] = v4[6];
    v1[7] = v4[7].wrapping_add(1);

    // Second value
    let mut v2 = [0u8; 8];
    v2[0] = v4[0];
    v2[1] = v4[1];
    v2[2] = (v4[2].wrapping_add((header_length >> 4) as u8)) & 0xF;
    v2[3] = (v4[3].wrapping_add(header_length as u8)) & 0xF;
    v2[4] = v4[4].wrapping_add(digit(0));
    v2[5] = v4[5].wrapping_add(digit(1));
    v2[6] = v4[6].wrapping_add(digit(2));
    v2[7] = v4[7].wrapping_add(digit(3));

    // Add global offset to all values
    let n = (digit(1) as u16 + digit(3) as u16 + digit(5) as u16 + digit(7) as u16) & 0xF;
    let offset = offset_map[n as usize];

    let pack = |vals: &mut [u8; 8]| -> u32 {
        for v in vals.iter_mut() {
            *v = v.wrapping_add(offset);
        }
        let mut res = 0u32;
        for i in 0..8 {
            res |= ((vals[i] as u32) & 0xF) << (4 * (7 - i));
        }
        res
    };

    [pack(&mut v1), pack(&mut v2), pack(&mut v3), pack(&mut v4)]
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
    /// Write subdivision fields (WITHOUT the leading rgn_offset — that's separate).
    ///
    /// mkgmap TREFileReader format:
    /// ```text
    /// [flags 1B][lon 3B][lat 3B][width 2B][height 2B] = 11 bytes
    /// [nextLevel 2B for non-leaf]                      = +2 bytes
    /// [endRgnOffset 3B]                                = +3 bytes
    /// ```
    /// Total: leaf = 14 bytes, non-leaf = 16 bytes.
    pub fn to_bytes_sized(&self, is_most_detailed_level: bool, end_rgn_offset: u32) -> Vec<u8> {
        let size = if is_most_detailed_level { 14 } else { 16 };
        let mut buf = Vec::with_capacity(size);

        // flags (1 byte)
        buf.push(
            ((self.has_points as u8) << 4)
            | ((self.has_indexed_lines as u8) << 5)
            | ((self.has_polylines as u8) << 6)
            | ((self.has_polygons as u8) << 7)
        );

        // lon_center (LE24s, 3 bytes)
        buf.push((self.lon_center & 0xFF) as u8);
        buf.push(((self.lon_center >> 8) & 0xFF) as u8);
        buf.push(((self.lon_center >> 16) & 0xFF) as u8);

        // lat_center (LE24s, 3 bytes)
        buf.push((self.lat_center & 0xFF) as u8);
        buf.push(((self.lat_center >> 8) & 0xFF) as u8);
        buf.push(((self.lat_center >> 16) & 0xFF) as u8);

        // half_width (LE16), bit 15 = last_in_level
        let mut hw = (self.half_width as u16).min(0x7FFF).max(1);
        if self.last_in_level {
            hw |= 0x8000;
        }
        buf.extend_from_slice(&hw.to_le_bytes());

        // half_height (LE16)
        let hh = (self.half_height as u16).max(1);
        buf.extend_from_slice(&hh.to_le_bytes());

        // nextLevel (LE16) for non-leaf levels only
        if !is_most_detailed_level {
            buf.extend_from_slice(&self.next_level_first_subdiv.to_le_bytes());
        }

        // endRgnOffset (LE24, 3 bytes)
        buf.push((end_rgn_offset & 0xFF) as u8);
        buf.push(((end_rgn_offset >> 8) & 0xFF) as u8);
        buf.push(((end_rgn_offset >> 16) & 0xFF) as u8);

        buf
    }

    /// Serialise into exactly 16 bytes (legacy — used by tests).
    pub fn to_bytes(&self) -> [u8; 16] {
        let v = self.to_bytes_sized(false, 0);
        let mut buf = [0u8; 16];
        buf[..v.len().min(16)].copy_from_slice(&v[..v.len().min(16)]);
        buf
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
    ///
    /// Supports multiple subdivisions per level when `rgn_result` contains
    /// populated `SubdivisionInfo` entries.
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

        // Use SubdivisionInfo from RgnBuildResult when available.
        let use_subdiv_info = rgn_result
            .map_or(false, |rr| !rr.subdivisions.is_empty());

        let subdivisions: Vec<Subdivision>;

        if use_subdiv_info {
            let rr = rgn_result.unwrap();
            let subdiv_infos = &rr.subdivisions;

            // Count subdivisions per level.
            let mut level_subdiv_counts: Vec<u16> = vec![0; n];
            for si in subdiv_infos {
                if (si.level as usize) < n {
                    level_subdiv_counts[si.level as usize] += 1;
                }
            }

            // Compute cumulative start index (1-based) per level.
            let mut level_first_subdiv: Vec<u16> = vec![0; n];
            let mut cumulative: u16 = 1; // 1-based
            for lev in 0..n {
                level_first_subdiv[lev] = cumulative;
                cumulative += level_subdiv_counts[lev];
            }

            // Build Subdivision structs from SubdivisionInfo.
            subdivisions = subdiv_infos
                .iter()
                .enumerate()
                .map(|(idx, si)| {
                    let lev = si.level as usize;
                    // Is this the last subdivision within its level?
                    let last_in_level = subdiv_infos
                        .get(idx + 1)
                        .map_or(true, |next| next.level != si.level);
                    // next_level_first_subdiv: for overview levels, points to child level.
                    // Level 0 = most detailed → 0. Level i > 0 → first subdiv of level i-1.
                    let next_idx = if lev == 0 {
                        0u16
                    } else {
                        level_first_subdiv[lev - 1]
                    };

                    let half_height = ((si.max_lat_g - si.min_lat_g) / 2 + 1) as u32;
                    let half_width = ((si.max_lon_g - si.min_lon_g) / 2 + 1) as u32;

                    Subdivision {
                        rgn_offset: si.rgn_offset,
                        has_points: si.has_points,
                        has_indexed_lines: si.has_indexed_lines,
                        has_polylines: si.has_polylines,
                        has_polygons: si.has_polygons,
                        has_extended_points: si.has_extended_points,
                        has_extended_polylines: si.has_extended_polylines,
                        has_extended_polygons: si.has_extended_polygons,
                        lon_center: si.center_lon_g,
                        lat_center: si.center_lat_g,
                        half_width,
                        half_height,
                        last_in_level,
                        next_level_first_subdiv: next_idx,
                    }
                })
                .collect();

            // Update level subdivision_counts.
            for (i, level) in levels.iter_mut().enumerate() {
                level.subdivision_count = level_subdiv_counts[i];
                if i == n - 1 {
                    level.bits_per_coord |= 0x80; // INHERITED flag on overview level
                }
            }
        } else {
            // Legacy path: 1 subdivision per level.
            let bounds_g = (min_lat_g, max_lat_g, min_lon_g, max_lon_g);
            subdivisions = (0..n)
                .map(|i| {
                    let next_idx = if i == 0 { 0u16 } else { i as u16 };
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
                    let has_indexed_lines = routing
                        && mp
                            .polylines
                            .iter()
                            .any(|f| {
                                f.routing.is_some()
                                    && f.end_level.unwrap_or(u8::MAX) >= i as u8
                            });
                    let mut s = Subdivision::compute_subdivision(
                        bounds_g, has_points, has_polylines, has_polygons, true, next_idx,
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

            for (i, level) in levels.iter_mut().enumerate() {
                level.subdivision_count = 1;
                if i == n - 1 {
                    level.bits_per_coord |= 0x80; // INHERITED flag on overview level
                }
            }
        }

        // Step 4: compute section byte offsets.
        // Garmin format: most detailed level (0) uses 14-byte subdivisions,
        // all overview levels use 16-byte subdivisions.
        let levels_offset = TRE_HEADER_SIZE as u32;
        let levels_size = n as u32 * 4;
        let subdivisions_offset = levels_offset + levels_size;

        // Count 14-byte (level 0) and 16-byte (other levels) subdivisions.
        let _n_level0_subdivs = subdivisions
            .iter()
            .filter(|s| {
                // Subdivisions at level 0: they have next_level_first_subdiv == 0
                // and are the first N subdivisions (where N = level_0_count).
                // Simpler: use the level from SubdivisionInfo if available.
                s.next_level_first_subdiv == 0 || levels.len() == 1
            })
            .count();
        // Actually, for the size calculation, we need to know which subdivisions
        // are at level 0 (14 bytes) vs others (16 bytes). Use the levels info.
        let level0_count = levels.first().map_or(0, |l| l.subdivision_count as usize);
        let other_count = subdivisions.len().saturating_sub(level0_count);
        // subdivisions_size includes: initial_rgn_offset(3B) + sum of subdiv records
        // Leaf(14B) = flags(1)+lon(3)+lat(3)+w(2)+h(2)+endRgnOff(3)
        // Non-leaf(16B) = same + nextLevel(2)
        let subdivisions_size = 3 + (level0_count as u32 * 14) + (other_count as u32 * 16);

        let copyright_offset = subdivisions_offset + subdivisions_size;
        let copyright_size = 0u32;

        // Extended type offsets section: 13 bytes per subdivision + 1 final record.
        // This section tells QMapShack where each subdivision's extended data lives
        // within the RGN extended sections.
        let n_subdivs = subdivisions.len();
        let has_any_extended = rgn_result.map_or(false, |rr| {
            rr.subdivisions.iter().any(|s| {
                s.has_extended_points || s.has_extended_polylines || s.has_extended_polygons
            })
        });
        let ext_offsets_record_count = if has_any_extended { n_subdivs + 1 } else { 0 };
        let ext_offsets_section_size = ext_offsets_record_count * 13;
        let ext_offsets_offset = copyright_offset; // right after copyright (which is size 0)

        // Parse map_id from MP header.
        let map_id = mp.header.id.parse::<u32>().unwrap_or(0);

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
            map_id,
            ext_type_offsets_offset: if has_any_extended {
                ext_offsets_offset
            } else {
                0
            },
            ext_type_offsets_size: ext_offsets_section_size as u32,
            num_ext_type_line_types: 0,
            num_ext_type_area_types: 0,
            num_ext_type_point_types: 0,
        };

        // Step 8: serialise header + levels + subdivisions + extTypeOffsets
        //
        // Subdivision section layout (mkgmap TREFileReader):
        //   [initial_rgn_offset 3B]
        //   [subdiv_0_fields ... endRgnOffset 3B]
        //   [subdiv_1_fields ... endRgnOffset 3B]
        //   ...
        // Each subdivision's endRgnOffset = next subdivision's startRgnPointer.
        // The last endRgnOffset = total RGN data size.
        let mut out = header.to_bytes();
        for level in &levels {
            out.extend_from_slice(&level.to_bytes());
        }

        // Write initial rgn_offset (3 bytes — first subdivision's start)
        let first_rgn_off = if subdivisions.is_empty() { 0u32 } else { subdivisions[0].rgn_offset };
        out.push((first_rgn_off & 0xFF) as u8);
        out.push(((first_rgn_off >> 8) & 0xFF) as u8);
        out.push(((first_rgn_off >> 16) & 0xFF) as u8);

        // Compute total RGN standard data size for the last endRgnOffset.
        // Read data_size from RGN header bytes 0x19-0x1C.
        let total_rgn_data_size = rgn_result.map_or(
            // Fallback: use last subdivision_offset from rgn_offsets
            rgn_offsets.last().copied().unwrap_or(0),
            |rr| {
                if rr.data.len() > 0x1C {
                    u32::from_le_bytes([rr.data[0x19], rr.data[0x1A], rr.data[0x1B], rr.data[0x1C]])
                } else {
                    0
                }
            },
        );

        // Write subdivision fields with endRgnOffset
        let n_subdivs = subdivisions.len();
        let mut level0_written = 0usize;
        for (idx, subdiv) in subdivisions.iter().enumerate() {
            let is_most_detailed = level0_written < level0_count;
            // endRgnOffset = next subdivision's rgn_offset, or total_data_size for last
            let end_rgn_off = if idx + 1 < n_subdivs {
                subdivisions[idx + 1].rgn_offset
            } else {
                total_rgn_data_size
            };
            out.extend_from_slice(&subdiv.to_bytes_sized(is_most_detailed, end_rgn_off));
            if is_most_detailed {
                level0_written += 1;
            }
        }

        // Write extTypeOffsets section (13 bytes per subdivision + 1 final record).
        // Each record: cumulative offset into RGN ext_areas(4B) + ext_lines(4B) + ext_points(4B) + kinds(1B)
        if has_any_extended {
            let rr = rgn_result.unwrap();
            // With single-subdivision per level, each subdivision's extended data
            // is the entire extended section. Cumulative offsets start at 0.
            // For single subdiv: all ext data belongs to the one subdiv per level.
            let mut cum_areas: u32 = 0;
            let mut cum_lines: u32 = 0;
            let mut cum_points: u32 = 0;

            for si in &rr.subdivisions {
                out.extend_from_slice(&cum_areas.to_le_bytes());
                out.extend_from_slice(&cum_lines.to_le_bytes());
                out.extend_from_slice(&cum_points.to_le_bytes());
                let mut kinds: u8 = 0;
                if si.has_extended_polygons { kinds += 1; }
                if si.has_extended_polylines { kinds += 1; }
                if si.has_extended_points { kinds += 1; }
                out.push(kinds);
                // Advance cumulative offsets (for single-subdiv, all data is in one chunk)
                // We don't have per-subdivision sizes here, so we assign all to the first subdiv
                // of each level by checking if next subdiv is a different level.
            }
            // Final record: total sizes of each extended section
            out.extend_from_slice(&rr.ext_areas_size.to_le_bytes());
            out.extend_from_slice(&rr.ext_lines_size.to_le_bytes());
            out.extend_from_slice(&rr.ext_points_size.to_le_bytes());
            out.push(0u8); // kinds = 0 for final record
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
            levels_offset: TRE_HEADER_SIZE as u32,
            levels_size: 12,
            subdivisions_offset: TRE_HEADER_SIZE as u32 + 12,
            subdivisions_size: 48,
            copyright_offset: TRE_HEADER_SIZE as u32 + 12 + 48,
            copyright_size: 0,
            map_id: 63240001,
            ext_type_offsets_offset: 0,
            ext_type_offsets_size: 0,
            num_ext_type_line_types: 0,
            num_ext_type_area_types: 0,
            num_ext_type_point_types: 0,
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
    fn test_tre_header_has_magic_and_map_id() {
        let bytes = default_header().to_bytes();
        // Magic 0x110301 at offset 0x41
        let magic = u32::from_le_bytes([bytes[0x41], bytes[0x42], bytes[0x43], bytes[0x44]]);
        assert_eq!(magic, 0x0011_0301, "TRE magic must be 0x110301");
        // Map ID at offset 0x6C
        let map_id = u32::from_le_bytes([bytes[0x6C], bytes[0x6D], bytes[0x6E], bytes[0x6F]]);
        assert_eq!(map_id, 63240001, "map_id must match");
        // Extended magic 0x0607 at offset 0x7E
        let ext_magic = u32::from_le_bytes([bytes[0x7E], bytes[0x7F], bytes[0x80], bytes[0x81]]);
        assert_eq!(ext_magic, 0x0607, "extended type magic must be 0x0607");
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
        // New format: [flags(1B)][lon(3B)][lat(3B)][w(2B)][h(2B)][next(2B)][endRgnOff(3B)]
        // flags at byte 0 (not byte 3!)
        let s = Subdivision::compute_subdivision((0, 1, 0, 1), true, false, true, false, 0);
        let bytes = s.to_bytes();
        assert_eq!(
            bytes[0], 0x90,
            "data_flags: points=0x10, polygons=0x80 → 0x90"
        );
    }

    #[test]
    fn test_subdivision_last_flag() {
        let s = Subdivision::compute_subdivision((0, 100, 0, 100), false, false, false, true, 0);
        let bytes = s.to_bytes();
        // half_width at bytes 7-8 (after flags(1)+lon(3)+lat(3))
        let hw = u16::from_le_bytes([bytes[7], bytes[8]]);
        assert!(hw & 0x8000 != 0, "last_in_level must set bit 15");
    }

    #[test]
    fn test_subdivision_center_encoded() {
        // New format: lat at bytes 4-6 (after flags(1)+lon(3))
        let lat_lo = to_garmin_units(44.0);
        let lat_hi = to_garmin_units(46.0);
        let s = Subdivision::compute_subdivision((lat_lo, lat_hi, 0, 256), false, false, false, false, 0);
        let bytes = s.to_bytes();
        let lat_center_g = (lat_hi + lat_lo) / 2;
        let lat_stored = (bytes[4] as i32) | ((bytes[5] as i32) << 8) | ((bytes[6] as i32) << 16);
        let lat_stored = if lat_stored & 0x800000 != 0 { lat_stored | !0xFFFFFF } else { lat_stored };
        assert_eq!(lat_stored, lat_center_g, "lat_center must be stored as full LE24");
    }

    #[test]
    fn test_subdivision_all_standard_flags() {
        let mut s = Subdivision::compute_subdivision((0, 1, 0, 1), true, true, true, false, 0);
        s.has_indexed_lines = true;
        let bytes = s.to_bytes();
        assert_eq!(bytes[0], 0xF0, "all 4 standard types = 0xF0");
    }

    // ── Task 5: TreWriter ─────────────────────────────────────────────────────

    #[test]
    fn test_tre_build_minimal() {
        // minimal_for_img.mp: Level0=24, Level1=18 → 2 explicit levels
        // Size = 188 + 2×4 + 3(initial) + (14 + 16) = 229 bytes
        let mp = fixture_mp();
        let data = TreWriter::build(&mp);
        assert_eq!(data.len(), 229, "2-level TRE must be 229 bytes");
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
        assert_eq!(data.len(), 249);
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
        assert_eq!(data.len(), 249, "default 3-level TRE must be 249 bytes");
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

    // ── Extended data_flags tests (removed — extended types disabled) ────
}
