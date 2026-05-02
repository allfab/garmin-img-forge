use super::model::*;
use crate::error::{Result, TypforgeError};

/// Sérialise un [`TypDocument`] en bytes CP1252.
///
/// Séparateur de ligne CRLF pour compatibilité maximale avec TYPViewer.
pub fn write(doc: &TypDocument) -> Result<Vec<u8>> {
    let mut s = String::new();
    emit_id(&mut s, &doc.param);
    if !doc.draw_order.is_empty() {
        emit_draw_order(&mut s, &doc.draw_order);
    }
    for p in &doc.polygons {
        emit_polygon(&mut s, p);
    }
    for l in &doc.lines {
        emit_line(&mut s, l);
    }
    for p in &doc.points {
        emit_point(&mut s, p);
    }
    for ic in &doc.icons {
        emit_iconset(&mut s, ic);
    }
    if !doc.comments.is_empty() {
        s.push_str("[_comments]\r\n");
        for line in doc.comments.lines() {
            s.push_str(line);
            s.push_str("\r\n");
        }
        s.push_str("[end]\r\n\r\n");
    }

    encode_cp1252(&s)
}

fn encode_cp1252(s: &str) -> Result<Vec<u8>> {
    let (bytes, _, had_errors) = encoding_rs::WINDOWS_1252.encode(s);
    if had_errors {
        return Err(TypforgeError::Encode(
            "Caractère non représentable en CP1252".into(),
        ));
    }
    Ok(bytes.into_owned())
}

fn crlf(s: &mut String, line: &str) {
    s.push_str(line);
    s.push_str("\r\n");
}

