//! NET subfile writer — road definitions and search index for Garmin IMG.
//!
//! The NET subfile stores:
//! - **NET1**: Road definitions (labels, flags, length, subdivision references)
//! - **NET2**: Segmented roads (empty — not used for basic routing)
//! - **NET3**: Sorted road index for name search on GPS
//!
//! Format: `[NET Header — 55 B] [NET1 records…] [NET3 records…]`

use std::collections::HashMap;

use crate::img::common_header::build_common_header;
use crate::routing::{polyline_length, RoadDef, RoadNetwork};

// ── Constants ────────────────────────────────────────────────────────────────

/// NET header size in bytes.
const NET_HEADER_SIZE: u32 = 0x37; // 55 bytes

/// NET flags (byte in each NET1 record).
const NET_FLAG_ONEWAY: u8 = 0x02;
/// Unknown flag — "lock on road", always set (mkgmap convention).
const NET_FLAG_UNK1: u8 = 0x04;
/// Toll road flag (confirmed in mkgmap RoadDef.java::writeNet1()).
const NET_FLAG_TOLL: u8 = 0x08;
/// Access restriction present — 2-byte access_mask follows flags byte in record.
const NET_FLAG_ACCESS: u8 = 0x20;
/// NOD2 reference present in the record.
const NET_FLAG_NODINFO: u8 = 0x40;

/// Distance multiplier: raw = round(metres / (DISTANCE_MULT * UNIT_TO_METER)).
/// DISTANCE_MULT=2, UNIT_TO_METER=2.4 → divisor = 4.8.
const DISTANCE_DIVISOR: f64 = 4.8;

/// Bit mask for 22-bit LBL/NET1 offsets.
const OFFSET_22BIT_MASK: u32 = 0x3F_FFFF;

/// Last-label marker bit (bit 23 of the 3-byte label field).
const LAST_LABEL_BIT: u32 = 0x80_0000;

// ── SubdivRoadRef ────────────────────────────────────────────────────────────

/// Cross-reference between a RGN subdivision polyline and a NET road definition.
///
/// Used by NET1 to encode the "level divisions" section (3 bytes per reference:
/// polyline_index u8 + subdiv_number u16 LE).
#[derive(Debug, Clone)]
pub struct SubdivRoadRef {
    /// Index into `RoadNetwork::road_defs`.
    pub road_def_idx: usize,
    /// Subdivision number (1-based) in the TRE/RGN hierarchy.
    pub subdiv_number: u16,
    /// Index of the polyline within its subdivision's polyline group in RGN.
    pub polyline_index: u8,
}

// ── NetBuildResult ───────────────────────────────────────────────────────────

/// Result of `NetWriter::build`: the complete NET binary and per-road offsets.
pub struct NetBuildResult {
    /// Complete NET subfile binary: `[header || NET1 || NET3]`.
    pub data: Vec<u8>,
    /// `road_offsets[i]` = byte offset of the i-th RoadDef's NET1 record,
    /// relative to the start of the NET1 section (not the file start).
    /// Used by RGN to embed NET1 cross-references in routable polylines.
    pub road_offsets: Vec<u32>,
    /// `nod2_patch_positions[i]` = absolute byte offset within `data` of the first
    /// byte of the 2-byte NOD2 placeholder for the i-th RoadDef.
    /// Allows the NOD writer to patch offsets without re-parsing the NET1 binary.
    pub nod2_patch_positions: Vec<usize>,
}

// ── NET Header ───────────────────────────────────────────────────────────────

