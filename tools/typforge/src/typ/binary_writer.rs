/// Writer binaire TYP.
///
/// Port de `imgforge/src/typ/binary_writer.rs` adapté au modèle `TypDocument`.
use super::model::*;
use crate::error::{Result, TypforgeError};

const HEADER_LEN: usize = 0x5B;
const COMMON_HEADER_LEN: usize = 21;

const S_NIGHT: u8 = 0x1;
const S_DAY_TRANSPARENT: u8 = 0x2;
const S_NIGHT_TRANSPARENT: u8 = 0x4;
const S_HAS_BITMAP: u8 = 0x8;

const PT_F_BITMAP: u8 = 0x01;
const PT_F_NIGHT_XPM: u8 = 0x02;
const PT_F_LABEL: u8 = 0x04;
const PT_F_EXTENDED_FONT: u8 = 0x08;

const LN_F_LABEL: u8 = 0x01;
const LN_F_USE_ROTATION: u8 = 0x02;
const LN_F_EXTENDED: u8 = 0x04;

const PG_F_LABEL: u8 = 0x10;
const PG_F_EXTENDED: u8 = 0x20;

/// Compile un [`TypDocument`] en binaire TYP.
pub fn compile(doc: &TypDocument) -> Result<Vec<u8>> {
    let cp = if doc.param.codepage == 0 { 1252 } else { doc.param.codepage };
    if cp != 1252 && cp != 65001 {
        return Err(TypforgeError::Binary(format!("Codepage non supporté: {}", cp)));
    }

    let mut points: Vec<(u32, Vec<u8>)> = doc
        .points
        .iter()
        .map(|e| Ok((pack_type(e.type_code, e.sub_type), write_point(e, cp)?)))
        .collect::<Result<_>>()?;
    points.sort_by_key(|(t, _)| *t);

    let mut lines: Vec<(u32, Vec<u8>)> = doc
        .lines
        .iter()
        .map(|e| Ok((pack_type(e.type_code, e.sub_type), write_line(e, cp)?)))
        .collect::<Result<_>>()?;
    lines.sort_by_key(|(t, _)| *t);

    let mut polygons: Vec<(u32, Vec<u8>)> = doc
        .polygons
        .iter()
        .map(|e| Ok((pack_type(e.type_code, e.sub_type), write_polygon(e, cp)?)))
        .collect::<Result<_>>()?;
    polygons.sort_by_key(|(t, _)| *t);

    let (poly_data, poly_offsets) = concat_with_offsets(&polygons);
    let (line_data, line_offsets) = concat_with_offsets(&lines);
    let (point_data, point_offsets) = concat_with_offsets(&points);
    let shape_stacking = write_shape_stacking(&doc.draw_order);

    let poly_idx_psize = pointer_size(poly_data.len());
    let line_idx_psize = pointer_size(line_data.len());
    let point_idx_psize = pointer_size(point_data.len());

    let type_size = 2u16;
    let poly_index = build_index(&polygons, &poly_offsets, type_size, poly_idx_psize);
    let line_index = build_index(&lines, &line_offsets, type_size, line_idx_psize);
    let point_index = build_index(&points, &point_offsets, type_size, point_idx_psize);

    let mut pos = HEADER_LEN;
    let poly_data_pos = pos; pos += poly_data.len();
    let poly_idx_pos = pos; pos += poly_index.len();
    let line_data_pos = pos; pos += line_data.len();
    let line_idx_pos = pos; pos += line_index.len();
    let point_data_pos = pos; pos += point_data.len();
    let point_idx_pos = pos; pos += point_index.len();
    let stack_pos = pos; pos += shape_stacking.len();
    let total_size = pos;

    let header = build_header(HdrInfo {
        codepage: cp,
        family_id: doc.param.family_id,
        product_id: doc.param.product_id,
        point_data_pos, point_data_size: point_data.len(),
        line_data_pos, line_data_size: line_data.len(),
        poly_data_pos, poly_data_size: poly_data.len(),
        point_idx_pos, point_idx_size: point_index.len(),
        point_idx_item_size: type_size + point_idx_psize as u16,
        line_idx_pos, line_idx_size: line_index.len(),
        line_idx_item_size: type_size + line_idx_psize as u16,
        poly_idx_pos, poly_idx_size: poly_index.len(),
        poly_idx_item_size: type_size + poly_idx_psize as u16,
        stack_pos, stack_size: shape_stacking.len(), stack_item_size: 5,
    });

    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&header);
    out.extend_from_slice(&poly_data);
    out.extend_from_slice(&poly_index);
    out.extend_from_slice(&line_data);
    out.extend_from_slice(&line_index);
    out.extend_from_slice(&point_data);
    out.extend_from_slice(&point_index);
    out.extend_from_slice(&shape_stacking);
    Ok(out)
}

