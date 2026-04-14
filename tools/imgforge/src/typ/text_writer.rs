//! Writer texte TYP : [`TypData`] → bytes encodés (UTF-8 ou CP1252).
//!
//! Format compatible avec [`super::text_reader`] (séparateur `=`, sections
//! délimitées par `[end]`, XPM multi-ligne). Ne vise pas la fidélité
//! byte-à-byte avec TYPViewer — seulement la cohérence sémantique.

use std::fmt::Write as _;

use super::data::*;
use super::encoding::encode;
use super::TypEncoding;
use crate::error::TypError;

/// Sérialise `data` en texte, encodé selon `target`.
///
/// `Auto` est traité comme UTF-8 (BOM inclus).
pub fn write_typ_text(data: &TypData, target: TypEncoding) -> Result<Vec<u8>, TypError> {
    let mut s = String::new();
    emit_id(&mut s, &data.params);
    if !data.draw_order.is_empty() {
        emit_draw_order(&mut s, &data.draw_order);
    }
    for p in &data.polygons {
        emit_polygon(&mut s, p);
    }
    for l in &data.lines {
        emit_line(&mut s, l);
    }
    for p in &data.points {
        emit_point(&mut s, p);
    }
    for ic in &data.icons {
        emit_iconset(&mut s, ic);
    }

    let cp = match target {
        TypEncoding::Cp1252 => 1252,
        TypEncoding::Utf8 | TypEncoding::Auto => 65001,
    };
    encode(&s, cp)
}

// ============================================================ sections

fn emit_id(s: &mut String, p: &TypParams) {
    s.push_str("[_id]\n");
    let _ = writeln!(s, "ProductCode={}", p.product_id);
    let _ = writeln!(s, "FID={}", p.family_id);
    let _ = writeln!(s, "CodePage={}", p.codepage);
    s.push_str("[end]\n\n");
}

fn emit_draw_order(s: &mut String, entries: &[DrawOrderEntry]) {
    s.push_str("[_drawOrder]\n");
    for e in entries {
        let _ = writeln!(s, "Type=0x{:x},{}", e.type_code, e.level);
    }
    s.push_str("[end]\n\n");
}

fn emit_polygon(s: &mut String, p: &TypPolygon) {
    s.push_str("[_polygon]\n");
    emit_type_subtype(s, p.type_code, p.subtype);
    emit_font_style(s, p.font_style);
    if let Some(x) = &p.day_xpm {
        emit_xpm(s, x, "Xpm");
    }
    if let Some(x) = &p.night_xpm {
        emit_xpm(s, x, "DayNightXpm");
    }
    emit_font_colors(s, p.day_font_color, p.night_font_color);
    emit_labels(s, &p.labels);
    s.push_str("[end]\n\n");
}

fn emit_line(s: &mut String, l: &TypLine) {
    s.push_str("[_line]\n");
    emit_type_subtype(s, l.type_code, l.subtype);
    if !l.use_orientation {
        s.push_str("UseOrientation=N\n");
    }
    if l.line_width > 0 {
        let _ = writeln!(s, "LineWidth={}", l.line_width);
    }
    if l.border_width > 0 {
        let _ = writeln!(s, "BorderWidth={}", l.border_width);
    }
    emit_font_style(s, l.font_style);
    if let Some(x) = &l.day_xpm {
        emit_xpm(s, x, "Xpm");
    }
    if let Some(x) = &l.night_xpm {
        emit_xpm(s, x, "DayNightXpm");
    }
    emit_font_colors(s, l.day_font_color, l.night_font_color);
    emit_labels(s, &l.labels);
    s.push_str("[end]\n\n");
}

fn emit_point(s: &mut String, p: &TypPoint) {
    s.push_str("[_point]\n");
    emit_type_subtype(s, p.type_code, p.subtype);
    emit_font_style(s, p.font_style);
    if let Some(x) = &p.day_xpm {
        emit_xpm(s, x, "DayXpm");
    }
    if let Some(x) = &p.night_xpm {
        emit_xpm(s, x, "NightXpm");
    }
    emit_font_colors(s, p.day_font_color, p.night_font_color);
    emit_labels(s, &p.labels);
    s.push_str("[end]\n\n");
}

fn emit_iconset(s: &mut String, ic: &TypIconSet) {
    s.push_str("[_icons]\n");
    emit_type_subtype(s, ic.type_code, ic.subtype);
    for x in &ic.icons {
        emit_xpm(s, x, "Xpm");
    }
    s.push_str("[end]\n\n");
}

// ============================================================ helpers