/// Write the 55-byte NET header into `buf`.
///
/// Binary layout:
/// ```text
/// 0x00  21B   Common header "GARMIN NET" (via write_common_header)
/// 0x15  LE32  NET1 section offset
/// 0x19  LE32  NET1 section length
/// 0x1D  u8    Road shift (0)
/// 0x1E  LE32  NET2 section offset
/// 0x22  LE32  NET2 section length (0)
/// 0x26  u8    Segment shift (0)
/// 0x27  LE32  NET3 section offset
/// 0x2B  LE32  NET3 section length
/// 0x2F  LE16  NET3 record size (3)
/// 0x31  LE32  Reserved (0)
/// 0x35  u8    Reserved (0x01)
/// 0x36  u8    Reserved (0x00)
/// ```
fn write_net_header(
    buf: &mut Vec<u8>,
    net1_offset: u32,
    net1_len: u32,
    net3_offset: u32,
    net3_len: u32,
) {
    // Common header: 21 bytes (standard Garmin subfile format).
    buf.extend_from_slice(&build_common_header("NET", NET_HEADER_SIZE as u16));

    // 0x15: NET1 section offset (LE32)
    buf.extend_from_slice(&net1_offset.to_le_bytes());
    // 0x19: NET1 section length (LE32)
    buf.extend_from_slice(&net1_len.to_le_bytes());
    // 0x1D: road_shift = 0 (u8)
    buf.push(0u8);
    // 0x1E: NET2 section offset (LE32) = net1_offset + net1_len (empty section)
    let net2_offset = net1_offset + net1_len;
    buf.extend_from_slice(&net2_offset.to_le_bytes());
    // 0x22: NET2 section length = 0 (LE32)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // 0x26: segment_shift = 0 (u8)
    buf.push(0u8);
    // 0x27: NET3 section offset (LE32)
    buf.extend_from_slice(&net3_offset.to_le_bytes());
    // 0x2B: NET3 section length (LE32)
    buf.extend_from_slice(&net3_len.to_le_bytes());
    // 0x2F: NET3 record size = 3 (LE16)
    buf.extend_from_slice(&3u16.to_le_bytes());
    // 0x31: reserved (LE32)
    buf.extend_from_slice(&0u32.to_le_bytes());
    // 0x35: reserved (u8)
    buf.push(0x01u8);
    // 0x36: reserved (u8)
    buf.push(0x00u8);

    debug_assert_eq!(buf.len(), NET_HEADER_SIZE as usize);
}

// ── NET1 record encoder ──────────────────────────────────────────────────────

/// Encode a single NET1 road definition record.
///
/// Returns the variable-length binary for one road definition.
fn encode_road_def(
    road_def_idx: usize,
    road_def: &RoadDef,
    label_offsets: &HashMap<String, u32>,
    subdiv_refs: &[SubdivRoadRef],
    polyline_coords: &[(f64, f64)],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32);

    // 1. Labels (3 bytes each, 1 label for now)
    let lbl_offset = road_def
        .label
        .as_ref()
        .and_then(|l| label_offsets.get(l.as_str()).copied())
        .unwrap_or(0)
        & OFFSET_22BIT_MASK;
    // Single label with last-label marker (bit 23)
    let label_bytes = lbl_offset | LAST_LABEL_BIT;
    buf.push((label_bytes & 0xFF) as u8);
    buf.push(((label_bytes >> 8) & 0xFF) as u8);
    buf.push(((label_bytes >> 16) & 0xFF) as u8);

    // 2. Net flags (1 byte)
    let mut flags: u8 = NET_FLAG_UNK1; // always set
    if road_def.one_way {
        flags |= NET_FLAG_ONEWAY;
    }
    if road_def.toll {
        flags |= NET_FLAG_TOLL;
    }
    if road_def.access_mask != 0 {
        flags |= NET_FLAG_ACCESS;
    }
    // Set NODINFO flag — placeholder offset written, patched by NOD writer (Story 14.4)
    flags |= NET_FLAG_NODINFO;
    buf.push(flags);

    // 3. Access mask (2 bytes LE16) — UNIQUEMENT si NET_FLAG_ACCESS activé
    if road_def.access_mask != 0 {
        buf.extend_from_slice(&road_def.access_mask.to_le_bytes());
    }

    // 4. Road length (3 bytes LE24 unsigned)
    let length_meters = polyline_length(polyline_coords);
    let raw_length = (length_meters / DISTANCE_DIVISOR).round() as u32;
    buf.push((raw_length & 0xFF) as u8);
    buf.push(((raw_length >> 8) & 0xFF) as u8);
    buf.push(((raw_length >> 16) & 0xFF) as u8);

    // 5. Level counts (1 byte per level, bit 7 = last level marker)
    // Collect references for this road_def, grouped by subdiv
    let my_refs: Vec<&SubdivRoadRef> = subdiv_refs
        .iter()
        .filter(|r| r.road_def_idx == road_def_idx)
        .collect();

    if my_refs.is_empty() {
        // Single level with count=1 and last-level marker
        buf.push(0x01 | 0x80);
        // No level divisions to write (fallback — should not happen in practice)
    } else {
        // Group refs by subdiv_number to count per level
        // For imgforge-cli (single subdivision per level), each ref is one level entry
        // Write count=1 for each unique subdiv, with bit 7 on the last one
        let count = my_refs.len();
        for (i, _) in my_refs.iter().enumerate() {
            let mut level_byte: u8 = 0x01; // 1 polyline per level
            if i + 1 == count {
                level_byte |= 0x80; // last level marker
            }
            buf.push(level_byte);
        }

        // 6. Level divisions (3 bytes per reference)
        for r in &my_refs {
            buf.push(r.polyline_index);
            buf.extend_from_slice(&r.subdiv_number.to_le_bytes());
        }
    }

    // 7. NOD2 reference placeholder (NET_FLAG_NODINFO is set)
    // Format: 1 byte size indicator + 2 bytes offset (placeholder = 0)
    buf.push(0x01); // size indicator: 1 = 2-byte offset
    buf.extend_from_slice(&0u16.to_le_bytes()); // placeholder offset = 0x0000

    buf
}

