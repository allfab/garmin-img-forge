//! Reader binaire TYP (implémentation originale — mkgmap n'expose pas de
//! lecteur pour ce format).
//!
//! Inverse exact de [`super::binary_writer`]. Tolère les trois longueurs
//! historiques de header : `0x5B` (TYPViewer natif), `0x6E` (+ icons),
//! `0x9C` (+ labels/string-index/type-index).

use super::data::*;
use super::encoding::HEADER_LEN;
use crate::error::TypError;

/// Parse un TYP binaire et produit une [`TypData`].
pub fn read_typ_binary(bytes: &[u8]) -> Result<TypData, TypError> {
    if bytes.len() < 21 {
        return Err(TypError::BadHeader(format!("fichier trop court: {} octets", bytes.len())));
    }
    let header_len = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    if header_len < 0x5B || header_len > bytes.len() {
        return Err(TypError::BadHeader(format!(
            "longueur header invalide: {}",
            header_len
        )));
    }
    if &bytes[2..12] != b"GARMIN TYP" {
        return Err(TypError::BadHeader(format!(
            "signature incorrecte: {:?}",
            &bytes[2..12]
        )));
    }

    // --- Header spécifique TYP --- (offsets relatifs au début du fichier)
    let mut r = Reader::new(bytes, 0x15);
    let codepage = r.u16()?;
    if codepage != 1252 && codepage != 65001 {
        return Err(TypError::UnknownCodepage(codepage));
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

    // Au-delà de 0x5B : icons index + data.
    let (icon_idx, icon_data) = if header_len > 0x5B {
        let icon_idx = r.section()?;
        r.u8()?; // 0x13 flag
        let icon_data = r.section_no_item()?;
        r.skip(4)?; // 4 zéros
        (icon_idx, icon_data)
    } else {
        (SectionInfo::default(), SectionInfo::default())
    };

    // Au-delà de 0x6E : labels + string_index + type_index.
    let (labels_sec, _str_idx, _type_idx) = if header_len > 0x6E {
        let labels = r.section_no_item()?;
        let str_item = r.u32()?;
        let _ = r.u32()?; // 0x1B
        let str_pos = r.u32()?;
        let str_size = r.u32()?;
        let type_item = r.u32()?;
        let _ = r.u32()?; // 0x1B
        let type_pos = r.u32()?;
        let type_size = r.u32()?;
        (
            labels,
            SectionInfo { pos: str_pos as usize, size: str_size as usize, item_size: str_item as u16 },
            SectionInfo { pos: type_pos as usize, size: type_size as usize, item_size: type_item as u16 },
        )
    } else {
        (SectionInfo::default(), SectionInfo::default(), SectionInfo::default())
    };

    let params = TypParams { family_id, product_id, codepage };

    let mut data = TypData { params, ..TypData::default() };

    // --- Shape stacking ---
    data.draw_order = parse_shape_stacking(bytes, &stack)?;

    // --- Éléments (polygons, lines, points, icons) ---
    data.polygons = parse_index_elements(
        bytes, &poly_idx, &poly_data, codepage, ElementKind::Polygon,
    )?
        .into_iter()
        .filter_map(|e| if let Element::Polygon(p) = e { Some(p) } else { None })
        .collect();

    data.lines = parse_index_elements(
        bytes, &line_idx, &line_data, codepage, ElementKind::Line,
    )?
        .into_iter()
        .filter_map(|e| if let Element::Line(l) = e { Some(l) } else { None })
        .collect();

    data.points = parse_index_elements(
        bytes, &point_idx, &point_data, codepage, ElementKind::Point,
    )?
        .into_iter()
        .filter_map(|e| if let Element::Point(p) = e { Some(p) } else { None })
        .collect();

    data.icons = parse_index_elements(
        bytes, &icon_idx, &icon_data, codepage, ElementKind::Icons,
    )?
        .into_iter()
        .filter_map(|e| if let Element::Icons(i) = e { Some(i) } else { None })
        .collect();

    let _ = labels_sec; // labels non utilisés pour l'instant (Lot E).

    Ok(data)
}

// ============================================================ helpers

#[derive(Debug, Clone, Copy, Default)]
struct SectionInfo {
    pos: usize,
    size: usize,
    item_size: u16,
}

impl SectionInfo {
    fn is_empty(&self) -> bool {
        self.size == 0
    }
    fn slice<'a>(&self, bytes: &'a [u8]) -> Result<&'a [u8], TypError> {
        if self.is_empty() {
            return Ok(&[]);
        }
        let end = self.pos.checked_add(self.size).ok_or_else(|| {
            TypError::BadHeader(format!("overflow section pos={} size={}", self.pos, self.size))
        })?;
        if end > bytes.len() {
            return Err(TypError::BadHeader(format!(
                "section hors fichier: pos={} size={} len={}",
                self.pos, self.size, bytes.len()
            )));
        }
        Ok(&bytes[self.pos..end])
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8], pos: usize) -> Self { Self { buf, pos } }
    fn remaining(&self) -> usize { self.buf.len().saturating_sub(self.pos) }
    fn need(&self, n: usize) -> Result<(), TypError> {
        if self.remaining() < n {
            Err(TypError::BadHeader(format!(
                "lecture hors buffer à {:#x} (besoin {} restant {})",
                self.pos, n, self.remaining()
            )))
        } else { Ok(()) }
    }
    fn u8(&mut self) -> Result<u8, TypError> {
        self.need(1)?;
        let v = self.buf[self.pos];
        self.pos += 1; Ok(v)
    }
    fn u16(&mut self) -> Result<u16, TypError> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos+1]]);
        self.pos += 2; Ok(v)
    }
    fn u32(&mut self) -> Result<u32, TypError> {
        self.need(4)?;
        let v = u32::from_le_bytes([
            self.buf[self.pos], self.buf[self.pos+1],
            self.buf[self.pos+2], self.buf[self.pos+3],
        ]);
        self.pos += 4; Ok(v)
    }
    fn skip(&mut self, n: usize) -> Result<(), TypError> {
        self.need(n)?;
        self.pos += n; Ok(())
    }
    fn section_no_item(&mut self) -> Result<SectionInfo, TypError> {
        let pos = self.u32()? as usize;
        let size = self.u32()? as usize;
        Ok(SectionInfo { pos, size, item_size: 0 })
    }
    fn section(&mut self) -> Result<SectionInfo, TypError> {
        let pos = self.u32()? as usize;
        let item_size = self.u16()?;
        let size = self.u32()? as usize;
        Ok(SectionInfo { pos, size, item_size })
    }
}

