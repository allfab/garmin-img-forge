/// Reader binaire TYP.
///
/// Port de `imgforge/src/typ/binary_reader.rs` adapté au modèle `TypDocument`.
use super::model::*;
use crate::error::{Result, TypforgeError};

/// Décompile un binaire TYP en [`TypDocument`].
pub fn decompile(bytes: &[u8]) -> Result<TypDocument> {
    if bytes.len() < 21 {
        return Err(TypforgeError::Binary(format!("Fichier trop court: {} octets", bytes.len())));
    }
    let header_len = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    if header_len < 0x5B || header_len > bytes.len() {
        return Err(TypforgeError::Binary(format!("Longueur header invalide: {:#x}", header_len)));
    }
    if &bytes[2..12] != b"GARMIN TYP" {
        return Err(TypforgeError::Binary("Signature GARMIN TYP absente".into()));
    }

    let mut r = Reader::new(bytes, 0x15);
    let codepage = r.u16()?;
    if codepage != 1252 && codepage != 65001 {
        return Err(TypforgeError::Binary(format!("Codepage non supporté: {}", codepage)));
    }

    let point_data = r.section_no_item()?;
    let line_data = r.section_no_item()?;
    let poly_data = r.section_no_item()?;

    let family_id = r.u16()?;
    let product_id = r.u16()?;

    let point_idx = r.section()?;
    let line_idx = r.section()?;
    let poly_idx = r.section()?;
    let stack = r.section()?;

    let (icon_idx, icon_data) = if header_len > 0x5B {
        let icon_idx = r.section()?;
        r.skip(1)?; // 0x13 flag
        let icon_data = r.section_no_item()?;
        r.skip(4)?;
        (icon_idx, icon_data)
    } else {
        (SecInfo::default(), SecInfo::default())
    };

    let mut doc = TypDocument::default();
    doc.param = TypParam {
        family_id,
        product_id,
        codepage,
        header_str: String::new(),
    };

    doc.draw_order = parse_shape_stacking(bytes, &stack)?;

    doc.polygons = parse_index_elements(bytes, &poly_idx, &poly_data, codepage, Kind::Polygon)?
        .into_iter().filter_map(|e| if let Elem::Polygon(p) = e { Some(p) } else { None }).collect();

    doc.lines = parse_index_elements(bytes, &line_idx, &line_data, codepage, Kind::Line)?
        .into_iter().filter_map(|e| if let Elem::Line(l) = e { Some(l) } else { None }).collect();

    doc.points = parse_index_elements(bytes, &point_idx, &point_data, codepage, Kind::Point)?
        .into_iter().filter_map(|e| if let Elem::Point(p) = e { Some(p) } else { None }).collect();

    doc.icons = parse_index_elements(bytes, &icon_idx, &icon_data, codepage, Kind::Icons)?
        .into_iter().filter_map(|e| if let Elem::Icons(i) = e { Some(i) } else { None }).collect();

    Ok(doc)
}

// ─── helpers ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
struct SecInfo { pos: usize, size: usize, item_size: u16 }

impl SecInfo {
    fn is_empty(&self) -> bool { self.size == 0 }
    fn slice<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8]> {
        if self.is_empty() { return Ok(&[]); }
        let end = self.pos.checked_add(self.size)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| TypforgeError::Binary(
                format!("Section hors fichier: pos={} size={} len={}", self.pos, self.size, bytes.len())))?;
        Ok(&bytes[self.pos..end])
    }
}