fn emit_type_subtype(s: &mut String, type_code: u32, subtype: u8) {
    if subtype != 0 {
        let _ = writeln!(s, "Type=0x{:x}", type_code);
        let _ = writeln!(s, "SubType=0x{:02x}", subtype);
    } else {
        let _ = writeln!(s, "Type=0x{:x}", type_code);
    }
}

fn emit_font_style(s: &mut String, style: FontStyle) {
    match style {
        FontStyle::Default => {}
        FontStyle::NoLabel => s.push_str("FontStyle=NoLabel\n"),
        FontStyle::Small => s.push_str("FontStyle=Small\n"),
        FontStyle::Normal => s.push_str("FontStyle=Normal\n"),
        FontStyle::Large => s.push_str("FontStyle=Large\n"),
        FontStyle::Custom(n) => {
            let _ = writeln!(s, "FontStyle={}", n);
        }
    }
}

fn emit_font_colors(s: &mut String, day: Option<Rgba>, night: Option<Rgba>) {
    if let Some(c) = day {
        let _ = writeln!(s, "DayFontColor=#{:02X}{:02X}{:02X}", c.r, c.g, c.b);
    }
    if let Some(c) = night {
        let _ = writeln!(s, "NightFontColor=#{:02X}{:02X}{:02X}", c.r, c.g, c.b);
    }
}

fn emit_labels(s: &mut String, labels: &[TypLabel]) {
    for (i, l) in labels.iter().enumerate() {
        let _ = writeln!(s, "String{}=0x{:02x},{}", i + 1, l.lang, l.text);
    }
}

fn emit_xpm(s: &mut String, xpm: &Xpm, key: &str) {
    let cpp = if xpm.pixels.is_empty() { 0 } else { 1 };
    let c = xpm.colors.len();
    let _ = writeln!(s, "{}=\"{} {} {} {}\"", key, xpm.width, xpm.height, c, cpp);

    // Palette : une ligne par couleur.
    let keys = palette_keys(c, cpp);
    for (i, color) in xpm.colors.iter().enumerate() {
        let k = &keys[i];
        if color.a == 0xff {
            // Couleur marquée « transparente » dans notre modèle (parseur a posé a=0xff pour `none`).
            let _ = writeln!(s, "\"{} c none\"", k);
        } else {
            let _ = writeln!(
                s,
                "\"{} c #{:02X}{:02X}{:02X}\"",
                k, color.r, color.g, color.b
            );
        }
    }

    // Pixels : si cpp > 0, émettre H lignes de W caractères.
    if cpp > 0 && xpm.width > 0 && xpm.height > 0 {
        let w = xpm.width as usize;
        let h = xpm.height as usize;
        for row in 0..h {
            let mut line = String::with_capacity(w);
            for col in 0..w {
                let idx = xpm.pixels.get(row * w + col).copied().unwrap_or(0) as usize;
                let k = keys.get(idx).map(|s| s.as_str()).unwrap_or(" ");
                line.push_str(k);
            }
            let _ = writeln!(s, "\"{}\"", line);
        }
    }
}

