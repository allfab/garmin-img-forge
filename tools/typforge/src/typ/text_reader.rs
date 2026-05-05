use super::model::*;
use crate::error::{Result, TypforgeError};

/// Parse un fichier TYP texte (bytes CP1252 ou UTF-8) et produit un [`TypDocument`].
pub fn parse(bytes: &[u8]) -> Result<TypDocument> {
    let text = decode_cp1252_or_utf8(bytes);
    parse_str(&text)
}

fn decode_cp1252_or_utf8(bytes: &[u8]) -> String {
    const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
    if bytes.starts_with(&BOM) {
        String::from_utf8_lossy(&bytes[3..]).into_owned()
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        s.to_owned()
    } else {
        let (s, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
        s.into_owned()
    }
}

fn parse_str(input: &str) -> Result<TypDocument> {
    let mut doc = TypDocument::default();
    let mut state = ParseState::None;
    let mut acc = ElementAcc::default();

    for (i, raw) in input.lines().enumerate() {
        let line_num = i + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with(';') {
            if let Some(val) = line.strip_prefix(";GRMN_TYPE:") {
                match state {
                    ParseState::Point | ParseState::Line | ParseState::Polygon => {
                        acc.grmn_type = val.trim().to_string();
                    }
                    _ => {}
                }
            }
            continue;
        }

        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                let name = line[1..end].trim().to_ascii_lowercase();
                flush(&mut doc, state, &mut acc);
                acc = ElementAcc::default();
                state = match name.as_str() {
                    "end" => ParseState::None,
                    "_id" => ParseState::Id,
                    "_draworder" => ParseState::DrawOrder,
                    "_point" => ParseState::Point,
                    "_line" => ParseState::Line,
                    "_polygon" => ParseState::Polygon,
                    "_icons" => ParseState::Icons,
                    "_comments" => ParseState::Comments,
                    _ => ParseState::Ignore,
                };
                continue;
            }
        }

        if state == ParseState::Comments {
            if !doc.comments.is_empty() {
                doc.comments.push('\n');
            }
            doc.comments.push_str(line);
            continue;
        }

        // Lignes XPM entre guillemets
        if line.starts_with('"') {
            if let Some(inner) = parse_quoted(line) {
                push_xpm_line(&mut acc, inner, line_num)?;
            }
            continue;
        }

        let sep_pos = line.find(|c: char| c == '=' || c == ':');
        let sep_pos = match sep_pos {
            Some(p) => p,
            None => continue, // ligne malformée : on tolère
        };
        let key = line[..sep_pos].trim();
        let value = line[sep_pos + 1..].trim();

        match state {
            ParseState::None | ParseState::Ignore => {}
            ParseState::Id => parse_id_field(&mut doc.param, key, value, line_num)?,
            ParseState::DrawOrder => parse_draworder_field(&mut doc.draw_order, key, value, line_num)?,
            ParseState::Point | ParseState::Line | ParseState::Polygon | ParseState::Icons => {
                parse_element_field(&mut acc, key, value, line_num)?;
            }
            ParseState::Comments => {}
        }
    }
    flush(&mut doc, state, &mut acc);
    Ok(doc)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    None,
    Id,
    DrawOrder,
    Point,
    Line,
    Polygon,
    Icons,
    Comments,
    Ignore,
}

#[derive(Debug, Default)]
struct ElementAcc {
    type_code: u16,
    sub_type: u8,
    grmn_type: String,
    labels: Vec<TypLabel>,
    font_style: FontStyle,
    day_font_colour: Option<Rgb>,
    night_font_colour: Option<Rgb>,
    line_width: u8,
    border_width: u8,
    use_orientation: Option<bool>,
    extended_labels: bool,
    contour_color: ContourColor,
    pending: Option<PendingXpm>,
    xpms: Vec<Xpm>,
}

#[derive(Debug)]
struct PendingXpm {
    width: u16,
    height: u16,
    colors_expected: usize,
    chars_per_pixel: usize,
    palette: Vec<(String, Rgba)>,
    pixel_rows: Vec<String>,
}

fn parse_quoted(line: &str) -> Option<&str> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.rfind('"')?;
    Some(&rest[..end])
}

fn parse_int<T: ParseHex>(s: &str) -> Option<T> {
    T::parse(s.trim())
}