struct Reader<'a> { buf: &'a [u8], pos: usize }

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8], pos: usize) -> Self { Self { buf, pos } }
    fn remaining(&self) -> usize { self.buf.len().saturating_sub(self.pos) }
    fn need(&self, n: usize) -> Result<()> {
        if self.remaining() < n {
            Err(TypforgeError::Binary(format!("Lecture hors buffer à {:#x}", self.pos)))
        } else { Ok(()) }
    }
    fn u8(&mut self) -> Result<u8> {
        self.need(1)?; let v = self.buf[self.pos]; self.pos += 1; Ok(v)
    }
    fn u16(&mut self) -> Result<u16> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos+1]]);
        self.pos += 2; Ok(v)
    }
    fn u32(&mut self) -> Result<u32> {
        self.need(4)?;
        let v = u32::from_le_bytes([
            self.buf[self.pos], self.buf[self.pos+1],
            self.buf[self.pos+2], self.buf[self.pos+3],
        ]);
        self.pos += 4; Ok(v)
    }
    fn skip(&mut self, n: usize) -> Result<()> { self.need(n)?; self.pos += n; Ok(()) }
    fn section_no_item(&mut self) -> Result<SecInfo> {
        let pos = self.u32()? as usize;
        let size = self.u32()? as usize;
        Ok(SecInfo { pos, size, item_size: 0 })
    }
    fn section(&mut self) -> Result<SecInfo> {
        let pos = self.u32()? as usize;
        let item_size = self.u16()?;
        let size = self.u32()? as usize;
        Ok(SecInfo { pos, size, item_size })
    }
}

// ─── shape stacking ──────────────────────────────────────────────────────────

fn parse_shape_stacking(bytes: &[u8], sec: &SecInfo) -> Result<Vec<DrawOrderEntry>> {
    let data = sec.slice(bytes)?;
    if data.is_empty() { return Ok(Vec::new()); }
    let mut out = Vec::new();
    let mut level = 1u8;
    let mut i = 0;
    while i + 5 <= data.len() {
        let t = data[i];
        let subs = u32::from_le_bytes([data[i+1], data[i+2], data[i+3], data[i+4]]);
        i += 5;
        if t == 0 && subs == 0 {
            level = level.saturating_add(1);
            continue;
        }
        if subs == 0 {
            out.push(DrawOrderEntry { type_code: t as u16, sub_type: 0, level });
        } else {
            for bit in 0..32u32 {
                if subs & (1 << bit) != 0 {
                    out.push(DrawOrderEntry {
                        type_code: t as u16,
                        sub_type: (bit & 0x1f) as u8,
                        level,
                    });
                }
            }
        }
    }
    Ok(out)
}

// ─── element index ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)] enum Kind { Point, Line, Polygon, Icons }

enum Elem {
    Point(TypPoint),
    Line(TypLine),
    Polygon(TypPolygon),
    Icons(TypIconSet),
}

fn parse_index_elements(
    bytes: &[u8], idx: &SecInfo, data: &SecInfo, codepage: u16, kind: Kind,
) -> Result<Vec<Elem>> {
    let idx_bytes = idx.slice(bytes)?;
    if idx_bytes.is_empty() || idx.item_size == 0 { return Ok(Vec::new()); }
    let type_size = 2usize;
    let item_size = idx.item_size as usize;
    if item_size < type_size {
        return Err(TypforgeError::Binary(format!("item_size {} < type_size", item_size)));
    }
    let psize = item_size - type_size;
    let data_bytes = data.slice(bytes)?;

    let n = idx_bytes.len() / item_size;
    let mut entries: Vec<(u32, usize)> = Vec::with_capacity(n);
    for i in 0..n {
        let start = i * item_size;
        let mut tfp = 0u64;
        for j in 0..type_size { tfp |= (idx_bytes[start + j] as u64) << (j * 8); }
        let mut off = 0u64;
        for j in 0..psize { off |= (idx_bytes[start + type_size + j] as u64) << (j * 8); }
        entries.push((tfp as u32, off as usize));
    }

    let mut by_offset = entries.clone();
    by_offset.sort_by_key(|(_, o)| *o);
    let mut size_for: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (i, (_, off)) in by_offset.iter().enumerate() {
        let end = if i + 1 < by_offset.len() { by_offset[i+1].1 } else { data_bytes.len() };
        size_for.insert(*off, end.saturating_sub(*off));
    }

    let mut out = Vec::with_capacity(entries.len());
    for (tfp, off) in entries {
        let size = *size_for.get(&off).unwrap_or(&0);
        if off + size > data_bytes.len() {
            return Err(TypforgeError::Binary(format!("Élément hors section: off={} size={}", off, size)));
        }
        let slice = &data_bytes[off..off + size];
        let (type_code_u32, subtype) = unpack_type(tfp);
        let type_code = type_code_u32 as u16;
        let el = match kind {
            Kind::Polygon => Elem::Polygon(parse_polygon(slice, type_code, subtype, codepage)?),
            Kind::Line => Elem::Line(parse_line(slice, type_code, subtype, codepage)?),
            Kind::Point => Elem::Point(parse_point(slice, type_code, subtype, codepage)?),
            Kind::Icons => Elem::Icons(parse_iconset(slice, type_code, subtype)?),
        };
        out.push(el);
    }
    Ok(out)
}

