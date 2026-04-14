//! Parser texte TYP.
//!
//! Port de `mkgmap/.../mkgmap/typ/TypTextReader.java` + classes `*Section`.
//! État-machine linéaire sur les lignes du fichier ; accumulation des XPM
//! multi-ligne via attribut pendant.
//!
//! Syntaxe :
//! - Sections : `[_id]`, `[_drawOrder]`, `[_point]`, `[_line]`, `[_polygon]`,
//!   `[_icons]`, `[_comments]` (ignoré), terminées par `[end]` (ou nouveau
//!   `[...]` ou EOF).
//! - Key/value : `key=value` **ou** `key:value`.
//! - Commentaires : lignes commençant par `;`.
//! - XPM : attribut `Xpm="W H C P"` suivi de `C` lignes palette puis `H`
//!   lignes pixels, toutes entourées de `"…"`.

use super::data::*;
use crate::error::TypError;

/// Parse un fichier TYP texte et produit une [`TypData`].
pub fn read_typ_text(input: &str) -> Result<TypData, TypError> {
    let mut data = TypData::new();
    let mut state = State::None;
    let mut acc = ElementAcc::default();

    for (i, raw) in input.lines().enumerate() {
        let line_num = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        // Début de section.
        if line.starts_with('[') {
            if let Some(end) = line.find(']') {
                let name = line[1..end].trim().to_lowercase();
                flush(&mut data, state, &mut acc);
                acc = ElementAcc::default();
                state = match name.as_str() {
                    "end" => State::None,
                    "_id" => State::Id,
                    "_draworder" => State::DrawOrder,
                    "_point" => State::Point,
                    "_line" => State::Line,
                    "_polygon" => State::Polygon,
                    "_icons" => State::Icons,
                    "_comments" => State::Ignore,
                    other => {
                        tracing::warn!("ligne {} : section inconnue '{}'", line_num, other);
                        State::Ignore
                    }
                };
                continue;
            }
        }

        // Lignes XPM multi-ligne (commencent par `"` en dehors d'une valeur
        // key=value). Appartiennent à l'XPM en cours d'accumulation.
        if line.starts_with('"') {
            let inner = parse_quoted(line).ok_or_else(|| TypError::InvalidValue {
                line: line_num,
                context: format!("ligne XPM mal formée: {}", line),
            })?;
            push_xpm_line(&mut acc, inner, line_num)?;
            continue;
        }

        // Key/value : séparateur `=` **ou** `:` (premier trouvé gagne).
        let sep_pos = line
            .find(|c: char| c == '=' || c == ':')
            .ok_or_else(|| TypError::InvalidValue {
                line: line_num,
                context: format!("ni '=' ni ':' : {}", line),
            })?;
        let key = line[..sep_pos].trim();
        let value = line[sep_pos + 1..].trim();

        match state {
            State::None => {
                return Err(TypError::InvalidValue {
                    line: line_num,
                    context: "valeur hors section".into(),
                });
            }
            State::Ignore => {}
            State::Id => parse_id_field(&mut data.params, key, value, line_num)?,
            State::DrawOrder => parse_draworder(&mut data.draw_order, key, value, line_num)?,
            State::Point | State::Line | State::Polygon | State::Icons => {
                parse_element_field(&mut acc, state, key, value, line_num)?;
            }
        }
    }
    flush(&mut data, state, &mut acc);
    Ok(data)
}

// ============================================================ état interne

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    None,
    Id,
    DrawOrder,
    Point,
    Line,
    Polygon,
    Icons,
    Ignore,
}