trait ParseHex: Sized {
    fn parse(s: &str) -> Option<Self>;
}

macro_rules! impl_parse_hex {
    ($t:ty) => {
        impl ParseHex for $t {
            fn parse(s: &str) -> Option<Self> {
                if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    <$t>::from_str_radix(rest, 16).ok()
                } else {
                    s.parse().ok()
                }
            }
        }
    };
}
impl_parse_hex!(u8);
impl_parse_hex!(u16);
impl_parse_hex!(u32);

fn parse_id_field(p: &mut TypParam, key: &str, value: &str, line: usize) -> Result<()> {
    match key.to_ascii_lowercase().as_str() {
        "fid" => {
            p.family_id = parse_int::<u16>(value).ok_or_else(|| TypforgeError::Parse {
                line,
                context: format!("FID invalide: {}", value),
            })?;
        }
        "productcode" => {
            p.product_id = parse_int::<u16>(value).ok_or_else(|| TypforgeError::Parse {
                line,
                context: format!("ProductCode invalide: {}", value),
            })?;
        }
        "codepage" => {
            p.codepage = parse_int::<u16>(value).ok_or_else(|| TypforgeError::Parse {
                line,
                context: format!("CodePage invalide: {}", value),
            })?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_draworder_field(out: &mut Vec<DrawOrderEntry>, key: &str, value: &str, line: usize) -> Result<()> {
    if !key.eq_ignore_ascii_case("type") {
        return Ok(());
    }
    let (t, lvl) = value.split_once(',').ok_or_else(|| TypforgeError::Parse {
        line,
        context: format!("DrawOrder attend 'type,level': {}", value),
    })?;
    let type_packed = parse_int::<u32>(t).ok_or_else(|| TypforgeError::Parse {
        line,
        context: format!("DrawOrder type invalide: {}", t),
    })?;
    let level = parse_int::<u8>(lvl).ok_or_else(|| TypforgeError::Parse {
        line,
        context: format!("DrawOrder level invalide: {}", lvl),
    })?;
    // type_packed peut être empaqueté (type<<5)|subtype ou simple type_code
    let (type_code, sub_type) = if type_packed >= 0x10000 {
        ((type_packed >> 8) as u16, (type_packed & 0xff) as u8)
    } else {
        (type_packed as u16, 0u8)
    };
    out.push(DrawOrderEntry { level, type_code, sub_type });
    Ok(())
}

fn parse_element_field(acc: &mut ElementAcc, key: &str, value: &str, line: usize) -> Result<()> {
    match key.to_ascii_lowercase().as_str() {
        "type" => {
            let (t, s) = match value.split_once(',') {
                Some((t, s)) => (t, Some(s)),
                None => (value, None),
            };
            let full = parse_int::<u32>(t).ok_or_else(|| TypforgeError::Parse {
                line,
                context: format!("Type invalide: {}", t),
            })?;
            if full >= 0x10000 && s.is_none() {
                acc.type_code = (full >> 8) as u16;
                acc.sub_type = (full & 0xff) as u8;
            } else {
                acc.type_code = full as u16;
                if let Some(s) = s {
                    acc.sub_type = parse_int::<u8>(s).ok_or_else(|| TypforgeError::Parse {
                        line,
                        context: format!("SubType invalide: {}", s),
                    })?;
                }
            }
        }
        "subtype" => {
            acc.sub_type = parse_int::<u8>(value).ok_or_else(|| TypforgeError::Parse {
                line,
                context: format!("SubType invalide: {}", value),
            })?;
        }
        "xpm" | "dayxpm" | "nightxpm" => {
            finish_pending_xpm(acc);
            if let Some(inner) = parse_quoted_value(value) {
                match parse_xpm_header(inner, line) {
                    Ok(p) => acc.pending = Some(p),
                    Err(e) => return Err(e),
                }
            }
        }
        "linewidth" => acc.line_width = parse_int::<u8>(value).unwrap_or(0),
        "borderwidth" => acc.border_width = parse_int::<u8>(value).unwrap_or(0),
        "fontstyle" => acc.font_style = parse_font_style(value),
        "dayfontcolor" | "daycustomcolor" => {
            acc.day_font_colour = parse_color_rgb(value);
        }
        "nightfontcolor" | "nightcustomcolor" => {
            acc.night_font_colour = parse_color_rgb(value);
        }
        "useorientation" => {
            let v = value.trim();
            acc.use_orientation = Some(
                v.eq_ignore_ascii_case("y") || v == "1" || v.eq_ignore_ascii_case("true"),
            );
        }
        "extendedlabels" => {
            let v = value.trim();
            acc.extended_labels =
                v.eq_ignore_ascii_case("y") || v == "1" || v.eq_ignore_ascii_case("true");
        }
        "contourcolor" => {
            let v = value.trim();
            acc.contour_color = if v.eq_ignore_ascii_case("no") || v.eq_ignore_ascii_case("none") {
                ContourColor::No
            } else if let Some(c) = parse_color_rgb(v) {
                ContourColor::Solid(c)
            } else {
                ContourColor::No
            };
        }
        k if k.starts_with("string") => {
            acc.labels.push(parse_label(value, line)?);
        }
        _ => {}
    }
    Ok(())
}

fn parse_quoted_value(v: &str) -> Option<&str> {
    let start = v.find('"')?;
    let rest = &v[start + 1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn parse_xpm_header(s: &str, line: usize) -> Result<PendingXpm> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 4 {
        return Err(TypforgeError::Parse {
            line,
            context: format!("Header XPM attend 4 valeurs: {:?}", parts),
        });
    }
    let width = parts[0].parse::<u16>().map_err(|_| TypforgeError::Parse {
        line,
        context: format!("XPM W invalide: {}", parts[0]),
    })?;
    let height = parts[1].parse::<u16>().map_err(|_| TypforgeError::Parse {
        line,
        context: format!("XPM H invalide: {}", parts[1]),
    })?;
    let colors = parts[2].parse::<usize>().map_err(|_| TypforgeError::Parse {
        line,
        context: format!("XPM C invalide: {}", parts[2]),
    })?;
    let cpp = parts[3].parse::<usize>().map_err(|_| TypforgeError::Parse {
        line,
        context: format!("XPM P invalide: {}", parts[3]),
    })?;
    Ok(PendingXpm {
        width,
        height,
        colors_expected: colors,
        chars_per_pixel: cpp,
        palette: Vec::with_capacity(colors),
        pixel_rows: Vec::with_capacity(height as usize),
    })
}

fn push_xpm_line(acc: &mut ElementAcc, inner: &str, line: usize) -> Result<()> {
    let pending = match acc.pending.as_mut() {
        Some(p) => p,
        None => return Ok(()), // XPM inattendu : tolérance
    };
    if pending.palette.len() < pending.colors_expected {
        let cpp = pending.chars_per_pixel;
        let key_chars: String = if cpp == 0 {
            String::new()
        } else {
            inner.chars().take(cpp).collect()
        };
        let after_key = match inner.char_indices().nth(cpp) {
            Some((i, _)) => &inner[i..],
            None => "",
        };
        // Cherche " c " (séparateur XPM standard)
        let color_str = extract_xpm_color_value(after_key).ok_or_else(|| TypforgeError::Parse {
            line,
            context: format!("Palette XPM invalide: '{}'", inner),
        })?;
        let color = parse_rgba_from_str(color_str).ok_or_else(|| TypforgeError::Parse {
            line,
            context: format!("Couleur XPM invalide: '{}'", color_str),
        })?;
        pending.palette.push((key_chars, color));
    } else {
        pending.pixel_rows.push(inner.to_string());
    }
    Ok(())
}

fn extract_xpm_color_value(s: &str) -> Option<&str> {
    // Cherche le marqueur `c` entouré de whitespace
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'c'
            && i > 0
            && (bytes[i - 1] as char).is_whitespace()
            && i + 1 < bytes.len()
            && (bytes[i + 1] as char).is_whitespace()
        {
            return Some(s[i + 2..].trim());
        }
        i += 1;
    }
    // Fallback: tout ce qui suit le tag est la couleur (format compact)
    Some(s.trim())
}

fn parse_rgba_from_str(v: &str) -> Option<Rgba> {
    let v = v.trim();
    if v.eq_ignore_ascii_case("none") {
        return Some(Rgba::transparent());
    }
    if let Some(hex) = v.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Rgba::opaque(r, g, b));
        }
        if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(Rgba { r, g, b, a });
        }
    }
    None
}