fn unpack_type(v: u32) -> (u32, u8) { (v >> 5, (v & 0x1f) as u8) }

// ─── element parsers ─────────────────────────────────────────────────────────

const POLYGON_W: u16 = 32;
const POLYGON_H: u16 = 32;
const LINE_W: u16 = 32;

fn parse_polygon(bytes: &[u8], type_code: u16, sub_type: u8, cp: u16) -> Result<TypPolygon> {
    if bytes.is_empty() {
        return Ok(TypPolygon { type_code, sub_type, ..TypPolygon::default() });
    }
    let mut r = Reader::new(bytes, 0);
    let scheme = r.u8()?;
    let has_bitmap = scheme & 0x08 != 0;
    let has_label = scheme & 0x10 != 0;
    let has_ext = scheme & 0x20 != 0;
    let colours = read_simple_colours(&mut r, scheme & 0x0F)?;
    let (w, h, flat_pix) = if has_bitmap {
        let pix = read_bitmap_rows(&mut r, POLYGON_W, POLYGON_H, 1)?;
        (POLYGON_W, POLYGON_H, pix)
    } else { (0, 0, vec![]) };
    let labels = if has_label { read_label_block(&mut r, cp)? } else { vec![] };
    let (font_style, day_c, night_c) = if has_ext { read_extended_font(&mut r)? } else { (FontStyle::default(), None, None) };
    let day_xpm = Some(make_xpm(w, h, colours, flat_pix));
    Ok(TypPolygon {
        type_code, sub_type, grmn_type: String::new(), labels,
        day_xpm, night_xpm: None,
        font_style,
        day_font_colour: day_c.map(rgba_to_rgb),
        night_font_colour: night_c.map(rgba_to_rgb),
        extended_labels: false,
        contour_color: ContourColor::No,
    })
}

fn parse_line(bytes: &[u8], type_code: u16, sub_type: u8, cp: u16) -> Result<TypLine> {
    if bytes.is_empty() {
        return Ok(TypLine { type_code, sub_type, use_orientation: true, ..TypLine::default() });
    }
    let mut r = Reader::new(bytes, 0);
    let byte0 = r.u8()?;
    let scheme = byte0 & 0x7;
    let height = (byte0 >> 3) as u16;
    let flags = r.u8()?;
    let has_label = flags & 0x01 != 0;
    let use_orientation = flags & 0x02 == 0;
    let has_ext = flags & 0x04 != 0;
    let colours = read_simple_colours(&mut r, scheme)?;
    let (w, flat_pix) = if height > 0 {
        let pix = read_bitmap_rows(&mut r, LINE_W, height, 1)?;
        (LINE_W, pix)
    } else { (0, vec![]) };
    let (line_width, border_width) = if height == 0 {
        let lw = r.u8()?;
        let bw = if (scheme & !1) != 6 {
            let combined = r.u8()?;
            combined.saturating_sub(lw) / 2
        } else { 0 };
        (lw, bw)
    } else { (0, 0) };
    let labels = if has_label { read_label_block(&mut r, cp)? } else { vec![] };
    let (font_style, day_c, night_c) = if has_ext { read_extended_font(&mut r)? } else { (FontStyle::default(), None, None) };
    Ok(TypLine {
        type_code, sub_type, grmn_type: String::new(), labels,
        day_xpm: Some(make_xpm(w, height, colours, flat_pix)),
        night_xpm: None,
        line_width, border_width,
        font_style, use_orientation,
        day_font_colour: day_c.map(rgba_to_rgb),
        night_font_colour: night_c.map(rgba_to_rgb),
        extended_labels: false,
    })
}