// ─── header ──────────────────────────────────────────────────────────────────

struct HdrInfo {
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
}

fn build_header(h: HdrInfo) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(&(HEADER_LEN as u16).to_le_bytes());
    out.extend_from_slice(b"GARMIN TYP");
    out.push(0x01);
    out.push(0x00);
    // date fixe pour déterminisme
    out.extend_from_slice(&[0xEA, 0x07, 0x04, 0x0E, 0x10, 0x00, 0x00]);
    assert_eq!(out.len(), COMMON_HEADER_LEN);
    out.extend_from_slice(&h.codepage.to_le_bytes());
    zap_pos_size(&mut out, h.point_data_pos, h.point_data_size);
    zap_pos_size(&mut out, h.line_data_pos, h.line_data_size);
    zap_pos_size(&mut out, h.poly_data_pos, h.poly_data_size);
    out.extend_from_slice(&h.family_id.to_le_bytes());
    out.extend_from_slice(&h.product_id.to_le_bytes());
    zap_pos_isize_size(&mut out, h.point_idx_pos, h.point_idx_item_size, h.point_idx_size);
    zap_pos_isize_size(&mut out, h.line_idx_pos, h.line_idx_item_size, h.line_idx_size);
    zap_pos_isize_size(&mut out, h.poly_idx_pos, h.poly_idx_item_size, h.poly_idx_size);
    zap_pos_isize_size(&mut out, h.stack_pos, h.stack_item_size, h.stack_size);
    out
}