fn parse_color_rgb(value: &str) -> Option<Rgb> {
    let rgba = parse_rgba_from_str(value)?;
    Some(Rgb { r: rgba.r, g: rgba.g, b: rgba.b })
}

fn parse_font_style(value: &str) -> FontStyle {
    let head = value.trim()
        .split(|c: char| c == '(' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match head.as_str() {
        "" | "default" => FontStyle::Default,
        "nolabel" => FontStyle::NoLabel,
        "small" | "smallfont" => FontStyle::Small,
        "normal" | "normalfont" => FontStyle::Normal,
        "large" | "largefont" => FontStyle::Large,
        other => {
            if let Ok(n) = other.parse::<u8>() {
                FontStyle::Custom(n)
            } else {
                FontStyle::Default
            }
        }
    }
}

fn parse_label(value: &str, line: usize) -> Result<TypLabel> {
    let (code, text) = value.split_once(',').ok_or_else(|| TypforgeError::Parse {
        line,
        context: format!("Label attend 'code,texte': {}", value),
    })?;
    let lang = parse_int::<u8>(code).ok_or_else(|| TypforgeError::Parse {
        line,
        context: format!("Code langue invalide: {}", code),
    })?;
    Ok(TypLabel { lang, text: text.to_string() })
}

fn finish_pending_xpm(acc: &mut ElementAcc) {
    if let Some(p) = acc.pending.take() {
        acc.xpms.push(finalize_xpm(p));
    }
}

fn finalize_xpm(p: PendingXpm) -> Xpm {
    let colour_mode = if p.chars_per_pixel == 0 || p.palette.len() <= 256 {
        ColorMode::Indexed
    } else {
        ColorMode::True32
    };

    // Construire les pixels 2D comme indices dans la palette
    let cpp = p.chars_per_pixel;
    let mut pixels: Vec<Vec<usize>> = Vec::with_capacity(p.height as usize);
    for row in &p.pixel_rows {
        let chars: Vec<char> = row.chars().collect();
        let mut pixel_row = Vec::with_capacity(p.width as usize);
        let mut j = 0;
        while j + cpp <= chars.len() && pixel_row.len() < p.width as usize {
            let key: String = chars[j..j + cpp].iter().collect();
            let idx = p.palette.iter().position(|(k, _)| *k == key).unwrap_or(0);
            pixel_row.push(idx);
            j += cpp;
        }
        // Padding si la ligne est trop courte
        while pixel_row.len() < p.width as usize {
            pixel_row.push(0);
        }
        pixels.push(pixel_row);
    }
    // Padding en hauteur si nécessaire
    while pixels.len() < p.height as usize {
        pixels.push(vec![0usize; p.width as usize]);
    }

    Xpm {
        width: p.width,
        height: p.height,
        colour_mode,
        palette: p.palette,
        pixels,
    }
}

fn split_day_night(xpms: Vec<Xpm>) -> (Option<Xpm>, Option<Xpm>) {
    let mut iter = xpms.into_iter();
    let day = iter.next();
    let night = iter.next();
    (day, night)
}

fn flush(doc: &mut TypDocument, state: ParseState, acc: &mut ElementAcc) {
    finish_pending_xpm(acc);
    match state {
        ParseState::Point => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(std::mem::take(&mut acc.xpms));
                doc.points.push(TypPoint {
                    type_code: acc.type_code,
                    sub_type: acc.sub_type,
                    grmn_type: std::mem::take(&mut acc.grmn_type),
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    font_style: acc.font_style,
                    day_font_colour: acc.day_font_colour,
                    night_font_colour: acc.night_font_colour,
                    extended_labels: acc.extended_labels,
                });
            }
        }
        ParseState::Line => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(std::mem::take(&mut acc.xpms));
                doc.lines.push(TypLine {
                    type_code: acc.type_code,
                    sub_type: acc.sub_type,
                    grmn_type: std::mem::take(&mut acc.grmn_type),
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    line_width: acc.line_width,
                    border_width: acc.border_width,
                    font_style: acc.font_style,
                    day_font_colour: acc.day_font_colour,
                    night_font_colour: acc.night_font_colour,
                    extended_labels: acc.extended_labels,
                    use_orientation: acc.use_orientation.unwrap_or(true),
                });
            }
        }
        ParseState::Polygon => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(std::mem::take(&mut acc.xpms));
                doc.polygons.push(TypPolygon {
                    type_code: acc.type_code,
                    sub_type: acc.sub_type,
                    grmn_type: std::mem::take(&mut acc.grmn_type),
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    font_style: acc.font_style,
                    day_font_colour: acc.day_font_colour,
                    night_font_colour: acc.night_font_colour,
                    extended_labels: acc.extended_labels,
                    contour_color: acc.contour_color,
                });
            }
        }
        ParseState::Icons => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                doc.icons.push(TypIconSet {
                    type_code: acc.type_code,
                    sub_type: acc.sub_type,
                    icons: std::mem::take(&mut acc.xpms),
                });
            }
        }
        _ => {}
    }
}

