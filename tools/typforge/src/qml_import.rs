use quick_xml::events::Event;
use quick_xml::reader::Reader;
use crate::typ::model::{Rgb, Rgba, Xpm, ColorMode, TypLabel, TypPolygon, TypLine, TypPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeomType { Polygon, Line, Point }

#[derive(Debug)]
struct Category {
    value: String,
    label: String,
    symbol_name: String,
}

#[derive(Debug)]
struct Symbol {
    name: String,
    geom_type: GeomType,
    fill_color: Option<Rgb>,
    outline_color: Option<Rgb>,
    line_width: u8,
}

/// Parse une couleur QGIS "R,G,B,A,rgb:..." → Rgb.
/// Retourne None si l'alpha est < 128 (feature transparente).
fn parse_qgis_color(s: &str) -> Option<Rgb> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() < 3 { return None; }
    let r = parts[0].trim().parse::<u8>().ok()?;
    let g = parts[1].trim().parse::<u8>().ok()?;
    let b = parts[2].trim().parse::<u8>().ok()?;
    if parts.len() >= 4 {
        let a = parts[3].trim().parse::<u8>().unwrap_or(255);
        if a < 128 { return None; }
    }
    Some(Rgb { r, g, b })
}

fn solid_xpm(c: Rgb) -> Xpm {
    Xpm {
        width: 1, height: 1,
        colour_mode: ColorMode::Indexed,
        palette: vec![(".".to_string(), Rgba::opaque(c.r, c.g, c.b))],
        pixels: vec![vec![0]],
    }
}

fn make_label(text: &str) -> Vec<TypLabel> {
    if text.is_empty() { vec![] }
    else { vec![TypLabel { lang: 0x00, text: text.to_string() }] }
}

pub struct QmlImport {
    pub polygons: Vec<TypPolygon>,
    pub lines: Vec<TypLine>,
    pub points: Vec<TypPoint>,
}

