// OverviewMap — overview tile with real polygon geometry for gmapsupp
//
// Many Garmin devices (Alpha 100, etc.) require an overview map in the gmapsupp
// to render any tiles. The overview map is a low-resolution tile covering the
// entire map set bounds, with bounding-box polygons (type 0x4A = background)
// representing each detail tile.

use super::common_header::{self, CommonHeader};
use super::tre::TRE_HEADER_LEN;
use super::lbl::LBL_HEADER_LEN;
use super::assembler::TileSubfiles;
use super::line_preparer;

const RGN_HEADER_LEN: u16 = 125;

pub struct OverviewMapData {
    pub map_number: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
}

/// Generate an overview map with bounding-box polygons for each tile
pub fn build_overview_map(tiles: &[TileSubfiles], map_id: u32, codepage: u16) -> OverviewMapData {
    let (north, east, south, west) = compute_merged_bounds(tiles);

    // Collect per-tile bounds for polygon generation
    let tile_bounds: Vec<(i32, i32, i32, i32)> = tiles.iter()
        .filter(|t| t.tre.len() >= 33)
        .map(|t| common_header::read_tre_bounds(&t.tre))
        .collect();

    let tre = build_tre(north, east, south, west, map_id, &tile_bounds);
    let rgn = build_rgn(north, east, south, west, &tile_bounds);
    let lbl = build_lbl(codepage);

    OverviewMapData {
        map_number: format!("{:08}", map_id),
        tre, rgn, lbl,
    }
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

// ── TRE: two levels, subdivision with polygon content ──

fn build_tre(north: i32, east: i32, south: i32, west: i32, map_id: u32,
             tile_bounds: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(TRE_HEADER_LEN, "GARMIN TRE");
    common.write(&mut buf);

    // Bounds @21
    common_header::write_i24(&mut buf, north);
    common_header::write_i24(&mut buf, east);
    common_header::write_i24(&mut buf, south);
    common_header::write_i24(&mut buf, west);

    // Two levels: level 1 = inherited topdiv, level 0 = data level with polygons
    let mut levels_data = Vec::new();
    levels_data.extend_from_slice(&[0x01 | 0x80, 15, 1, 0]); // level 1 inherited, res=15, 1 subdiv
    levels_data.extend_from_slice(&[0x00, 17, 1, 0]);          // level 0, res=17, 1 subdiv

    let shift_top = 24 - 15i32;
    let shift_data = 24 - 17i32;
    let clat_top = (((north + south) / 2) >> shift_top) << shift_top;
    let clon_top = (((east + west) / 2) >> shift_top) << shift_top;
    let clat_dat = (((north + south) / 2) >> shift_data) << shift_data;
    let clon_dat = (((east + west) / 2) >> shift_data) << shift_data;
    let w_top = (((east - west) >> shift_top) as u16).max(1);
    let h_top = (((north - south) >> shift_top) as u16).max(1);
    let w_dat = (((east - west) >> shift_data) as u16).max(1);
    let h_dat = (((north - south) >> shift_data) as u16).max(1);

    // Build RGN polygon data to know its size for the subdiv RGN pointer
    let rgn_polygon_data = build_rgn_polygon_data(
        clat_dat, clon_dat, shift_data, tile_bounds
    );
    let has_polygons = !rgn_polygon_data.is_empty();

    let mut subdivs_data = Vec::new();
    // Subdiv 1 (topdiv, level 1): 16 bytes — has children
    put_u24(&mut subdivs_data, 0);
    subdivs_data.push(0x00); // no content in topdiv
    put_i24(&mut subdivs_data, clon_top);
    put_i24(&mut subdivs_data, clat_top);
    subdivs_data.extend_from_slice(&(w_top | 0x8000).to_le_bytes());
    subdivs_data.extend_from_slice(&h_top.to_le_bytes());
    subdivs_data.extend_from_slice(&2u16.to_le_bytes()); // next_level = subdiv 2

    // Subdiv 2 (level 0): 14 bytes — leaf, with polygon content
    // RGN offset: start of polygon data in RGN data section
    put_u24(&mut subdivs_data, 0); // RGN offset placeholder (will be 0 = start of data section)
    let content_flags = if has_polygons { 0x80u8 } else { 0x00u8 }; // 0x80 = has polygons
    subdivs_data.push(content_flags);
    put_i24(&mut subdivs_data, clon_dat);
    put_i24(&mut subdivs_data, clat_dat);
    subdivs_data.extend_from_slice(&(w_dat | 0x8000).to_le_bytes());
    subdivs_data.extend_from_slice(&h_dat.to_le_bytes());
    // 4-byte terminator for last subdiv at this level
    subdivs_data.extend_from_slice(&0u32.to_le_bytes());

    let mut offset = TRE_HEADER_LEN as u32;

    // Map levels @33
    common_header::write_section(&mut buf, offset, levels_data.len() as u32);
    offset += levels_data.len() as u32;
    // Subdivisions @41
    common_header::write_section(&mut buf, offset, subdivs_data.len() as u32);
    offset += subdivs_data.len() as u32;
    // Copyright @49 (empty)
    common_header::write_section(&mut buf, offset, 0);
    buf.extend_from_slice(&3u16.to_le_bytes());
    // Reserved @59
    buf.extend_from_slice(&0u32.to_le_bytes());
    // POI flags @63
    buf.push(0x01);
    // Display priority @64
    common_header::write_u24(&mut buf, 0x19);
    // Map format marker @67 — overview = 0x00040101
    buf.extend_from_slice(&0x00040101u32.to_le_bytes());
    // Reserved @71
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.push(0x00);

    // Polyline overview @74 (empty)
    assert_eq!(buf.len(), 74);
    common_header::write_section(&mut buf, offset, 0);
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    // Polygon overview @88 — one entry for type 0x4A
    assert_eq!(buf.len(), 88);
    let polygon_overview = if has_polygons {
        vec![0x4A, 0x00] // type 0x4A, max_level 0
    } else {
        Vec::new()
    };
    common_header::write_section(&mut buf, offset, polygon_overview.len() as u32);
    offset += polygon_overview.len() as u32;
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    // Point overview @102 (empty)
    assert_eq!(buf.len(), 102);
    common_header::write_section(&mut buf, offset, 0);
    buf.extend_from_slice(&3u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    // MapID @116
    buf.extend_from_slice(&map_id.to_le_bytes());
    // Reserved @120
    buf.extend_from_slice(&0u32.to_le_bytes());
    // Pad to header
    common_header::pad_to(&mut buf, TRE_HEADER_LEN as usize);

    // Section data
    buf.extend_from_slice(&levels_data);
    buf.extend_from_slice(&subdivs_data);
    buf.extend_from_slice(&polygon_overview);
    buf
}

// ── RGN: real polygon data (bounding boxes) ──

fn build_rgn(north: i32, east: i32, south: i32, west: i32,
             tile_bounds: &[(i32, i32, i32, i32)]) -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(RGN_HEADER_LEN, "GARMIN RGN");
    common.write(&mut buf);

    // Build polygon data for subdivision 2 (data level)
    let shift = 24 - 17i32;
    let clat = (((north + south) / 2) >> shift) << shift;
    let clon = (((east + west) / 2) >> shift) << shift;
    let polygon_data = build_rgn_polygon_data(clat, clon, shift, tile_bounds);

    // Data section @21: offset=header_len, size=polygon_data.len()
    common_header::write_section(&mut buf, RGN_HEADER_LEN as u32, polygon_data.len() as u32);
    common_header::pad_to(&mut buf, RGN_HEADER_LEN as usize);

    buf.extend_from_slice(&polygon_data);
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
        assert!(ov.tre.len() >= TRE_HEADER_LEN as usize);
        assert_eq!(&ov.tre[2..12], b"GARMIN TRE");
        assert_eq!(u32::from_le_bytes([ov.tre[67], ov.tre[68], ov.tre[69], ov.tre[70]]), 0x00040101);
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
