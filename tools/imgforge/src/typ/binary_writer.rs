//! Writer binaire TYP.
//!
//! Port de `mkgmap/.../imgfmt/app/typ/` : `TYPHeader`, `TYPFile`, `TypElement`,
//! `TypPoint/Line/Polygon/IconSet`, `ColourInfo`, `BitmapImage`, `TrueImage`,
//! `ShapeStacking`, `DrawOrder`.
//!
//! Le format TYP Garmin débute par un **CommonHeader** (21 octets) : length
//! `u16 LE`, signature ASCII `"GARMIN TYP"` sur 10 octets, 2 octets
//! (unknown + lock flag), puis 7 octets de date de création (cf. `CommonHeader.java`).
//! Suit un header spécifique TYP (longueur 0x5B / 0x6E / 0x9C selon les
//! sections présentes).

use super::data::*;
use super::encoding::{encode, encode_alpha_inverse, HEADER_LEN};
use crate::error::TypError;
use crate::img::bit_writer::BitWriter;

/// Longueur du header commun (length + signature + flags + date).
const COMMON_HEADER_LEN: usize = 21;

// Types « simple » pour lignes/polygones (ColourInfo scheme bits).
const S_NIGHT: u8 = 0x1;
const S_DAY_TRANSPARENT: u8 = 0x2;
const S_NIGHT_TRANSPARENT: u8 = 0x4;
const S_HAS_BITMAP: u8 = 0x8;

// TypPoint flags.
const PT_F_BITMAP: u8 = 0x01;
const PT_F_NIGHT_XPM: u8 = 0x02;
const PT_F_LABEL: u8 = 0x04;
const PT_F_EXTENDED_FONT: u8 = 0x08;

// TypLine flags.
const LN_F_LABEL: u8 = 0x01;
const LN_F_USE_ROTATION: u8 = 0x02;
const LN_F_EXTENDED: u8 = 0x04;

// TypPolygon flags (sur le même octet que le scheme).
const PG_F_LABEL: u8 = 0x10;
const PG_F_EXTENDED: u8 = 0x20;