fn zap_pos_size(out: &mut Vec<u8>, pos: usize, size: usize) {
    let p = if size == 0 { 0u32 } else { pos as u32 };
    out.extend_from_slice(&p.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
}

fn zap_pos_isize_size(out: &mut Vec<u8>, pos: usize, item_size: u16, size: usize) {
    let (p, is) = if size == 0 { (0u32, 0u16) } else { (pos as u32, item_size) };
    out.extend_from_slice(&p.to_le_bytes());
    out.extend_from_slice(&is.to_le_bytes());
    out.extend_from_slice(&(size as u32).to_le_bytes());
}

// ─── helpers index ───────────────────────────────────────────────────────────

fn pack_type(type_code: u16, sub_type: u8) -> u32 {
    (u32::from(type_code) << 5) | u32::from(sub_type & 0x1f)
}

fn pointer_size(size: usize) -> u8 {
    if size <= 0xff { 1 } else if size <= 0xffff { 2 } else if size <= 0xff_ffff { 3 } else { 4 }
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

fn build_index(elems: &[(u32, Vec<u8>)], offsets: &[usize], type_size: u16, psize: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(elems.len() * (type_size as usize + psize as usize));
    for (i, (t, _)) in elems.iter().enumerate() {
        put_u_le(&mut out, *t as u64, type_size as usize);
        put_u_le(&mut out, offsets[i] as u64, psize as usize);
    }
    out
}

fn put_u_le(out: &mut Vec<u8>, v: u64, n: usize) {
    for i in 0..n { out.push(((v >> (i * 8)) & 0xff) as u8); }
}

// ─── ColourInfo ──────────────────────────────────────────────────────────────

struct ColourInfo {
    width: u16,
    height: u16,
    colours: Vec<Rgba>,
    pixels: Option<Vec<u8>>,
    colour_mode: u8,
    simple: bool,
    has_bitmap: bool,
    has_border: bool,
    nr_solid_colours: u8,
}

fn analyse_colours(xpm: &Xpm, simple: bool, has_border: bool) -> ColourInfo {
    // Extraire les couleurs depuis la palette (sans les tags)
    let mut colours: Vec<Rgba> = xpm.palette.iter().map(|(_, c)| *c).collect();
    let mut colour_mode: u8 = 0;

    if simple {
        if !colours.is_empty() && colours[0].is_transparent() && colours.len() >= 2 {
            colours.swap(0, 1);
        }
        if colours.len() > 2 && colours[2].is_transparent() && colours.len() >= 4 {
            colours.swap(2, 3);
        }
    } else {
        let n_trans = colours.iter().filter(|c| c.is_transparent()).count();
        let n_alpha = colours.iter().filter(|c| c.a != 0 && !c.is_transparent()).count();
        let count = colours.len();
        if n_alpha > 0 || (count > 0 && count == n_trans) {
            colour_mode = 0x20;
        } else if n_trans == 1 {
            colour_mode = 0x10;
            if let Some(idx) = colours.iter().position(|c| c.is_transparent()) {
                let t = colours.remove(idx);
                colours.push(t);
            }
        }
    }

    let nr_solid_colours = colours.iter().filter(|c| !c.is_transparent()).count() as u8;

    // Aplatir les pixels 2D en Vec<u8>
    let has_pixels = !xpm.pixels.is_empty() && xpm.width > 0 && xpm.height > 0;
    let pixels = if has_pixels {
        let flat: Vec<u8> = xpm.pixels.iter()
            .flat_map(|row| row.iter().map(|&idx| idx as u8))
            .collect();
        Some(flat)
    } else {
        None
    };

    ColourInfo {
        width: xpm.width,
        height: xpm.height,
        colours,
        pixels,
        colour_mode,
        simple,
        has_bitmap: has_pixels,
        has_border,
        nr_solid_colours,
    }
}

impl ColourInfo {
    fn number_of_colours(&self) -> usize { self.colours.len() }

    fn bits_per_pixel(&self) -> usize {
        if self.simple { return 1; }
        match self.number_of_colours() {
            0 => 24, 1 => 1, 2 | 3 => 2, 4..=15 => 4, _ => 8,
        }
    }

    fn colour_scheme(&self) -> u8 {
        let n = self.number_of_colours();
        let mut scheme = 0u8;
        if self.has_bitmap { scheme |= S_HAS_BITMAP; }
        if n == 4 { scheme |= S_NIGHT; }
        if !self.has_bitmap && !self.has_border && n == 2 {
            scheme |= S_NIGHT | S_DAY_TRANSPARENT | S_NIGHT_TRANSPARENT;
        }
        if n < 2 || (n >= 2 && self.colours[1].is_transparent()) {
            scheme |= S_DAY_TRANSPARENT;
        }
        if n == 4 && self.colours.get(3).map_or(false, |c| c.is_transparent()) {
            scheme |= S_NIGHT_TRANSPARENT;
        }
        if (scheme & S_NIGHT) == 0 && (scheme & S_DAY_TRANSPARENT) != 0 {
            scheme |= S_NIGHT_TRANSPARENT;
        }
        scheme
    }

    fn write_colours(&self, out: &mut Vec<u8>) {
        if self.colour_mode == 0x20 {
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
                if !c.is_transparent() {
                    out.push(c.b); out.push(c.g); out.push(c.r);
                }
            }
        }
    }

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
                    let val = if self.simple {
                        (!u32::from(idx)) & ((1u32 << bits) - 1)
                    } else {
                        u32::from(idx)
                    };
                    bw.putn(val, bits);
                    pi += 1;
                }
                out.extend_from_slice(bw.bytes());
            }
        }
    }

    fn number_of_s_colours_for_cm(&self) -> u8 {
        if self.colour_mode == 0x10 { self.nr_solid_colours } else { self.colours.len() as u8 }
    }
}