fn parse_point(bytes: &[u8], type_code: u16, sub_type: u8, cp: u16) -> Result<TypPoint> {
    if bytes.is_empty() {
        return Ok(TypPoint { type_code, sub_type, ..TypPoint::default() });
    }
    let mut r = Reader::new(bytes, 0);
    let flags = r.u8()?;
    let has_night = flags & 0x02 != 0;
    let has_label = flags & 0x04 != 0;
    let has_ext = flags & 0x08 != 0;
    let width = r.u8()? as u16;
    let height = r.u8()? as u16;
    let day_xpm = Some(read_full_image(&mut r, width, height)?);
    let night_xpm = if has_night { Some(read_full_image(&mut r, width, height)?) } else { None };
    let labels = if has_label { read_label_block(&mut r, cp)? } else { vec![] };
    let (font_style, day_c, night_c) = if has_ext { read_extended_font(&mut r)? } else { (FontStyle::default(), None, None) };
    Ok(TypPoint {
        type_code, sub_type, grmn_type: String::new(), labels,
        day_xpm, night_xpm,
        font_style,
        day_font_colour: day_c.map(rgba_to_rgb),
        night_font_colour: night_c.map(rgba_to_rgb),
        extended_labels: false,
    })
}

fn parse_iconset(bytes: &[u8], type_code: u16, sub_type: u8) -> Result<TypIconSet> {
    if bytes.is_empty() {
        return Ok(TypIconSet { type_code, sub_type, icons: vec![] });
    }
    let mut r = Reader::new(bytes, 0);
    let n = r.u8()? as usize;
    let mut icons = Vec::with_capacity(n);
    for _ in 0..n {
        let _nbits_half = r.u16()?;
        let _one = r.u8()?;
        let w = r.u8()? as u16;
        let h = r.u8()? as u16;
        icons.push(read_full_image(&mut r, w, h)?);
    }
    Ok(TypIconSet { type_code, sub_type, icons })
}

// ─── image helpers ───────────────────────────────────────────────────────────

fn make_xpm(w: u16, h: u16, colours: Vec<Rgba>, flat: Vec<u8>) -> Xpm {
    let palette: Vec<(String, Rgba)> = colours.iter().enumerate()
        .map(|(i, &c)| (make_tag(i), c))
        .collect();
    let pixels: Vec<Vec<usize>> = if w == 0 || h == 0 {
        Vec::new()
    } else {
        flat.chunks(w as usize)
            .map(|row| row.iter().map(|&b| b as usize).collect())
            .collect()
    };
    Xpm { width: w, height: h, colour_mode: ColorMode::Indexed, palette, pixels }
}

fn make_tag(i: usize) -> String {
    let chars: Vec<char> = (0x21u8..=0x7E)
        .filter(|&b| b != b'"' && b != b'\\')
        .map(|b| b as char)
        .collect();
    chars[i % chars.len()].to_string()
}

fn read_simple_colours(r: &mut Reader, scheme: u8) -> Result<Vec<Rgba>> {
    let has_night = scheme & 0x1 != 0;
    let day_trans = scheme & 0x2 != 0;
    let night_trans = scheme & 0x4 != 0;
    let mut n = if day_trans { 1 } else { 2 };
    if has_night { n += if night_trans { 1 } else { 2 }; }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let b = r.u8()?; let g = r.u8()?; let rv = r.u8()?;
        out.push(Rgba::opaque(rv, g, b));
    }
    Ok(out)
}

