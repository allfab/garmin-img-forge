// OverviewMap — overview sub-map with real simplified geometries for gmapsupp
//
// Many Garmin devices (Alpha 100) require an overview map in the gmapsupp to
// render any tiles at low zoom levels. This module aggregates features from all
// detail tiles' .mp files, selects those eligible for the overview (EndLevel
// structural criterion + optional CLI whitelist), and writes a 2-level TRE/RGN/LBL
// sub-map with bits 17 (inherited parent) / 18 (leaf data).
//
// Overview structure replicates the mkgmap osmmap.img layout measured from the
// reference build: map format marker 0x00040101, 2 map levels, typed overview
// tables (point/polyline/polygon) populated from the distinct type_codes present.

use std::collections::HashSet;

use super::assembler::TileSubfiles;
use super::common_header::{self, CommonHeader};
use super::coord::Coord;
use super::lbl::LBL_HEADER_LEN;
use super::line_preparer;
use super::point::Point;
use super::polygon::Polygon;
use super::polyline::Polyline;
use super::rgn::RgnWriter;
use super::tre::TRE_HEADER_LEN;
use crate::parser::mp_types::MpFile;

// ── Feature view piped from .mp to overview builder ───────────────────────

/// Feature extracted from a parsed `.mp` for overview rendering.
/// Flat: overview v1 does not carry labels.
#[derive(Debug, Clone)]
pub struct OverviewFeature {
    pub type_code: u32,
    pub is_point: bool,
    pub is_line: bool,
    pub is_polygon: bool,
    pub coords: Vec<Coord>,
}

/// Extract the features eligible for the overview from one `.mp`.
///
/// Two filters:
/// 1. Structural: `end_level >= detail_max_level`. Scope rules (garmin-rules.yaml)
///    already control EndLevel per feature, so imgforge stays scope-agnostic.
/// 2. Optional whitelist: if `type_whitelist` is `Some`, only features whose
///    `type_code` is in the set are retained.
///
/// For lines/polygons, the geometry is taken from bucket `overview_level`, with
/// a fallback down to a coarser bucket if the requested one is absent.
pub fn extract_overview_features(
    mp: &MpFile,
    overview_level: u8,
    detail_max_level: u8,
    type_whitelist: Option<&HashSet<u32>>,
) -> Vec<OverviewFeature> {
    let mut out = Vec::new();

    let passes_whitelist = |t: u32| -> bool {
        type_whitelist.map(|w| w.contains(&t)).unwrap_or(true)
    };
    let passes_endlevel = |el: Option<u8>| -> bool {
        el.unwrap_or(0) >= detail_max_level
    };

    for p in &mp.points {
        if !passes_endlevel(p.end_level) { continue; }
        if !passes_whitelist(p.type_code) { continue; }
        out.push(OverviewFeature {
            type_code: p.type_code,
            is_point: true,
            is_line: false,
            is_polygon: false,
            coords: vec![p.coord],
        });
    }

    for pl in &mp.polylines {
        if !passes_endlevel(pl.end_level) { continue; }
        if !passes_whitelist(pl.type_code) { continue; }
        if let Some(coords) = pick_bucket(&pl.geometries, overview_level, 2) {
            out.push(OverviewFeature {
                type_code: pl.type_code,
                is_point: false,
                is_line: true,
                is_polygon: false,
                coords,
            });
        }
    }

    for pg in &mp.polygons {
        if !passes_endlevel(pg.end_level) { continue; }
        if !passes_whitelist(pg.type_code) { continue; }
        if let Some(coords) = pick_bucket(&pg.geometries, overview_level, 3) {
            out.push(OverviewFeature {
                type_code: pg.type_code,
                is_point: false,
                is_line: false,
                is_polygon: true,
                coords,
            });
        }
    }

    out
}

/// Priorité pour choisir le bucket de géométrie :
/// 1. Exact `level`.
/// 2. Plus grossier (N > level) — le plus proche au-dessus ; toujours moins dense, donc sûr
///    pour une overview.
/// 3. Un seul palier plus fin (`level - 1`) — fallback AC 3 quand seul Data5 existe pour
///    un overview_level=6. Au-delà d'un palier d'écart on rejette pour éviter qu'une
///    feature ne gonfle le RGN overview avec une géométrie Data0/Data1.
///
/// Note mpforge : les paliers sont normalement contigus (`fill_level_gaps`), donc le cas 3
/// ne se déclenche qu'en configuration dégénérée ou sur des `MpFile` synthétiques de test.
fn pick_bucket(
    geoms: &std::collections::BTreeMap<u8, Vec<Coord>>,
    level: u8,
    min_coords: usize,
) -> Option<Vec<Coord>> {
    let candidate = geoms
        .get(&level)
        .cloned()
        .or_else(|| geoms.range((level.saturating_add(1))..).next().map(|(_, v)| v.clone()))
        .or_else(|| {
            if level == 0 { None }
            else { geoms.get(&(level - 1)).cloned() }
        })?;
    if candidate.len() >= min_coords {
        Some(candidate)
    } else {
        None
    }
}