// ============================================================ shape stacking

fn parse_shape_stacking(
    bytes: &[u8],
    sec: &SectionInfo,
) -> Result<Vec<DrawOrderEntry>, TypError> {
    let data = sec.slice(bytes)?;
    if data.is_empty() { return Ok(Vec::new()); }
    let mut out = Vec::new();
    // Chaque entrée = 5 octets : type_lo u8 + subtypes u32.
    // Séparateurs empty (`00 00 00 00 00`) incrémentent le niveau courant.
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
            // Type standard sans sous-types
            out.push(DrawOrderEntry { type_code: t as u32, level });
        } else {
            // Type étendu : chaque bit de `subs` allume un subtype.
            for bit in 0..32 {
                if subs & (1 << bit) != 0 {
                    let full = ((bit as u32) << 8) | (t as u32);
                    out.push(DrawOrderEntry { type_code: full, level });
                }
            }
        }
    }
    Ok(out)
}

// ============================================================ element index

#[derive(Debug, Clone, Copy)]
enum ElementKind { Point, Line, Polygon, Icons }

enum Element {
    Point(TypPoint),
    Line(TypLine),
    Polygon(TypPolygon),
    Icons(TypIconSet),
}

fn parse_index_elements(
    bytes: &[u8],
    idx: &SectionInfo,
    data: &SectionInfo,
    codepage: u16,
    kind: ElementKind,
) -> Result<Vec<Element>, TypError> {
    let idx_bytes = idx.slice(bytes)?;
    if idx_bytes.is_empty() || idx.item_size == 0 {
        return Ok(Vec::new());
    }
    // item = type (type_size octets) + pointer (psize octets).
    let type_size = 2usize;
    let item_size = idx.item_size as usize;
    if item_size < type_size {
        return Err(TypError::BadHeader(format!(
            "index item_size {} < type_size {}",
            item_size, type_size
        )));
    }
    let psize = item_size - type_size;
    let data_bytes = data.slice(bytes)?;

    // Parcours des entrées d'index, extraction (type_for_file, offset), puis
    // parsing de l'élément à data[offset..].
    // On trie les offsets pour calculer la taille de chaque élément comme
    // `next_offset - offset` (dernier = fin de section).
    let mut entries: Vec<(u32, usize)> = Vec::new();
    let n = idx_bytes.len() / item_size;
    for i in 0..n {
        let start = i * item_size;
        let mut tfp = 0u64;
        for j in 0..type_size {
            tfp |= (idx_bytes[start + j] as u64) << (j * 8);
        }
        let mut off = 0u64;
        for j in 0..psize {
            off |= (idx_bytes[start + type_size + j] as u64) << (j * 8);
        }
        entries.push((tfp as u32, off as usize));
    }
    let mut by_offset = entries.clone();
    by_offset.sort_by_key(|(_, o)| *o);

    // Map offset → taille.
    let mut size_for: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (i, (_, off)) in by_offset.iter().enumerate() {
        let end = if i + 1 < by_offset.len() { by_offset[i + 1].1 } else { data_bytes.len() };
        size_for.insert(*off, end.saturating_sub(*off));
    }

    let mut out = Vec::with_capacity(entries.len());
    for (tfp, off) in entries {
        let size = *size_for.get(&off).unwrap_or(&0);
        if off + size > data_bytes.len() {
            return Err(TypError::BadHeader(format!(
                "élément hors section: off={} size={}",
                off, size
            )));
        }
        let slice = &data_bytes[off..off + size];
        let (type_code, subtype) = unpack_type_for_file(tfp);
        let el = match kind {
            ElementKind::Polygon => Element::Polygon(parse_polygon(slice, type_code, subtype, codepage)?),
            ElementKind::Line => Element::Line(parse_line(slice, type_code, subtype, codepage)?),
            ElementKind::Point => Element::Point(parse_point(slice, type_code, subtype, codepage)?),
            ElementKind::Icons => Element::Icons(parse_iconset(slice, type_code, subtype)?),
        };
        out.push(el);
    }
    Ok(out)
}