/// Accumulateur générique pour les éléments point/line/polygon/icons.
///
/// Les champs non pertinents pour le type courant sont simplement ignorés
/// à la fermeture de section.
#[derive(Debug, Default)]
struct ElementAcc {
    type_code: u32,
    subtype: u8,
    labels: Vec<TypLabel>,
    font_style: FontStyle,
    day_font_color: Option<Rgba>,
    night_font_color: Option<Rgba>,
    line_width: u8,
    border_width: u8,
    /// XPM en cours d'accumulation : après le header `Xpm="..."`, on lit
    /// `colors_expected` palette puis `pixel_rows_expected` pixels.
    pending: Option<PendingXpm>,
    /// XPM complets déjà accumulés (ordre : day, night ; multiple pour icons).
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

// ============================================================ helpers parse

fn parse_quoted(line: &str) -> Option<&str> {
    let start = line.find('"')?;
    let end = line[start + 1..].rfind('"')?;
    Some(&line[start + 1..start + 1 + end])
}

/// Parse un entier int avec support `0x` / décimal.
fn parse_int<T: num_parse::ParseIntMaybeHex>(s: &str) -> Option<T> {
    T::parse_maybe_hex(s.trim())
}

mod num_parse {
    pub trait ParseIntMaybeHex: Sized {
        fn parse_maybe_hex(s: &str) -> Option<Self>;
    }
    macro_rules! imp {
        ($t:ty) => {
            impl ParseIntMaybeHex for $t {
                fn parse_maybe_hex(s: &str) -> Option<Self> {
                    let s = s.trim();
                    if let Some(rest) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                        <$t>::from_str_radix(rest, 16).ok()
                    } else {
                        s.parse().ok()
                    }
                }
            }
        };
    }
    imp!(u8);
    imp!(u16);
    imp!(u32);
    imp!(i32);
}

fn parse_id_field(
    p: &mut TypParams,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), TypError> {
    match key.to_ascii_lowercase().as_str() {
        "fid" => {
            p.family_id = parse_int::<u16>(value).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("FID: {}", value),
            })?;
        }
        "productcode" => {
            p.product_id = parse_int::<u16>(value).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("ProductCode: {}", value),
            })?;
        }
        "codepage" => {
            p.codepage = parse_int::<u16>(value).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("CodePage: {}", value),
            })?;
        }
        _ => {
            tracing::debug!("ligne {} : clé ID ignorée '{}'", line, key);
        }
    }
    Ok(())
}

fn parse_draworder(
    out: &mut Vec<DrawOrderEntry>,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), TypError> {
    if !key.eq_ignore_ascii_case("type") {
        return Ok(());
    }
    // value : `0xHHHH,level` (level 1-8)
    let (t, lvl) = value.split_once(',').ok_or_else(|| TypError::InvalidValue {
        line,
        context: format!("drawOrder: {}", value),
    })?;
    let type_code = parse_int::<u32>(t).ok_or_else(|| TypError::InvalidValue {
        line,
        context: format!("drawOrder type: {}", t),
    })?;
    let level = parse_int::<u8>(lvl).ok_or_else(|| TypError::InvalidValue {
        line,
        context: format!("drawOrder level: {}", lvl),
    })?;
    out.push(DrawOrderEntry { type_code, level });
    Ok(())
}