// ── Overview sub-map output bundle ────────────────────────────────────────

pub struct OverviewMapData {
    pub map_number: String,
    pub tre: Vec<u8>,
    pub rgn: Vec<u8>,
    pub lbl: Vec<u8>,
}

/// Build the overview sub-map from tile bounds + aggregated features.
pub fn build_overview_map(
    tiles: &[TileSubfiles],
    features: &[OverviewFeature],
    map_id: u32,
    codepage: u16,
) -> OverviewMapData {
    let (north, east, south, west) = compute_merged_bounds(tiles);

    // Level 0 is the leaf data level with bits=18.
    let leaf_resolution: i32 = 18;
    let shift_leaf = 24 - leaf_resolution;
    let clat_leaf = center_aligned(north, south, shift_leaf);
    let clon_leaf = center_aligned(east, west, shift_leaf);

    // Encode all features into a single leaf subdivision RGN block.
    let (rgn, leaf_flags) = build_rgn(features, clat_leaf, clon_leaf, shift_leaf);

    let tre = build_tre(
        north, east, south, west, map_id,
        features, leaf_flags, clat_leaf, clon_leaf,
    );
    let lbl = build_lbl(codepage);

    OverviewMapData {
        map_number: format!("{:08}", map_id),
        tre, rgn, lbl,
    }
}

fn center_aligned(hi: i32, lo: i32, shift: i32) -> i32 {
    let c = (hi + lo) / 2;
    if shift <= 0 { c } else { (c >> shift) << shift }
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
    if n == i32::MIN {
        (0, 0, 0, 0)
    } else {
        (n, e, s, w)
    }
}

// ── RGN: encode real geometries into one leaf subdivision ────────────────

/// Returns (RGN bytes, leaf_content_flags).
/// `leaf_content_flags` = bitmask of HAS_POINTS(0x10)|HAS_POLYLINES(0x40)|HAS_POLYGONS(0x80).
fn build_rgn(
    features: &[OverviewFeature],
    subdiv_center_lat: i32,
    subdiv_center_lon: i32,
    shift: i32,
) -> (Vec<u8>, u8) {
    let mut points_data: Vec<u8> = Vec::new();
    let mut polylines_data: Vec<u8> = Vec::new();
    let mut polygons_data: Vec<u8> = Vec::new();

    for f in features {
        if f.is_point {
            encode_point(&mut points_data, f, subdiv_center_lat, subdiv_center_lon, shift);
        } else if f.is_line {
            encode_line(&mut polylines_data, f, subdiv_center_lat, subdiv_center_lon, shift);
        } else if f.is_polygon {
            encode_polygon(&mut polygons_data, f, subdiv_center_lat, subdiv_center_lon, shift);
        }
    }

    let mut flags: u8 = 0;
    if !points_data.is_empty() { flags |= 0x10; }
    if !polylines_data.is_empty() { flags |= 0x40; }
    if !polygons_data.is_empty() { flags |= 0x80; }

    // Use RgnWriter to compose the subdivision (handles the section pointers).
    let mut rgn_writer = RgnWriter::new();
    rgn_writer.write_subdivision(&points_data, &[], &polylines_data, &polygons_data);
    (rgn_writer.build(), flags)
}

fn encode_point(out: &mut Vec<u8>, f: &OverviewFeature, clat: i32, clon: i32, shift: i32) {
    if f.coords.is_empty() { return; }
    if f.type_code >= 0x10000 {
        // Extended point — skipped in v1 (overview keeps standard types only for Alpha 100 parity).
        return;
    }
    let mut p = Point::new(f.type_code, f.coords[0]);
    // Subtype, if any, is encoded only for standard 0x100-0xFFFF types.
    if f.type_code >= 0x100 {
        p.has_sub_type = true;
        p.sub_type = (f.type_code & 0xFF) as u8;
        p.type_code = (f.type_code >> 8) & 0xFF;
    }
    out.extend_from_slice(&p.write(clat, clon, shift));
}