/// Génère `n` codes palette uniques de longueur `cpp`. Pour `cpp=1`, utilise
/// des caractères printables en évitant `"` et `\\`.
fn palette_keys(n: usize, cpp: usize) -> Vec<String> {
    if cpp == 0 {
        return (0..n).map(|i| (i + 1).to_string()).collect();
    }
    let mut out = Vec::with_capacity(n);
    // Caractères printables `!`..`~` sauf `"` et `\\`. **N'INCLUT PAS ' '** :
    // l'espace est utilisé comme séparateur dans la syntaxe palette
    // (`"<key> c <color>"`), ce qui casserait le round-trip.
    let chars: Vec<char> = (0x21u8..=0x7E)
        .filter(|&b| b != b'"' && b != b'\\')
        .map(|b| b as char)
        .collect();
    for i in 0..n {
        let mut k = String::new();
        let mut v = i;
        for _ in 0..cpp {
            k.push(chars[v % chars.len()]);
            v /= chars.len();
        }
        out.push(k);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::text_reader::read_typ_text;

    fn sample() -> TypData {
        let mut d = TypData {
            params: TypParams { family_id: 1100, product_id: 1, codepage: 1252 },
            ..TypData::default()
        };
        d.draw_order.push(DrawOrderEntry { type_code: 0x10, level: 1 });
        d.draw_order.push(DrawOrderEntry { type_code: 0x20, level: 2 });
        d.polygons.push(TypPolygon {
            type_code: 0x01, subtype: 0,
            labels: vec![TypLabel { lang: 4, text: "Fôret".into() }],
            day_xpm: Some(Xpm {
                width: 0, height: 0,
                colors: vec![
                    Rgba { r: 0xE0, g: 0xE4, b: 0xE0, a: 0 },
                    Rgba { r: 0x10, g: 0x10, b: 0x10, a: 0 },
                ],
                pixels: vec![], mode: ColorMode::Indexed,
            }),
            night_xpm: None,
            font_style: FontStyle::NoLabel,
            day_font_color: None, night_font_color: None,
        });
        d.lines.push(TypLine {
            type_code: 0x02, subtype: 0,
            labels: vec![],
            day_xpm: Some(Xpm {
                width: 0, height: 0,
                colors: vec![
                    Rgba { r: 0xF8, g: 0x00, b: 0x00, a: 0 },
                    Rgba { r: 0x00, g: 0x00, b: 0x00, a: 0 },
                ],
                pixels: vec![], mode: ColorMode::Indexed,
            }),
            night_xpm: None,
            line_width: 4, border_width: 1,
            font_style: FontStyle::Default,
            day_font_color: None, night_font_color: None,
            use_orientation: true,
        });
        d
    }

    #[test]
    fn utf8_starts_with_bom() {
        let bytes = write_typ_text(&sample(), TypEncoding::Utf8).unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn cp1252_roundtrip_keeps_accents() {
        let bytes = write_typ_text(&sample(), TypEncoding::Cp1252).unwrap();
        // 'ô' = 0xF4 en CP1252
        assert!(bytes.contains(&0xF4));
    }

    #[test]
    fn semantic_roundtrip_id_draworder() {
        let d = sample();
        let bytes = write_typ_text(&d, TypEncoding::Utf8).unwrap();
        // Strip BOM avant parse.
        let text = std::str::from_utf8(&bytes[3..]).unwrap();
        let d2 = read_typ_text(text).unwrap();
        assert_eq!(d2.params.family_id, 1100);
        assert_eq!(d2.params.product_id, 1);
        assert_eq!(d2.params.codepage, 1252);
        assert_eq!(d2.draw_order.len(), 2);
        assert_eq!(d2.draw_order[0].type_code, 0x10);
        assert_eq!(d2.draw_order[0].level, 1);
        assert_eq!(d2.draw_order[1].type_code, 0x20);
        assert_eq!(d2.draw_order[1].level, 2);
    }

    #[test]
    fn semantic_roundtrip_polygon_line() {
        let d = sample();
        let bytes = write_typ_text(&d, TypEncoding::Utf8).unwrap();
        let text = std::str::from_utf8(&bytes[3..]).unwrap();
        let d2 = read_typ_text(text).unwrap();
        assert_eq!(d2.polygons.len(), 1);
        let p = &d2.polygons[0];
        assert_eq!(p.type_code, 0x01);
        assert_eq!(p.font_style, FontStyle::NoLabel);
        assert_eq!(p.labels.len(), 1);
        assert_eq!(p.labels[0].text, "Fôret");

        assert_eq!(d2.lines.len(), 1);
        let l = &d2.lines[0];
        assert_eq!(l.line_width, 4);
        assert_eq!(l.border_width, 1);
    }

    #[test]
    fn roundtrip_point_with_bitmap() {
        let mut d = TypData {
            params: TypParams { family_id: 5, product_id: 2, codepage: 1252 },
            ..TypData::default()
        };
        d.points.push(TypPoint {
            type_code: 0x2A, subtype: 3,
            labels: vec![TypLabel { lang: 4, text: "Hello".into() }],
            day_xpm: Some(Xpm {
                width: 2, height: 2,
                colors: vec![
                    Rgba { r: 0xFF, g: 0x00, b: 0x00, a: 0 },
                    Rgba { r: 0, g: 0, b: 0, a: 0xff },
                ],
                pixels: vec![0, 1, 1, 0],
                mode: ColorMode::Indexed,
            }),
            night_xpm: None,
            font_style: FontStyle::Default,
            day_font_color: None, night_font_color: None,
        });
        let bytes = write_typ_text(&d, TypEncoding::Utf8).unwrap();
        let text = std::str::from_utf8(&bytes[3..]).unwrap();
        let d2 = read_typ_text(text).unwrap();
        assert_eq!(d2.points.len(), 1);
        let p = &d2.points[0];
        assert_eq!(p.type_code, 0x2A);
        assert_eq!(p.subtype, 3);
        let x = p.day_xpm.as_ref().unwrap();
        assert_eq!(x.width, 2);
        assert_eq!(x.height, 2);
        assert_eq!(x.pixels.len(), 4);
        assert_eq!(p.labels[0].text, "Hello");
    }
}