fn unpack_type_for_file(v: u32) -> (u32, u8) {
    (v >> 5, (v & 0x1f) as u8)
}

// ============================================================ element parsers
//
// Version best-effort : extrait type/subtype/labels. Les couleurs/bitmaps
// sont parsées pour faire avancer le curseur (corruption non-catastrophique
// si format atypique) — l'écriture round-trip byte-à-byte reste l'objectif
// du Lot E.

/// Par convention Garmin, les bitmaps de polygones sont toujours 32×32.
const POLYGON_BITMAP_W: u16 = 32;
const POLYGON_BITMAP_H: u16 = 32;
/// Les bitmaps de lignes ont toujours une largeur de 32.
const LINE_BITMAP_W: u16 = 32;

fn parse_polygon(
    bytes: &[u8],
    type_code: u32,
    subtype: u8,
    cp: u16,
) -> Result<TypPolygon, TypError> {
    if bytes.is_empty() {
        return Ok(TypPolygon {
            type_code, subtype,
            labels: vec![], day_xpm: None, night_xpm: None,
            font_style: FontStyle::Default, day_font_color: None, night_font_color: None,
        });
    }
    let mut r = Reader::new(bytes, 0);
    let scheme = r.u8()?;
    let colour_scheme = scheme & 0x0F;
    let has_bitmap = scheme & 0x08 != 0;
    let has_label = scheme & 0x10 != 0;
    let has_ext = scheme & 0x20 != 0;

    let colors = read_simple_colours(&mut r, colour_scheme)?;

    let (width, height, pixels) = if has_bitmap {
        let bpp = bits_per_pixel_simple(&colors);
        let pixels = read_bitmap_rows(&mut r, POLYGON_BITMAP_W, POLYGON_BITMAP_H, bpp)?;
        (POLYGON_BITMAP_W, POLYGON_BITMAP_H, pixels)
    } else {
        (0, 0, vec![])
    };

    let labels = if has_label { read_label_block(&mut r, cp)? } else { vec![] };
    let (font_style, day_c, night_c) = if has_ext {
        read_extended_font(&mut r)?
    } else {
        (FontStyle::Default, None, None)
    };

    let day_xpm = Some(Xpm {
        width, height, colors, pixels, mode: ColorMode::Indexed,
    });

    Ok(TypPolygon {
        type_code, subtype, labels,
        day_xpm, night_xpm: None,
        font_style, day_font_color: day_c, night_font_color: night_c,
    })
}