fn emit_id(s: &mut String, p: &TypParam) {
    crlf(s, "[_id]");
    crlf(s, &format!("ProductCode={}", p.product_id));
    crlf(s, &format!("FID={}", p.family_id));
    crlf(s, &format!("CodePage={}", p.codepage));
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_draw_order(s: &mut String, entries: &[DrawOrderEntry]) {
    crlf(s, "[_drawOrder]");
    for e in entries {
        if e.sub_type != 0 {
            crlf(s, &format!("Type=0x{:x}{:02x},{}", e.type_code, e.sub_type, e.level));
        } else {
            crlf(s, &format!("Type=0x{:x},{}", e.type_code, e.level));
        }
    }
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_polygon(s: &mut String, p: &TypPolygon) {
    crlf(s, "[_polygon]");
    emit_type_subtype(s, p.type_code, p.sub_type);
    if p.extended_labels {
        crlf(s, "ExtendedLabels=Y");
    }
    emit_font_style(s, p.font_style);
    if let Some(x) = &p.day_xpm {
        emit_xpm(s, x, "Xpm");
    }
    if let Some(x) = &p.night_xpm {
        emit_xpm(s, x, "NightXpm");
    }
    emit_font_colours(s, p.day_font_colour, p.night_font_colour);
    emit_labels(s, &p.labels);
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_line(s: &mut String, l: &TypLine) {
    crlf(s, "[_line]");
    emit_type_subtype(s, l.type_code, l.sub_type);
    if !l.use_orientation {
        crlf(s, "UseOrientation=N");
    }
    if l.line_width > 0 {
        crlf(s, &format!("LineWidth={}", l.line_width));
    }
    if l.border_width > 0 {
        crlf(s, &format!("BorderWidth={}", l.border_width));
    }
    if l.extended_labels {
        crlf(s, "ExtendedLabels=Y");
    }
    emit_font_style(s, l.font_style);
    if let Some(x) = &l.day_xpm {
        emit_xpm(s, x, "Xpm");
    }
    if let Some(x) = &l.night_xpm {
        emit_xpm(s, x, "NightXpm");
    }
    emit_font_colours(s, l.day_font_colour, l.night_font_colour);
    emit_labels(s, &l.labels);
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_point(s: &mut String, p: &TypPoint) {
    crlf(s, "[_point]");
    emit_type_subtype(s, p.type_code, p.sub_type);
    if p.extended_labels {
        crlf(s, "ExtendedLabels=Y");
    }
    emit_font_style(s, p.font_style);
    if let Some(x) = &p.day_xpm {
        emit_xpm(s, x, "DayXpm");
    }
    if let Some(x) = &p.night_xpm {
        emit_xpm(s, x, "NightXpm");
    }
    emit_font_colours(s, p.day_font_colour, p.night_font_colour);
    emit_labels(s, &p.labels);
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_iconset(s: &mut String, ic: &TypIconSet) {
    crlf(s, "[_icons]");
    emit_type_subtype(s, ic.type_code, ic.sub_type);
    for x in &ic.icons {
        emit_xpm(s, x, "Xpm");
    }
    crlf(s, "[end]");
    crlf(s, "");
}

fn emit_type_subtype(s: &mut String, type_code: u16, sub_type: u8) {
    crlf(s, &format!("Type=0x{:x}", type_code));
    if sub_type != 0 {
        crlf(s, &format!("SubType=0x{:02x}", sub_type));
    }
}

fn emit_font_style(s: &mut String, style: FontStyle) {
    match style {
        FontStyle::Default => {}
        FontStyle::NoLabel => crlf(s, "FontStyle=NoLabel"),
        FontStyle::Small => crlf(s, "FontStyle=Small"),
        FontStyle::Normal => crlf(s, "FontStyle=Normal"),
        FontStyle::Large => crlf(s, "FontStyle=Large"),
        FontStyle::Custom(n) => crlf(s, &format!("FontStyle={}", n)),
    }
}

fn emit_font_colours(s: &mut String, day: Option<Rgb>, night: Option<Rgb>) {
    if let Some(c) = day {
        crlf(s, &format!("DayFontColor=#{:02X}{:02X}{:02X}", c.r, c.g, c.b));
    }
    if let Some(c) = night {
        crlf(s, &format!("NightFontColor=#{:02X}{:02X}{:02X}", c.r, c.g, c.b));
    }
}

fn emit_labels(s: &mut String, labels: &[TypLabel]) {
    for (i, l) in labels.iter().enumerate() {
        crlf(s, &format!("String{}=0x{:02x},{}", i + 1, l.lang, l.text));
    }
}

fn emit_xpm(s: &mut String, xpm: &Xpm, key: &str) {
    // Déterminer cpp à partir de la palette (taille des tags)
    let cpp = xpm.palette.first().map(|(tag, _)| tag.chars().count()).unwrap_or(1);
    let cpp_actual = if xpm.pixels.is_empty() { 0 } else { cpp };
    let n = xpm.palette.len();

    crlf(s, &format!("{}=\"{} {} {} {}\"", key, xpm.width, xpm.height, n, cpp_actual));

    // Palette : utiliser les tags originaux pour préserver le round-trip
    for (tag, color) in &xpm.palette {
        if color.is_transparent() {
            crlf(s, &format!("\"{}  c none\"", tag));
        } else {
            crlf(s, &format!("\"{}  c #{:02X}{:02X}{:02X}\"", tag, color.r, color.g, color.b));
        }
    }

    // Pixels
    if cpp_actual > 0 {
        for row in &xpm.pixels {
            let mut line_buf = String::with_capacity(xpm.width as usize * cpp);
            for &idx in row {
                if let Some((tag, _)) = xpm.palette.get(idx) {
                    line_buf.push_str(tag);
                } else if let Some((tag, _)) = xpm.palette.first() {
                    line_buf.push_str(tag);
                }
            }
            crlf(s, &format!("\"{}\"", line_buf));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::text_reader::parse;

    #[test]
    fn round_trip_id() {
        let mut doc = TypDocument::default();
        doc.param = TypParam { family_id: 1100, product_id: 1, codepage: 1252, header_str: String::new() };
        let bytes = write(&doc).unwrap();
        let doc2 = parse(&bytes).unwrap();
        assert_eq!(doc2.param.family_id, 1100);
        assert_eq!(doc2.param.product_id, 1);
        assert_eq!(doc2.param.codepage, 1252);
    }

    #[test]
    fn round_trip_polygon() {
        let mut doc = TypDocument::default();
        doc.param = TypParam { family_id: 1, product_id: 1, codepage: 1252, header_str: String::new() };
        doc.polygons.push(TypPolygon {
            type_code: 0x01,
            sub_type: 0,
            font_style: FontStyle::NoLabel,
            extended_labels: true,
            ..TypPolygon::default()
        });
        let bytes = write(&doc).unwrap();
        let doc2 = parse(&bytes).unwrap();
        assert_eq!(doc2.polygons.len(), 1);
        assert_eq!(doc2.polygons[0].type_code, 0x01);
        assert_eq!(doc2.polygons[0].font_style, FontStyle::NoLabel);
        assert!(doc2.polygons[0].extended_labels);
    }
}