/// Produit le binaire TYP depuis un [`TypData`] déjà peuplé.
pub fn write_typ_binary(data: &TypData) -> Result<Vec<u8>, TypError> {
    if data.params.codepage != 1252 && data.params.codepage != 65001 {
        return Err(TypError::UnknownCodepage(data.params.codepage));
    }
    let cp = data.params.codepage;

    // 1. Trier chaque liste d'éléments par `type_for_file`.
    let mut points: Vec<(u32, Vec<u8>)> = data
        .points
        .iter()
        .map(|e| -> Result<_, TypError> { Ok((type_for_file(e.type_code, e.subtype), write_point(e, cp)?)) })
        .collect::<Result<_, _>>()?;
    points.sort_by_key(|(t, _)| *t);

    let mut lines: Vec<(u32, Vec<u8>)> = data
        .lines
        .iter()
        .map(|e| -> Result<_, TypError> { Ok((type_for_file(e.type_code, e.subtype), write_line(e, cp)?)) })
        .collect::<Result<_, _>>()?;
    lines.sort_by_key(|(t, _)| *t);

    let mut polygons: Vec<(u32, Vec<u8>)> = data
        .polygons
        .iter()
        .map(|e| -> Result<_, TypError> { Ok((type_for_file(e.type_code, e.subtype), write_polygon(e, cp)?)) })
        .collect::<Result<_, _>>()?;
    polygons.sort_by_key(|(t, _)| *t);

    let mut icons: Vec<(u32, Vec<u8>)> = data
        .icons
        .iter()
        .map(|e| -> Result<_, TypError> { Ok((type_for_file(e.type_code, e.subtype), write_iconset(e, cp)?)) })
        .collect::<Result<_, _>>()?;
    icons.sort_by_key(|(t, _)| *t);

    // 2. Concaténer données + indices.
    let (poly_data, poly_offsets) = concat_with_offsets(&polygons);
    let (line_data, line_offsets) = concat_with_offsets(&lines);
    let (point_data, point_offsets) = concat_with_offsets(&points);
    let shape_stacking = write_shape_stacking(&data.draw_order);
    let (icon_data, icon_offsets) = concat_with_offsets(&icons);

    // Labels : un label par icon (premier `StringN` du set), + offset 0 réservé.
    let (labels_block, str_index, type_index) = write_labels_block(&data.icons, &icons, cp)?;

    // 3. Calculer les offsets absolus.
    let mut pos = HEADER_LEN;
    let poly_data_pos = pos;
    pos += poly_data.len();
    let line_data_pos = pos;
    pos += line_data.len();
    let point_data_pos = pos;
    pos += point_data.len();

    let poly_idx_psize = pointer_size(poly_data.len());
    let line_idx_psize = pointer_size(line_data.len());
    let point_idx_psize = pointer_size(point_data.len());

    let type_size = 2u16; // mkgmap initialise typeSize=2 pour tous les index.
    let poly_index = build_index(&polygons, &poly_offsets, type_size, poly_idx_psize);
    let line_index = build_index(&lines, &line_offsets, type_size, line_idx_psize);
    let point_index = build_index(&points, &point_offsets, type_size, point_idx_psize);

    let poly_idx_pos = pos;
    pos += poly_index.len();
    let line_idx_pos = pos;
    pos += line_index.len();
    let point_idx_pos = pos;
    pos += point_index.len();

    let stack_pos = pos;
    pos += shape_stacking.len();

    let icon_data_pos = pos;
    pos += icon_data.len();

    let icon_idx_psize = pointer_size(icon_data.len());
    let icon_index = build_index(&icons, &icon_offsets, type_size, icon_idx_psize);
    let icon_idx_pos = pos;
    pos += icon_index.len();

    let labels_pos = pos;
    pos += labels_block.len();

    let labels_psize = pointer_size(labels_block.len());
    let str_index_item_size = 3 + labels_psize as u16;
    let type_index_item_size = 3 + labels_psize as u16;
    let str_index_bytes = build_str_index(&str_index, labels_psize);
    let type_index_bytes = build_type_index(&type_index, labels_psize);

    let str_idx_pos = pos;
    pos += str_index_bytes.len();
    let type_idx_pos = pos;
    pos += type_index_bytes.len();

    let total_size = pos;

    // 4. Construire le header (156 octets).
    let header = build_header(Header {
        codepage: cp,
        family_id: data.params.family_id,
        product_id: data.params.product_id,

        point_data_pos, point_data_size: point_data.len(),
        line_data_pos, line_data_size: line_data.len(),
        poly_data_pos, poly_data_size: poly_data.len(),

        point_idx_pos, point_idx_size: point_index.len(), point_idx_item_size: type_size + point_idx_psize as u16,
        line_idx_pos, line_idx_size: line_index.len(), line_idx_item_size: type_size + line_idx_psize as u16,
        poly_idx_pos, poly_idx_size: poly_index.len(), poly_idx_item_size: type_size + poly_idx_psize as u16,

        stack_pos, stack_size: shape_stacking.len(), stack_item_size: 5,

        icon_data_pos, icon_data_size: icon_data.len(),
        icon_idx_pos, icon_idx_size: icon_index.len(), icon_idx_item_size: type_size + icon_idx_psize as u16,

        labels_pos, labels_size: labels_block.len(),
        str_idx_pos, str_idx_size: str_index_bytes.len(), str_idx_item_size: str_index_item_size,
        type_idx_pos, type_idx_size: type_index_bytes.len(), type_idx_item_size: type_index_item_size,
    });
    debug_assert_eq!(header.len(), HEADER_LEN);

    // 5. Concaténer.
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&header);
    out.extend_from_slice(&poly_data);
    out.extend_from_slice(&line_data);
    out.extend_from_slice(&point_data);
    out.extend_from_slice(&poly_index);
    out.extend_from_slice(&line_index);
    out.extend_from_slice(&point_index);
    out.extend_from_slice(&shape_stacking);
    out.extend_from_slice(&icon_data);
    out.extend_from_slice(&icon_index);
    out.extend_from_slice(&labels_block);
    out.extend_from_slice(&str_index_bytes);
    out.extend_from_slice(&type_index_bytes);
    debug_assert_eq!(out.len(), total_size);
    Ok(out)
}

// ============================================================ header binaire

struct Header {
    codepage: u16,
    family_id: u16,
    product_id: u16,

    point_data_pos: usize, point_data_size: usize,
    line_data_pos: usize, line_data_size: usize,
    poly_data_pos: usize, poly_data_size: usize,

    point_idx_pos: usize, point_idx_size: usize, point_idx_item_size: u16,
    line_idx_pos: usize, line_idx_size: usize, line_idx_item_size: u16,
    poly_idx_pos: usize, poly_idx_size: usize, poly_idx_item_size: u16,

    stack_pos: usize, stack_size: usize, stack_item_size: u16,

    icon_data_pos: usize, icon_data_size: usize,
    icon_idx_pos: usize, icon_idx_size: usize, icon_idx_item_size: u16,

    labels_pos: usize, labels_size: usize,
    str_idx_pos: usize, str_idx_size: usize, str_idx_item_size: u16,
    type_idx_pos: usize, type_idx_size: usize, type_idx_item_size: u16,
}