fn alpha_round4(alpha: i32) -> i32 {
    let top = (alpha >> 4) & 0xf;
    let low = alpha & 0xf;
    let diff = low - top;
    if diff > 8 { top + 1 } else if diff < -8 { top - 1 } else { top }
}

// ─── BitWriter ───────────────────────────────────────────────────────────────

struct BitWriter { buf: Vec<u8>, bit_offset: usize }

impl BitWriter {
    fn new() -> Self { Self { buf: vec![0u8; 32], bit_offset: 0 } }

    fn putn(&mut self, bval: u32, nb: usize) {
        assert!(nb < 24);
        let mut val = bval & ((1u32 << nb) - 1);
        let mut n = nb;
        let needed = (self.bit_offset + n + 7) / 8;
        if needed >= self.buf.len() {
            self.buf.resize((self.buf.len() * 2).max(needed + 1), 0);
        }
        while n > 0 {
            let ind = self.bit_offset / 8;
            let rem = self.bit_offset % 8;
            self.buf[ind] |= ((val << rem) & 0xff) as u8;
            val >>= 8 - rem;
            let nput = (8 - rem).min(n);
            self.bit_offset += nput;
            n -= nput;
        }
    }

    fn bytes(&self) -> &[u8] { &self.buf[..(self.bit_offset + 7) / 8] }
}

// ─── element writers ─────────────────────────────────────────────────────────

fn write_polygon(p: &TypPolygon, cp: u16) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let xpm = p.day_xpm.as_ref().ok_or_else(|| TypforgeError::Binary(
        format!("polygon type=0x{:x} sans day_xpm", p.type_code),
    ))?;
    let ci = analyse_colours(xpm, true, false);
    let mut scheme = ci.colour_scheme();
    if !p.labels.is_empty() { scheme |= PG_F_LABEL; }
    let has_ext = p.font_style != FontStyle::Default || p.day_font_colour.is_some();
    if has_ext { scheme |= PG_F_EXTENDED; }
    out.push(scheme);
    ci.write_colours(&mut out);
    if ci.has_bitmap { ci.write_bitmap(&mut out); }
    if !p.labels.is_empty() { write_label_block(&mut out, &p.labels, cp)?; }
    if has_ext { write_extended_font(&mut out, p.font_style, p.day_font_colour, p.night_font_colour); }
    Ok(out)
}

fn write_line(l: &TypLine, cp: u16) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let xpm = l.day_xpm.as_ref().ok_or_else(|| TypforgeError::Binary(
        format!("line type=0x{:x} sans day_xpm", l.type_code),
    ))?;
    let ci = analyse_colours(xpm, true, l.border_width != 0);
    let mut flags: u8 = 0;
    if !l.labels.is_empty() { flags |= LN_F_LABEL; }
    if l.font_style != FontStyle::Default || l.day_font_colour.is_some() { flags |= LN_F_EXTENDED; }
    if !l.use_orientation { flags |= LN_F_USE_ROTATION; }
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
    if (flags & LN_F_LABEL) != 0 { write_label_block(&mut out, &l.labels, cp)?; }
    if (flags & LN_F_EXTENDED) != 0 { write_extended_font(&mut out, l.font_style, l.day_font_colour, l.night_font_colour); }
    Ok(out)
}