fn encode_line(out: &mut Vec<u8>, f: &OverviewFeature, clat: i32, clon: i32, shift: i32) {
    if f.coords.len() < 2 { return; }
    // v1 : RGN overview ne supporte que les types standards à 1 octet.
    // Polyline::write encode `(type_code & 0xFF)` → pour 0x100-0xFFFF le high byte
    // serait silencieusement tronqué, pour ≥0x10000 il faudrait le chemin étendu.
    if f.type_code >= 0x100 {
        tracing::debug!(type_code = format!("0x{:X}", f.type_code),
            "Overview line skipped: type ≥ 0x100 non supporté en v1");
        return;
    }
    if !first_delta_fits_i16(&f.coords[0], clat, clon, shift) {
        tracing::warn!(type_code = format!("0x{:X}", f.type_code),
            "Overview line skipped: premier delta hors i16 (overview trop large pour bits=18)");
        return;
    }
    let deltas = compute_local_deltas(&f.coords, shift);
    let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) else { return; };
    let pl = Polyline::new(f.type_code, f.coords.clone());
    out.extend_from_slice(&pl.write(clat, clon, shift, &bitstream, false));
}

fn encode_polygon(out: &mut Vec<u8>, f: &OverviewFeature, clat: i32, clon: i32, shift: i32) {
    if f.coords.len() < 3 { return; }
    if f.type_code >= 0x100 {
        tracing::debug!(type_code = format!("0x{:X}", f.type_code),
            "Overview polygon skipped: type ≥ 0x100 non supporté en v1");
        return;
    }
    if !first_delta_fits_i16(&f.coords[0], clat, clon, shift) {
        tracing::warn!(type_code = format!("0x{:X}", f.type_code),
            "Overview polygon skipped: premier delta hors i16 (overview trop large pour bits=18)");
        return;
    }
    let deltas = compute_local_deltas(&f.coords, shift);
    let Some(bitstream) = line_preparer::prepare_line(&deltas, false, None, false) else { return; };
    let pg = Polygon::new(f.type_code, f.coords.clone());
    out.extend_from_slice(&pg.write(clat, clon, shift, &bitstream));
}

/// Vérifie que le premier delta (lat, lon) d'une feature tient dans i16 signé après
/// shift. `Polyline::write`/`Polygon::write` clampent sinon, ce qui décalerait visiblement
/// la géométrie. Protège contre les overviews couvrant > ~3° où la subdiv leaf unique
/// ne suffit plus (scopes futurs hors département / france-quadrant).
fn first_delta_fits_i16(first: &Coord, clat: i32, clon: i32, shift: i32) -> bool {
    let half = if shift > 0 { (1i32 << shift) / 2 } else { 0 };
    let dx = (first.longitude() - clon + half) >> shift;
    let dy = (first.latitude() - clat + half) >> shift;
    (-32768..=32767).contains(&dx) && (-32768..=32767).contains(&dy)
}

/// Compute successive (dx, dy) deltas in shifted local coordinates, with the
/// rounding bias used elsewhere in the codebase (mkgmap MapObject).
fn compute_local_deltas(coords: &[Coord], shift: i32) -> Vec<(i32, i32)> {
    let half = if shift > 0 { (1i32 << shift) / 2 } else { 0 };
    let quant = |lat: i32, lon: i32| -> (i32, i32) {
        ((lon + half) >> shift, (lat + half) >> shift)
    };
    let mut deltas = Vec::with_capacity(coords.len().saturating_sub(1));
    for window in coords.windows(2) {
        let (ax, ay) = quant(window[0].latitude(), window[0].longitude());
        let (bx, by) = quant(window[1].latitude(), window[1].longitude());
        deltas.push((bx - ax, by - ay));
    }
    deltas
}

// ── TRE: 2-level hierarchy with typed overview tables ─────────────────────