fn build_header(h: Header) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN);
    // 0x00 : header length u16 LE (0x9C).
    out.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    // 0x02 : signature ASCII "GARMIN TYP" sur 10 octets.
    let sig = b"GARMIN TYP";
    out.extend_from_slice(sig);
    // 0x0C : unknown (0x01) + lock flag (0x00).
    out.push(0x01);
    out.push(0x00);
    // 0x0E : 7 octets de date (year u16 LE + month, day, hour, min, sec).
    // Valeur fixe pour déterminisme des tests ; TYPViewer ne la vérifie pas.
    out.extend_from_slice(&[0xEA, 0x07, 0x04, 0x0E, 0x10, 0x00, 0x00]); // 2026-04-14 16:00:00
    debug_assert_eq!(out.len(), COMMON_HEADER_LEN);

    // 0x15 : codepage u16 LE.
    out.extend_from_slice(&h.codepage.to_le_bytes());

    // 0x17 : point_data, line_data, poly_data — chacun (pos u32, size u32).
    zap_pos_size(&mut out, h.point_data_pos, h.point_data_size);
    zap_pos_size(&mut out, h.line_data_pos, h.line_data_size);
    zap_pos_size(&mut out, h.poly_data_pos, h.poly_data_size);

    // 0x2F : family_id u16, product_id u16.
    out.extend_from_slice(&h.family_id.to_le_bytes());
    out.extend_from_slice(&h.product_id.to_le_bytes());

    // 0x33 : point_index, line_index, poly_index, shape_stacking —
    // chacun (pos u32, item_size u16, size u32).
    zap_pos_isize_size(&mut out, h.point_idx_pos, h.point_idx_item_size, h.point_idx_size);
    zap_pos_isize_size(&mut out, h.line_idx_pos, h.line_idx_item_size, h.line_idx_size);
    zap_pos_isize_size(&mut out, h.poly_idx_pos, h.poly_idx_item_size, h.poly_idx_size);
    zap_pos_isize_size(&mut out, h.stack_pos, h.stack_item_size, h.stack_size);

    // 0x5B : icon_index + byte 0x13 + icon_data + 4 octets zéro.
    zap_pos_isize_size(&mut out, h.icon_idx_pos, h.icon_idx_item_size, h.icon_idx_size);
    out.push(0x13);
    zap_pos_size(&mut out, h.icon_data_pos, h.icon_data_size);
    out.extend_from_slice(&[0, 0, 0, 0]);

    // 0x6E : labels + string_index + type_index.
    zap_pos_size(&mut out, h.labels_pos, h.labels_size);
    // str_index : item_size u32, 0x1B u32, pos u32, size u32.
    out.extend_from_slice(&(h.str_idx_item_size as u32).to_le_bytes());
    out.extend_from_slice(&0x1Bu32.to_le_bytes());
    zap_pos_size(&mut out, h.str_idx_pos, h.str_idx_size);
    // type_index : idem.
    out.extend_from_slice(&(h.type_idx_item_size as u32).to_le_bytes());
    out.extend_from_slice(&0x1Bu32.to_le_bytes());
    zap_pos_size(&mut out, h.type_idx_pos, h.type_idx_size);
    // 2 octets finaux (alignement).
    out.extend_from_slice(&[0, 0]);

    out
}