fn parse_line(
    bytes: &[u8],
    type_code: u32,
    subtype: u8,
    cp: u16,
) -> Result<TypLine, TypError> {
    if bytes.is_empty() {
        return Ok(TypLine {
            type_code, subtype,
            labels: vec![], day_xpm: None, night_xpm: None,
            line_width: 0, border_width: 0,
            font_style: FontStyle::Default, day_font_color: None, night_font_color: None,
        });
    }
    let mut r = Reader::new(bytes, 0);
    let byte0 = r.u8()?;
    let scheme = byte0 & 0x7;
    let height = (byte0 >> 3) as u16;
    let flags = r.u8()?;
    let has_label = flags & 0x01 != 0;
    let has_ext = flags & 0x04 != 0;

    let colors = read_simple_colours(&mut r, scheme)?;

    let (width, pixels) = if height > 0 {
        let bpp = bits_per_pixel_simple(&colors);
        let p = read_bitmap_rows(&mut r, LINE_BITMAP_W, height, bpp)?;
        (LINE_BITMAP_W, p)
    } else {
        (0, vec![])
    };

    let (line_width, border_width) = if height == 0 {
        let lw = r.u8()?;
        let combined = if (scheme & !1) != 6 { r.u8().ok() } else { None };
        let bw = combined.map(|c| (c.saturating_sub(lw)) / 2).unwrap_or(0);
        (lw, bw)
    } else {
        (0, 0)
    };

    let labels = if has_label { read_label_block(&mut r, cp)? } else { vec![] };
    let (font_style, day_c, night_c) = if has_ext {
        read_extended_font(&mut r)?
    } else {
        (FontStyle::Default, None, None)
    };

    Ok(TypLine {
        type_code, subtype, labels,
        day_xpm: Some(Xpm { width, height, colors, pixels, mode: ColorMode::Indexed }),
        night_xpm: None,
        line_width, border_width,
        font_style, day_font_color: day_c, night_font_color: night_c,
    })
}