fn parse_element_field(
    acc: &mut ElementAcc,
    _state: State,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), TypError> {
    let key_l = key.to_ascii_lowercase();
    match key_l.as_str() {
        "type" => {
            // "0xHHH" ou "0xHHH,sub" — certains fichiers utilisent `SubType=` séparé.
            let (t, s) = match value.split_once(',') {
                Some((t, s)) => (t, Some(s)),
                None => (value, None),
            };
            acc.type_code = parse_int::<u32>(t).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("Type: {}", t),
            })?;
            if let Some(s) = s {
                acc.subtype = parse_int::<u8>(s).ok_or_else(|| TypError::InvalidValue {
                    line,
                    context: format!("Subtype: {}", s),
                })?;
            }
        }
        "subtype" => {
            acc.subtype = parse_int::<u8>(value).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("SubType: {}", value),
            })?;
        }
        "xpm" | "dayxpm" | "nightxpm" => {
            finish_pending_xpm(acc, line)?;
            let inner = parse_quoted_value(value).ok_or_else(|| TypError::InvalidValue {
                line,
                context: format!("Xpm entête attendue entre guillemets : {}", value),
            })?;
            acc.pending = Some(parse_xpm_header(inner, line)?);
        }
        "colormode" | "daycolormode" | "nightcolormode" | "extendedlabels"
        | "customcolor" | "contourcolor" | "useorientation" => {
            // attributs décoratifs : ignorés pour Lot B, semés en Lot C.
        }
        "linewidth" => {
            acc.line_width = parse_int::<u8>(value).unwrap_or(0);
        }
        "borderwidth" => {
            acc.border_width = parse_int::<u8>(value).unwrap_or(0);
        }
        "fontstyle" => {
            acc.font_style = parse_font_style(value);
        }
        "daycustomcolor" | "dayfontcolor" => {
            acc.day_font_color = parse_color(value);
        }
        "nightcustomcolor" | "nightfontcolor" => {
            acc.night_font_color = parse_color(value);
        }
        k if k.starts_with("string") => {
            acc.labels.push(parse_label(value, line)?);
        }
        _ => {
            // Clé inconnue : on accepte et trace, pour robustesse sur fichier
            // réel (beaucoup de clés décoratives : CustomColor, ContourColor,
            // UseOrientation, ExtendedLabels, …).
            tracing::trace!("ligne {} : clé élément ignorée '{}'", line, key);
        }
    }
    Ok(())
}