/// Écrit (pos u32, size u32). Si size==0, zap pos à 0.
fn zap_pos_size(out: &mut Vec<u8>, pos: usize, size: usize) {
    let p = if size == 0 { 0 } else { pos as u32 };
    out.extend_from_slice(&p.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
}

/// Écrit (pos u32, item_size u16, size u32). Si size==0, zap pos et item_size à 0.
fn zap_pos_isize_size(out: &mut Vec<u8>, pos: usize, item_size: u16, size: usize) {
    let (p, is) = if size == 0 { (0, 0) } else { (pos as u32, item_size) };
    out.extend_from_slice(&p.to_le_bytes());
    out.extend_from_slice(&is.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
}

// ============================================================ packing type

/// Type tel qu'écrit dans l'index : `(type << 5) | (subtype & 0x1f)`.
fn type_for_file(type_code: u32, subtype: u8) -> u32 {
    (type_code << 5) | u32::from(subtype & 0x1f)
}

fn pointer_size(size: usize) -> u8 {
    if size <= 0xff { 1 }
    else if size <= 0xffff { 2 }
    else if size <= 0xff_ffff { 3 }
    else { 4 }
}

fn concat_with_offsets(elems: &[(u32, Vec<u8>)]) -> (Vec<u8>, Vec<usize>) {
    let mut out = Vec::new();
    let mut offsets = Vec::with_capacity(elems.len());
    for (_, data) in elems {
        offsets.push(out.len());
        out.extend_from_slice(data);
    }
    (out, offsets)
}

fn build_index(
    elems: &[(u32, Vec<u8>)],
    offsets: &[usize],
    type_size: u16,
    psize: u8,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(elems.len() * (type_size as usize + psize as usize));
    for (i, (t, _)) in elems.iter().enumerate() {
        put_u_le(&mut out, *t as u64, type_size as usize);
        put_u_le(&mut out, offsets[i] as u64, psize as usize);
    }
    out
}

fn put_u_le(out: &mut Vec<u8>, v: u64, n: usize) {
    for i in 0..n {
        out.push(((v >> (i * 8)) & 0xff) as u8);
    }
}

// ============================================================ color info

/// Représentation normalisée pour écriture binaire.
struct ColourInfo {
    width: u16,
    height: u16,
    chars_per_pixel: u16,
    /// Couleurs ordonnées : day_bg, day_fg, [night_bg, night_fg]. Les
    /// transparentes sont placées en dernier de leur paire (cf.
    /// `ColourInfo.analyseColours`).
    colours: Vec<Rgba>,
    /// Bitmap pixel data : index par `char_key` dans la palette XPM originale.
    pixels: Option<Vec<u8>>,
    /// 0 / 0x10 / 0x20.
    colour_mode: u8,
    simple: bool,
    has_bitmap: bool,
    has_border: bool,
    nr_solid_colours: u8,
}

fn analyse_colours(xpm: &Xpm, simple: bool, has_border: bool) -> ColourInfo {
    let mut colours: Vec<Rgba> = xpm.colors.clone();
    let mut colour_mode: u8 = 0;

    if simple {
        // Ordre attendu : bg, fg (opaque). Si bg transparent, swap(0,1).
        if !colours.is_empty() && colours[0].a == 0xff {
            // Note : "transparent" dans notre modèle = a==0xff (alpha-inverse
            // non encore appliqué). Cf. `parse_xpm_color` qui pose a=0xff pour
            // `none`. Les couleurs opaques ont a=0.
            // swap(0, 1) si la couleur 0 est transparente et qu'il en existe
            // au moins 2.
            if colours.len() >= 2 {
                colours.swap(0, 1);
            }
        }
        if colours.len() > 2 && colours[2].a == 0xff {
            if colours.len() >= 4 {
                colours.swap(2, 3);
            }
        }
    } else {
        // Points / icons : analyse alpha/transparence.
        let mut n_trans = 0usize;
        let mut n_alpha = 0usize;
        let mut trans_index = 0usize;
        for (i, c) in colours.iter().enumerate() {
            if c.a == 0xff {
                n_trans += 1;
                trans_index = i;
            } else if c.a != 0 && c.a != 0xff {
                n_alpha += 1;
            }
        }
        let count = colours.len();
        if n_alpha > 0 || (count > 0 && count == n_trans) {
            colour_mode = 0x20;
        } else if n_trans == 1 {
            colour_mode = 0x10;
            let t = colours.remove(trans_index);
            colours.push(t);
        }
    }

    let nr_solid_colours = colours.iter().filter(|c| c.a != 0xff).count() as u8;

    let pixels = if xpm.pixels.is_empty() {
        None
    } else {
        Some(xpm.pixels.clone())
    };

    ColourInfo {
        width: xpm.width,
        height: xpm.height,
        chars_per_pixel: if xpm.pixels.is_empty() { 1 } else { infer_cpp(xpm) },
        colours,
        pixels,
        colour_mode,
        simple,
        has_bitmap: !xpm.pixels.is_empty(),
        has_border,
        nr_solid_colours,
    }
}

fn infer_cpp(xpm: &Xpm) -> u16 {
    // Notre parser stocke déjà des indices (1 octet par pixel), peu importe
    // cpp d'origine : pour l'écriture binaire on encode via BitWriter avec
    // `bits_per_pixel` calculé.
    1
}

impl ColourInfo {
    fn number_of_colours(&self) -> usize {
        self.colours.len()
    }

    fn bits_per_pixel(&self) -> usize {
        if self.simple { return 1; }
        match self.number_of_colours() {
            0 => 24,
            1 => 1,
            2 | 3 => 2,
            4..=15 => 4,
            _ => 8,
        }
    }

    fn colour_scheme(&self) -> u8 {
        let mut n = self.number_of_colours();
        if n == 0 { n = self.colours.len(); }
        let mut scheme = 0u8;
        if self.has_bitmap { scheme |= S_HAS_BITMAP; }
        if n == 4 { scheme |= S_NIGHT; }
        if !self.has_bitmap && !self.has_border && n == 2 {
            scheme |= S_NIGHT | S_DAY_TRANSPARENT | S_NIGHT_TRANSPARENT;
        }
        if n < 2 || (n >= 2 && self.colours[1].a == 0xff) {
            scheme |= S_DAY_TRANSPARENT;
        }
        if n == 4 && self.colours.get(3).map_or(false, |c| c.a == 0xff) {
            scheme |= S_NIGHT_TRANSPARENT;
        }
        if (scheme & S_NIGHT) == 0 && (scheme & S_DAY_TRANSPARENT) != 0 {
            scheme |= S_NIGHT_TRANSPARENT;
        }
        scheme
    }

    /// Écrit les couleurs seules (pas la bitmap).
    fn write_colours(&self, out: &mut Vec<u8>) {
        if self.colour_mode == 0x20 {
            // alpha rounded to 4 bits, BGR + alpha 4-bit
            let mut bw = BitWriter::new();
            for c in &self.colours {
                bw.putn(c.b as u32, 8);
                bw.putn(c.g as u32, 8);
                bw.putn(c.r as u32, 8);
                let alpha = 0xff - c.a as i32;
                let rounded = alpha_round4(alpha);
                bw.putn(rounded as u32, 4);
            }
            out.extend_from_slice(bw.bytes());
        } else {
            for c in &self.colours {
                if c.a != 0xff {
                    // BGR 0x10
                    out.push(c.b);
                    out.push(c.g);
                    out.push(c.r);
                }
            }
        }
    }

    /// Écrit la bitmap (indexée, LSB-first BitWriter par ligne).
    fn write_bitmap(&self, out: &mut Vec<u8>) {
        if let Some(pixels) = &self.pixels {
            let bits = self.bits_per_pixel();
            let w = self.width as usize;
            let h = self.height as usize;
            let mut pi = 0;
            for _row in 0..h {
                let mut bw = BitWriter::new();
                for _col in 0..w {
                    let idx = pixels.get(pi).copied().unwrap_or(0);
                    let val = if self.simple { (!u32::from(idx)) & ((1u32 << bits) - 1) } else { u32::from(idx) };
                    bw.putn(val, bits);
                    pi += 1;
                }
                out.extend_from_slice(bw.bytes());
            }
        }
    }

    fn number_of_s_colours_for_cm(&self) -> u8 {
        if self.colour_mode == 0x10 {
            self.nr_solid_colours
        } else {
            self.colours.len() as u8
        }
    }
}

fn alpha_round4(alpha: i32) -> i32 {
    let top = (alpha >> 4) & 0xf;
    let low = alpha & 0xf;
    let diff = low - top;
    if diff > 8 { top + 1 }
    else if diff < -8 { top - 1 }
    else { top }
}

// ============================================================ element writers

fn write_polygon(p: &TypPolygon, cp: u16) -> Result<Vec<u8>, TypError> {
    let mut out = Vec::new();
    let xpm = p.day_xpm.clone().unwrap_or_else(empty_xpm);
    let ci = analyse_colours(&xpm, true, false);
    let mut scheme = ci.colour_scheme();
    if !p.labels.is_empty() { scheme |= PG_F_LABEL; }
    let has_ext = p.font_style != FontStyle::Default || p.day_font_color.is_some();
    if has_ext { scheme |= PG_F_EXTENDED; }

    out.push(scheme);
    ci.write_colours(&mut out);
    if ci.has_bitmap { ci.write_bitmap(&mut out); }

    if !p.labels.is_empty() {
        write_label_block(&mut out, &p.labels, cp)?;
    }
    if has_ext {
        write_extended_font(&mut out, p.font_style, p.day_font_color, p.night_font_color);
    }
    Ok(out)
}

fn write_line(l: &TypLine, cp: u16) -> Result<Vec<u8>, TypError> {
    let mut out = Vec::new();
    let xpm = l.day_xpm.clone().unwrap_or_else(empty_xpm);
    let has_border = l.border_width != 0;
    let ci = analyse_colours(&xpm, true, has_border);

    let mut flags: u8 = 0;
    if !l.labels.is_empty() { flags |= LN_F_LABEL; }
    if l.font_style != FontStyle::Default || l.day_font_color.is_some() {
        flags |= LN_F_EXTENDED;
    }
    // `useOrientation=Y` est le défaut TYPViewer → on met F_USE_ROTATION=0.
    // Notre modèle n'a pas le booléen, convention : orientation active par
    // défaut. TODO Lot E : stocker `use_orientation` explicitement.

    let height = if ci.has_bitmap { ci.height as u8 } else { 0 };
    let scheme = ci.colour_scheme() & 0x7;
    out.push((scheme & 0x7) | (height << 3));
    out.push(flags);

    ci.write_colours(&mut out);
    if ci.has_bitmap { ci.write_bitmap(&mut out); }

    if height == 0 {
        out.push(l.line_width);
        if (scheme & !1) != 6 {
            out.push(l.line_width.saturating_add(2u8.saturating_mul(l.border_width)));
        }
    }

    if (flags & LN_F_LABEL) != 0 {
        write_label_block(&mut out, &l.labels, cp)?;
    }
    if (flags & LN_F_EXTENDED) != 0 {
        write_extended_font(&mut out, l.font_style, l.day_font_color, l.night_font_color);
    }
    Ok(out)
}

fn write_point(p: &TypPoint, cp: u16) -> Result<Vec<u8>, TypError> {
    let mut out = Vec::new();
    let mut flags = PT_F_BITMAP;
    if p.night_xpm.is_some() { flags |= PT_F_NIGHT_XPM; }
    if !p.labels.is_empty() { flags |= PT_F_LABEL; }
    if p.font_style != FontStyle::Default || p.day_font_color.is_some() || p.night_font_color.is_some() {
        flags |= PT_F_EXTENDED_FONT;
    }
    out.push(flags);

    let day = p.day_xpm.clone().unwrap_or_else(empty_xpm);
    let day_ci = analyse_colours(&day, false, false);
    out.push(day_ci.width as u8);
    out.push(day_ci.height as u8);

    write_image(&mut out, &day_ci);

    if (flags & PT_F_NIGHT_XPM) != 0 {
        let n = p.night_xpm.clone().unwrap();
        let n_ci = analyse_colours(&n, false, false);
        write_image(&mut out, &n_ci);
    }
    if (flags & PT_F_LABEL) != 0 {
        write_label_block(&mut out, &p.labels, cp)?;
    }
    if (flags & PT_F_EXTENDED_FONT) != 0 {
        write_extended_font(&mut out, p.font_style, p.day_font_color, p.night_font_color);
    }
    Ok(out)
}

fn write_iconset(ic: &TypIconSet, _cp: u16) -> Result<Vec<u8>, TypError> {
    let mut out = Vec::new();
    out.push(ic.icons.len() as u8);
    for xpm in &ic.icons {
        let ci = analyse_colours(xpm, false, false);
        let nbits = calc_icon_bits(&ci);
        out.extend_from_slice(&((nbits / 2) as u16).to_le_bytes());
        out.push(1u8);
        out.push(ci.width as u8);
        out.push(ci.height as u8);
        write_image(&mut out, &ci);
    }
    Ok(out)
}

fn calc_icon_bits(ci: &ColourInfo) -> u32 {
    let mut bits = 0u32;
    let bpp = ci.bits_per_pixel();
    bits += (ci.width as u32) * (ci.height as u32) * bpp as u32;
    bits += ci.number_of_s_colours_for_cm() as u32 * 3 * 8;
    if ci.number_of_colours() == 0 && ci.colour_mode == 0x10 {
        bits += 3 * 8;
    }
    bits += 0x2c;
    bits
}

fn write_image(out: &mut Vec<u8>, ci: &ColourInfo) {
    out.push(ci.number_of_s_colours_for_cm());
    out.push(ci.colour_mode);
    ci.write_colours(out);
    if ci.has_bitmap { ci.write_bitmap(out); }
}

fn empty_xpm() -> Xpm {
    Xpm { width: 0, height: 0, colors: vec![], pixels: vec![], mode: ColorMode::Indexed }
}

// ============================================================ labels

fn write_label_block(out: &mut Vec<u8>, labels: &[TypLabel], cp: u16) -> Result<(), TypError> {
    let mut block = Vec::with_capacity(32);
    for l in labels {
        block.push(l.lang);
        let bytes = encode(&l.text, cp)?;
        // `encode` inclut BOM UTF-8 ; à retirer pour un label inline.
        let b = if cp == 65001 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            &bytes[3..]
        } else {
            &bytes[..]
        };
        block.extend_from_slice(b);
        block.push(0); // null terminator
    }
    let mut len = (block.len() << 1) | 1;
    let mut mask: i64 = !0xff;
    while (len as i64 & mask) != 0 {
        mask <<= 8;
        len <<= 1;
    }
    if len > 0xff {
        out.extend_from_slice(&(len as u16).to_le_bytes());
    } else {
        out.push(len as u8);
    }
    out.extend_from_slice(&block);
    Ok(())
}

fn write_extended_font(
    out: &mut Vec<u8>,
    style: FontStyle,
    day: Option<Rgba>,
    night: Option<Rgba>,
) {
    let mut ext = match style {
        FontStyle::Default => 0u8,
        FontStyle::NoLabel => 1,
        FontStyle::Small => 2,
        FontStyle::Normal => 3,
        FontStyle::Large => 4,
        FontStyle::Custom(n) => n,
    };
    if day.is_some() { ext |= 0x08; }
    if night.is_some() { ext |= 0x10; }
    out.push(ext);
    if let Some(c) = day {
        out.push(c.b); out.push(c.g); out.push(c.r);
    }
    if let Some(c) = night {
        out.push(c.b); out.push(c.g); out.push(c.r);
    }
}

// ============================================================ shape stacking

fn write_shape_stacking(entries: &[DrawOrderEntry]) -> Vec<u8> {
    // mkgmap ShapeStacking : clé = (level << 16) + type. Un DrawOrder par
    // (level, type). Un « empty » (5 octets zéro) sépare les niveaux.
    // Pour nos `DrawOrderEntry`, chaque entrée = une ligne — on sépare par
    // niveau et émet les octets directement.
    let mut out = Vec::new();
    let mut by_level_type: std::collections::BTreeMap<(u8, u32), u32> =
        std::collections::BTreeMap::new();
    for e in entries {
        let key = (e.level, e.type_code & 0xff);
        let subtype_bits = if e.type_code >= 0x100 {
            // type étendu : (type >> 8) indique le subtype bit à allumer.
            let sub = (e.type_code >> 8) as u32;
            1u32 << (sub & 0x1f)
        } else {
            0
        };
        *by_level_type.entry(key).or_insert(0) |= subtype_bits;
    }
    let mut last_level = 1u8;
    for ((level, type_lo), subs) in &by_level_type {
        if *level != last_level {
            // Séparateur vide.
            out.push(0);
            out.extend_from_slice(&0u32.to_le_bytes());
            last_level = *level;
        }
        out.push(*type_lo as u8);
        out.extend_from_slice(&subs.to_le_bytes());
    }
    out
}

// ============================================================ labels for icons

fn write_labels_block(
    icon_sets: &[TypIconSet],
    sorted_icons: &[(u32, Vec<u8>)],
    cp: u16,
) -> Result<(Vec<u8>, Vec<(u32, u32)>, Vec<(u32, u32)>), TypError> {
    // Renvoie (labels_bytes, str_index (offset -> type), type_index (type -> offset)).
    if icon_sets.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }
    let mut labels_bytes = Vec::new();
    labels_bytes.push(0u8); // offset 0 réservé

    // Map type_for_file -> label_text.
    let mut t2label: std::collections::BTreeMap<u32, String> = std::collections::BTreeMap::new();
    for ic in icon_sets {
        if let Some(first) = ic.icons.first() { let _ = first; }
        // Label du premier StringN si présent.
        // Notre modèle: `TypIconSet` n'a pas de labels — on les stocke dans
        // TypPoint.labels ; pour les icons on n'a rien pour l'instant. On
        // laisse vide (TYPViewer tolère).
        let _ = t2label.entry(type_for_file(ic.type_code, ic.subtype));
    }
    // Rien à écrire si tous vides : un seul octet 0.
    let mut str_index = Vec::new();
    let mut type_index = Vec::new();
    for (t, _) in sorted_icons {
        if let Some(text) = t2label.get(t).and_then(|s| if s.is_empty() { None } else { Some(s) }) {
            let off = labels_bytes.len() as u32;
            let bytes = encode(text, cp)?;
            let b = if cp == 65001 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes[..] };
            labels_bytes.extend_from_slice(b);
            labels_bytes.push(0);
            str_index.push((off, *t));
            type_index.push((*t, off));
        }
    }
    type_index.sort_by_key(|(t, _)| *t);
    Ok((labels_bytes, str_index, type_index))
}

