// OverviewMap — overview tile with real polygon geometry for gmapsupp
//
// Many Garmin devices (Alpha 100, etc.) require an overview map in the gmapsupp
// to render any tiles. The overview map is a low-resolution tile covering the
// entire map set bounds, with bounding-box polygons (type 0x4A = background)
// representing each detail tile.

use super::common_header::{self, CommonHeader};
use super::lbl::LBL_HEADER_LEN;
use super::assembler::TileSubfiles;
use super::line_preparer;

const RGN_HEADER_LEN: u16 = 125;

/// Header TRE overview — parité SUD Alpha 100 : 120 bytes (MapID à l'offset 116 = fin
/// du header). Les firmwares Garmin refusent l'overview si header_length déclare 188
/// bytes alors que le MapID est à 116 (padding de zéros après MapID rejeté).
const OVERVIEW_TRE_HEADER_LEN: u16 = 120;

pub struct OverviewMapData {
    pub map_number: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
}

/// Generate an overview map with bounding-box polygons, parité SUD Alpha 100 :
/// header 120 bytes, 1 topdiv non-leaf + 3 leaf subdivs en strips ouest→est,
/// polygon overview 2 types (0x4A + 0x4B), copyright "DEFAULT\0" inline.
pub fn build_overview_map(tiles: &[TileSubfiles], map_id: u32, codepage: u16) -> OverviewMapData {
    let (north, east, south, west) = compute_merged_bounds(tiles);

    let tile_bounds: Vec<(i32, i32, i32, i32)> = tiles.iter()
        .filter(|t| t.tre.len() >= 33)
        .map(|t| common_header::read_tre_bounds(&t.tre))
        .collect();

    // Structure SUD Alpha 100 : leaf 1 dégénéré (placeholder, type polylines, w=0 h=1),
    // leaf 2 = coverage pleine avec TOUS les polygones, leaf 3 = coverage pleine vide (LAST).
    // Les centres des 3 leaves matchent le centre D038 (vs SUD qui a des leaves très proches
    // du centre bounds mais pas identiques — firmware tolère les 2).
    let shift_data = 24 - 16i32;
    let clat_center = (north + south) / 2;
    let clon_center = (east + west) / 2;
    let w_half_full = ((((east - west) / 2) >> shift_data) as u16).max(1);
    let h_half_full = ((((north - south) / 2) >> shift_data) as u16).max(1);

    let rgn_all = build_rgn_polygon_data(clat_center, clon_center, shift_data, &tile_bounds);
    let has_polygons = !rgn_all.is_empty();

    // leaf 1 : dégénéré (w=0 h=1) — marqueur SUD
    // leaf 2 : w/h pleins, type=0x80 (polygons), RGN=0, contient tous les polygones
    // leaf 3 : w/h pleins, type=0x80, RGN=size(rgn_all), vide, LAST
    let leaf_specs: Vec<(i32, i32, u16, u16, u8)> = vec![
        (clat_center, clon_center, 0u16, 1u16, 0x00),  // leaf 1 dégénéré, no content (évite parse polyline sur RGN@0 polygone)
        (clat_center, clon_center, w_half_full, h_half_full,
            if has_polygons { 0x80 } else { 0x00 }),    // leaf 2 main (polygons)
        (clat_center, clon_center, w_half_full, h_half_full, 0x80), // leaf 3 placeholder LAST
    ];
    let leaf_rgn_data: Vec<Vec<u8>> = vec![Vec::new(), rgn_all, Vec::new()];

    let tre = build_tre(north, east, south, west, map_id, &leaf_specs, &leaf_rgn_data);
    let rgn = build_rgn(&leaf_rgn_data);
    let lbl = build_lbl(codepage);

    OverviewMapData {
        map_number: format!("{:08}", map_id),
        tre, rgn, lbl,
    }
}

/// Divise [west, east] en `n` strips égaux. Retourne la liste [(west_i, east_i)].
fn split_strips(west: i32, east: i32, n: usize) -> Vec<(i32, i32)> {
    let total = (east - west) as i64;
    (0..n).map(|i| {
        let sw = west + ((total * i as i64 / n as i64) as i32);
        let se = if i + 1 == n { east } else { west + ((total * (i + 1) as i64 / n as i64) as i32) };
        (sw, se)
    }).collect()
}

fn strip_index(lon: i32, strips: &[(i32, i32)]) -> usize {
    for (i, (sw, se)) in strips.iter().enumerate() {
        if lon >= *sw && lon < *se { return i; }
    }
    strips.len() - 1
}