// ── NET3 record encoder ──────────────────────────────────────────────────────

/// Generate sorted NET3 records (3 bytes each).
///
/// Each record: `(label_index << 22) | offset_net1`
/// Records are sorted by road label name (case-insensitive alphabetical).
fn build_net3_records(
    road_defs: &[RoadDef],
    road_offsets: &[u32],
) -> Vec<u8> {
    // Collect (label_name, label_index=0, net1_offset) for each road with a label
    let mut entries: Vec<(String, u32)> = Vec::new();
    for (i, rd) in road_defs.iter().enumerate() {
        if rd.label.is_some() {
            entries.push((
                rd.label.clone().unwrap_or_default(),
                road_offsets[i],
            ));
        }
    }

    // Sort by label name (case-insensitive)
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Encode as 3-byte records
    let mut data = Vec::with_capacity(entries.len() * 3);
    for (_, net1_offset) in &entries {
        // label_index = 0 (first/only label), shifted to bits 22-23
        // label_index = 0 (first/only label) → bits 22-23 are 0
        let record: u32 = net1_offset & OFFSET_22BIT_MASK;
        data.push((record & 0xFF) as u8);
        data.push(((record >> 8) & 0xFF) as u8);
        data.push(((record >> 16) & 0xFF) as u8);
    }
    data
}

// ── NetWriter ────────────────────────────────────────────────────────────────

/// Builds the NET subfile binary from a road network and supporting data.
pub struct NetWriter;