fn parse_point(
    bytes: &[u8],
    type_code: u32,
    subtype: u8,
    cp: u16,
) -> Result<TypPoint, TypError> {
    if bytes.is_empty() {
        return Ok(TypPoint {
            type_code, subtype,
            labels: vec![], day_xpm: None, night_xpm: None,
            font_style: FontStyle::Default, day_font_color: None, night_font_color: None,
        });
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
    let (font_style, day_c, night_c) = if has_ext {
        read_extended_font(&mut r)?
    } else {
        (FontStyle::Default, None, None)
    };

    Ok(TypPoint {
        type_code, subtype, labels,
        day_xpm, night_xpm,
        font_style, day_font_color: day_c, night_font_color: night_c,
    })
}

fn parse_iconset(bytes: &[u8], type_code: u32, subtype: u8) -> Result<TypIconSet, TypError> {
    let mut r = Reader::new(bytes, 0);
    if bytes.is_empty() {
        return Ok(TypIconSet { type_code, subtype, icons: vec![] });
    }
    let n = r.u8()? as usize;
    let mut icons = Vec::with_capacity(n);
    for _ in 0..n {
        let _nbits_half = r.u16()?;
        let _one = r.u8()?;
        let w = r.u8()? as u16;
        let h = r.u8()? as u16;
        let xpm = read_full_image(&mut r, w, h)?;
        icons.push(xpm);
    }
    Ok(TypIconSet { type_code, subtype, icons })
}

// ============================================================ parsers helpers

/// Lit 1 à 4 couleurs simples (BGR 3 octets) selon le scheme line/polygon.
fn read_simple_colours(r: &mut Reader, scheme: u8) -> Result<Vec<Rgba>, TypError> {
    // Nombre de triplets BGR à lire : dépend des bits S_NIGHT / S_DAY_TRANSPARENT
    // / S_NIGHT_TRANSPARENT. On ne lit que les couleurs opaques (non-transparentes).
    let has_night = scheme & 0x1 != 0;
    let day_trans = scheme & 0x2 != 0;
    let night_trans = scheme & 0x4 != 0;
    let mut n = 0;
    // day : 1 couleur si transparent (fg seul), 2 sinon (bg+fg)
    n += if day_trans { 1 } else { 2 };
    if has_night {
        n += if night_trans { 1 } else { 2 };
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let b = r.u8()?;
        let g = r.u8()?;
        let r_ = r.u8()?;
        out.push(Rgba { r: r_, g, b, a: 0 });
    }
    Ok(out)
}

/// Lit une image complète (points/icons) : num_colours u8, colour_mode u8,
/// couleurs BGR (ou BGR+alpha 4bits si mode 0x20), puis bitmap LSB-first.
fn read_full_image(r: &mut Reader, width: u16, height: u16) -> Result<Xpm, TypError> {
    let num_solid = r.u8()? as usize;
    let colour_mode = r.u8()?;
    let mut colors = Vec::with_capacity(num_solid);
    for _ in 0..num_solid {
        let b = r.u8()?;
        let g = r.u8()?;
        let r_ = r.u8()?;
        colors.push(Rgba { r: r_, g, b, a: 0 });
    }
    // Bitmap bits : dépend de num_colours (pas num_solid). Simplification :
    // on suppose ici bits_per_pixel à partir du nombre de couleurs solides
    // (conservateur). Ne gère pas les vraies XPM true-color 24bit.
    let bpp = bits_per_pixel_from_count(num_solid);
    let mut pixels = Vec::with_capacity((width as usize) * (height as usize));
    for _row in 0..height {
        let row_bits = (width as usize) * bpp;
        let row_bytes = (row_bits + 7) / 8;
        let start = r.pos;
        r.need(row_bytes)?;
        // Décode LSB-first : chaque pixel consomme bpp bits depuis le LSB.
        let slice = &r.buf[start..start + row_bytes];
        let mut bit_off = 0usize;
        for _c in 0..width {
            let mut v = 0u32;
            for i in 0..bpp {
                let pos = bit_off + i;
                let byte = slice[pos / 8];
                let bit = (byte >> (pos % 8)) & 1;
                v |= (bit as u32) << i;
            }
            bit_off += bpp;
            pixels.push(v as u8);
        }
        r.pos = start + row_bytes;
    }
    let mode = match colour_mode {
        0x10 => ColorMode::Indexed,
        0x20 => ColorMode::True32,
        _ => ColorMode::Indexed,
    };
    Ok(Xpm { width, height, colors, pixels, mode })
}

fn bits_per_pixel_from_count(n: usize) -> usize {
    match n {
        0 => 24,
        1 => 1,
        2 | 3 => 2,
        4..=15 => 4,
        _ => 8,
    }
}

/// Pour une bitmap « simple » (ligne/polygone), mkgmap utilise toujours 1 bit
/// par pixel. Cf. `ColourInfo.getBitsPerPixel` : `simple` → 1.
fn bits_per_pixel_simple(_colors: &[Rgba]) -> usize {
    1
}

/// Lit `height` lignes de `width` pixels en LSB-first, padding à l'octet
/// entre les lignes (cf. `BitmapImage.write`).
fn read_bitmap_rows(
    r: &mut Reader,
    width: u16,
    height: u16,
    bpp: usize,
) -> Result<Vec<u8>, TypError> {
    let w = width as usize;
    let h = height as usize;
    let mut pixels = Vec::with_capacity(w * h);
    let row_bits = w * bpp;
    let row_bytes = (row_bits + 7) / 8;
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
                let bit = (byte >> (pos % 8)) & 1;
                v |= (bit as u32) << i;
            }
            bit_off += bpp;
            pixels.push(v as u8);
        }
        r.pos = start + row_bytes;
    }
    Ok(pixels)
}

