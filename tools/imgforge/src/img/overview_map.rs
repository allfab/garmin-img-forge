// OverviewMap — minimal overview tile for gmapsupp
//
// Many Garmin devices (Alpha 100, etc.) require an overview map in the gmapsupp
// to render any tiles. The overview map is a low-resolution tile covering the
// entire map set bounds, with minimal or no geometry.

use super::common_header::{self, CommonHeader};
use super::tre::TRE_HEADER_LEN;
use super::lbl::LBL_HEADER_LEN;
use super::assembler::TileSubfiles;

const RGN_HEADER_LEN: u16 = 125;

pub struct OverviewMapData {
    pub map_number: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
}

/// Generate a minimal overview map from tile bounds
pub fn build_overview_map(tiles: &[TileSubfiles], map_id: u32, codepage: u16) -> OverviewMapData {
    let (north, east, south, west) = compute_merged_bounds(tiles);
    let tre = build_tre(north, east, south, west, map_id);
    let rgn = build_rgn();
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

// ── TRE: single inherited level, single empty subdivision ──

fn build_tre(north: i32, east: i32, south: i32, west: i32, map_id: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(TRE_HEADER_LEN, "GARMIN TRE");
    common.write(&mut buf);

    // Bounds @21
    common_header::write_i24(&mut buf, north);
    common_header::write_i24(&mut buf, east);
    common_header::write_i24(&mut buf, south);
    common_header::write_i24(&mut buf, west);

    // Two levels (like MapSetToolkit): level 1 = inherited topdiv, level 0 = empty data level
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

    let mut subdivs_data = Vec::new();
    // Subdiv 1 (topdiv, level 1): 16 bytes — has children
    put_u24(&mut subdivs_data, 0);
    subdivs_data.push(0x00); // no content
    put_i24(&mut subdivs_data, clon_top);
    put_i24(&mut subdivs_data, clat_top);
    subdivs_data.extend_from_slice(&(w_top | 0x8000).to_le_bytes());
    subdivs_data.extend_from_slice(&h_top.to_le_bytes());
    subdivs_data.extend_from_slice(&2u16.to_le_bytes()); // next_level = subdiv 2

    // Subdiv 2 (level 0): 14 bytes — leaf, no content
    put_u24(&mut subdivs_data, 0);
    subdivs_data.push(0x00); // no content
    put_i24(&mut subdivs_data, clon_dat);
    put_i24(&mut subdivs_data, clat_dat);
    subdivs_data.extend_from_slice(&(w_dat | 0x8000).to_le_bytes());
    subdivs_data.extend_from_slice(&h_dat.to_le_bytes());
    // 4-byte terminator
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
    // Polygon overview @88 (empty)
    assert_eq!(buf.len(), 88);
    common_header::write_section(&mut buf, offset, 0);
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
    buf
}

// ── RGN: header only, no body data ──

fn build_rgn() -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(RGN_HEADER_LEN, "GARMIN RGN");
    common.write(&mut buf);
    // Data section @21: offset=header_len, size=0
    common_header::write_section(&mut buf, RGN_HEADER_LEN as u32, 0);
    common_header::pad_to(&mut buf, RGN_HEADER_LEN as usize);
    buf
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
    #[test]
    fn test_overview_tre_valid() {
        let tre = build_tre(2143196, 262632, 2138930, 255409, 12345);
        assert!(tre.len() >= TRE_HEADER_LEN as usize);
        assert_eq!(&tre[2..12], b"GARMIN TRE");
        assert_eq!(u32::from_le_bytes([tre[67], tre[68], tre[69], tre[70]]), 0x00040101);
    }
    #[test]
    fn test_overview_rgn_header_only() {
        let rgn = build_rgn();
        assert_eq!(rgn.len(), RGN_HEADER_LEN as usize);
    }
    #[test]
    fn test_overview_lbl_sections() {
        let lbl = build_lbl(1252);
        assert_eq!(lbl.len(), LBL_HEADER_LEN as usize + 1);
        // Country section offset should be label_end, not 0
        let country_off = u32::from_le_bytes([lbl[31], lbl[32], lbl[33], lbl[34]]);
        assert_eq!(country_off, LBL_HEADER_LEN as u32 + 1);
    }
}