impl NetWriter {
    /// Build the complete NET subfile.
    ///
    /// # Arguments
    /// - `road_network`: The road network graph (from graph builder)
    /// - `label_offsets`: Map from label string → LBL data-section-relative offset
    /// - `subdiv_road_refs`: Cross-references from RGN subdivisions to road definitions
    /// - `polylines`: The original polyline features (for coordinate/length data)
    pub fn build(
        road_network: &RoadNetwork,
        label_offsets: &HashMap<String, u32>,
        subdiv_road_refs: &[SubdivRoadRef],
        polylines: &[crate::parser::mp_types::MpPolyline],
    ) -> NetBuildResult {
        let mut net1_data: Vec<u8> = Vec::new();
        let mut road_offsets: Vec<u32> = Vec::with_capacity(road_network.road_defs.len());
        let mut nod2_patch_positions: Vec<usize> = Vec::with_capacity(road_network.road_defs.len());

        // Encode each road definition as a NET1 record
        for (rd_idx, rd) in road_network.road_defs.iter().enumerate() {
            let offset = net1_data.len() as u32;
            road_offsets.push(offset);

            // Get polyline coordinates for length calculation
            let coords: &[(f64, f64)] = if rd.polyline_idx < polylines.len() {
                &polylines[rd.polyline_idx].coords
            } else {
                &[]
            };

            let record = encode_road_def(rd_idx, rd, label_offsets, subdiv_road_refs, coords);
            // Capture position of 2-byte NOD2 placeholder (last 2 bytes of record)
            // in the final `data` (= NET_HEADER_SIZE + offset_in_net1 + record.len() - 2)
            nod2_patch_positions.push(NET_HEADER_SIZE as usize + offset as usize + record.len() - 2);
            net1_data.extend_from_slice(&record);
        }

        // Build NET3 sorted index
        let net3_data = build_net3_records(&road_network.road_defs, &road_offsets);

        // Compute section offsets
        let net1_offset = NET_HEADER_SIZE;
        let net1_len = net1_data.len() as u32;
        // NET2 is empty — offset right after NET1
        let net3_offset = net1_offset + net1_len; // NET2 size = 0
        let net3_len = net3_data.len() as u32;

        // Assemble complete NET binary
        let total_size = NET_HEADER_SIZE as usize + net1_data.len() + net3_data.len();
        let mut data = Vec::with_capacity(total_size);
        write_net_header(&mut data, net1_offset, net1_len, net3_offset, net3_len);
        data.extend_from_slice(&net1_data);
        data.extend_from_slice(&net3_data);

        tracing::info!(
            road_defs = road_network.road_defs.len(),
            net1_size = net1_len,
            net3_size = net3_len,
            net3_records = net3_data.len() / 3,
            total_size = data.len(),
            "NET subfile built"
        );

        NetBuildResult { data, road_offsets, nod2_patch_positions }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::RoadNetwork;

    // ── Task 1: NET Header ──────────────────────────────────────────────────

    #[test]
    fn test_net_header_size() {
        let mut buf = Vec::new();
        write_net_header(&mut buf, 55, 0, 55, 0);
        assert_eq!(buf.len(), 55, "NET header must be exactly 55 bytes");
    }

    #[test]
    fn test_net_header_signature() {
        let mut buf = Vec::new();
        write_net_header(&mut buf, 55, 100, 155, 30);
        // Signature "GARMIN NET" at offset 0x02 (standard common header)
        assert_eq!(&buf[0x02..0x0C], b"GARMIN NET");
    }

    #[test]
    fn test_net_header_offsets() {
        let mut buf = Vec::new();
        write_net_header(&mut buf, 55, 200, 255, 30);

        // NET1 offset at 0x15
        let net1_off = u32::from_le_bytes([buf[0x15], buf[0x16], buf[0x17], buf[0x18]]);
        assert_eq!(net1_off, 55);

        // NET1 length at 0x19
        let net1_len = u32::from_le_bytes([buf[0x19], buf[0x1A], buf[0x1B], buf[0x1C]]);
        assert_eq!(net1_len, 200);

        // Road shift at 0x1D
        assert_eq!(buf[0x1D], 0);

        // NET2 offset at 0x1E = net1_offset + net1_len
        let net2_off = u32::from_le_bytes([buf[0x1E], buf[0x1F], buf[0x20], buf[0x21]]);
        assert_eq!(net2_off, 255, "NET2 offset = NET1 offset + NET1 len");

        // NET2 length at 0x22 = 0
        let net2_len = u32::from_le_bytes([buf[0x22], buf[0x23], buf[0x24], buf[0x25]]);
        assert_eq!(net2_len, 0, "NET2 is empty");

        // NET3 offset at 0x27
        let net3_off = u32::from_le_bytes([buf[0x27], buf[0x28], buf[0x29], buf[0x2A]]);
        assert_eq!(net3_off, 255);

        // NET3 length at 0x2B
        let net3_len = u32::from_le_bytes([buf[0x2B], buf[0x2C], buf[0x2D], buf[0x2E]]);
        assert_eq!(net3_len, 30);

        // NET3 record size at 0x2F
        let net3_rec_size = u16::from_le_bytes([buf[0x2F], buf[0x30]]);
        assert_eq!(net3_rec_size, 3);
    }

    // ── Task 2: NET1 record encoding ────────────────────────────────────────

    fn make_simple_road_network() -> (RoadNetwork, Vec<crate::parser::mp_types::MpPolyline>) {
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};
        let polylines = vec![MpPolyline {
            type_code: "0x02".to_string(),
            label: Some("D1075".to_string()),
            end_level: None,
            coords: vec![(45.0, 5.0), (45.045, 5.0)], // ~5000m
            routing: Some(MpRoutingAttrs {
                road_id: Some("1".to_string()),
                route_param: Some("6,3,1,1,0,0,0,0,0,0,0,0".to_string()),
                speed_type: None,
                dir_indicator: Some(1),
                roundabout: None,
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields: HashMap::new(),
        }];

        let road_defs = vec![RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 6,
            road_class: 3,
            one_way: true,
            toll: true,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("D1075".to_string()),
        }];

        let network = RoadNetwork {
            nodes: vec![],
            arcs: vec![],
            road_defs,
        };

        (network, polylines)
    }

    #[test]
    fn test_encode_road_def_simple() {
        let (network, polylines) = make_simple_road_network();
        let mut label_offsets = HashMap::new();
        label_offsets.insert("D1075".to_string(), 42u32);

        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];

        let record = encode_road_def(
            0,
            &network.road_defs[0],
            &label_offsets,
            &subdiv_refs,
            &polylines[0].coords,
        );

        // Label: 3 bytes — offset 42 with last-label bit
        let label_val = u32::from_le_bytes([record[0], record[1], record[2], 0]);
        assert_eq!(label_val & OFFSET_22BIT_MASK, 42, "label offset = 42");
        assert_ne!(label_val & LAST_LABEL_BIT, 0, "last-label bit must be set");

        // Flags: NET_FLAG_UNK1 | NET_FLAG_ONEWAY | NET_FLAG_TOLL | NET_FLAG_NODINFO
        // = 0x04 | 0x02 | 0x08 | 0x40 = 0x4E (toll=true in make_simple_road_network)
        assert_eq!(record[3], 0x4E, "flags: UNK1 + ONEWAY + TOLL + NODINFO");

        // Road length: ~5000m / 4.8 ≈ 1042
        let raw_len = u32::from_le_bytes([record[4], record[5], record[6], 0]);
        assert!(
            (raw_len as i32 - 1042).unsigned_abs() < 10,
            "road length raw ≈ 1042, got {}",
            raw_len
        );
    }