pub fn parse(data: &[u8]) -> anyhow::Result<QmlImport> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);

    let mut categories: Vec<Category> = Vec::new();
    let mut symbols: Vec<Symbol> = Vec::new();

    let mut in_renderer = false;
    let mut in_symbols = false;
    // Profondeur de nesting <symbol> pour ignorer les symbols imbriqués (renderers composites)
    let mut symbol_depth: u32 = 0;
    let mut current_symbol: Option<Symbol> = None;
    // <layerGeometryType> est la source autoritaire du type géométrique (0=Point,1=Line,2=Polygon)
    let mut layer_geom_type: Option<GeomType> = None;
    let mut reading_geom_type = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"renderer-v2" => { in_renderer = true; }
                    b"symbols" if in_renderer => { in_symbols = true; }
                    b"symbol" if in_symbols => {
                        symbol_depth += 1;
                        if symbol_depth == 1 {
                            let mut sym_name = String::new();
                            let mut sym_type = String::new();
                            for attr in e.attributes().flatten() {
                                match attr.key.as_ref() {
                                    b"name" => sym_name = attr.unescape_value().unwrap_or_default().into_owned(),
                                    b"type" => sym_type = attr.unescape_value().unwrap_or_default().into_owned(),
                                    _ => {}
                                }
                            }
                            let geom = match sym_type.as_str() {
                                "fill"   => GeomType::Polygon,
                                "line"   => GeomType::Line,
                                "marker" => GeomType::Point,
                                _        => GeomType::Point,
                            };
                            current_symbol = Some(Symbol {
                                name: sym_name, geom_type: geom,
                                fill_color: None, outline_color: None, line_width: 1,
                            });
                        }
                    }
                    b"layerGeometryType" => { reading_geom_type = true; }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"category" if in_renderer => {
                        let mut value = String::new();
                        let mut label = String::new();
                        let mut symbol_name = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"value"  => value       = attr.unescape_value().unwrap_or_default().into_owned(),
                                b"label"  => label       = attr.unescape_value().unwrap_or_default().into_owned(),
                                b"symbol" => symbol_name = attr.unescape_value().unwrap_or_default().into_owned(),
                                _ => {}
                            }
                        }
                        categories.push(Category { value, label, symbol_name });
                    }
                    // Les <Option name="..." value="..."/> portant les couleurs sont des éléments vides
                    b"Option" if current_symbol.is_some() && symbol_depth == 1 => {
                        let mut opt_name  = String::new();
                        let mut opt_value = String::new();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name"  => opt_name  = attr.unescape_value().unwrap_or_default().into_owned(),
                                b"value" => opt_value = attr.unescape_value().unwrap_or_default().into_owned(),
                                _ => {}
                            }
                        }
                        if let Some(ref mut sym) = current_symbol {
                            match opt_name.as_str() {
                                "color" | "fillColor" => {
                                    sym.fill_color = parse_qgis_color(&opt_value);
                                }
                                "outline_color" | "line_color" => {
                                    if sym.outline_color.is_none() {
                                        sym.outline_color = parse_qgis_color(&opt_value);
                                    }
                                }
                                "line_width" | "width" => {
                                    if let Ok(w) = opt_value.trim().parse::<f64>() {
                                        // QGIS : mm → pixels Garmin (facteur ~2)
                                        sym.line_width = ((w * 2.0).round() as u8).max(1).min(20);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if reading_geom_type => {
                let text = std::str::from_utf8(e.as_ref()).unwrap_or("");
                layer_geom_type = match text.trim() {
                    "0" => Some(GeomType::Point),
                    "1" => Some(GeomType::Line),
                    "2" => Some(GeomType::Polygon),
                    _   => None,
                };
                reading_geom_type = false;
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"symbol" if in_symbols => {
                        symbol_depth = symbol_depth.saturating_sub(1);
                        if symbol_depth == 0 {
                            if let Some(sym) = current_symbol.take() {
                                symbols.push(sym);
                            }
                        }
                    }
                    b"symbols"    => { in_symbols = false; }
                    b"renderer-v2" => { in_renderer = false; }
                    b"layerGeometryType" => { reading_geom_type = false; }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Erreur XML QML: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    // Compteurs indépendants par namespace TYP (polygones/lignes/points sont des espaces séparés)
    let mut poly_counter:  u16 = 0x01;
    let mut line_counter:  u16 = 0x01;
    let mut point_counter: u16 = 0x01;

    let mut polygons: Vec<TypPolygon> = Vec::new();
    let mut lines:    Vec<TypLine>    = Vec::new();
    let mut points:   Vec<TypPoint>   = Vec::new();

    for cat in &categories {
        let sym = symbols.iter().find(|s| s.name == cat.symbol_name);
        // layerGeometryType est autoritatif ; inférer depuis le symbol en fallback
        let geom = layer_geom_type
            .or_else(|| sym.map(|s| s.geom_type))
            .unwrap_or(GeomType::Point);

        let fill = sym.and_then(|s| s.fill_color)
            .or_else(|| sym.and_then(|s| s.outline_color));
        let label_text = if !cat.label.is_empty() { &cat.label } else { &cat.value };
        let labels  = make_label(label_text);
        let day_xpm = fill.map(solid_xpm);

        match geom {
            GeomType::Polygon => {
                let tc = poly_counter;
                poly_counter = poly_counter.saturating_add(1);
                polygons.push(TypPolygon {
                    type_code: tc, sub_type: 0, labels, day_xpm, night_xpm: None,
                    ..Default::default()
                });
            }
            GeomType::Line => {
                let tc = line_counter;
                line_counter = line_counter.saturating_add(1);
                let lw = sym.map(|s| s.line_width).unwrap_or(1);
                lines.push(TypLine {
                    type_code: tc, sub_type: 0, labels, day_xpm, night_xpm: None,
                    line_width: lw,
                    ..Default::default()
                });
            }
            GeomType::Point => {
                let tc = point_counter;
                point_counter = point_counter.saturating_add(1);
                points.push(TypPoint {
                    type_code: tc, sub_type: 0, labels, day_xpm, night_xpm: None,
                    ..Default::default()
                });
            }
        }
    }

    Ok(QmlImport { polygons, lines, points })
}