fn build_tre(
    north: i32, east: i32, south: i32, west: i32,
    map_id: u32,
    features: &[OverviewFeature],
    leaf_flags: u8,
    clat_leaf: i32,
    clon_leaf: i32,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let common = CommonHeader::new(TRE_HEADER_LEN, "GARMIN TRE");
    common.write(&mut buf);

    // Bounds @21
    common_header::write_i24(&mut buf, north);
    common_header::write_i24(&mut buf, east);
    common_header::write_i24(&mut buf, south);
    common_header::write_i24(&mut buf, west);

    // Map levels: level 1 (inherited parent, bits=17) + level 0 (leaf, bits=18, 1 subdiv).
    let mut levels_data = Vec::new();
    levels_data.extend_from_slice(&[0x01 | 0x80, 17, 1, 0]); // level 1, inherited, 1 subdiv
    levels_data.extend_from_slice(&[0x00,        18, 1, 0]); // level 0, leaf, 1 subdiv

    let shift_top = 24 - 17i32;
    let clat_top = center_aligned(north, south, shift_top);
    let clon_top = center_aligned(east, west, shift_top);
    let w_top = (((east - west) >> shift_top) as u16).max(1);
    let h_top = (((north - south) >> shift_top) as u16).max(1);
    let shift_leaf = 24 - 18i32;
    let w_leaf = (((east - west) >> shift_leaf) as u16).max(1);
    let h_leaf = (((north - south) >> shift_leaf) as u16).max(1);

    // Subdivisions: topdiv (16 B, has_children, next_level=2) + leaf (14 B, is_last) + 4-byte terminator.
    let mut subdivs_data = Vec::new();

    // Subdiv 1 (topdiv at level 1): 16 bytes.
    put_u24(&mut subdivs_data, 0);                // RGN offset = 0 (no direct data)
    subdivs_data.push(0x00);                      // no content flags
    put_i24(&mut subdivs_data, clon_top);
    put_i24(&mut subdivs_data, clat_top);
    subdivs_data.extend_from_slice(&(w_top | 0x8000).to_le_bytes()); // last at level 1
    subdivs_data.extend_from_slice(&h_top.to_le_bytes());
    subdivs_data.extend_from_slice(&2u16.to_le_bytes()); // next_level pointer = subdiv #2

    // Subdiv 2 (leaf at level 0): 14 bytes.
    put_u24(&mut subdivs_data, 0);                // RGN offset = start of data section
    subdivs_data.push(leaf_flags);                // 0x10|0x40|0x80 as applicable
    put_i24(&mut subdivs_data, clon_leaf);
    put_i24(&mut subdivs_data, clat_leaf);
    subdivs_data.extend_from_slice(&(w_leaf | 0x8000).to_le_bytes()); // last at level 0
    subdivs_data.extend_from_slice(&h_leaf.to_le_bytes());

    // 4-byte terminator after all subdivs.
    subdivs_data.extend_from_slice(&0u32.to_le_bytes());

    // Overview tables: one entry per distinct type_code present in `features`, max_level=1.
    let (point_ov, polyline_ov, polygon_ov) = build_overview_tables(features);

    // ── Header section pointers ──
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
    // Map format marker @67 — overview = 0x00040101 (mkgmap osmmap.img parity).
    buf.extend_from_slice(&0x00040101u32.to_le_bytes());
    // Reserved @71
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.push(0x00);

    // Polyline overview @74
    assert_eq!(buf.len(), 74);
    common_header::write_section(&mut buf, offset, polyline_ov.len() as u32);
    offset += polyline_ov.len() as u32;
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

    // Polygon overview @88
    assert_eq!(buf.len(), 88);
    common_header::write_section(&mut buf, offset, polygon_ov.len() as u32);
    offset += polygon_ov.len() as u32;
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

    // Point overview @102
    assert_eq!(buf.len(), 102);
    common_header::write_section(&mut buf, offset, point_ov.len() as u32);
    offset += point_ov.len() as u32;
    buf.extend_from_slice(&3u16.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());

    // MapID @116
    buf.extend_from_slice(&map_id.to_le_bytes());
    // Reserved @120
    buf.extend_from_slice(&0u32.to_le_bytes());

    // MapValues @154-169 (4 × u32) — integrity checksum required by Alpha 100 firmware.
    // Réutilise tre::calc_map_values pour parité binaire avec les sub-maps détail acceptées.
    // Voir docs/firmwares/Alpha_100_FR/jalon-0-pre-test-hash-mapid.md pour la preuve.
    // NOTE : on N'écrit PAS le bloc ExtType @124-153 présent dans TreWriter détail — un
    // test hardware du 2026-04-20 a montré que son ajout rend l'IMG non reconnu par
    // l'Alpha 100. Scope strict du tech-spec : MapValues uniquement.
    common_header::pad_to(&mut buf, 154);
    let map_values = super::tre::calc_map_values(map_id, TRE_HEADER_LEN as u32);
    for v in &map_values {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    assert_eq!(buf.len(), 170);

    // Pad to header length
    common_header::pad_to(&mut buf, TRE_HEADER_LEN as usize);

    // ── Section data ──
    buf.extend_from_slice(&levels_data);
    buf.extend_from_slice(&subdivs_data);
    buf.extend_from_slice(&polyline_ov);
    buf.extend_from_slice(&polygon_ov);
    buf.extend_from_slice(&point_ov);

    // F11 : garde-fou invariant — toute nouvelle section doit incrémenter `offset`.
    // Si on ajoute une section (ex: copyright non vide) sans tenir compte de son offset
    // cumulé, cet assert échouera et forcera la mise à jour du tracking.
    let expected_body_len = levels_data.len() + subdivs_data.len()
        + polyline_ov.len() + polygon_ov.len() + point_ov.len();
    assert_eq!(
        offset as usize,
        TRE_HEADER_LEN as usize + expected_body_len,
        "TRE section offset tracking désynchronisé — vérifier l'ordre des write_section"
    );
    buf
}

/// Build the three overview tables (point/polyline/polygon), 2 bytes per line/polygon entry
/// and 3 bytes per point entry. Only standard types (< 0x10000) are emitted — extended types
/// are skipped in v1 to keep parity with the mkgmap osmmap.img reference.
fn build_overview_tables(features: &[OverviewFeature]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut points: Vec<(u8, u8)> = Vec::new();   // (type, subtype)
    let mut polylines: Vec<u8> = Vec::new();
    let mut polygons: Vec<u8> = Vec::new();

    for f in features {
        if f.type_code >= 0x10000 { continue; }
        if f.is_point {
            // Points supportent 0x100-0xFFFF via (type, subtype).
            let t = if f.type_code < 0x100 {
                (f.type_code as u8, 0u8)
            } else {
                ((f.type_code >> 8) as u8, f.type_code as u8)
            };
            if !points.contains(&t) { points.push(t); }
        } else if f.is_line {
            // v1 : même contrainte que encode_line — skip types ≥0x100 pour rester
            // cohérent avec le RGN écrit et éviter une table qui mentionne des types
            // que l'overview ne rend pas.
            if f.type_code >= 0x100 { continue; }
            let t = f.type_code as u8;
            if !polylines.contains(&t) { polylines.push(t); }
        } else if f.is_polygon {
            if f.type_code >= 0x100 { continue; }
            let t = f.type_code as u8;
            if !polygons.contains(&t) { polygons.push(t); }
        }
    }
    points.sort();
    polylines.sort();
    polygons.sort();

    let mut point_bytes = Vec::with_capacity(points.len() * 3);
    for (t, st) in &points {
        point_bytes.push(*t);
        point_bytes.push(1); // max_level = 1 (inherited parent level)
        point_bytes.push(*st);
    }
    let mut polyline_bytes = Vec::with_capacity(polylines.len() * 2);
    for t in &polylines {
        polyline_bytes.push(*t);
        polyline_bytes.push(1);
    }
    let mut polygon_bytes = Vec::with_capacity(polygons.len() * 2);
    for t in &polygons {
        polygon_bytes.push(*t);
        polygon_bytes.push(1);
    }

    (point_bytes, polyline_bytes, polygon_bytes)
}

// ── LBL: minimal with proper PlacesHeader (v1 = no labels) ───────────────

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

    for &rec in &[3u16, 5, 5, 4] {
        write_empty_sec(&mut buf, lbl_end, rec);
    }
    buf.extend_from_slice(&lbl_end.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 5]);
    for &rec in &[4u16, 3, 6, 5, 3] {
        write_empty_sec(&mut buf, lbl_end, rec);
    }

    assert_eq!(buf.len(), 170);
    buf.extend_from_slice(&codepage.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
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
    use crate::parser::mp_types::{MpFile, MpHeader, MpPoint, MpPolyline, MpPolygon, RoutingMode};
    use std::collections::BTreeMap;

    fn c(lat: i32, lon: i32) -> Coord { Coord::new(lat, lon) }

    fn make_test_tile(north: i32, east: i32, south: i32, west: i32) -> TileSubfiles {
        let mut tre = vec![0u8; 33];
        tre[0] = 188; tre[1] = 0;
        tre[2..12].copy_from_slice(b"GARMIN TRE");
        tre[21..24].copy_from_slice(&north.to_le_bytes()[..3]);
        tre[24..27].copy_from_slice(&east.to_le_bytes()[..3]);
        tre[27..30].copy_from_slice(&south.to_le_bytes()[..3]);
        tre[30..33].copy_from_slice(&west.to_le_bytes()[..3]);
        TileSubfiles {
            map_number: "11000001".to_string(),
            description: "Test tile".to_string(),
            tre, rgn: vec![0u8; 125], lbl: vec![0u8; 196],
            net: None, nod: None, dem: None,
        }
    }

    fn empty_mp() -> MpFile {
        MpFile {
            header: MpHeader {
                id: 0, name: String::new(), copyright: String::new(),
                levels: Vec::new(), codepage: 1252, datum: String::new(),
                transparent: false, draw_priority: 0, preview_lat: 0.0, preview_lon: 0.0,
                lower_case: false, order_by_decreasing_area: false,
                reduce_point_density: None, simplify_polygons: None,
                min_size_polygon: None, merge_lines: false,
                routing_mode: RoutingMode::Auto,
                country_name: String::new(), country_abbr: String::new(),
                region_name: String::new(), region_abbr: String::new(),
                product_version: 0,
            },
            points: Vec::new(), polylines: Vec::new(), polygons: Vec::new(),
        }
    }

    fn pl_with(type_code: u32, end_level: u8, buckets: &[(u8, Vec<Coord>)]) -> MpPolyline {
        let mut g = BTreeMap::new();
        for (n, v) in buckets { g.insert(*n, v.clone()); }
        MpPolyline {
            type_code, label: String::new(), geometries: g,
            end_level: Some(end_level), direction: false, road_id: None, route_param: None,
        }
    }

    fn pg_with(type_code: u32, end_level: u8, buckets: &[(u8, Vec<Coord>)]) -> MpPolygon {
        let mut g = BTreeMap::new();
        for (n, v) in buckets { g.insert(*n, v.clone()); }
        MpPolygon { type_code, label: String::new(), geometries: g, end_level: Some(end_level) }
    }

    // ── Extraction tests (AC 1/2/3) ──

    #[test]
    fn test_extract_overview_features_endlevel_filter() {
        let mut mp = empty_mp();
        mp.polylines.push(pl_with(0x01, 0, &[(5, vec![c(0,0), c(1,1)])]));
        mp.polylines.push(pl_with(0x01, 3, &[(5, vec![c(0,0), c(1,1)])]));
        mp.polylines.push(pl_with(0x01, 6, &[(5, vec![c(0,0), c(1,1)])]));
        let out = extract_overview_features(&mp, 5, 6, None);
        assert_eq!(out.len(), 1, "only EndLevel=6 should pass");
    }

    #[test]
    fn test_extract_overview_features_whitelist() {
        let mut mp = empty_mp();
        for t in [0x01u32, 0x02, 0x3F, 0x50] {
            mp.polylines.push(pl_with(t, 6, &[(5, vec![c(0,0), c(1,1)])]));
        }
        let wl: HashSet<u32> = [0x01u32, 0x3F].iter().copied().collect();
        let out = extract_overview_features(&mp, 5, 6, Some(&wl));
        assert_eq!(out.len(), 2);
        let kept: HashSet<u32> = out.iter().map(|f| f.type_code).collect();
        assert!(kept.contains(&0x01) && kept.contains(&0x3F));
    }

    #[test]
    fn test_extract_overview_features_fallback_level() {
        let mut mp = empty_mp();
        mp.polylines.push(pl_with(0x01, 6, &[(5, vec![c(0,0), c(1,1), c(2,2)])]));
        let out = extract_overview_features(&mp, 6, 6, None);
        assert_eq!(out.len(), 1, "should fall back to bucket 5 when 6 absent");
        assert_eq!(out[0].coords.len(), 3);
    }

    /// F5 : si seuls des paliers très détaillés existent (>1 palier sous l'overview_level),
    /// la feature doit être rejetée plutôt qu'écrire Data0/1 dans le RGN overview.
    #[test]
    fn test_pick_bucket_rejects_too_fine_fallback() {
        let mut mp = empty_mp();
        // overview_level=6 mais uniquement Data0 disponible → rejet.
        mp.polylines.push(pl_with(0x01, 6, &[(0, vec![c(0,0), c(1,1)])]));
        let out = extract_overview_features(&mp, 6, 6, None);
        assert!(out.is_empty(), "bucket too fine (>1 level finer) must be rejected");
    }

    /// F5 : priorité au plus grossier (N > level) avant de retomber plus fin.
    #[test]
    fn test_pick_bucket_prefers_coarser_over_finer() {
        let mut mp = empty_mp();
        // overview_level=5 : buckets 4 (fin) et 6 (grossier) présents → on prend 6.
        mp.polylines.push(pl_with(0x01, 6, &[
            (4, vec![c(0,0), c(1,1), c(2,2), c(3,3)]),
            (6, vec![c(0,0), c(1,1)]),
        ]));
        let out = extract_overview_features(&mp, 5, 6, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].coords.len(), 2, "must use coarser bucket 6 (2 pts) not finer bucket 4");
    }

    #[test]
    fn test_extract_point_endlevel() {
        let mut mp = empty_mp();
        mp.points.push(MpPoint { type_code: 0x2C, label: String::new(), coord: c(1,1), end_level: Some(6) });
        mp.points.push(MpPoint { type_code: 0x2C, label: String::new(), coord: c(2,2), end_level: Some(0) });
        let out = extract_overview_features(&mp, 5, 6, None);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_point);
    }

    #[test]
    fn test_extract_polygon_requires_3_coords() {
        let mut mp = empty_mp();
        // Polygon with only 2 coords should be rejected.
        mp.polygons.push(pg_with(0x03, 6, &[(5, vec![c(0,0), c(1,1)])]));
        mp.polygons.push(pg_with(0x03, 6, &[(5, vec![c(0,0), c(1,1), c(2,2)])]));
        let out = extract_overview_features(&mp, 5, 6, None);
        assert_eq!(out.len(), 1);
    }

    // ── TRE structure tests (AC 4/5) ──

    #[test]
    fn test_build_tre_two_levels_mkgmap_parity() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let features = vec![
            OverviewFeature {
                type_code: 0x01, is_point: false, is_line: true, is_polygon: false,
                coords: vec![c(2140000, 258000), c(2141000, 259000)],
            },
        ];
        let ov = build_overview_map(&tiles, &features, 12345, 1252);
        // Map format marker @67
        assert_eq!(u32::from_le_bytes([ov.tre[67], ov.tre[68], ov.tre[69], ov.tre[70]]), 0x00040101);
        // Map levels section offset @33
        let ml_off = u32::from_le_bytes([ov.tre[33], ov.tre[34], ov.tre[35], ov.tre[36]]) as usize;
        let ml_size = u32::from_le_bytes([ov.tre[37], ov.tre[38], ov.tre[39], ov.tre[40]]) as usize;
        assert_eq!(ml_size, 8, "two 4-byte level records");
        let ml = &ov.tre[ml_off..ml_off + ml_size];
        assert_eq!(ml[0] & 0x80, 0x80, "level 1 inherited flag");
        assert_eq!(ml[1], 17, "level 1 bits");
        assert_eq!(ml[4] & 0x80, 0x00, "level 0 not inherited");
        assert_eq!(ml[5], 18, "level 0 bits");
    }

    #[test]
    fn test_build_tre_type_tables_populated() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let features = vec![
            OverviewFeature {
                type_code: 0x01, is_point: false, is_line: true, is_polygon: false,
                coords: vec![c(2140000, 258000), c(2141000, 259000)],
            },
            OverviewFeature {
                type_code: 0x03, is_point: false, is_line: false, is_polygon: true,
                coords: vec![c(2140000, 258000), c(2141000, 259000), c(2140500, 258500)],
            },
            OverviewFeature {
                type_code: 0x2C, is_point: true, is_line: false, is_polygon: false,
                coords: vec![c(2140000, 258000)],
            },
        ];
        let ov = build_overview_map(&tiles, &features, 12345, 1252);
        // Polyline overview table — offset(@74) + size(@78)
        let pl_size = u32::from_le_bytes([ov.tre[78], ov.tre[79], ov.tre[80], ov.tre[81]]);
        assert_eq!(pl_size, 2, "one polyline overview entry = 2 bytes");
        // Polygon overview table — size @92
        let pg_size = u32::from_le_bytes([ov.tre[92], ov.tre[93], ov.tre[94], ov.tre[95]]);
        assert_eq!(pg_size, 2, "one polygon overview entry = 2 bytes");
        // Point overview table — size @106
        let pt_size = u32::from_le_bytes([ov.tre[106], ov.tre[107], ov.tre[108], ov.tre[109]]);
        assert_eq!(pt_size, 3, "one point overview entry = 3 bytes");
        // 0x4B (background) must NOT be auto-added into the polygon table.
        let pg_off = u32::from_le_bytes([ov.tre[88], ov.tre[89], ov.tre[90], ov.tre[91]]) as usize;
        assert_eq!(ov.tre[pg_off], 0x03, "polygon overview should contain source type, not 0x4B");
    }

    #[test]
    fn test_overview_tre_common_header() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, &[], 12345, 1252);
        assert_eq!(&ov.tre[2..12], b"GARMIN TRE");
    }

    #[test]
    fn test_overview_rgn_has_feature_data() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let features = vec![OverviewFeature {
            type_code: 0x01, is_point: false, is_line: true, is_polygon: false,
            coords: vec![c(2140000, 258000), c(2141000, 259000)],
        }];
        let ov = build_overview_map(&tiles, &features, 12345, 1252);
        assert!(ov.rgn.len() > super::super::rgn::RGN_HEADER_LEN as usize);
    }

    #[test]
    fn test_overview_lbl_sections() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, &[], 12345, 1252);
        assert_eq!(ov.lbl.len(), LBL_HEADER_LEN as usize + 1);
    }

    #[test]
    fn test_overview_map_number_format() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let ov = build_overview_map(&tiles, &[], 11000000, 1252);
        assert_eq!(ov.map_number, "11000000");
    }

    /// F4 : vérifie que l'offset RGN de la subdiv TRE (0) pointe bien sur le
    /// premier byte des données RGN (post-header 125 B) et que ce byte correspond
    /// au type encodé. Protège contre un futur refactor de RgnWriter qui ferait
    /// décrocher les deux silencieusement.
    #[test]
    fn test_tre_rgn_offset_matches_rgn_data_start() {
        let rgn_header_len = super::super::rgn::RGN_HEADER_LEN as usize;
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        // Cas polyline-only : pas de pointers en tête de subdiv, le premier byte
        // est directement le type byte (ici 0x01).
        let features = vec![OverviewFeature {
            type_code: 0x01, is_point: false, is_line: true, is_polygon: false,
            coords: vec![c(2140000, 258000), c(2141000, 259000)],
        }];
        let ov = build_overview_map(&tiles, &features, 12345, 1252);

        // TRE : subdiv #2 (leaf) — rgn_offset (3 octets u24 LE) en tête du record de 14 B.
        // Section subdivisions @41 dans le header.
        let subdivs_off = u32::from_le_bytes([ov.tre[41], ov.tre[42], ov.tre[43], ov.tre[44]]) as usize;
        // Subdiv 1 (topdiv) = 16 B ; subdiv 2 démarre après.
        let leaf_off = subdivs_off + 16;
        let tre_rgn_offset = (ov.tre[leaf_off] as u32)
            | ((ov.tre[leaf_off + 1] as u32) << 8)
            | ((ov.tre[leaf_off + 2] as u32) << 16);
        assert_eq!(tre_rgn_offset, 0, "leaf RGN offset must point to start of RGN data section");

        // RGN : data section @21, offset(4)+size(4). Data commence à RGN_HEADER_LEN.
        let rgn_data_off = u32::from_le_bytes([ov.rgn[21], ov.rgn[22], ov.rgn[23], ov.rgn[24]]) as usize;
        assert_eq!(rgn_data_off, rgn_header_len, "RGN data section must start immediately after header");
        // Premier byte des données = type byte de la polyline (pas de pointer car une seule section).
        assert_eq!(ov.rgn[rgn_data_off] & 0x7F, 0x01, "first RGN byte must be the polyline type");
    }

    /// F4 (bis) : avec contenu mixte points+polylines, le début de la subdiv RGN
    /// doit être un pointer de 2 octets vers la section polylines (cf. RgnWriter).
    /// Garantit que la disposition multi-sections reste synchronisée.
    #[test]
    fn test_rgn_mixed_content_has_section_pointer() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        let features = vec![
            OverviewFeature {
                type_code: 0x2C, is_point: true, is_line: false, is_polygon: false,
                coords: vec![c(2140000, 258000)],
            },
            OverviewFeature {
                type_code: 0x01, is_point: false, is_line: true, is_polygon: false,
                coords: vec![c(2140000, 258000), c(2141000, 259000)],
            },
        ];
        let ov = build_overview_map(&tiles, &features, 12345, 1252);
        let rgn_header_len = super::super::rgn::RGN_HEADER_LEN as usize;
        // Avec points + polylines, RgnWriter écrit 1 pointer de 2 octets en tête.
        let pointer = u16::from_le_bytes([ov.rgn[rgn_header_len], ov.rgn[rgn_header_len + 1]]);
        // Pointer = offset polylines = 2 (pointer size) + points_data_len. Points standard = 8 B.
        assert_eq!(pointer, 2 + 8, "pointer must skip 2B pointer + 8B point record");
    }

    /// MapValues @TRE+0x9A — intégrité exigée par le firmware Alpha 100.
    /// Garantit que l'overview émet les 16 bytes non-nuls calculés via
    /// `tre::calc_map_values` (parité binaire avec les sub-maps détail).
    #[test]
    fn test_overview_tre_has_mapvalues() {
        let tiles = vec![make_test_tile(2143196, 262632, 2138930, 255409)];
        // F10 : map_id au format hex aligné sur tre.rs::test_map_values_known_id (0x00380001).
        // Overview utilise la convention <family>0000, donc 0x00380000.
        const OVERVIEW_MAP_ID: u32 = 0x0038_0000;
        let ov = build_overview_map(&tiles, &[], OVERVIEW_MAP_ID, 1252);
        assert!(ov.tre.len() >= TRE_HEADER_LEN as usize);
        let mv = &ov.tre[0x9A..0x9A + 16];
        assert!(
            mv.iter().any(|&b| b != 0),
            "MapValues @0x9A doit être non-nul, trouvé: {:?}",
            mv
        );
        let expected: [u32; 4] =
            super::super::tre::calc_map_values(OVERVIEW_MAP_ID, TRE_HEADER_LEN as u32);
        let mut expected_bytes = Vec::with_capacity(16);
        for v in &expected {
            expected_bytes.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(mv, &expected_bytes[..], "MapValues divergent du calc_map_values de référence");
    }
}