    #[test]
    fn test_encode_road_def_bidirectional() {
        let road_def = RoadDef {
            road_id: 2,
            polyline_idx: 0,
            speed: 3,
            road_class: 1,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("Rue Test".to_string()),
        };
        let mut label_offsets = HashMap::new();
        label_offsets.insert("Rue Test".to_string(), 10u32);
        let coords = vec![(45.0, 5.0), (45.001, 5.0)];

        let record = encode_road_def(0, &road_def, &label_offsets, &[], &coords);

        // Flags: NET_FLAG_UNK1 | NET_FLAG_NODINFO (NO oneway) = 0x04 | 0x40 = 0x44
        assert_eq!(record[3], 0x44, "bidirectional: UNK1 + NODINFO, no ONEWAY");
    }

    #[test]
    fn test_encode_road_def_label_offset_truncated() {
        // Label offset > 0x3FFFFF should be truncated
        let road_def = RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 6,
            road_class: 3,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("BigLabel".to_string()),
        };
        let mut label_offsets = HashMap::new();
        label_offsets.insert("BigLabel".to_string(), 0x00FF_FFFF); // > 22 bits

        let record = encode_road_def(0, &road_def, &label_offsets, &[], &[(45.0, 5.0)]);

        let label_val = u32::from_le_bytes([record[0], record[1], record[2], 0]);
        let offset_part = label_val & OFFSET_22BIT_MASK;
        assert_eq!(
            offset_part, OFFSET_22BIT_MASK,
            "offset > 22 bits must be truncated to 0x3FFFFF"
        );
    }

    // ── Task 3: NET3 sorted index ───────────────────────────────────────────

    #[test]
    fn test_net3_records_sorted() {
        let road_defs = vec![
            RoadDef {
                road_id: 1,
                polyline_idx: 0,
                speed: 5,
                road_class: 2,
                one_way: false,
                toll: false,
                roundabout: false,
                access_mask: 0,
                label: Some("Rue Zola".to_string()),
            },
            RoadDef {
                road_id: 2,
                polyline_idx: 1,
                speed: 5,
                road_class: 2,
                one_way: false,
                toll: false,
                roundabout: false,
                access_mask: 0,
                label: Some("Avenue Ampere".to_string()),
            },
            RoadDef {
                road_id: 3,
                polyline_idx: 2,
                speed: 3,
                road_class: 1,
                one_way: false,
                toll: false,
                roundabout: false,
                access_mask: 0,
                label: Some("Boulevard Hugo".to_string()),
            },
        ];
        let road_offsets = vec![0u32, 50, 120];
        let label_offsets: HashMap<String, u32> = HashMap::new(); // not used in NET3

        let data = build_net3_records(&road_defs, &road_offsets);

        // 3 roads with labels → 3 records × 3 bytes = 9 bytes
        assert_eq!(data.len(), 9);

        // Extract offsets from records (bits 0-21)
        let rec0 = u32::from_le_bytes([data[0], data[1], data[2], 0]) & OFFSET_22BIT_MASK;
        let rec1 = u32::from_le_bytes([data[3], data[4], data[5], 0]) & OFFSET_22BIT_MASK;
        let rec2 = u32::from_le_bytes([data[6], data[7], data[8], 0]) & OFFSET_22BIT_MASK;

        // Sorted: "Avenue Ampere" (offset=50), "Boulevard Hugo" (offset=120), "Rue Zola" (offset=0)
        assert_eq!(rec0, 50, "first sorted entry: Avenue Ampere → offset 50");
        assert_eq!(rec1, 120, "second sorted entry: Boulevard Hugo → offset 120");
        assert_eq!(rec2, 0, "third sorted entry: Rue Zola → offset 0");
    }

    #[test]
    fn test_net3_empty_when_no_labels() {
        let road_defs = vec![RoadDef {
            road_id: 1,
            polyline_idx: 0,
            speed: 3,
            road_class: 1,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0,
            label: None,
        }];
        let road_offsets = vec![0u32];
        let data = build_net3_records(&road_defs, &road_offsets);
        assert_eq!(data.len(), 0, "no labels → no NET3 records");
    }

    // ── Full build test ─────────────────────────────────────────────────────

    #[test]
    fn test_net_build_complete() {
        let (network, polylines) = make_simple_road_network();
        let mut label_offsets = HashMap::new();
        label_offsets.insert("D1075".to_string(), 1u32);

        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];

        let result = NetWriter::build(&network, &label_offsets, &subdiv_refs, &polylines);

        // Header = 55 bytes
        assert!(result.data.len() >= 55, "NET data must include 55-byte header");
        // Signature
        assert_eq!(&result.data[0x02..0x0C], b"GARMIN NET");
        // road_offsets should have 1 entry
        assert_eq!(result.road_offsets.len(), 1);
        assert_eq!(result.road_offsets[0], 0, "first road starts at offset 0 in NET1");

        // Verify NET1 length from header
        let net1_len =
            u32::from_le_bytes([result.data[0x19], result.data[0x1A], result.data[0x1B], result.data[0x1C]]);
        assert!(net1_len > 0, "NET1 section must be non-empty");

        // Verify NET3 exists
        let net3_len =
            u32::from_le_bytes([result.data[0x2B], result.data[0x2C], result.data[0x2D], result.data[0x2E]]);
        assert_eq!(net3_len, 3, "1 labeled road → 1 NET3 record = 3 bytes");
    }

    // ── Task 3: nod2_patch_positions ────────────────────────────────────────

    #[test]
    fn test_nod2_patch_positions_single_road() {
        let (network, polylines) = make_simple_road_network();
        let mut label_offsets = HashMap::new();
        label_offsets.insert("D1075".to_string(), 1u32);
        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];

        let result = NetWriter::build(&network, &label_offsets, &subdiv_refs, &polylines);

        // Must have exactly 1 patch position
        assert_eq!(result.nod2_patch_positions.len(), 1);
        let pos = result.nod2_patch_positions[0];

        // Bytes at pos must be the 0x0000 placeholder
        assert!(pos + 1 < result.data.len(), "patch position must be within data");
        assert_eq!(result.data[pos], 0x00, "NOD2 placeholder byte 0 must be 0x00");
        assert_eq!(result.data[pos + 1], 0x00, "NOD2 placeholder byte 1 must be 0x00");

        // Byte before the placeholder must be the 0x01 indicator
        assert!(pos >= 1, "indicator byte must precede placeholder");
        assert_eq!(result.data[pos - 1], 0x01, "indicator byte before placeholder must be 0x01");
    }

    #[test]
    fn test_nod2_patch_positions_after_patch() {
        let (network, polylines) = make_simple_road_network();
        let mut label_offsets = HashMap::new();
        label_offsets.insert("D1075".to_string(), 1u32);
        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];

        let result = NetWriter::build(&network, &label_offsets, &subdiv_refs, &polylines);
        let pos = result.nod2_patch_positions[0];

        // Simulate a patch: write offset 0x1234 at pos
        let mut data = result.data.clone();
        data[pos] = 0x34;
        data[pos + 1] = 0x12;

        let patched = u16::from_le_bytes([data[pos], data[pos + 1]]);
        assert_eq!(patched, 0x1234, "patched LE16 must read back as 0x1234");
        // Indicator byte must be unchanged
        assert_eq!(data[pos - 1], 0x01, "indicator byte must still be 0x01 after patch");
    }

    #[test]
    fn test_net_build_empty_network() {
        let network = RoadNetwork {
            nodes: vec![],
            arcs: vec![],
            road_defs: vec![],
        };
        let result = NetWriter::build(&network, &HashMap::new(), &[], &[]);

        assert_eq!(result.data.len(), 55, "empty network → header only (55 bytes)");
        assert_eq!(result.road_offsets.len(), 0);
        assert_eq!(result.nod2_patch_positions.len(), 0, "empty network → no patch positions");

        // NET1 length = 0
        let net1_len =
            u32::from_le_bytes([result.data[0x19], result.data[0x1A], result.data[0x1B], result.data[0x1C]]);
        assert_eq!(net1_len, 0);

        // NET3 length = 0
        let net3_len =
            u32::from_le_bytes([result.data[0x2B], result.data[0x2C], result.data[0x2D], result.data[0x2E]]);
        assert_eq!(net3_len, 0);
    }

    // ── Task 2.4 : Tests access_mask et toll ────────────────────────────────

    #[test]
    fn test_encode_road_def_with_access_mask() {
        // access_mask=0x0040 (denied_truck) → NET_FLAG_ACCESS activé + 2 bytes dans record
        let road_def = RoadDef {
            road_id: 10,
            polyline_idx: 0,
            speed: 4,
            road_class: 2,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0x0040, // denied_truck
            label: Some("Zone Test".to_string()),
        };
        let mut label_offsets = HashMap::new();
        label_offsets.insert("Zone Test".to_string(), 5u32);
        let coords = vec![(45.0, 5.0), (45.001, 5.0)];

        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];
        let record = encode_road_def(0, &road_def, &label_offsets, &subdiv_refs, &coords);

        // Flags: NET_FLAG_UNK1 | NET_FLAG_ACCESS | NET_FLAG_NODINFO = 0x04 | 0x20 | 0x40 = 0x64
        assert_eq!(record[3], 0x64, "flags doit avoir NET_FLAG_ACCESS (0x20) activé, got 0x{:02X}", record[3]);

        // Bytes 4-5 = access_mask LE16 = 0x0040
        let access_mask = u16::from_le_bytes([record[4], record[5]]);
        assert_eq!(access_mask, 0x0040, "access_mask LE16 doit être 0x0040 (denied_truck)");

        // Record plus long de 2 bytes qu'une route sans accès (même structure)
        let road_no_access = RoadDef { access_mask: 0, ..road_def.clone() };
        let record_no_access = encode_road_def(0, &road_no_access, &label_offsets, &subdiv_refs, &coords);
        assert_eq!(record.len(), record_no_access.len() + 2, "record avec access_mask 2 bytes plus long");
    }

    #[test]
    fn test_encode_road_def_no_access_mask() {
        // access_mask=0 → NET_FLAG_ACCESS absent, pas de bytes access dans le record
        let road_def = RoadDef {
            road_id: 11,
            polyline_idx: 0,
            speed: 5,
            road_class: 2,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("Route Libre".to_string()),
        };
        let mut label_offsets = HashMap::new();
        label_offsets.insert("Route Libre".to_string(), 8u32);
        let coords = vec![(45.0, 5.0), (45.001, 5.0)];

        let record = encode_road_def(0, &road_def, &label_offsets, &[], &coords);

        // Flags: NET_FLAG_UNK1 | NET_FLAG_NODINFO = 0x04 | 0x40 = 0x44
        // NET_FLAG_ACCESS (0x20) doit être absent
        assert_eq!(record[3] & 0x20, 0x00, "NET_FLAG_ACCESS ne doit pas être activé si access_mask=0");
        assert_eq!(record[3], 0x44, "flags sans access: UNK1 + NODINFO = 0x44");
    }

    #[test]
    fn test_encode_road_def_toll_flag() {
        // toll=true → NET_FLAG_TOLL (0x08) activé dans flags
        let road_def = RoadDef {
            road_id: 12,
            polyline_idx: 0,
            speed: 7,
            road_class: 4,
            one_way: false,
            toll: true,
            roundabout: false,
            access_mask: 0x0000,
            label: Some("A480".to_string()),
        };
        let mut label_offsets = HashMap::new();
        label_offsets.insert("A480".to_string(), 3u32);
        let coords = vec![(45.0, 5.0), (45.001, 5.0)];

        let record = encode_road_def(0, &road_def, &label_offsets, &[], &coords);

        // Flags: NET_FLAG_UNK1 | NET_FLAG_TOLL | NET_FLAG_NODINFO = 0x04 | 0x08 | 0x40 = 0x4C
        assert_eq!(record[3] & 0x08, 0x08, "NET_FLAG_TOLL (0x08) doit être activé si toll=true");
        assert_eq!(record[3], 0x4C, "flags avec toll: UNK1 + TOLL + NODINFO = 0x4C");

        // Vérifier que toll=false n'active pas le flag
        let road_no_toll = RoadDef { toll: false, ..road_def };
        let record_no_toll = encode_road_def(0, &road_no_toll, &label_offsets, &[], &coords);
        assert_eq!(record_no_toll[3] & 0x08, 0x00, "NET_FLAG_TOLL ne doit pas être activé si toll=false");
    }

    #[test]
    fn test_nod2_patch_position_correct_with_access() {
        // access_mask=0x0040 → 2 bytes access insérés après flags
        // La position NOD2 dans nod2_patch_positions doit pointer sur les 2 derniers bytes du record
        use crate::parser::mp_types::{MpPolyline, MpRoutingAttrs};

        let polylines = vec![MpPolyline {
            type_code: "0x06".to_string(),
            label: Some("Zone Restreinte".to_string()),
            end_level: None,
            coords: vec![(45.0, 5.0), (45.001, 5.0)],
            routing: Some(MpRoutingAttrs {
                road_id: Some("20".to_string()),
                route_param: Some("4,2,0,0,0,0,0,0,0,0,0,1".to_string()), // denied_truck
                speed_type: None,
                dir_indicator: Some(0),
                roundabout: None,
                max_height: None,
                max_weight: None,
                max_width: None,
                max_length: None,
            }),
            other_fields: HashMap::new(),
        }];

        let road_defs = vec![RoadDef {
            road_id: 20,
            polyline_idx: 0,
            speed: 4,
            road_class: 2,
            one_way: false,
            toll: false,
            roundabout: false,
            access_mask: 0x0040, // denied_truck
            label: Some("Zone Restreinte".to_string()),
        }];

        let network = RoadNetwork {
            nodes: vec![],
            arcs: vec![],
            road_defs,
        };

        let mut label_offsets = HashMap::new();
        label_offsets.insert("Zone Restreinte".to_string(), 1u32);
        let subdiv_refs = vec![SubdivRoadRef {
            road_def_idx: 0,
            subdiv_number: 1,
            polyline_index: 0,
        }];

        let result = NetWriter::build(&network, &label_offsets, &subdiv_refs, &polylines);

        assert_eq!(result.nod2_patch_positions.len(), 1);
        let pos = result.nod2_patch_positions[0];

        // Le placeholder NOD2 doit être à la fin du record (derniers 2 bytes)
        assert!(pos + 1 < result.data.len(), "position NOD2 dans les limites du binaire");
        assert_eq!(result.data[pos], 0x00, "placeholder NOD2 byte 0 = 0x00");
        assert_eq!(result.data[pos + 1], 0x00, "placeholder NOD2 byte 1 = 0x00");
        assert_eq!(result.data[pos - 1], 0x01, "indicateur 0x01 précède le placeholder");

        // Vérifier que les bytes access sont bien AVANT le placeholder NOD2
        // Record structure: 3B label + 1B flags(0x64) + 2B access + 3B length + 1B lvl + 3B div + 1B ind + 2B nod2
        // = 16 bytes total
        // Flags à offset 3 dans le record, access à offset 4
        let net1_start = 55usize; // NET_HEADER_SIZE
        let record_start = net1_start; // first road at offset 0
        let flags = result.data[record_start + 3];
        assert_eq!(flags & 0x20, 0x20, "NET_FLAG_ACCESS doit être activé dans le record avec access_mask");
        let access_val = u16::from_le_bytes([result.data[record_start + 4], result.data[record_start + 5]]);
        assert_eq!(access_val, 0x0040, "access_mask 0x0040 dans le record NET1");
    }
}