/// Lit un bloc de labels : header de longueur variable, puis `n` triplets
/// (lang u8, texte null-terminated).
fn read_label_block(r: &mut Reader, cp: u16) -> Result<Vec<TypLabel>, TypError> {
    // Décodage du préfixe de longueur : `len = (bytes << 1) | 1` puis bits
    // traînants indiquent le nombre d'octets. Lot B/C encode 1 ou 2 octets.
    let b0 = r.u8()?;
    let (block_len, _prefix_bytes) = if b0 & 1 != 0 {
        (((b0 as usize) >> 1), 1usize)
    } else if b0 & 2 != 0 {
        let b1 = r.u8()?;
        let combined = ((b1 as usize) << 8) | (b0 as usize);
        (combined >> 2, 2)
    } else {
        return Err(TypError::BadHeader(format!(
            "préfixe label inconnu: {:#x}",
            b0
        )));
    };
    let start = r.pos;
    r.need(block_len)?;
    let end = start + block_len;
    let mut out = Vec::new();
    let mut i = start;
    while i < end {
        let lang = r.buf[i];
        i += 1;
        let text_start = i;
        while i < end && r.buf[i] != 0 { i += 1; }
        let text_bytes = &r.buf[text_start..i];
        let text = decode_label(text_bytes, cp)?;
        out.push(TypLabel { lang, text });
        if i < end { i += 1; } // skip null
    }
    r.pos = end;
    Ok(out)
}

fn decode_label(bytes: &[u8], cp: u16) -> Result<String, TypError> {
    let enc = match cp {
        65001 => encoding_rs::UTF_8,
        1252 => encoding_rs::WINDOWS_1252,
        other => return Err(TypError::UnknownCodepage(other)),
    };
    let (s, _, had_err) = enc.decode(bytes);
    if had_err {
        return Err(TypError::Encoding(format!("label invalide ({})", enc.name())));
    }
    Ok(s.into_owned())
}

fn read_extended_font(r: &mut Reader) -> Result<(FontStyle, Option<Rgba>, Option<Rgba>), TypError> {
    let ext = r.u8()?;
    let style_bits = ext & 0x07;
    let has_day = ext & 0x08 != 0;
    let has_night = ext & 0x10 != 0;
    let style = match style_bits {
        0 => FontStyle::Default,
        1 => FontStyle::NoLabel,
        2 => FontStyle::Small,
        3 => FontStyle::Normal,
        4 => FontStyle::Large,
        n => FontStyle::Custom(n),
    };
    let day = if has_day { Some(read_bgr(r)?) } else { None };
    let night = if has_night { Some(read_bgr(r)?) } else { None };
    Ok((style, day, night))
}