fn write_point(p: &TypPoint, cp: u16) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut flags = PT_F_BITMAP;
    if p.night_xpm.is_some() { flags |= PT_F_NIGHT_XPM; }
    if !p.labels.is_empty() { flags |= PT_F_LABEL; }
    if p.font_style != FontStyle::Default || p.day_font_colour.is_some() || p.night_font_colour.is_some() {
        flags |= PT_F_EXTENDED_FONT;
    }
    out.push(flags);
    let day = p.day_xpm.as_ref().ok_or_else(|| TypforgeError::Binary(
        format!("point type=0x{:x} sans day_xpm", p.type_code),
    ))?;
    let day_ci = analyse_colours(day, false, false);
    out.push(day_ci.width as u8);
    out.push(day_ci.height as u8);
    write_image(&mut out, &day_ci);
    if (flags & PT_F_NIGHT_XPM) != 0 {
        let n_ci = analyse_colours(p.night_xpm.as_ref().unwrap(), false, false);
        write_image(&mut out, &n_ci);
    }
    if (flags & PT_F_LABEL) != 0 { write_label_block(&mut out, &p.labels, cp)?; }
    if (flags & PT_F_EXTENDED_FONT) != 0 {
        write_extended_font(&mut out, p.font_style, p.day_font_colour, p.night_font_colour);
    }
    Ok(out)
}

fn write_image(out: &mut Vec<u8>, ci: &ColourInfo) {
    out.push(ci.number_of_s_colours_for_cm());
    out.push(ci.colour_mode);
    ci.write_colours(out);
    if ci.has_bitmap { ci.write_bitmap(out); }
}

fn write_label_block(out: &mut Vec<u8>, labels: &[TypLabel], cp: u16) -> Result<()> {
    let mut block = Vec::with_capacity(32);
    for l in labels {
        block.push(l.lang);
        let bytes = encode_text(&l.text, cp)?;
        block.extend_from_slice(&bytes);
        block.push(0);
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

fn encode_text(s: &str, cp: u16) -> Result<Vec<u8>> {
    if cp == 65001 {
        Ok(s.as_bytes().to_vec())
    } else {
        // Remplacer les caractères non-encodables en CP1252 (ex. U+FFFD) par '?'
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(s);
        // encoding_rs remplace les caractères inconnus par '?' automatiquement
        // et retourne `had_errors=true` ; on l'ignore volontairement ici pour
        // tolérer les fichiers source avec des caractères dégradés.
        Ok(bytes.into_owned())
    }
}

fn write_extended_font(out: &mut Vec<u8>, style: FontStyle, day: Option<Rgb>, night: Option<Rgb>) {
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
    if let Some(c) = day { out.push(c.b); out.push(c.g); out.push(c.r); }
    if let Some(c) = night { out.push(c.b); out.push(c.g); out.push(c.r); }
}

// ─── shape stacking ──────────────────────────────────────────────────────────

fn write_shape_stacking(entries: &[DrawOrderEntry]) -> Vec<u8> {
    // Grouper par (level, actual_type).
    // - Type simple (type_code ≤ 0xFF, sub_type == 0) : subs = 0.
    // - Type étendu (type_code > 0xFF OU sub_type != 0) : subs |= 1 << sub_type.
    // Note : le format binaire TYP ne stocke que l'octet bas de actual_type ;
    // l'octet haut (si type_code > 0xFF) est perdu — limitation connue du format.
    let mut by_level_type: std::collections::BTreeMap<(u8, u32), u32> =
        std::collections::BTreeMap::new();
    for e in entries {
        if e.sub_type != 0 || e.type_code > 0xFF {
            let actual = u32::from(e.type_code);
            let bit = 1u32 << (e.sub_type & 0x1f);
            *by_level_type.entry((e.level, actual)).or_insert(0) |= bit;
        } else {
            // Type simple : subs = 0, clé = type_code exact
            by_level_type.entry((e.level, u32::from(e.type_code))).or_insert(0);
        }
    }

    let mut out = Vec::new();
    let mut last_level = 1u8;
    for ((level, actual_type), subs) in &by_level_type {
        if *level != last_level {
            out.push(0);
            out.extend_from_slice(&0u32.to_le_bytes());
            last_level = *level;
        }
        out.push((actual_type & 0xff) as u8);
        out.extend_from_slice(&subs.to_le_bytes());
    }
    out
}