fn compute_merged_bounds(tiles: &[TileSubfiles]) -> (i32, i32, i32, i32) {
    let mut n = i32::MIN;
    let mut e = i32::MIN;
    let mut s = i32::MAX;
    let mut w = i32::MAX;
    for tile in tiles {
        if tile.tre.len() >= 33 {
            let (tn, te, ts, tw) = common_header::read_tre_bounds(&tile.tre);
            n = n.max(tn); e = e.max(te); s = s.min(ts); w = w.min(tw);
        }
    }
    (n, e, s, w)
}

// ── TRE: layout cgpsmapper (120-byte header + data sections en ordre SUD) ──

/// Construit un TRE overview 120-byte header, 1 topdiv + 3 leaf subdivs,
/// polygon overview 2 types (0x4A + 0x4B), copyright "DEFAULT\0" inline.
fn build_tre(
    north: i32, east: i32, south: i32, west: i32, map_id: u32,
    leaf_specs: &[(i32, i32, u16, u16, u8)],  // (clat, clon, w_half, h_half, type_byte)
    leaf_rgn_data: &[Vec<u8>],
) -> Vec<u8> {
    let shift_top = 24 - 14i32;
    let shift_data = 24 - 16i32;
    let clat_top = (north + south) / 2;
    let clon_top = (east + west) / 2;
    let w_top_half = (((east - west) >> shift_top) as u16 / 2).max(1);
    let h_top_half = (((north - south) >> shift_top) as u16 / 2).max(1);

    // Section data contents (parité stricte SUD Alpha 100)
    // "cgpsmapper version0096a\0DEFAULT\0" — la signature cgpsmapper semble obligatoire
    // sur Alpha 100 pour qu'un overview standalone soit accepté.
    let copyright_strings: Vec<u8> = b"cgpsmapper version0096a\0DEFAULT\0".to_vec();  // 32 bytes
    // 2 records × 3 bytes : LBL offsets reproduits tels quels depuis SUD
    let copyright_records: Vec<u8> = vec![0x01, 0x00, 0x00, 0x37, 0x0e, 0x00];  // 6 bytes
    let subdivs = build_subdivs(
        clat_top, clon_top, w_top_half, h_top_half,
        leaf_specs, leaf_rgn_data,
    );                                                                      // 62 bytes (1×16 + 3×14 + 4)
    let map_levels: Vec<u8> = {
        let mut v = Vec::with_capacity(8);
        v.extend_from_slice(&[0x01 | 0x80, 14, 1, 0]);  // level 1 inherited, bits=14, 1 subdiv
        v.extend_from_slice(&[0x00, 16, 3, 0]);         // level 0 leaf, bits=16, 3 subdivs
        _ = shift_data; v
    };
    let polygon_overview: Vec<u8> = vec![0x4A, 0x00, 0x4B, 0x00];          // 4 bytes = 2 entries
    let point_overview: Vec<u8> = vec![0x0B, 0x00, 0x00];                  // 3 bytes = 1 entry (parité SUD)

    // Offsets des sections dans le fichier (ordre SUD)
    let data_start = OVERVIEW_TRE_HEADER_LEN as u32;
    let copy_str_off = data_start;
    let copy_rec_off = copy_str_off + copyright_strings.len() as u32;
    let subdivs_off = copy_rec_off + copyright_records.len() as u32;
    let map_levels_off = subdivs_off + subdivs.len() as u32;
    let polygon_ov_off = map_levels_off + map_levels.len() as u32;
    let point_ov_off = polygon_ov_off + polygon_overview.len() as u32;

    // Construction du header (120 bytes)
    let mut buf = Vec::with_capacity(OVERVIEW_TRE_HEADER_LEN as usize);
    CommonHeader::new(OVERVIEW_TRE_HEADER_LEN, "GARMIN TRE").write(&mut buf);

    // Bounds @21 (12 bytes)
    common_header::write_i24(&mut buf, north);
    common_header::write_i24(&mut buf, east);
    common_header::write_i24(&mut buf, south);
    common_header::write_i24(&mut buf, west);

    // Map levels section @33 (8 bytes: off + size)
    common_header::write_section(&mut buf, map_levels_off, map_levels.len() as u32);
    // Subdivisions section @41 (8 bytes)
    common_header::write_section(&mut buf, subdivs_off, subdivs.len() as u32);
    // Copyright section @49 (10 bytes: off + size + rec_size)
    common_header::write_section(&mut buf, copy_rec_off, copyright_records.len() as u32);
    buf.extend_from_slice(&3u16.to_le_bytes());
    // Reserved u32 @59
    buf.extend_from_slice(&[0u8; 4]);
    // POI flags @63
    buf.push(0x01);
    // Display priority u24 @64
    common_header::write_u24(&mut buf, 0x19);
    // Map format marker @67 (overview)
    buf.extend_from_slice(&0x00040101u32.to_le_bytes());
    // Reserved u16 + byte @71-73
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.push(0x00);

    // Polyline overview @74 (14 bytes: off + size + rec_size + 4 extras), size=0
    assert_eq!(buf.len(), 74);
    common_header::write_section(&mut buf, polygon_ov_off, 0);
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    // Polygon overview @88 (14 bytes)
    assert_eq!(buf.len(), 88);
    common_header::write_section(&mut buf, polygon_ov_off, polygon_overview.len() as u32);
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    // Point overview @102 (14 bytes)
    assert_eq!(buf.len(), 102);
    common_header::write_section(&mut buf, point_ov_off, point_overview.len() as u32);
    buf.extend_from_slice(&3u16.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    // MapID @116 (4 bytes)
    assert_eq!(buf.len(), 116);
    buf.extend_from_slice(&map_id.to_le_bytes());
    assert_eq!(buf.len(), OVERVIEW_TRE_HEADER_LEN as usize);

    // Data sections dans l'ordre SUD
    buf.extend_from_slice(&copyright_strings);
    buf.extend_from_slice(&copyright_records);
    buf.extend_from_slice(&subdivs);
    buf.extend_from_slice(&map_levels);
    buf.extend_from_slice(&polygon_overview);
    buf.extend_from_slice(&point_overview);
    // Padding final : parité stricte SUD (301 bytes total). Certains firmwares
    // Alpha 100 semblent valider la taille du sous-fichier TRE overview.
    const SUD_TRE_TOTAL_LEN: usize = 301;
    if buf.len() < SUD_TRE_TOTAL_LEN {
        buf.resize(SUD_TRE_TOTAL_LEN, 0);
    }
    buf
}

/// Construit la section subdivs : 1 non-leaf topdiv (16 bytes) + 3 leaf (14 bytes chacun)
/// + terminator 4 bytes = 62 bytes total.
fn build_subdivs(
    clat_top: i32, clon_top: i32, w_top_half: u16, h_top_half: u16,
    leaf_specs: &[(i32, i32, u16, u16, u8)],
    leaf_rgn_data: &[Vec<u8>],
) -> Vec<u8> {
    assert_eq!(leaf_specs.len(), 3);
    let mut buf = Vec::with_capacity(62);

    // Subdiv 1 : topdiv non-leaf (16 bytes)
    put_u24(&mut buf, 0);                // RGN offset = 0 (pas de contenu direct)
    buf.push(0x00);                      // content flags = 0 (enfants portent le contenu)
    put_i24(&mut buf, clon_top);
    put_i24(&mut buf, clat_top);
    buf.extend_from_slice(&(w_top_half | 0x8000).to_le_bytes()); // MSB = last at level
    buf.extend_from_slice(&h_top_half.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());  // next_level = subdiv 2

    // Subdivs 2,3,4 : leafs (14 bytes chacun). RGN offset cumulatif.
    let mut rgn_off: u32 = 0;
    for (i, ((clat, clon, w_half, h_half, type_byte), rgn_bytes)) in
        leaf_specs.iter().zip(leaf_rgn_data.iter()).enumerate()
    {
        put_u24(&mut buf, rgn_off);
        buf.push(*type_byte);
        put_i24(&mut buf, *clon);
        put_i24(&mut buf, *clat);
        let is_last = i == 2;
        let w_field = if is_last { w_half | 0x8000 } else { *w_half };
        buf.extend_from_slice(&w_field.to_le_bytes());
        buf.extend_from_slice(&h_half.to_le_bytes());
        rgn_off += rgn_bytes.len() as u32;
    }

    // Terminator 4 bytes
    buf.extend_from_slice(&[0u8; 4]);
    assert_eq!(buf.len(), 62);
    buf
}

// ── RGN: data section = concat des polygones des 3 leafs ──

fn build_rgn(leaf_rgn_data: &[Vec<u8>]) -> Vec<u8> {
    let total_data: Vec<u8> = leaf_rgn_data.iter().flatten().copied().collect();
    let mut buf = Vec::with_capacity(RGN_HEADER_LEN as usize + total_data.len());
    CommonHeader::new(RGN_HEADER_LEN, "GARMIN RGN").write(&mut buf);
    common_header::write_section(&mut buf, RGN_HEADER_LEN as u32, total_data.len() as u32);
    common_header::pad_to(&mut buf, RGN_HEADER_LEN as usize);
    buf.extend_from_slice(&total_data);
    buf
}

/// Build RGN polygon records: one type 0x4A background polygon per tile
fn build_rgn_polygon_data(
    subdiv_center_lat: i32,
    subdiv_center_lon: i32,
    shift: i32,
    tile_bounds: &[(i32, i32, i32, i32)],
) -> Vec<u8> {
    let mut polygons_data = Vec::new();

    for &(tn, te, ts, tw) in tile_bounds {
        // Build a rectangle polygon: SW → SE → NE → NW
        // Garmin closes polygons implicitly (first point connects back to last)
        let corners: [(i32, i32); 4] = [
            (ts, tw), // SW (lat, lon)
            (ts, te), // SE
            (tn, te), // NE
            (tn, tw), // NW
        ];

        // First point: delta from subdivision center
        let half = (1i32 << shift) / 2;
        let first_dx = ((corners[0].1 - subdiv_center_lon + half) >> shift).clamp(-32768, 32767);
        let first_dy = ((corners[0].0 - subdiv_center_lat + half) >> shift).clamp(-32768, 32767);

        // Remaining deltas (each relative to previous point, in shifted units)
        let mut deltas = Vec::new();
        for i in 1..corners.len() {
            let prev_lon = (corners[i-1].1 + half) >> shift;
            let prev_lat = (corners[i-1].0 + half) >> shift;
            let cur_lon = (corners[i].1 + half) >> shift;
            let cur_lat = (corners[i].0 + half) >> shift;
            deltas.push((cur_lon - prev_lon, cur_lat - prev_lat));
        }

        if let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) {
            // Type byte: 0x4A (background/transparent polygon)
            let mut type_byte = 0x4Au8;
            if bitstream.len() > 256 {
                type_byte |= 0x80; // 2-byte length flag
            }
            polygons_data.push(type_byte);

            // Label offset: 0 (no label)
            polygons_data.push(0x00);
            polygons_data.push(0x00);
            polygons_data.push(0x00);

            // First point delta (signed i16 LE)
            polygons_data.extend_from_slice(&(first_dx as i16).to_le_bytes());
            polygons_data.extend_from_slice(&(first_dy as i16).to_le_bytes());

            // Bitstream length (Garmin convention: stored as actual_bytes - 1)
            let blen = bitstream.len() - 1;
            if blen >= 256 {
                polygons_data.extend_from_slice(&(blen as u16).to_le_bytes());
            } else {
                polygons_data.push(blen as u8);
            }

            polygons_data.extend_from_slice(&bitstream);
        }
    }

    polygons_data
}