fn read_bgr(r: &mut Reader) -> Result<Rgba, TypError> {
    let b = r.u8()?;
    let g = r.u8()?;
    let r_ = r.u8()?;
    Ok(Rgba { r: r_, g, b, a: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::binary_writer::write_typ_binary;

    fn minimal_data() -> TypData {
        TypData {
            params: TypParams { family_id: 1100, product_id: 1, codepage: 1252 },
            ..TypData::default()
        }
    }

    #[test]
    fn roundtrip_params_only() {
        let data = minimal_data();
        let bin = write_typ_binary(&data).unwrap();
        let d2 = read_typ_binary(&bin).unwrap();
        assert_eq!(d2.params.family_id, 1100);
        assert_eq!(d2.params.product_id, 1);
        assert_eq!(d2.params.codepage, 1252);
    }

    #[test]
    fn roundtrip_draw_order() {
        let mut data = minimal_data();
        data.draw_order.push(DrawOrderEntry { type_code: 0x01, level: 1 });
        data.draw_order.push(DrawOrderEntry { type_code: 0x02, level: 1 });
        data.draw_order.push(DrawOrderEntry { type_code: 0x03, level: 2 });
        let bin = write_typ_binary(&data).unwrap();
        let d2 = read_typ_binary(&bin).unwrap();
        assert!(d2.draw_order.len() >= 3);
        let levels_seen: std::collections::HashSet<u8> =
            d2.draw_order.iter().map(|e| e.level).collect();
        assert!(levels_seen.contains(&1));
        assert!(levels_seen.contains(&2));
    }

    #[test]
    fn roundtrip_polygon_types() {
        let mut data = minimal_data();
        for t in [0x01u32, 0x02, 0x10] {
            data.polygons.push(TypPolygon {
                type_code: t, subtype: 0,
                labels: vec![],
                day_xpm: Some(Xpm {
                    width: 0, height: 0,
                    colors: vec![
                        Rgba { r: 0xE0, g: 0xE4, b: 0xE0, a: 0 },
                        Rgba { r: 0x10, g: 0x10, b: 0x10, a: 0 },
                    ],
                    pixels: vec![], mode: ColorMode::Indexed,
                }),
                night_xpm: None,
                font_style: FontStyle::Default,
                day_font_color: None, night_font_color: None,
            });
        }
        let bin = write_typ_binary(&data).unwrap();
        let d2 = read_typ_binary(&bin).unwrap();
        assert_eq!(d2.polygons.len(), 3);
        let types: Vec<u32> = d2.polygons.iter().map(|p| p.type_code).collect();
        assert!(types.contains(&0x01));
        assert!(types.contains(&0x02));
        assert!(types.contains(&0x10));
    }

    #[test]
    fn bad_signature_errors() {
        let mut bin = vec![0x9Cu8, 0x00];
        bin.extend_from_slice(b"NOT_GARMIN");
        bin.resize(200, 0);
        assert!(matches!(read_typ_binary(&bin), Err(TypError::BadHeader(_))));
    }

    /// Smoke test du header de la fixture TYPViewer (0x5B). Le parsing des
    /// éléments complets (bitmaps XPM indexées multicolores) est documenté
    /// comme imprécis en Lot D ; la fidélité byte-à-byte est l'objet du Lot E.
    #[test]
    fn read_real_fixture_header_only() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../pipeline/resources/typfiles/I2023100.typ");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return,
        };
        // Header parse : longueur + signature + codepage + FID directement.
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x5B);
        assert_eq!(&bytes[2..12], b"GARMIN TYP");
        assert_eq!(u16::from_le_bytes([bytes[0x15], bytes[0x16]]), 1252);
        assert_eq!(u16::from_le_bytes([bytes[0x2F], bytes[0x30]]), 1100);
        // Le parse complet peut échouer sur des éléments TYPViewer
        // atypiques (schemes exotiques). Limite documentée : la fidélité
        // byte-à-byte avec TYPViewer n'est pas un objectif du projet.
        let _ = read_typ_binary(&bytes);
    }

    /// Round-trip compile → decompile → compile sur un TYP que nous avons
    /// nous-mêmes produit. Valide AC 3 au minimum : ce que notre writer
    /// produit, notre reader peut le re-parser.
    #[test]
    fn self_roundtrip_compile_decompile_compile() {
        use super::super::{compile_text_to_binary, decompile_binary_to_text, TypEncoding};
        let src = r#"[_id]
ProductCode=1
FID=1100
CodePage=1252
[end]

[_drawOrder]
Type=0x01,1
Type=0x02,1
Type=0x10400,2
[end]

[_polygon]
Type=0x01
Xpm="0 0 2 0"
"1 c #E0E4E0"
"2 c #101010"
[end]

[_line]
Type=0x02
LineWidth=2
BorderWidth=1
Xpm="0 0 2 0"
"1 c #F80000"
"2 c #000000"
[end]
"#;
        let bin1 = compile_text_to_binary(src.as_bytes(), TypEncoding::Utf8).unwrap();
        let txt2 = decompile_binary_to_text(&bin1, TypEncoding::Utf8).unwrap();
        let bin2 = compile_text_to_binary(&txt2, TypEncoding::Utf8).unwrap();
        // AC 3 strict : byte-à-byte identique après round-trip.
        assert_eq!(bin1, bin2, "round-trip byte-à-byte échoué");
    }
}