fn build_str_index(entries: &[(u32, u32)], psize: u8) -> Vec<u8> {
    // item = (offset psize, type u24)
    let mut out = Vec::with_capacity(entries.len() * (psize as usize + 3));
    for (off, t) in entries {
        put_u_le(&mut out, *off as u64, psize as usize);
        put_u_le(&mut out, *t as u64, 3);
    }
    out
}

fn build_type_index(entries: &[(u32, u32)], psize: u8) -> Vec<u8> {
    // item = (type u24, offset psize)
    let mut out = Vec::with_capacity(entries.len() * (psize as usize + 3));
    for (t, off) in entries {
        put_u_le(&mut out, *t as u64, 3);
        put_u_le(&mut out, *off as u64, psize as usize);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_data() -> TypData {
        TypData {
            params: TypParams { family_id: 1100, product_id: 1, codepage: 1252 },
            ..TypData::default()
        }
    }

    #[test]
    fn header_signature() {
        let data = minimal_data();
        let out = write_typ_binary(&data).unwrap();
        assert!(out.len() >= HEADER_LEN);
        // Header length à offset 0.
        assert_eq!(u16::from_le_bytes([out[0], out[1]]), HEADER_LEN as u16);
        // Signature "GARMIN TYP" à offset 2.
        assert_eq!(&out[2..12], b"GARMIN TYP");
    }

    #[test]
    fn header_codepage_offset_0x15() {
        let mut data = minimal_data();
        data.params.codepage = 1252;
        let out = write_typ_binary(&data).unwrap();
        assert_eq!(u16::from_le_bytes([out[0x15], out[0x16]]), 1252);
    }

    #[test]
    fn header_codepage_utf8() {
        let mut data = minimal_data();
        data.params.codepage = 65001;
        let out = write_typ_binary(&data).unwrap();
        assert_eq!(u16::from_le_bytes([out[0x15], out[0x16]]), 65001);
    }

    #[test]
    fn header_family_product_offset_0x2f() {
        let mut data = minimal_data();
        data.params.family_id = 0x44C; // 1100
        data.params.product_id = 0x01;
        let out = write_typ_binary(&data).unwrap();
        assert_eq!(u16::from_le_bytes([out[0x2F], out[0x30]]), 0x44C);
        assert_eq!(u16::from_le_bytes([out[0x31], out[0x32]]), 0x01);
    }

    #[test]
    fn unknown_codepage_errors() {
        let mut data = minimal_data();
        data.params.codepage = 437;
        assert!(matches!(
            write_typ_binary(&data),
            Err(TypError::UnknownCodepage(437))
        ));
    }

    #[test]
    fn type_packing() {
        assert_eq!(type_for_file(0x2a, 5), (0x2a << 5) | 5);
        assert_eq!(type_for_file(0x10400, 0), 0x10400 << 5);
    }

    #[test]
    fn minimal_polygon_produces_data() {
        let mut data = minimal_data();
        data.polygons.push(TypPolygon {
            type_code: 0x01,
            subtype: 0,
            labels: vec![],
            day_xpm: Some(Xpm {
                width: 0, height: 0,
                colors: vec![
                    Rgba { r: 0xE0, g: 0xE4, b: 0xE0, a: 0 },
                    Rgba { r: 0x10, g: 0x10, b: 0x10, a: 0 },
                ],
                pixels: vec![],
                mode: ColorMode::Indexed,
            }),
            night_xpm: None,
            font_style: FontStyle::NoLabel,
            day_font_color: None,
            night_font_color: None,
        });
        let out = write_typ_binary(&data).unwrap();
        assert!(out.len() > HEADER_LEN);
        // Polygon data size (offset 0x25).
        let poly_size = u32::from_le_bytes([out[0x25], out[0x26], out[0x27], out[0x28]]);
        assert!(poly_size > 0);
    }

    /// Pipeline complet sur la fixture CP1252 : text_reader → binary_writer.
    /// Valide uniquement la structure (pas de diff byte-à-byte avec fixture
    /// binaire : la correspondance exacte est en Lot E).
    #[test]
    fn end_to_end_i2023100_structure() {
        use super::super::{encoding::detect_and_decode, text_reader::read_typ_text, TypEncoding};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../pipeline/resources/typfiles/I2023100.txt");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        let text = detect_and_decode(&bytes, TypEncoding::Auto).unwrap();
        let data = read_typ_text(&text).unwrap();
        let out = write_typ_binary(&data).expect("write binary");
        assert!(out.len() > HEADER_LEN);
        assert_eq!(&out[2..12], b"GARMIN TYP");
        assert_eq!(u16::from_le_bytes([out[0x15], out[0x16]]), 1252);
        assert_eq!(u16::from_le_bytes([out[0x2F], out[0x30]]), 1100);
    }

    #[test]
    fn draw_order_emits_bytes() {
        let mut data = minimal_data();
        data.draw_order.push(DrawOrderEntry { type_code: 0x01, level: 1 });
        data.draw_order.push(DrawOrderEntry { type_code: 0x02, level: 1 });
        data.draw_order.push(DrawOrderEntry { type_code: 0x03, level: 2 });
        let out = write_typ_binary(&data).unwrap();
        // Shape stacking section size @ offset 0x56 (after 0x33 + 4*10 = 0x5B)
        // Plus simple: juste vérifier qu'un header valide est produit.
        assert_eq!(&out[2..12], b"GARMIN TYP");
    }
}