// ── LBL: minimal with proper PlacesHeader ──

fn build_lbl(codepage: u16) -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(LBL_HEADER_LEN, "GARMIN LBL");
    common.write(&mut buf);

    let label_data: Vec<u8> = vec![0x00];
    let lbl_off = LBL_HEADER_LEN as u32;
    let lbl_size = label_data.len() as u32;
    let lbl_end = lbl_off + lbl_size;

    common_header::write_section(&mut buf, lbl_off, lbl_size); // @21
    buf.push(0x00); // mult @29
    buf.push(6);    // enc=Format6 @30

    // PlacesHeader — all empty, valid offsets
    for &rec in &[3u16, 5, 5, 4] { // Country, Region, City, POI index
        write_empty_sec(&mut buf, lbl_end, rec);
    }
    // POI properties (13 bytes)
    buf.extend_from_slice(&lbl_end.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 5]);
    for &rec in &[4u16, 3, 6, 5, 3] { // POI type, Zip, Highway, Exit, HwyData
        write_empty_sec(&mut buf, lbl_end, rec);
    }

    assert_eq!(buf.len(), 170);
    buf.extend_from_slice(&codepage.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]); // sort IDs
    buf.extend_from_slice(&(LBL_HEADER_LEN as u32).to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&lbl_end.to_le_bytes());
    buf.extend_from_slice(&[0u8; 8]);
    assert_eq!(buf.len(), LBL_HEADER_LEN as usize);

    buf.extend_from_slice(&label_data);
    buf
}