/// Extrait le premier segment entre `"…"` dans `v`. Le reste de la ligne
/// (attributs décoratifs éventuels, ex. `ColorMode=16`) est ignoré.
fn parse_quoted_value(v: &str) -> Option<&str> {
    let v = v.trim_start();
    let start = v.find('"')?;
    let rest = &v[start + 1..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Parse l'entête XPM : `W H C P` en décimal.
fn parse_xpm_header(s: &str, line: usize) -> Result<PendingXpm, TypError> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 4 {
        return Err(TypError::InvalidValue {
            line,
            context: format!("Xpm header attend 4 valeurs, trouvé {}: {:?}", parts.len(), parts),
        });
    }
    let width = parts[0].parse::<u16>().map_err(|_| TypError::InvalidValue {
        line,
        context: format!("Xpm W: {}", parts[0]),
    })?;
    let height = parts[1].parse::<u16>().map_err(|_| TypError::InvalidValue {
        line,
        context: format!("Xpm H: {}", parts[1]),
    })?;
    let colors = parts[2].parse::<usize>().map_err(|_| TypError::InvalidValue {
        line,
        context: format!("Xpm C: {}", parts[2]),
    })?;
    let cpp = parts[3].parse::<usize>().map_err(|_| TypError::InvalidValue {
        line,
        context: format!("Xpm P: {}", parts[3]),
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

/// Ajoute une ligne `"…"` à l'XPM en cours : palette puis pixels.
fn push_xpm_line(acc: &mut ElementAcc, inner: &str, line: usize) -> Result<(), TypError> {
    let pending = acc.pending.as_mut().ok_or_else(|| TypError::InvalidValue {
        line,
        context: "ligne entre guillemets hors XPM".into(),
    })?;
    if pending.palette.len() < pending.colors_expected {
        // Palette : `<chars> c <color>` où <chars> fait `cpp` caractères
        // (peut inclure des espaces) et <color> = `#RRGGBB` / `#RRGGBBAA` /
        // `none`. Le séparateur ` c ` (c entouré d'espaces) marque la fin
        // du code couleur et le début de la valeur.
        let cpp = pending.chars_per_pixel;
        let key_chars = if cpp == 0 {
            String::new()
        } else {
            inner.chars().take(cpp).collect::<String>()
        };
        // Cherche le marqueur `c` entouré de whitespace après `cpp` chars.
        // TYPViewer utilise souvent TAB comme séparateur.
        let after_key = &inner[cpp..];
        let bytes = after_key.as_bytes();
        let mut sep_end = None;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'c'
                && i > 0
                && (bytes[i - 1] as char).is_whitespace()
                && i + 1 < bytes.len()
                && (bytes[i + 1] as char).is_whitespace()
            {
                sep_end = Some(i + 2);
                break;
            }
            i += 1;
        }
        let sep_end = sep_end.ok_or_else(|| TypError::InvalidValue {
            line,
            context: format!("Xpm palette attend 'c' séparé: '{}'", inner),
        })?;
        let value = after_key[sep_end..].trim();
        let color = parse_xpm_color(value).ok_or_else(|| TypError::InvalidValue {
            line,
            context: format!("Xpm palette couleur invalide: '{}'", value),
        })?;
        pending.palette.push((key_chars, color));
        return Ok(());
    }
    // Ligne de pixels.
    pending.pixel_rows.push(inner.to_string());
    Ok(())
}

fn parse_xpm_color(value: &str) -> Option<Rgba> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("none") {
        return Some(Rgba { r: 0, g: 0, b: 0, a: 0xff }); // alpha inverse max
    }
    if let Some(hex) = v.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Rgba { r, g, b, a: 0 });
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

fn parse_color(value: &str) -> Option<Rgba> {
    parse_xpm_color(value)
}

fn parse_font_style(value: &str) -> FontStyle {
    let v = value.trim();
    // Exemples rencontrés : "Default", "NoLabel (invisible)", "Small",
    // "Normal", "Large".
    let head = v.split(|c: char| c == '(' || c.is_whitespace()).next().unwrap_or("");
    match head.to_ascii_lowercase().as_str() {
        "" | "default" => FontStyle::Default,
        "nolabel" => FontStyle::NoLabel,
        "small" => FontStyle::Small,
        "normal" => FontStyle::Normal,
        "large" => FontStyle::Large,
        _ => parse_int::<u8>(head).map(FontStyle::Custom).unwrap_or(FontStyle::Default),
    }
}

/// Parse un label : `code,texte` où code est int (langue).
fn parse_label(value: &str, line: usize) -> Result<TypLabel, TypError> {
    let (code, text) = value.split_once(',').ok_or_else(|| TypError::InvalidValue {
        line,
        context: format!("Label attend 'code,texte': {}", value),
    })?;
    let lang = parse_int::<u8>(code).ok_or_else(|| TypError::InvalidValue {
        line,
        context: format!("Label code langue: {}", code),
    })?;
    Ok(TypLabel { lang, text: text.to_string() })
}

// ============================================================ finalisation

fn finish_pending_xpm(acc: &mut ElementAcc, _line: usize) -> Result<(), TypError> {
    if let Some(p) = acc.pending.take() {
        let xpm = finalize_xpm(p);
        acc.xpms.push(xpm);
    }
    Ok(())
}

fn finalize_xpm(p: PendingXpm) -> Xpm {
    let mode = infer_color_mode(&p);
    let colors: Vec<Rgba> = p.palette.iter().map(|(_, c)| *c).collect();
    // Indexation des pixels : mapping char_key → index palette.
    let pixels = if p.chars_per_pixel == 0 || p.pixel_rows.is_empty() {
        Vec::new()
    } else {
        let cpp = p.chars_per_pixel;
        let mut out = Vec::with_capacity((p.width as usize) * (p.height as usize));
        for row in &p.pixel_rows {
            let chars: Vec<char> = row.chars().collect();
            let mut j = 0;
            while j + cpp <= chars.len() && out.len() < (p.width as usize) * (p.height as usize) {
                let key: String = chars[j..j + cpp].iter().collect();
                let idx = p
                    .palette
                    .iter()
                    .position(|(k, _)| *k == key)
                    .unwrap_or(0) as u8;
                out.push(idx);
                j += cpp;
            }
        }
        out
    };
    Xpm {
        width: p.width,
        height: p.height,
        colors,
        pixels,
        mode,
    }
}

fn infer_color_mode(p: &PendingXpm) -> ColorMode {
    // Heuristique simple : palette <= 256 & cpp > 0 → Indexed ; sinon True32.
    // Les modes True16/True32 sont distingués plus tard par le writer binaire
    // selon la présence d'alpha ; conservation pour sémantique.
    if p.chars_per_pixel == 0 {
        // "fill pattern" : palette-only. Indexed (bg/fg).
        ColorMode::Indexed
    } else if p.palette.len() <= 256 {
        ColorMode::Indexed
    } else {
        ColorMode::True32
    }
}

fn flush(data: &mut TypData, state: State, acc: &mut ElementAcc) {
    // Vider l'XPM pending avant de pousser l'élément.
    if let Some(p) = acc.pending.take() {
        acc.xpms.push(finalize_xpm(p));
    }
    match state {
        State::Point => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(&acc.xpms);
                data.points.push(TypPoint {
                    type_code: acc.type_code,
                    subtype: acc.subtype,
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    font_style: acc.font_style,
                    day_font_color: acc.day_font_color,
                    night_font_color: acc.night_font_color,
                });
            }
        }
        State::Line => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(&acc.xpms);
                data.lines.push(TypLine {
                    type_code: acc.type_code,
                    subtype: acc.subtype,
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    line_width: acc.line_width,
                    border_width: acc.border_width,
                    font_style: acc.font_style,
                    day_font_color: acc.day_font_color,
                    night_font_color: acc.night_font_color,
                });
            }
        }
        State::Polygon => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                let (day, night) = split_day_night(&acc.xpms);
                data.polygons.push(TypPolygon {
                    type_code: acc.type_code,
                    subtype: acc.subtype,
                    labels: std::mem::take(&mut acc.labels),
                    day_xpm: day,
                    night_xpm: night,
                    font_style: acc.font_style,
                    day_font_color: acc.day_font_color,
                    night_font_color: acc.night_font_color,
                });
            }
        }
        State::Icons => {
            if acc.type_code != 0 || !acc.xpms.is_empty() {
                data.icons.push(TypIconSet {
                    type_code: acc.type_code,
                    subtype: acc.subtype,
                    icons: std::mem::take(&mut acc.xpms),
                });
            }
        }
        _ => {}
    }
}