fn read_full_image(r: &mut Reader, width: u16, height: u16) -> Result<Xpm> {
    let num_solid = r.u8()? as usize;
    let colour_mode_byte = r.u8()?;
    let mut colours = Vec::with_capacity(num_solid + 1);
    for _ in 0..num_solid {
        let b = r.u8()?; let g = r.u8()?; let rv = r.u8()?;
        colours.push(Rgba::opaque(rv, g, b));
    }
    // colour_mode=0x10 : une couleur transparente en fin de palette
    if colour_mode_byte == 0x10 {
        colours.push(Rgba::transparent());
    }
    let bpp = bits_per_pixel_from_count(colours.len());
    let flat = read_bitmap_rows(r, width, height, bpp)?;
    let colour_mode = match colour_mode_byte { 0x20 => ColorMode::True32, _ => ColorMode::Indexed };
    let _ = colour_mode;
    Ok(make_xpm(width, height, colours, flat))
}

fn bits_per_pixel_from_count(n: usize) -> usize {
    match n { 0 => 24, 1 => 1, 2 | 3 => 2, 4..=15 => 4, _ => 8 }
}

fn read_bitmap_rows(r: &mut Reader, width: u16, height: u16, bpp: usize) -> Result<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    let mut pixels = Vec::with_capacity(w * h);
    let row_bytes = (w * bpp + 7) / 8;
    for _row in 0..h {
        let start = r.pos;
        r.need(row_bytes)?;
        let slice = &r.buf[start..start + row_bytes];
        let mut bit_off = 0usize;
        for _col in 0..w {
            let mut v = 0u32;
            for i in 0..bpp {
                let pos = bit_off + i;
                let byte = slice[pos / 8];
                v |= ((byte >> (pos % 8)) as u32 & 1) << i;
            }
            bit_off += bpp;
            pixels.push(v as u8);
        }
        r.pos = start + row_bytes;
    }
    Ok(pixels)
}

fn read_label_block(r: &mut Reader, cp: u16) -> Result<Vec<TypLabel>> {
    let b0 = r.u8()?;
    let block_len = if b0 & 1 != 0 {
        (b0 as usize) >> 1
    } else if b0 & 2 != 0 {
        let b1 = r.u8()?;
        (((b1 as usize) << 8) | (b0 as usize)) >> 2
    } else {
        return Err(TypforgeError::Binary(format!("Préfixe label invalide: {:#x}", b0)));
    };
    let start = r.pos;
    r.need(block_len)?;
    let end = start + block_len;
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let lang = r.buf[i]; i += 1;
        let text_start = i;
        while i < end && r.buf[i] != 0 { i += 1; }
        let text = decode_label(&r.buf[text_start..i], cp)?;
        out.push(TypLabel { lang, text });
        if i < end { i += 1; }
    }
    r.pos = end;
    Ok(out)
}

fn decode_label(bytes: &[u8], cp: u16) -> Result<String> {
    let enc = match cp {
        65001 => encoding_rs::UTF_8,
        _ => encoding_rs::WINDOWS_1252,
    };
    let (s, _, had_err) = enc.decode(bytes);
    if had_err {
        return Err(TypforgeError::Binary("Label binaire invalide".into()));
    }
    Ok(s.into_owned())
}

fn read_extended_font(r: &mut Reader) -> Result<(FontStyle, Option<Rgba>, Option<Rgba>)> {
    let ext = r.u8()?;
    let style = match ext & 0x07 {
        0 => FontStyle::Default,
        1 => FontStyle::NoLabel,
        2 => FontStyle::Small,
        3 => FontStyle::Normal,
        4 => FontStyle::Large,
        n => FontStyle::Custom(n),
    };
    let day = if ext & 0x08 != 0 { Some(read_bgr(r)?) } else { None };
    let night = if ext & 0x10 != 0 { Some(read_bgr(r)?) } else { None };
    Ok((style, day, night))
}

fn read_bgr(r: &mut Reader) -> Result<Rgba> {
    let b = r.u8()?; let g = r.u8()?; let rv = r.u8()?;
    Ok(Rgba::opaque(rv, g, b))
}

fn rgba_to_rgb(c: Rgba) -> Rgb { Rgb { r: c.r, g: c.g, b: c.b } }