/// Parse un bloc XPM depuis du texte brut (sans clé `DayXpm=`).
///
/// Format attendu (lignes, avec ou sans guillemets) :
/// ```text
/// W H N CPP
/// TAG  c #RRGGBB
/// ROWDATA
/// ```
/// Retourne `None` si le texte est vide.
pub fn parse_xpm_lines(text: &str) -> crate::error::Result<Option<Xpm>> {
    let lines: Vec<&str> = text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Ok(None);
    }
    let header_raw = if lines[0].starts_with('"') {
        parse_quoted(lines[0]).unwrap_or(lines[0])
    } else {
        lines[0]
    };
    let mut pending = parse_xpm_header(header_raw, 1)?;
    for (i, &line) in lines[1..].iter().enumerate() {
        let inner = if line.starts_with('"') {
            parse_quoted(line).unwrap_or(line)
        } else {
            line
        };
        if pending.palette.len() < pending.colors_expected {
            let cpp = pending.chars_per_pixel;
            let key_chars: String = if cpp == 0 {
                String::new()
            } else {
                inner.chars().take(cpp).collect()
            };
            let after_key = match inner.char_indices().nth(cpp) {
            Some((i, _)) => &inner[i..],
            None => "",
        };
            let color_str = extract_xpm_color_value(after_key)
                .ok_or_else(|| TypforgeError::Parse {
                    line: i + 2,
                    context: format!("Palette XPM invalide: '{}'", inner),
                })?;
            let color = parse_rgba_from_str(color_str)
                .ok_or_else(|| TypforgeError::Parse {
                    line: i + 2,
                    context: format!("Couleur XPM invalide: '{}'", color_str),
                })?;
            pending.palette.push((key_chars, color));
        } else {
            pending.pixel_rows.push(inner.to_string());
        }
    }
    Ok(Some(finalize_xpm(pending)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_section() {
        let input = b"[_id]\nFID=1100\nProductCode=1\nCodePage=1252\n[end]\n";
        let doc = parse(input).unwrap();
        assert_eq!(doc.param.family_id, 1100);
        assert_eq!(doc.param.product_id, 1);
        assert_eq!(doc.param.codepage, 1252);
    }

    #[test]
    fn polygon_basic() {
        let input = b"[_polygon]\nType=0x01\nFontStyle=NoLabel\n[end]\n";
        let doc = parse(input).unwrap();
        assert_eq!(doc.polygons.len(), 1);
        assert_eq!(doc.polygons[0].type_code, 0x01);
        assert_eq!(doc.polygons[0].font_style, FontStyle::NoLabel);
    }

    #[test]
    fn point_with_xpm() {
        let input = b"[_point]\nType=0x2A\nSubType=3\nXpm=\"4 2 2 1\"\n\"! c #FF0000\"\n\". c none\"\n\"!!..\"\n\"!..!\"\nString1=0x04,Hello\n[end]\n";
        let doc = parse(input).unwrap();
        assert_eq!(doc.points.len(), 1);
        let p = &doc.points[0];
        assert_eq!(p.type_code, 0x2A);
        assert_eq!(p.sub_type, 3);
        let xpm = p.day_xpm.as_ref().unwrap();
        assert_eq!(xpm.width, 4);
        assert_eq!(xpm.height, 2);
        assert_eq!(xpm.palette.len(), 2);
        assert_eq!(p.labels[0].lang, 0x04);
        assert_eq!(p.labels[0].text, "Hello");
    }
}