/// Sépare la séquence XPM en jour / nuit (convention mkgmap : 1ère = jour,
/// 2ème = nuit). Pour icons, utiliser acc.xpms directement.
fn split_day_night(xpms: &[Xpm]) -> (Option<Xpm>, Option<Xpm>) {
    match xpms.len() {
        0 => (None, None),
        1 => (Some(xpms[0].clone()), None),
        _ => (Some(xpms[0].clone()), Some(xpms[1].clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_section() {
        let input = "[_id]\nProductCode=1\nFID=1100\nCodePage=1252\n[end]\n";
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.params.family_id, 1100);
        assert_eq!(d.params.product_id, 1);
        assert_eq!(d.params.codepage, 1252);
    }

    #[test]
    fn id_separator_colon() {
        let input = "[_id]\nProductCode:2\nFID:0x44C\nCodePage:65001\n[end]\n";
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.params.family_id, 0x44C);
        assert_eq!(d.params.product_id, 2);
        assert_eq!(d.params.codepage, 65001);
    }

    #[test]
    fn comments_and_blank_lines() {
        let input = "; header comment\n\n[_id]\n; inline\nFID=1\nProductCode=1\nCodePage=1252\n[end]\n";
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.params.family_id, 1);
    }

    #[test]
    fn draw_order() {
        let input = "[_drawOrder]\nType=0x054,1\nType=0x10400,2\n[end]\n";
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.draw_order.len(), 2);
        assert_eq!(d.draw_order[0].type_code, 0x54);
        assert_eq!(d.draw_order[0].level, 1);
        assert_eq!(d.draw_order[1].type_code, 0x10400);
        assert_eq!(d.draw_order[1].level, 2);
    }

    #[test]
    fn polygon_with_pattern_xpm() {
        let input = r#"[_polygon]
Type=0x01
ExtendedLabels=Y
FontStyle=NoLabel (invisible)
Xpm="0 0 2 0"
"1 c #E0E4E0"
"2 c #101010"
[end]
"#;
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.polygons.len(), 1);
        let p = &d.polygons[0];
        assert_eq!(p.type_code, 0x01);
        assert_eq!(p.font_style, FontStyle::NoLabel);
        let xpm = p.day_xpm.as_ref().unwrap();
        assert_eq!(xpm.width, 0);
        assert_eq!(xpm.height, 0);
        assert_eq!(xpm.colors.len(), 2);
        assert_eq!(xpm.colors[0], Rgba { r: 0xE0, g: 0xE4, b: 0xE0, a: 0 });
        assert_eq!(xpm.pixels.len(), 0);
    }

    #[test]
    fn point_with_full_bitmap() {
        let input = r#"[_point]
Type=0x2A
SubType=3
Xpm="4 2 2 1"
"! c #FF0000"
". c none"
"!!..!!.."
"!..!!..!"
String1=0x04,Hello
[end]
"#;
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.points.len(), 1);
        let p = &d.points[0];
        assert_eq!(p.type_code, 0x2A);
        assert_eq!(p.subtype, 3);
        let xpm = p.day_xpm.as_ref().unwrap();
        assert_eq!(xpm.width, 4);
        assert_eq!(xpm.height, 2);
        assert_eq!(xpm.pixels.len(), 8);
        // `!` → index 0, `.` → index 1 ; premier pixel = !
        assert_eq!(xpm.pixels[0], 0);
        assert_eq!(xpm.pixels[2], 1);
        assert_eq!(p.labels.len(), 1);
        assert_eq!(p.labels[0].lang, 4);
        assert_eq!(p.labels[0].text, "Hello");
    }

    #[test]
    fn line_width_and_fontstyle_custom() {
        let input = r#"[_line]
Type=0x01
LineWidth=2
BorderWidth=3
FontStyle=Small
Xpm="0 0 2 0"
"1 c #F8FCF8"
"2 c #0000F8"
[end]
"#;
        let d = read_typ_text(input).unwrap();
        let l = &d.lines[0];
        assert_eq!(l.line_width, 2);
        assert_eq!(l.border_width, 3);
        assert_eq!(l.font_style, FontStyle::Small);
    }

    #[test]
    fn icons_multi_xpm() {
        let input = r#"[_icons]
Type=0x100
Xpm="2 1 1 1"
"x c #FF0000"
"xx"
Xpm="4 1 1 1"
"y c #00FF00"
"yyyy"
[end]
"#;
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.icons.len(), 1);
        assert_eq!(d.icons[0].icons.len(), 2);
        assert_eq!(d.icons[0].icons[0].width, 2);
        assert_eq!(d.icons[0].icons[1].width, 4);
    }

    #[test]
    fn unknown_section_is_ignored() {
        let input = "[_comments]\nfoo=bar\n[end]\n[_id]\nFID=5\nProductCode=1\nCodePage=1252\n[end]\n";
        let d = read_typ_text(input).unwrap();
        assert_eq!(d.params.family_id, 5);
    }

    #[test]
    fn value_outside_section_errors() {
        let input = "FID=1\n";
        assert!(read_typ_text(input).is_err());
    }

    /// Smoke test sur la fixture réelle CP1252 (production).
    #[test]
    fn real_fixture_i2023100() {
        use super::super::{encoding::detect_and_decode, TypEncoding};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../pipeline/resources/typfiles/I2023100.txt");
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return, // fixture absente : skip silencieux
        };
        let text = detect_and_decode(&bytes, TypEncoding::Auto).unwrap();
        let d = read_typ_text(&text).expect("parse fixture réelle");
        assert_eq!(d.params.codepage, 1252);
        assert_eq!(d.params.family_id, 1100);
        assert!(d.draw_order.len() > 100);
        assert!(!d.polygons.is_empty());
        assert!(!d.lines.is_empty());
    }
}