fn put_u24(buf: &mut Vec<u8>, v: u32) { buf.extend_from_slice(&v.to_le_bytes()[..3]); }
fn put_i24(buf: &mut Vec<u8>, v: i32) { buf.extend_from_slice(&v.to_le_bytes()[..3]); }
fn write_empty_sec(buf: &mut Vec<u8>, end: u32, rec: u16) {
    buf.extend_from_slice(&end.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&rec.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::assembler::TileSubfiles;

    fn make_test_tile(north: i32, east: i32, south: i32, west: i32) -> TileSubfiles {
        // Build a minimal TRE with bounds at offsets 21-32
        let mut tre = vec![0u8; 33];
        // header_len at offset 0
        tre[0] = 188; tre[1] = 0;
        // type at offset 2
        tre[2..12].copy_from_slice(b"GARMIN TRE");
        // bounds at offset 21
        let nb = north.to_le_bytes();
        tre[21] = nb[0]; tre[22] = nb[1]; tre[23] = nb[2];
        let eb = east.to_le_bytes();
        tre[24] = eb[0]; tre[25] = eb[1]; tre[26] = eb[2];
        let sb = south.to_le_bytes();
        tre[27] = sb[0]; tre[28] = sb[1]; tre[29] = sb[2];
        let wb = west.to_le_bytes();
        tre[30] = wb[0]; tre[31] = wb[1]; tre[32] = wb[2];

        TileSubfiles {
            map_number: "11000001".to_string(),
            description: "Test tile".to_string(),
            tre,
            rgn: vec![0u8; 125],
            lbl: vec![0u8; 196],
            net: None,
            nod: None,
            dem: None,
        }
    }

    #[test]
    fn test_overview_tre_valid() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, 12345, 1252);
        assert!(ov.tre.len() >= OVERVIEW_TRE_HEADER_LEN as usize);
        assert_eq!(&ov.tre[2..12], b"GARMIN TRE");
        assert_eq!(u32::from_le_bytes([ov.tre[67], ov.tre[68], ov.tre[69], ov.tre[70]]), 0x00040101);

        // Map levels section header @ 0x21 (offset) / 0x25 (size)
        let levels_off = u32::from_le_bytes([ov.tre[0x21], ov.tre[0x22], ov.tre[0x23], ov.tre[0x24]]) as usize;
        let levels_size = u32::from_le_bytes([ov.tre[0x25], ov.tre[0x26], ov.tre[0x27], ov.tre[0x28]]) as usize;
        assert_eq!(levels_size, 8, "2 paliers × 4 bytes attendus");
        // Level 1 (inherited topdiv) : [0x81, 14, 1, 0]
        assert_eq!(ov.tre[levels_off], 0x81, "level 1 flag = inherited (0x81)");
        assert_eq!(ov.tre[levels_off + 1], 14, "level 1 bits = 14 (parité SUD)");
        // Level 0 (leaf data) : [0x00, 16, 1, 0]
        assert_eq!(ov.tre[levels_off + 4], 0x00, "level 0 flag = leaf (0x00)");
        assert_eq!(ov.tre[levels_off + 5], 16, "level 0 bits = 16 (parité SUD)");
    }

    #[test]
    fn test_overview_rgn_has_polygon_data() {
        let tiles = vec![
            make_test_tile(2143196, 262632, 2138930, 255409),
            make_test_tile(2148000, 270000, 2143196, 262632),
        ];
        let ov = build_overview_map(&tiles, 12345, 1252);
        // RGN should be larger than just the header (125 bytes)
        assert!(ov.rgn.len() > RGN_HEADER_LEN as usize,
            "RGN should contain polygon data, got {} bytes", ov.rgn.len());
        // Data section size should be > 0
        let data_size = u32::from_le_bytes([ov.rgn[25], ov.rgn[26], ov.rgn[27], ov.rgn[28]]);
        assert!(data_size > 0, "RGN data section size should be > 0");
    }

    #[test]
    fn test_overview_rgn_polygon_type() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, 12345, 1252);
        // First byte of data section should be polygon type 0x4A
        let data_start = RGN_HEADER_LEN as usize;
        assert_eq!(ov.rgn[data_start] & 0x7F, 0x4A,
            "First polygon type should be 0x4A (background)");
    }

    #[test]
    fn test_overview_lbl_sections() {
        let lbl = build_lbl(1252);
        assert_eq!(lbl.len(), LBL_HEADER_LEN as usize + 1);
        let country_off = u32::from_le_bytes([lbl[31], lbl[32], lbl[33], lbl[34]]);
        assert_eq!(country_off, LBL_HEADER_LEN as u32 + 1);
    }

    #[test]
    fn test_overview_map_number_format() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, 11001855, 1252);
        assert_eq!(ov.map_number, "11001855");
    }
}
