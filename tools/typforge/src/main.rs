mod app;
mod error;
mod typ;
mod qml_import;

slint::include_modules!();

use std::rc::Rc;
use std::cell::RefCell;
use slint::{ModelRc, VecModel, StandardListViewItem, SharedPixelBuffer, Rgb8Pixel};
// NOTE : winit_030 re-exporte winit 0.30.x via la feature Slint "unstable-winit-030".
// Cette feature est intentionnellement instable côté Slint — vérifier lors d'un bump de version.
use slint::winit_030::winit;
use typ::{TypDocument, TypPolygon, TypLine, TypPoint, TypIconSet, DrawOrderEntry, TypLabel, Xpm, Rgb, Rgba, ColorMode, FontStyle, ContourColor};
use app::App;

// Icône 512×512 embarquée à la compilation — aucune dépendance fichier runtime.
static ICON_BYTES: &[u8] = include_bytes!("../typforge.png");

fn setup_platform_with_icon(icon: winit::window::Icon) -> anyhow::Result<()> {
    let backend = i_slint_backend_winit::Backend::builder()
        .with_window_attributes_hook(move |attr| attr.with_window_icon(Some(icon.clone())))
        .build()?;
    slint::platform::set_platform(Box::new(backend))
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn load_window_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(ICON_BYTES).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), width, height).ok()
}

// ─── Render helpers ──────────────────────────────────────────────

fn first_opaque(xpm: Option<&Xpm>) -> Option<(u8, u8, u8)> {
    let xpm = xpm?;
    for (_, c) in &xpm.palette {
        if !c.is_transparent() {
            return Some((c.r, c.g, c.b));
        }
    }
    None
}

fn solid_thumb(r: u8, g: u8, b: u8, size: u32) -> slint::Image {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    for p in buf.make_mut_slice().iter_mut() {
        *p = Rgb8Pixel { r, g, b };
    }
    slint::Image::from_rgb8(buf)
}

fn solid_buf(r: u8, g: u8, b: u8, size: u32) -> SharedPixelBuffer<Rgb8Pixel> {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    for p in buf.make_mut_slice().iter_mut() { *p = Rgb8Pixel { r, g, b }; }
    buf
}

/// Applique un motif XPM en tile sur `pixels` (size×size).
/// Seuls les pixels opaques sont peints ; les pixels transparents laissent le fond intact.
fn tile_xpm_on_buf(pixels: &mut [Rgb8Pixel], size: u32, xpm: &Xpm) {
    let xw = xpm.width as u32;
    let xh = xpm.height as u32;
    if xw == 0 || xh == 0 { return; }
    for y in 0..size {
        for x in 0..size {
            let xi = (x % xw) as usize;
            let yi = (y % xh) as usize;
            if let Some(c_idx) = xpm.pixels.get(yi).and_then(|row| row.get(xi)) {
                if let Some((_, c)) = xpm.palette.get(*c_idx) {
                    if !c.is_transparent() {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                    }
                }
            }
        }
    }
}

/// Dessine une bande horizontale en tuilant le motif XPM (lignes en tirets).
/// Seuls les pixels opaques du XPM remplacent le fond — les pixels transparents sont ignorés.
fn draw_line_xpm(pixels: &mut [Rgb8Pixel], img_w: u32, y_start: u32, lw: u32, x0: u32, x1: u32, xpm: &Xpm) {
    let xw = xpm.width as u32;
    let xh = xpm.height as u32;
    if xw == 0 || xh == 0 { return; }
    for dy in 0..lw {
        let y = y_start + dy;
        let xpm_row = dy % xh;
        for x in x0..x1 {
            let xpm_col = x % xw;
            if let Some(c_idx) = xpm.pixels.get(xpm_row as usize).and_then(|r| r.get(xpm_col as usize)) {
                if let Some((_, c)) = xpm.palette.get(*c_idx) {
                    if !c.is_transparent() {
                        pixels[(y * img_w + x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                    }
                }
            }
        }
    }
}

fn tile_3x3(src: &SharedPixelBuffer<Rgb8Pixel>) -> slint::Image {
    let sz = src.width();
    debug_assert_eq!(src.height(), sz, "tile_3x3 : le buffer source doit être carré");
    let out_sz = sz * 3;
    let mut out = SharedPixelBuffer::<Rgb8Pixel>::new(out_sz, out_sz);
    let src_px = src.as_slice().to_vec();
    let dst_px = out.make_mut_slice();
    for tr in 0u32..3 {
        for tc in 0u32..3 {
            for y in 0..sz {
                for x in 0..sz {
                    let si = (y * sz + x) as usize;
                    let di = ((tr * sz + y) * out_sz + tc * sz + x) as usize;
                    dst_px[di] = src_px[si];
                }
            }
        }
    }
    slint::Image::from_rgb8(out)
}

fn render_element_buf(doc: &TypDocument, kind: i32, idx: usize, size: u32, night: bool) -> SharedPixelBuffer<Rgb8Pixel> {
    let blank = solid_buf(0x80, 0x80, 0x80, size);
    match kind {
        0 => match doc.polygons.get(idx) {
            Some(p) => {
                let xpm = if night && p.night_xpm.is_some() { p.night_xpm.as_ref() } else { p.day_xpm.as_ref() };
                let bg: u8 = if night { 0x33 } else { 0xd0 };
                let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
                let pixels = buf.make_mut_slice();
                for px in pixels.iter_mut() { *px = Rgb8Pixel { r: bg, g: bg, b: bg }; }
                if let Some(xpm) = xpm { tile_xpm_on_buf(pixels, size, xpm); }
                for y in 0..size { for x in 0..size {
                    if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: 0, g: 0, b: 0 };
                    }
                }}
                buf
            }
            None => blank,
        },
        1 => match doc.lines.get(idx) {
            Some(l) => {
                let xpm = if night && l.night_xpm.is_some() { l.night_xpm.as_ref() } else { l.day_xpm.as_ref() };
                let lc = first_opaque(xpm).unwrap_or(if night { (0xcc, 0xcc, 0xcc) } else { (0, 0, 0) });
                let bg: u8 = if night { 0x33 } else { 0xe0 };
                let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
                let pixels = buf.make_mut_slice();
                for p in pixels.iter_mut() { *p = Rgb8Pixel { r: bg, g: bg, b: bg }; }
                let xpm_tiled = xpm.filter(|x| x.width > 0 && x.height > 0);
                let lw = if let Some(x) = xpm_tiled {
                    (x.height as u32).clamp(1, size / 4)
                } else {
                    (l.line_width as u32).clamp(1, size / 4)
                };
                let y_start = (size / 2).saturating_sub(lw / 2);
                if let Some(xpm) = xpm_tiled {
                    draw_line_xpm(pixels, size, y_start, lw, 2, size.saturating_sub(2), xpm);
                } else {
                    for dy in 0..lw {
                        let y = y_start + dy;
                        if y < size {
                            for x in 2..size.saturating_sub(2) {
                                pixels[(y * size + x) as usize] = Rgb8Pixel { r: lc.0, g: lc.1, b: lc.2 };
                            }
                        }
                    }
                }
                buf
            }
            None => blank,
        },
        2 => match doc.points.get(idx) {
            Some(p) => {
                let xpm = if night && p.night_xpm.is_some() { p.night_xpm.as_ref() } else { p.day_xpm.as_ref() };
                let bg: u8 = if night { 0x22 } else { 0xff };
                let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
                let pixels = buf.make_mut_slice();
                for px in pixels.iter_mut() { *px = Rgb8Pixel { r: bg, g: bg, b: bg }; }
                if let Some(xpm) = xpm {
                    let xw = xpm.width as u32;
                    let xh = xpm.height as u32;
                    let ox = size.saturating_sub(xw) / 2;
                    let oy = size.saturating_sub(xh) / 2;
                    for (ri, row) in xpm.pixels.iter().enumerate() {
                        let py = oy + ri as u32;
                        if py >= size { break; }
                        for (ci, &c_idx) in row.iter().enumerate() {
                            let px_x = ox + ci as u32;
                            if px_x >= size { break; }
                            if let Some((_, c)) = xpm.palette.get(c_idx) {
                                if !c.is_transparent() {
                                    pixels[(py * size + px_x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                                }
                            }
                        }
                    }
                }
                buf
            }
            None => blank,
        },
        _ => blank,
    }
}

fn render_superposition(doc: &TypDocument, size: u32, night: bool) -> slint::Image {
    let bg: u8 = if night { 0x22 } else { 0xff };
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() { *p = Rgb8Pixel { r: bg, g: bg, b: bg }; }

    // Couche 1 : polygone (fond) — motif XPM tuilé, fond déjà initialisé ci-dessus
    if let Some(poly) = doc.polygons.first() {
        let xpm = if night && poly.night_xpm.is_some() { poly.night_xpm.as_ref() } else { poly.day_xpm.as_ref() };
        if let Some(xpm) = xpm {
            tile_xpm_on_buf(pixels, size, xpm);
        }
        for y in 0..size { for x in 0..size {
            if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
                pixels[(y * size + x) as usize] = Rgb8Pixel { r: 0, g: 0, b: 0 };
            }
        }}
    }

    // Couche 2 : ligne (bande centrale)
    if let Some(line) = doc.lines.first() {
        let xpm = if night && line.night_xpm.is_some() { line.night_xpm.as_ref() } else { line.day_xpm.as_ref() };
        let lc = first_opaque(xpm).unwrap_or(if night { (0xcc, 0xcc, 0xcc) } else { (0, 0, 0) });
        let xpm_tiled = xpm.filter(|x| x.width > 0 && x.height > 0);
        let lw = if let Some(x) = xpm_tiled {
            (x.height as u32).clamp(1, size / 8)
        } else {
            (line.line_width as u32).clamp(1, size / 8)
        };
        let y_start = size / 2;
        if let Some(xpm) = xpm_tiled {
            draw_line_xpm(pixels, size, y_start, lw, 4, size.saturating_sub(4), xpm);
        } else {
            for dy in 0..lw {
                let y = y_start + dy;
                if y < size {
                    for x in 4..size.saturating_sub(4) {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: lc.0, g: lc.1, b: lc.2 };
                    }
                }
            }
        }
    }

    // Couche 3 : POI centré
    if let Some(point) = doc.points.first() {
        let xpm = if night && point.night_xpm.is_some() { point.night_xpm.as_ref() } else { point.day_xpm.as_ref() };
        if let Some(xpm) = xpm {
            let xw = xpm.width as u32;
            let xh = xpm.height as u32;
            let ox = size.saturating_sub(xw) / 2;
            let oy = size.saturating_sub(xh) / 2;
            for (ri, row) in xpm.pixels.iter().enumerate() {
                let py = oy + ri as u32;
                if py >= size { break; }
                for (ci, &c_idx) in row.iter().enumerate() {
                    let px_x = ox + ci as u32;
                    if px_x >= size { break; }
                    if let Some((_, c)) = xpm.palette.get(c_idx) {
                        if !c.is_transparent() {
                            pixels[(py * size + px_x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                        }
                    }
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

fn render_polygon_thumb(poly: &TypPolygon, size: u32) -> slint::Image {
    render_polygon_thumb_xpm(poly.day_xpm.as_ref(), size, false, None)
}

fn render_polygon_thumb_xpm(xpm: Option<&Xpm>, size: u32, night: bool, contour: Option<Rgb>) -> slint::Image {
    let bg: u8 = if night { 0x33 } else { 0xd0 };
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() { *p = Rgb8Pixel { r: bg, g: bg, b: bg }; }
    if let Some(xpm) = xpm {
        tile_xpm_on_buf(pixels, size, xpm);
    }
    let bc = contour.unwrap_or(Rgb { r: 0, g: 0, b: 0 });
    let border = Rgb8Pixel { r: bc.r, g: bc.g, b: bc.b };
    for y in 0..size { for x in 0..size {
        if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
            pixels[(y * size + x) as usize] = border;
        }
    }}
    slint::Image::from_rgb8(buf)
}

fn render_line_thumb(line: &TypLine, size: u32) -> slint::Image {
    let xpm = line.day_xpm.as_ref();
    let lc = first_opaque(xpm).unwrap_or((0, 0, 0));
    let xpm_tiled = xpm.filter(|x| x.width > 0 && x.height > 0);
    let lw = if let Some(x) = xpm_tiled {
        (x.height as u32).clamp(1, size / 4)
    } else {
        (line.line_width as u32).clamp(1, size / 4)
    };
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() {
        *p = Rgb8Pixel { r: 0xe0, g: 0xe0, b: 0xe0 };
    }
    let y_start = (size / 2).saturating_sub(lw / 2);
    if let Some(xpm) = xpm_tiled {
        draw_line_xpm(pixels, size, y_start, lw, 4, size - 4, xpm);
    } else {
        for dy in 0..lw {
            let y = y_start + dy;
            if y < size {
                for x in 4..size - 4 {
                    pixels[(y * size + x) as usize] = Rgb8Pixel { r: lc.0, g: lc.1, b: lc.2 };
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

fn render_point_thumb(point: &TypPoint, size: u32) -> slint::Image {
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() {
        *p = Rgb8Pixel { r: 0xff, g: 0xff, b: 0xff };
    }
    if let Some(xpm) = &point.day_xpm {
        let xw = xpm.width as u32;
        let xh = xpm.height as u32;
        let ox = size.saturating_sub(xw) / 2;
        let oy = size.saturating_sub(xh) / 2;
        for (row_i, row) in xpm.pixels.iter().enumerate() {
            let y = oy + row_i as u32;
            if y >= size { break; }
            for (col_i, &idx) in row.iter().enumerate() {
                let x = ox + col_i as u32;
                if x >= size { break; }
                if let Some((_, c)) = xpm.palette.get(idx) {
                    if !c.is_transparent() {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                    }
                }
            }
        }
    } else {
        let mid = (size / 2) as i32;
        let r = (size / 4) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let y = (mid + dy) as u32;
                    let x = (mid + dx) as u32;
                    if y < size && x < size {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: 0x44, g: 0x88, b: 0xcc };
                    }
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

// ─── Editor helpers ──────────────────────────────────────────────

fn font_style_to_int(s: FontStyle) -> i32 {
    match s {
        FontStyle::Default   => 0,
        FontStyle::NoLabel   => 1,
        FontStyle::Small     => 2,
        FontStyle::Normal    => 3,
        FontStyle::Large     => 4,
        FontStyle::Custom(n) => n as i32 + 10,
    }
}

fn int_to_font_style(n: i32) -> FontStyle {
    match n {
        0 => FontStyle::Default,
        1 => FontStyle::NoLabel,
        2 => FontStyle::Small,
        3 => FontStyle::Normal,
        4 => FontStyle::Large,
        n if n >= 10 => FontStyle::Custom((n - 10) as u8),
        _ => FontStyle::Default,
    }
}

fn hex_to_rgb(s: &str) -> Option<Rgb> {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Rgb { r, g, b })
    } else {
        None
    }
}

fn hex_to_slint_color(hex: &str) -> slint::Color {
    if let Some(rgb) = hex_to_rgb(hex) {
        slint::Color::from_rgb_u8(rgb.r, rgb.g, rgb.b)
    } else {
        slint::Color::from_rgb_u8(0, 0, 0)
    }
}

fn parse_type_code(s: &str) -> Option<u16> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u16>().ok()
    }
}

fn parse_sub_type(s: &str) -> Option<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u8>().ok()
    }
}

fn normalize_color_string(s: &str) -> Option<String> {
    let s = s.trim();
    // "#RRGGBB" ou "#RRGGBBAA" — l'alpha zenity est toujours en fin, on prend les 6 premiers
    if let Some(hex) = s.strip_prefix('#') {
        let hex6 = if hex.len() >= 8 { &hex[..6] } else { hex };
        if hex6.len() == 6 && hex6.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("#{}", hex6.to_lowercase()));
        }
    }
    // "rgb(R,G,B)" ou "rgba(R,G,B,A)" — GTK3 émet 0-255, GTK4 émet 0-65535
    let inner_opt = s.strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))
        .and_then(|t| t.strip_suffix(')'));
    if let Some(inner) = inner_opt {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].trim().parse::<u32>(),
                parts[1].trim().parse::<u32>(),
                parts[2].trim().parse::<u32>(),
            ) {
                // GTK4 utilise la plage 0-65535 ; diviser par 257 ramène à 0-255
                let scale = if r > 255 || g > 255 || b > 255 { 257u32 } else { 1u32 };
                return Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    (r / scale) as u8,
                    (g / scale) as u8,
                    (b / scale) as u8,
                ));
            }
        }
    }
    None
}

// F1/F7/F8: -STA requis (COM MTA sinon dialog invisible) ; -ExecutionPolicy Bypass pour les
// environnements restreints ; powershell.exe cible explicitement Windows PowerShell v5 qui
// embarque System.Windows.Forms (pwsh / PowerShell 7+ n'a pas WinForms).
// F6: les `{{`/`}}` dans format! produisent des `{`/`}` littéraux dans le script ;
//     la string PS simple-quote + opérateur -f est correcte (ce n'est pas de l'interpolation PS).
#[cfg(windows)]
fn pick_color_windows(initial: &str) -> Option<String> {
    // F11: `initial` est déjà validé par l'appelant — pas de unwrap_or nécessaire
    let rgb = hex_to_rgb(initial)?;
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $d = New-Object System.Windows.Forms.ColorDialog; \
         $d.Color = [System.Drawing.Color]::FromArgb(255, {r}, {g}, {b}); \
         $d.FullOpen = $true; \
         if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) \
         {{ '#{{0:X2}}{{1:X2}}{{2:X2}}' -f $d.Color.R, $d.Color.G, $d.Color.B }}",
        r = rgb.r, g = rgb.g, b = rgb.b
    );
    let out = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .output()
        .ok()?;
    // F2: Cancel → stdout vide → normalize_color_string("") = None (correct) ;
    //     on vérifie stdout non-vide avant pour éviter tout parasite de sortie PS.
    if out.status.success() && !out.stdout.is_empty() {
        normalize_color_string(&String::from_utf8_lossy(&out.stdout))
    } else {
        None
    }
}

// osascript retourne "{R, G, B}" avec des valeurs 0-65535
#[cfg(target_os = "macos")]
fn pick_color_macos(initial: &str) -> Option<String> {
    // F11: `initial` est déjà validé par l'appelant
    let rgb = hex_to_rgb(initial)?;
    // Convertir 0-255 → 0-65535 pour AppleScript
    let (r16, g16, b16) = (
        rgb.r as u32 * 257,
        rgb.g as u32 * 257,
        rgb.b as u32 * 257,
    );
    let script = format!("choose color default color {{{r16}, {g16}, {b16}}}");
    let out = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // Sortie : "{R, G, B}\n" valeurs 0-65535
    // F3: strip_prefix/suffix supprime exactement une accolade (trim_matches enlèverait toutes)
    let s = String::from_utf8_lossy(&out.stdout);
    let inner = s.trim().strip_prefix('{')?.strip_suffix('}')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() == 3 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[0].trim().parse::<u32>(),
            parts[1].trim().parse::<u32>(),
            parts[2].trim().parse::<u32>(),
        ) {
            // F4: arrondi correct pour les valeurs 16-bit non multiples de 257
            let to_u8 = |v: u32| -> u8 { ((v * 255 + 32767) / 65535) as u8 };
            return Some(format!("#{:02x}{:02x}{:02x}", to_u8(r), to_u8(g), to_u8(b)));
        }
    }
    None
}

fn pick_color(current_hex: &str) -> Option<String> {
    let initial = if hex_to_rgb(current_hex).is_some() { current_hex } else { "#000000" };

    #[cfg(windows)]
    return pick_color_windows(initial);

    #[cfg(target_os = "macos")]
    return pick_color_macos(initial);

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        // zenity (GNOME / GTK) — distinguer "non trouvé" (Err) de "annulé" (Ok non-success)
        match std::process::Command::new("zenity")
            .args(["--color-selection", "--color", initial, "--title=Choisir une couleur"])
            .output()
        {
            Ok(out) if out.status.success() => {
                return normalize_color_string(&String::from_utf8_lossy(&out.stdout));
            }
            Ok(_) => return None,   // zenity présent, dialog annulé
            Err(_) => {}            // zenity absent → essayer kdialog
        }
        // kdialog (KDE / Qt)
        match std::process::Command::new("kdialog")
            .args(["--getcolor", "--default", initial])
            .output()
        {
            Ok(out) if out.status.success() => {
                normalize_color_string(&String::from_utf8_lossy(&out.stdout))
            }
            _ => None,
        }
    }
}

/// Crée ou met à jour la première entrée opaque de la palette XPM avec `color`.
/// Si la palette est entièrement transparente, remplace l'XPM par un 1×1 solide.
fn set_xpm_fill_color(xpm: &mut Option<Xpm>, color: Rgb) {
    let solid = || Xpm {
        width: 1, height: 1,
        colour_mode: ColorMode::Indexed,
        palette: vec![(".".to_string(), Rgba::opaque(color.r, color.g, color.b))],
        pixels: vec![vec![0]],
    };
    match xpm {
        Some(x) => {
            if let Some((_, c)) = x.palette.iter_mut().find(|(_, c)| !c.is_transparent()) {
                *c = Rgba::opaque(color.r, color.g, color.b);
            } else {
                *xpm = Some(solid());
            }
        }
        None => *xpm = Some(solid()),
    }
}

fn xpm_fill_color(xpm: Option<&Xpm>) -> (slint::SharedString, slint::Color) {
    match first_opaque(xpm) {
        Some((r, g, b)) => (
            format!("#{:02X}{:02X}{:02X}", r, g, b).into(),
            slint::Color::from_rgb_u8(r, g, b),
        ),
        None => (
            "#888888".into(),
            slint::Color::from_rgb_u8(0x88, 0x88, 0x88),
        ),
    }
}

fn xpm_to_text_opt(xpm: Option<&Xpm>) -> slint::SharedString {
    match xpm {
        Some(x) => typ::text_writer::xpm_to_text(x).into(),
        None => "".into(),
    }
}

// ─── UI bridge ───────────────────────────────────────────────────

fn make_item(text: impl Into<slint::SharedString>) -> StandardListViewItem {
    let mut item = StandardListViewItem::default();
    item.text = text.into();
    item
}

fn format_list_label(tc: u16, st: u8, grmn: &str) -> String {
    if grmn.is_empty() {
        format!("0x{:02X} / 0x{:02X}", tc, st)
    } else {
        format!("0x{:02X}/0x{:02X} — {}", tc, st, grmn)
    }
}

fn build_list_model(triples: impl Iterator<Item = (u16, u8, String)>) -> ModelRc<StandardListViewItem> {
    ModelRc::new(VecModel::from(
        triples.map(|(tc, st, grmn)| make_item(format_list_label(tc, st, &grmn)))
               .collect::<Vec<_>>()
    ))
}

fn rebuild_gallery(doc: &TypDocument, window: &AppWindow, nav_tab: i32) {
    let items: Vec<GalleryItem> = match nav_tab {
        0 => doc.polygons.iter().enumerate().map(|(i, p)| GalleryItem {
            thumb: render_polygon_thumb(p, 56),
            name: format_list_label(p.type_code, p.sub_type, &p.grmn_type).into(),
            kind: 0,
            index: i as i32,
        }).collect(),
        1 => doc.lines.iter().enumerate().map(|(i, l)| GalleryItem {
            thumb: render_line_thumb(l, 56),
            name: format_list_label(l.type_code, l.sub_type, &l.grmn_type).into(),
            kind: 1,
            index: i as i32,
        }).collect(),
        2 => doc.points.iter().enumerate().map(|(i, p)| GalleryItem {
            thumb: render_point_thumb(p, 56),
            name: format_list_label(p.type_code, p.sub_type, &p.grmn_type).into(),
            kind: 2,
            index: i as i32,
        }).collect(),
        _ => doc.icons.iter().enumerate().map(|(i, ic)| GalleryItem {
            thumb: slint::Image::default(),
            name: format!("0x{:02X}/0x{:02X}", ic.type_code, ic.sub_type).into(),
            kind: 3,
            index: i as i32,
        }).collect(),
    };
    window.set_gallery_items(ModelRc::new(VecModel::from(items)));
}

fn update_ui_from_doc(doc: &TypDocument, window: &AppWindow) {
    window.set_meta_family_id(doc.param.family_id.to_string().into());
    window.set_meta_product_code(doc.param.product_id.to_string().into());
    window.set_meta_codepage(doc.param.codepage as i32);
    window.set_meta_header_str(doc.param.header_str.as_str().into());

    window.set_poly_count(doc.polygons.len() as i32);
    window.set_line_count(doc.lines.len() as i32);
    window.set_point_count(doc.points.len() as i32);
    window.set_poi_count(doc.icons.len() as i32);

    window.set_polygons(build_list_model(doc.polygons.iter().map(|p| (p.type_code, p.sub_type, p.grmn_type.clone()))));
    window.set_lines(build_list_model(doc.lines.iter().map(|l| (l.type_code, l.sub_type, l.grmn_type.clone()))));
    window.set_points(build_list_model(doc.points.iter().map(|p| (p.type_code, p.sub_type, p.grmn_type.clone()))));
    window.set_extra_pois(build_list_model(doc.icons.iter().map(|ic| (ic.type_code, ic.sub_type, String::new()))));
    window.set_draworder(ModelRc::new(VecModel::from(
        doc.draw_order.iter()
            .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
            .collect::<Vec<_>>()
    )));

    window.set_selected_kind(-1);
    window.set_selected_idx(-1);
    window.set_active_nav_tab(0);
    rebuild_gallery(doc, window, 0);
}

fn render_preview(doc: &TypDocument, kind: i32, idx: usize) -> (slint::Image, slint::Image) {
    const SZ: u32 = 128;
    let blank = solid_thumb(0x80, 0x80, 0x80, SZ);
    match kind {
        0 => match doc.polygons.get(idx) {
            Some(p) => {
                let day = render_polygon_thumb(p, SZ);
                let night = render_polygon_thumb_xpm(
                    p.night_xpm.as_ref().or(p.day_xpm.as_ref()), SZ, true, None,
                );
                (day, night)
            }
            None => (blank.clone(), blank),
        },
        1 => match doc.lines.get(idx) {
            Some(l) => { let t = render_line_thumb(l, SZ); (t.clone(), t) }
            None => (blank.clone(), blank),
        },
        2 => match doc.points.get(idx) {
            Some(p) => {
                let day = render_point_thumb(p, SZ);
                let night = if p.night_xpm.is_some() {
                    let tmp = TypPoint { day_xpm: p.night_xpm.clone(), ..p.clone() };
                    render_point_thumb(&tmp, SZ)
                } else {
                    day.clone()
                };
                (day, night)
            }
            None => (blank.clone(), blank),
        },
        _ => (blank.clone(), blank),
    }
}

fn render_preview_with_mode(doc: &TypDocument, kind: i32, idx: usize, mode: i32) -> (slint::Image, slint::Image) {
    const SZ: u32 = 128;
    const TILE: u32 = 40;
    match mode {
        1 => {
            let day_buf = render_element_buf(doc, kind, idx, TILE, false);
            let night_buf = render_element_buf(doc, kind, idx, TILE, true);
            (tile_3x3(&day_buf), tile_3x3(&night_buf))
        }
        2 => (render_superposition(doc, SZ, false), render_superposition(doc, SZ, true)),
        _ => render_preview(doc, kind, idx),
    }
}

// ─── POI editor ──────────────────────────────────────────────────

struct PoiEditorState {
    doc_idx: usize,
    day_xpm: Option<Xpm>,
    night_xpm: Option<Xpm>,
    editing_night: bool,
    zoom: u32,
    tool: i32,
    brush_size: u32,
    active_color_idx: usize,
    line_start: Option<(u32, u32)>,
    has_night_bmp: bool,
    extended_labels: bool,
    font_style: FontStyle,
    day_font_colour: Option<Rgb>,
    night_font_colour: Option<Rgb>,
    labels: Vec<TypLabel>,
}

impl PoiEditorState {
    fn active_xpm_mut(&mut self) -> &mut Option<Xpm> {
        if self.editing_night { &mut self.night_xpm } else { &mut self.day_xpm }
    }
}

fn lang_name(code: u8) -> &'static str {
    match code {
        0x00 => "default",    0x01 => "french",     0x02 => "german",
        0x03 => "dutch",      0x04 => "english",    0x05 => "italian",
        0x06 => "finnish",    0x07 => "spanish",    0x08 => "spanish (LA)",
        0x09 => "basque",     0x0A => "catalan",    0x0B => "galician",
        0x0C => "welsh",      0x0D => "greek",      0x0E => "estonian",
        0x0F => "latvian",    0x10 => "lithuanian", 0x11 => "slovenian",
        0x12 => "romanian",   0x13 => "hungarian",  0x14 => "czech",
        0x15 => "slovak",     0x16 => "croatian",   0x17 => "polish",
        0x18 => "serbian",    0x19 => "serbian cyr",0x1A => "macedonian",
        0x1B => "bulgarian",  0x1C => "russian",    0x1D => "greek alt",
        0x1E => "arabic",     0x1F => "hebrew",     0x20 => "trad. chinese",
        0x21 => "simp. chinese", 0x22 => "japanese",
        _ => "unknown",
    }
}

fn build_lang_entries(labels: &[TypLabel]) -> slint::ModelRc<LangEntry> {
    let entries: Vec<LangEntry> = (0u8..=0x22u8).map(|code| {
        let has = labels.iter().any(|l| l.lang == code && !l.text.is_empty());
        LangEntry {
            code: code as i32,
            code_hex: format!("0x{:02X}", code).into(),
            name: lang_name(code).into(),
            has_label: has,
        }
    }).collect();
    slint::ModelRc::new(slint::VecModel::from(entries))
}

fn build_palette_entries(xpm: &Xpm) -> slint::ModelRc<PaletteEntry> {
    let entries: Vec<PaletteEntry> = xpm.palette.iter().map(|(_, c)| {
        if c.is_transparent() {
            PaletteEntry {
                hex: "transparent".into(),
                r: 0, g: 0, b: 0,
                is_transparent: true,
            }
        } else {
            PaletteEntry {
                hex: format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b).into(),
                r: c.r as i32,
                g: c.g as i32,
                b: c.b as i32,
                is_transparent: false,
            }
        }
    }).collect();
    slint::ModelRc::new(slint::VecModel::from(entries))
}

fn render_grid(xpm: &Xpm, zoom: u32) -> slint::Image {
    let cell = zoom;
    let stride = cell + 1;
    let w = (xpm.width as u32) * stride + 1;
    let h = (xpm.height as u32) * stride + 1;
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    let pixels = buf.make_mut_slice();

    // fond grille gris
    for p in pixels.iter_mut() {
        *p = Rgb8Pixel { r: 0xcc, g: 0xcc, b: 0xcc };
    }

    // dessiner les cellules
    for (row_i, row) in xpm.pixels.iter().enumerate() {
        for (col_i, &idx) in row.iter().enumerate() {
            let color = xpm.palette.get(idx)
                .map(|(_, c)| *c)
                .unwrap_or(Rgba::transparent());
            let (r, g, b) = if color.is_transparent() {
                (0xff, 0xff, 0xff) // blanc = transparent
            } else {
                (color.r, color.g, color.b)
            };
            let cx = col_i as u32 * stride + 1;
            let cy = row_i as u32 * stride + 1;
            for dy in 0..cell {
                for dx in 0..cell {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px < w && py < h {
                        pixels[(py * w + px) as usize] = Rgb8Pixel { r, g, b };
                    }
                }
            }
        }
    }

    // crosshairs rouges au centre
    let cx = (xpm.width as u32) * stride / 2;
    let cy = (xpm.height as u32) * stride / 2;
    for x in 0..w { pixels[(cy * w + x) as usize] = Rgb8Pixel { r: 0xff, g: 0x00, b: 0x00 }; }
    for y in 0..h { pixels[(y * w + cx) as usize] = Rgb8Pixel { r: 0xff, g: 0x00, b: 0x00 }; }

    slint::Image::from_rgb8(buf)
}

fn render_poi_xpm_preview(xpm: Option<&Xpm>, size: u32, night_bg: bool) -> slint::Image {
    let bg: u8 = if night_bg { 0x33 } else { 0xff };
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() { *p = Rgb8Pixel { r: bg, g: bg, b: bg }; }
    if let Some(xpm) = xpm {
        let xw = xpm.width as u32;
        let xh = xpm.height as u32;
        let ox = size.saturating_sub(xw) / 2;
        let oy = size.saturating_sub(xh) / 2;
        for (row_i, row) in xpm.pixels.iter().enumerate() {
            let y = oy + row_i as u32;
            if y >= size { break; }
            for (col_i, &idx) in row.iter().enumerate() {
                let x = ox + col_i as u32;
                if x >= size { break; }
                if let Some((_, c)) = xpm.palette.get(idx) {
                    if !c.is_transparent() {
                        pixels[(y * size + x) as usize] = Rgb8Pixel { r: c.r, g: c.g, b: c.b };
                    }
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

// ─── Previews éditeurs polygone / ligne ─────────────────────────

const EDITOR_PREVIEW_SIZE: u32 = 120;

fn solid_xpm(c: Rgb) -> Xpm {
    Xpm {
        width: 1, height: 1,
        colour_mode: ColorMode::Indexed,
        palette: vec![(".".to_string(), Rgba::opaque(c.r, c.g, c.b))],
        pixels: vec![vec![0]],
    }
}

fn render_line_xpm_preview(xpm: Option<&Xpm>, line_width: u8, night: bool, size: u32) -> slint::Image {
    let lc = first_opaque(xpm).unwrap_or(if night { (0xcc, 0xcc, 0xcc) } else { (0, 0, 0) });
    let bg: u8 = if night { 0x33 } else { 0xe0 };
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(size, size);
    let pixels = buf.make_mut_slice();
    for p in pixels.iter_mut() { *p = Rgb8Pixel { r: bg, g: bg, b: bg }; }
    let xpm_tiled = xpm.filter(|x| x.width > 0 && x.height > 0);
    let lw = if let Some(x) = xpm_tiled {
        // line_width inclut le contour (total_lw) — utiliser max pour que le contour
        // soit toujours reflété, même pour un XPM multi-lignes (motif pointillés)
        (x.height as u32).max(line_width as u32).clamp(1, size / 4)
    } else {
        (line_width as u32).clamp(1, size / 4)
    };
    let y_start = (size / 2).saturating_sub(lw / 2);
    if let Some(x) = xpm_tiled {
        draw_line_xpm(pixels, size, y_start, lw, 4, size.saturating_sub(4), x);
    } else {
        for dy in 0..lw {
            let y = y_start + dy;
            if y < size {
                for x in 4..size.saturating_sub(4) {
                    pixels[(y * size + x) as usize] = Rgb8Pixel { r: lc.0, g: lc.1, b: lc.2 };
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

fn push_ep_previews(w: &AppWindow, day_xpm: Option<&Xpm>, night_xpm: Option<&Xpm>) {
    let contour = if w.get_ep_contour_enabled() {
        Some(slint_to_rgb(w.get_ep_contour_color()))
    } else {
        None
    };
    w.set_ep_day_xpm_preview(render_polygon_thumb_xpm(day_xpm, EDITOR_PREVIEW_SIZE, false, contour));
    w.set_ep_night_xpm_preview(render_polygon_thumb_xpm(night_xpm.or(day_xpm), EDITOR_PREVIEW_SIZE, true, contour));
}

fn ep_rebuild_preview(w: &AppWindow) {
    let day_xpm = typ::text_reader::parse_xpm_lines(w.get_ep_xpm_text().as_str())
        .ok().flatten()
        .unwrap_or_else(|| solid_xpm(slint_to_rgb(w.get_ep_day_fill_color())));
    let night_xpm = typ::text_reader::parse_xpm_lines(w.get_ep_night_xpm_text().as_str())
        .ok().flatten()
        .unwrap_or_else(|| solid_xpm(slint_to_rgb(w.get_ep_night_fill_color())));
    push_ep_previews(w, Some(&day_xpm), Some(&night_xpm));
}

fn push_el_previews(w: &AppWindow, day_xpm: Option<&Xpm>, night_xpm: Option<&Xpm>, line_width: u8, border_width: u8) {
    let total_lw = line_width.saturating_add(2u8.saturating_mul(border_width));
    w.set_el_day_xpm_preview(render_line_xpm_preview(day_xpm, total_lw, false, EDITOR_PREVIEW_SIZE));
    w.set_el_night_xpm_preview(render_line_xpm_preview(night_xpm.or(day_xpm), total_lw, true, EDITOR_PREVIEW_SIZE));
}

fn el_rebuild_preview(w: &AppWindow, lw: u8, bw: u8) {
    let total_lw = lw.saturating_add(2u8.saturating_mul(bw));
    let day_xpm = typ::text_reader::parse_xpm_lines(w.get_el_xpm_text().as_str())
        .ok().flatten()
        .unwrap_or_else(|| solid_xpm(slint_to_rgb(w.get_el_day_color())));
    let night_xpm = typ::text_reader::parse_xpm_lines(w.get_el_night_xpm_text().as_str())
        .ok().flatten();
    w.set_el_day_xpm_preview(render_line_xpm_preview(Some(&day_xpm), total_lw, false, EDITOR_PREVIEW_SIZE));
    w.set_el_night_xpm_preview(render_line_xpm_preview(night_xpm.as_ref().or(Some(&day_xpm)), total_lw, true, EDITOR_PREVIEW_SIZE));
}

fn ensure_transparent(xpm: &mut Xpm) -> usize {
    if let Some(idx) = xpm.palette.iter().position(|(_, c)| c.is_transparent()) {
        return idx;
    }
    xpm.palette.push(("T".to_string(), Rgba::transparent()));
    xpm.palette.len() - 1
}

fn flood_fill(xpm: &mut Xpm, px: u32, py: u32, new_idx: usize) {
    let w = xpm.width as usize;
    let h = xpm.height as usize;
    if px as usize >= w || py as usize >= h { return; }
    let old_idx = xpm.pixels[py as usize][px as usize];
    if old_idx == new_idx { return; }
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((px as usize, py as usize));
    while let Some((x, y)) = queue.pop_front() {
        if xpm.pixels[y][x] != old_idx { continue; }
        xpm.pixels[y][x] = new_idx;
        if x > 0     { queue.push_back((x - 1, y)); }
        if x + 1 < w { queue.push_back((x + 1, y)); }
        if y > 0     { queue.push_back((x, y - 1)); }
        if y + 1 < h { queue.push_back((x, y + 1)); }
    }
}

fn draw_line(xpm: &mut Xpm, x0: u32, y0: u32, x1: u32, y1: u32, idx: usize) {
    let (mut x0, mut y0, x1, y1) = (x0 as i32, y0 as i32, x1 as i32, y1 as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as usize) < xpm.width as usize && (y0 as usize) < xpm.height as usize {
            xpm.pixels[y0 as usize][x0 as usize] = idx;
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}

fn paint_brush(xpm: &mut Xpm, px: u32, py: u32, brush: u32, idx: usize) {
    let half = (brush / 2) as i32;
    for dy in -half..=(brush as i32 - 1 - half) {
        for dx in -half..=(brush as i32 - 1 - half) {
            let x = px as i32 + dx;
            let y = py as i32 + dy;
            if x >= 0 && y >= 0 && (x as usize) < xpm.width as usize && (y as usize) < xpm.height as usize {
                xpm.pixels[y as usize][x as usize] = idx;
            }
        }
    }
}

fn open_poi_editor(
    doc: &TypDocument,
    kind: i32,
    idx: usize,
    window: &AppWindow,
) -> Option<PoiEditorState> {
    match kind {
        2 => doc.points.get(idx).map(|p| {
            let state = PoiEditorState {
                doc_idx: idx,
                day_xpm: p.day_xpm.clone(),
                night_xpm: p.night_xpm.clone(),
                editing_night: false,
                zoom: 12,
                tool: 0,
                brush_size: 1,
                active_color_idx: 0,
                line_start: None,
                has_night_bmp: p.night_xpm.is_some(),
                extended_labels: p.extended_labels,
                font_style: p.font_style,
                day_font_colour: p.day_font_colour,
                night_font_colour: p.night_font_colour,
                labels: p.labels.clone(),
            };
            push_poi_state_to_window(&state, window, p.type_code, p.sub_type);
            window.set_poi_grmn_type_text(p.grmn_type.as_str().into());
            state
        }),
        _ => None,
    }
}

fn push_poi_state_to_window(state: &PoiEditorState, window: &AppWindow, type_code: u16, sub_type: u8) {
    window.set_poi_type_code_text(format!("0x{:02X}", type_code).into());
    window.set_poi_sub_type_text(format!("0x{:02X}", sub_type).into());
    window.set_poi_editing_night(state.editing_night);
    window.set_poi_has_night_bmp(state.has_night_bmp);
    window.set_poi_extended_labels(state.extended_labels);
    window.set_poi_font_style_idx(font_style_to_int(state.font_style));
    let day_fc_str = state.day_font_colour
        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
        .unwrap_or_else(|| "#000000".to_string());
    let night_fc_str = state.night_font_colour
        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
        .unwrap_or_else(|| "#000000".to_string());
    window.set_poi_font_color_day_text(day_fc_str.clone().into());
    window.set_poi_font_color_night_text(night_fc_str.clone().into());
    window.set_poi_font_color_day(hex_to_slint_color(&day_fc_str));
    window.set_poi_font_color_night(hex_to_slint_color(&night_fc_str));
    window.set_poi_new_color_preview(slint::Color::from_rgb_u8(0, 0, 0));
    window.set_poi_lang_labels(build_lang_entries(&state.labels));

    let active_xpm = if state.editing_night { state.night_xpm.as_ref() } else { state.day_xpm.as_ref() };

    if let Some(xpm) = active_xpm {
        window.set_poi_bmp_width(xpm.width as i32);
        window.set_poi_bmp_height(xpm.height as i32);
        window.set_poi_colour_count(xpm.palette.len() as i32);
        window.set_poi_palette(build_palette_entries(xpm));
        window.set_poi_resize_width_text(xpm.width.to_string().into());
        window.set_poi_resize_height_text(xpm.height.to_string().into());
        window.set_poi_grid_image(render_grid(xpm, state.zoom));
    } else {
        let empty = Xpm::new(16, 16, ColorMode::Indexed);
        window.set_poi_bmp_width(16);
        window.set_poi_bmp_height(16);
        window.set_poi_colour_count(0);
        window.set_poi_palette(slint::ModelRc::default());
        window.set_poi_grid_image(render_grid(&empty, state.zoom));
    }

    window.set_poi_preview_day(render_poi_xpm_preview(state.day_xpm.as_ref(), 80, false));
    window.set_poi_preview_night(render_poi_xpm_preview(state.night_xpm.as_ref(), 80, true));
    window.set_poi_zoom(state.zoom as i32);
    window.set_poi_active_tool(state.tool);
    window.set_poi_brush_size(state.brush_size as i32);
    window.set_poi_active_palette_idx(state.active_color_idx as i32);
    window.set_poi_editor_error("".into());
    window.set_poi_editor_visible(true);
}

fn refresh_poi_grid(state: &PoiEditorState, window: &AppWindow) {
    let active_xpm = if state.editing_night { state.night_xpm.as_ref() } else { state.day_xpm.as_ref() };
    if let Some(xpm) = active_xpm {
        window.set_poi_colour_count(xpm.palette.len() as i32);
        window.set_poi_palette(build_palette_entries(xpm));
        window.set_poi_grid_image(render_grid(xpm, state.zoom));
    }
    window.set_poi_preview_day(render_poi_xpm_preview(state.day_xpm.as_ref(), 80, false));
    window.set_poi_preview_night(render_poi_xpm_preview(state.night_xpm.as_ref(), 80, true));
}

// ─── DrawMask editor ─────────────────────────────────────────────

struct DrawMaskState {
    doc_idx: usize,
    #[allow(dead_code)]
    kind: i32,
    day_pixels: Vec<Vec<u8>>,
    night_pixels: Vec<Vec<u8>>,
    has_night: bool,
    day_fg: Rgb,
    day_bg: Option<Rgb>,
    night_fg: Rgb,
    night_bg: Option<Rgb>,
    zoom: u32,
    editing_night: bool,
    tool: i32,
    wide_tool: bool,
}

fn slint_to_rgb(c: slint::Color) -> Rgb {
    Rgb { r: c.red(), g: c.green(), b: c.blue() }
}

fn xpm_to_dm_pixels(xpm: &Xpm) -> Vec<Vec<u8>> {
    let bg_idx = xpm.palette.iter().position(|(_, c)| c.is_transparent()).unwrap_or(0);
    xpm.pixels.iter().map(|row| {
        row.iter().map(|&idx| if idx == bg_idx { 0u8 } else { 1u8 }).collect()
    }).collect()
}

fn dm_pixels_to_xpm(pixels: &[Vec<u8>], fg: Rgb, bg: Option<Rgb>) -> Xpm {
    let height = pixels.len();
    let width = if height > 0 { pixels[0].len() } else { 32 };
    let bg_color = bg.map_or(Rgba::transparent(), |c| Rgba::opaque(c.r, c.g, c.b));
    Xpm {
        width: width as u16,
        height: height as u16,
        colour_mode: ColorMode::Indexed,
        palette: vec![
            (".".to_string(), bg_color),
            ("a".to_string(), Rgba::opaque(fg.r, fg.g, fg.b)),
        ],
        pixels: pixels.iter().map(|row| row.iter().map(|&b| b as usize).collect()).collect(),
    }
}

fn render_draw_mask_grid(pixels: &[Vec<u8>], fg: Rgb, bg: Option<Rgb>, zoom: u32) -> slint::Image {
    let rows = pixels.len().max(1);
    let cols = 32usize;
    let stride = zoom + 1;
    let w = cols as u32 * stride + 1;
    let h = rows as u32 * stride + 1;
    let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
    let px = buf.make_mut_slice();
    for p in px.iter_mut() { *p = Rgb8Pixel { r: 0xcc, g: 0xcc, b: 0xcc }; }
    for (ri, row) in pixels.iter().enumerate() {
        for (ci, &bit) in row.iter().enumerate() {
            let cx = ci as u32 * stride + 1;
            let cy = ri as u32 * stride + 1;
            for dy in 0..zoom {
                for dx in 0..zoom {
                    let (r, g, b) = if bit == 1 {
                        (fg.r, fg.g, fg.b)
                    } else {
                        match bg {
                            Some(c) => (c.r, c.g, c.b),
                            None => {
                                let v = if (dx + dy) % 2 == 0 { 0xcc_u8 } else { 0xff_u8 };
                                (v, v, v)
                            }
                        }
                    };
                    let idx = ((cy + dy) * w + (cx + dx)) as usize;
                    if idx < px.len() { px[idx] = Rgb8Pixel { r, g, b }; }
                }
            }
        }
    }
    slint::Image::from_rgb8(buf)
}

fn dm_flood_fill(pixels: &mut Vec<Vec<u8>>, col: usize, row: usize, value: u8) {
    if row >= pixels.len() || col >= 32 { return; }
    let target = pixels[row][col];
    if target == value { return; }
    let rows = pixels.len();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((col, row));
    while let Some((c, r)) = queue.pop_front() {
        if r >= rows || c >= 32 { continue; }
        if pixels[r][c] != target { continue; }
        pixels[r][c] = value;
        if c > 0 { queue.push_back((c - 1, r)); }
        if c + 1 < 32 { queue.push_back((c + 1, r)); }
        if r > 0 { queue.push_back((c, r - 1)); }
        if r + 1 < rows { queue.push_back((c, r + 1)); }
    }
}

fn xpm_fg(xpm: &Xpm, default: Rgb) -> Rgb {
    if xpm.palette.iter().any(|(_, c)| c.is_transparent()) {
        // Présence d'une entrée transparente : le premier opaque est l'avant-plan
        xpm.palette.iter().find(|(_, c)| !c.is_transparent())
            .map(|(_, c)| Rgb { r: c.r, g: c.g, b: c.b }).unwrap_or(default)
    } else {
        // Tout opaque : convention TYP — palette[1] = avant-plan
        xpm.palette.get(1).map(|(_, c)| Rgb { r: c.r, g: c.g, b: c.b }).unwrap_or(default)
    }
}

fn xpm_bg(xpm: &Xpm) -> Option<Rgb> {
    if xpm.palette.iter().any(|(_, c)| c.is_transparent()) {
        None
    } else {
        // Tout opaque : convention TYP — palette[0] = arrière-plan
        xpm.palette.get(0).map(|(_, c)| Rgb { r: c.r, g: c.g, b: c.b })
    }
}

fn init_dm_ep(p: &typ::TypPolygon) -> DrawMaskState {
    let (day_pixels, day_fg, day_bg) = match &p.day_xpm {
        Some(xpm) if xpm.width == 32 => {
            (xpm_to_dm_pixels(xpm), xpm_fg(xpm, Rgb { r: 0, g: 0, b: 0 }), xpm_bg(xpm))
        }
        _ => {
            let fg = first_opaque(p.day_xpm.as_ref())
                .map(|(r, g, b)| Rgb { r, g, b }).unwrap_or(Rgb { r: 0, g: 0, b: 0 });
            (vec![vec![0u8; 32]; 32], fg, None)
        }
    };
    let (night_pixels, night_fg, night_bg, has_night) = match &p.night_xpm {
        Some(xpm) if xpm.width == 32 => {
            (xpm_to_dm_pixels(xpm), xpm_fg(xpm, Rgb { r: 0xff, g: 0xff, b: 0xff }), xpm_bg(xpm), true)
        }
        _ => (day_pixels.clone(), day_fg, day_bg, false),
    };
    DrawMaskState {
        doc_idx: 0, kind: 0,
        day_pixels, night_pixels, has_night,
        day_fg, day_bg, night_fg, night_bg,
        zoom: 8, editing_night: false, tool: 0, wide_tool: false,
    }
}

fn init_dm_el(l: &typ::TypLine) -> DrawMaskState {
    let (day_pixels, day_fg, day_bg) = match &l.day_xpm {
        Some(xpm) if xpm.width == 32 => {
            (xpm_to_dm_pixels(xpm), xpm_fg(xpm, Rgb { r: 0, g: 0, b: 0 }), xpm_bg(xpm))
        }
        _ => {
            let fg = first_opaque(l.day_xpm.as_ref())
                .map(|(r, g, b)| Rgb { r, g, b }).unwrap_or(Rgb { r: 0, g: 0, b: 0 });
            (vec![vec![0u8; 32]; 1], fg, None)
        }
    };
    let (night_pixels, night_fg, night_bg, has_night) = match &l.night_xpm {
        Some(xpm) if xpm.width == 32 => {
            (xpm_to_dm_pixels(xpm), xpm_fg(xpm, Rgb { r: 0xff, g: 0xff, b: 0xff }), xpm_bg(xpm), true)
        }
        _ => (day_pixels.clone(), day_fg, day_bg, false),
    };
    DrawMaskState {
        doc_idx: 0, kind: 1,
        day_pixels, night_pixels, has_night,
        day_fg, day_bg, night_fg, night_bg,
        zoom: 8, editing_night: false, tool: 0, wide_tool: true,
    }
}

fn push_dm_to_window_ep(state: &DrawMaskState, w: &AppWindow) {
    w.set_ep_dm_grid_image(render_draw_mask_grid(&state.day_pixels, state.day_fg, state.day_bg, state.zoom));
    w.set_ep_dm_zoom(state.zoom as i32);
    w.set_ep_dm_num_rows(state.day_pixels.len() as i32);
    w.set_ep_dm_tool(state.tool);
    w.set_ep_dm_fg_color(slint::Color::from_rgb_u8(state.day_fg.r, state.day_fg.g, state.day_fg.b));
    w.set_ep_dm_bg_color(state.day_bg.map_or(
        slint::Color::from_rgb_u8(0xff, 0xff, 0xff),
        |c| slint::Color::from_rgb_u8(c.r, c.g, c.b)));
    w.set_ep_dm_bg_is_clear(state.day_bg.is_none());
    w.set_ep_dm_has_night(state.has_night);
    w.set_ep_dm_editing_night(false);
    w.set_ep_dm_night_fg_color(slint::Color::from_rgb_u8(state.night_fg.r, state.night_fg.g, state.night_fg.b));
    w.set_ep_dm_night_bg_color(state.night_bg.map_or(
        slint::Color::from_rgb_u8(0, 0, 0),
        |c| slint::Color::from_rgb_u8(c.r, c.g, c.b)));
    w.set_ep_dm_night_bg_is_clear(state.night_bg.is_none());
}

fn push_dm_to_window_el(state: &DrawMaskState, w: &AppWindow) {
    w.set_el_dm_grid_image(render_draw_mask_grid(&state.day_pixels, state.day_fg, state.day_bg, state.zoom));
    w.set_el_dm_zoom(state.zoom as i32);
    w.set_el_dm_num_rows(state.day_pixels.len() as i32);
    w.set_el_dm_tool(state.tool);
    w.set_el_dm_wide_tool(state.wide_tool);
    w.set_el_dm_fg_color(slint::Color::from_rgb_u8(state.day_fg.r, state.day_fg.g, state.day_fg.b));
    w.set_el_dm_bg_color(state.day_bg.map_or(
        slint::Color::from_rgb_u8(0xff, 0xff, 0xff),
        |c| slint::Color::from_rgb_u8(c.r, c.g, c.b)));
    w.set_el_dm_bg_is_clear(state.day_bg.is_none());
    w.set_el_dm_has_night(state.has_night);
    w.set_el_dm_editing_night(false);
    w.set_el_dm_night_fg_color(slint::Color::from_rgb_u8(state.night_fg.r, state.night_fg.g, state.night_fg.b));
    w.set_el_dm_night_bg_color(state.night_bg.map_or(
        slint::Color::from_rgb_u8(0, 0, 0),
        |c| slint::Color::from_rgb_u8(c.r, c.g, c.b)));
    w.set_el_dm_night_bg_is_clear(state.night_bg.is_none());
}

fn ep_dm_render_from_window(state: &DrawMaskState, w: &AppWindow) -> slint::Image {
    let night = w.get_ep_dm_editing_night();
    let pixels = if night { &state.night_pixels } else { &state.day_pixels };
    let fg = if night { slint_to_rgb(w.get_ep_dm_night_fg_color()) } else { slint_to_rgb(w.get_ep_dm_fg_color()) };
    let bg = if night {
        if w.get_ep_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_night_bg_color())) }
    } else {
        if w.get_ep_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_bg_color())) }
    };
    render_draw_mask_grid(pixels, fg, bg, w.get_ep_dm_zoom() as u32)
}

fn el_dm_render_from_window(state: &DrawMaskState, w: &AppWindow) -> slint::Image {
    let night = w.get_el_dm_editing_night();
    let pixels = if night { &state.night_pixels } else { &state.day_pixels };
    let fg = if night { slint_to_rgb(w.get_el_dm_night_fg_color()) } else { slint_to_rgb(w.get_el_dm_fg_color()) };
    let bg = if night {
        if w.get_el_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_night_bg_color())) }
    } else {
        if w.get_el_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_bg_color())) }
    };
    render_draw_mask_grid(pixels, fg, bg, w.get_el_dm_zoom() as u32)
}

// ─── Helpers édition TXT ─────────────────────────────────────────

fn apply_txt_edit(doc: &mut typ::TypDocument, kind: i32, idx: usize, txt: &str) -> Result<(), String> {
    let full = format!("[_id]\nFID=1\nProductCode=1\nCodePage=1252\n[end]\n\n{}\n", txt);
    let parsed = crate::typ::text_reader::parse(full.as_bytes()).map_err(|e| e.to_string())?;
    match kind {
        0 => {
            let elem = parsed.polygons.into_iter().next()
                .ok_or_else(|| "Aucun polygone trouvé dans le TXT édité".to_string())?;
            *doc.polygons.get_mut(idx).ok_or("Index invalide")? = elem;
        }
        1 => {
            let elem = parsed.lines.into_iter().next()
                .ok_or_else(|| "Aucune ligne trouvée dans le TXT édité".to_string())?;
            *doc.lines.get_mut(idx).ok_or("Index invalide")? = elem;
        }
        2 => {
            let elem = parsed.points.into_iter().next()
                .ok_or_else(|| "Aucun POI trouvé dans le TXT édité".to_string())?;
            *doc.points.get_mut(idx).ok_or("Index invalide")? = elem;
        }
        _ => return Err("Type d'élément inconnu".to_string()),
    }
    Ok(())
}

// ─── main ────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Configurer le backend winit avec l'icône de fenêtre (Linux/Windows).
    // Sur macOS, winit::window::Icon est ignoré — l'icône Slint dans app_window.slint couvre ce cas.
    if let Some(icon) = load_window_icon() {
        if let Err(e) = setup_platform_with_icon(icon) {
            eprintln!("typforge: impossible de configurer le backend avec l'icône: {}", e);
        }
    }

    let window = AppWindow::new()?;
    let app = Rc::new(RefCell::new(App::new()));
    let poi_state: Rc<RefCell<Option<PoiEditorState>>> = Rc::new(RefCell::new(None));
    let dm_state: Rc<RefCell<Option<DrawMaskState>>> = Rc::new(RefCell::new(None));

    if let Some(path_str) = args.get(1) {
        let path = std::path::Path::new(path_str);
        let mut a = app.borrow_mut();
        let res = if path.extension().map_or(false, |e| e == "typ") {
            a.import_typ(path)
        } else {
            a.open_txt(path)
        };
        match res {
            Ok(()) => {
                if let Some(doc) = &a.doc {
                    update_ui_from_doc(doc, &window);
                }
            }
            Err(e) => eprintln!("typforge: impossible d'ouvrir {:?}: {}", path, e),
        }
    }

    // on_open_file
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_open_file(move || {
            // Vérification modifications non enregistrées
            let (is_dirty, current_path) = {
                let a = app_c.borrow();
                (a.dirty, a.current_file_path.clone())
            };
            if is_dirty {
                let result = rfd::MessageDialog::new()
                    .set_title("Modifications non enregistrées")
                    .set_description("Le fichier courant a été modifié. Voulez-vous l'enregistrer avant d'ouvrir un autre fichier ?")
                    .set_buttons(rfd::MessageButtons::YesNoCancel)
                    .show();
                match result {
                    rfd::MessageDialogResult::Cancel => return,
                    rfd::MessageDialogResult::Yes => {
                        let save_path = if let Some(p) = current_path {
                            Some(p)
                        } else {
                            rfd::FileDialog::new()
                                .add_filter("TYP texte", &["txt"])
                                .set_title("Enregistrer le fichier TYP")
                                .save_file()
                        };
                        match save_path {
                            Some(p) => {
                                if let Err(e) = app_c.borrow_mut().save_txt(&p) {
                                    eprintln!("typforge: erreur sauvegarde: {}", e);
                                    return;
                                }
                            }
                            None => {
                                eprintln!("typforge: sauvegarde annulée — ouverture annulée");
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            }

            let picked = rfd::FileDialog::new()
                .add_filter("Fichiers TYP", &["txt", "typ"])
                .set_title("Ouvrir un fichier TYP")
                .pick_file();
            if let Some(p) = picked {
                let mut a = app_c.borrow_mut();
                let res = if p.extension().map_or(false, |e| e == "typ") {
                    a.import_typ(&p)
                } else {
                    a.open_txt(&p)
                };
                match res {
                    Ok(()) => {
                        if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                            update_ui_from_doc(doc, &w);
                        }
                    }
                    Err(e) => eprintln!("typforge: erreur ouverture: {}", e),
                }
            }
        });
    }

    // on_save_file — sauvegarde dans le fichier courant, sinon dialogue
    {
        let app_c = Rc::clone(&app);
        window.on_save_file(move || {
            let current_path = app_c.borrow().current_file_path.clone();
            let save_path = if let Some(p) = current_path {
                Some(p)
            } else {
                rfd::FileDialog::new()
                    .add_filter("TYP texte", &["txt"])
                    .set_title("Enregistrer le fichier TYP")
                    .save_file()
            };
            if let Some(p) = save_path {
                if let Err(e) = app_c.borrow_mut().save_txt(&p) {
                    eprintln!("typforge: erreur sauvegarde: {}", e);
                }
            }
        });
    }

    // on_save_file_as — dialogue systématique, pré-positionné sur le répertoire courant
    {
        let app_c = Rc::clone(&app);
        window.on_save_file_as(move || {
            let start_dir = app_c.borrow().current_file_path.as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            let mut dialog = rfd::FileDialog::new()
                .add_filter("TYP texte", &["txt"])
                .set_title("Enregistrer sous…");
            if let Some(dir) = start_dir {
                dialog = dialog.set_directory(dir);
            }
            if let Some(p) = dialog.save_file() {
                if let Err(e) = app_c.borrow_mut().save_txt(&p) {
                    eprintln!("typforge: erreur sauvegarde: {}", e);
                }
            }
        });
    }

    // on_export_typ
    {
        let app_c = Rc::clone(&app);
        window.on_export_typ(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("TYP binaire", &["typ"])
                .set_title("Exporter en .typ binaire")
                .save_file();
            if let Some(p) = picked {
                if let Err(e) = app_c.borrow().export_typ(&p) {
                    eprintln!("typforge: erreur export TYP: {}", e);
                }
            }
        });
    }

    // on_import_qml
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_import_qml(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Style QGIS", &["qml"])
                .set_title("Importer un style QGIS (.qml)")
                .pick_file();
            if let Some(p) = picked {
                let bytes = match std::fs::read(&p) {
                    Ok(b) => b,
                    Err(e) => { eprintln!("typforge: impossible de lire {:?}: {}", p, e); return; }
                };
                let imported = match qml_import::parse(&bytes) {
                    Ok(i) => i,
                    Err(e) => { eprintln!("typforge: erreur import QML: {}", e); return; }
                };
                let mut a = app_c.borrow_mut();
                if a.doc.is_none() {
                    eprintln!("typforge: avertissement — aucun fichier TYP ouvert, création d'un document vide");
                }
                let doc = a.doc.get_or_insert_with(TypDocument::default);
                // Offsets indépendants par namespace TYP (polygones/lignes/points non partagés)
                let max_poly  = doc.polygons.iter().map(|p| p.type_code).max().unwrap_or(0);
                let max_line  = doc.lines.iter().map(|l| l.type_code).max().unwrap_or(0);
                let max_point = doc.points.iter().map(|p| p.type_code).max().unwrap_or(0);
                let mut polys = imported.polygons;
                let mut lns   = imported.lines;
                let mut pts   = imported.points;
                for p in &mut polys { p.type_code = p.type_code.saturating_add(max_poly); }
                for l in &mut lns   { l.type_code = l.type_code.saturating_add(max_line); }
                for p in &mut pts   { p.type_code = p.type_code.saturating_add(max_point); }
                doc.polygons.extend(polys);
                doc.lines.extend(lns);
                doc.points.extend(pts);
                a.dirty = true;
                if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                    update_ui_from_doc(doc, &w);
                }
            }
        });
    }

    // on_quit
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_quit(move || {
            let (is_dirty, current_path) = {
                let a = app_c.borrow();
                (a.dirty, a.current_file_path.clone())
            };
            if is_dirty {
                let result = rfd::MessageDialog::new()
                    .set_title("Modifications non enregistrées")
                    .set_description("Le fichier courant a été modifié. Voulez-vous l'enregistrer avant de quitter ?")
                    .set_buttons(rfd::MessageButtons::YesNoCancel)
                    .show();
                match result {
                    rfd::MessageDialogResult::Cancel => return,
                    rfd::MessageDialogResult::Yes => {
                        let save_path = if let Some(p) = current_path {
                            Some(p)
                        } else {
                            rfd::FileDialog::new()
                                .add_filter("TYP texte", &["txt"])
                                .set_title("Enregistrer le fichier TYP")
                                .save_file()
                        };
                        match save_path {
                            Some(p) => {
                                if let Err(e) = app_c.borrow_mut().save_txt(&p) {
                                    eprintln!("typforge: erreur sauvegarde: {}", e);
                                    return;
                                }
                            }
                            None => {
                                eprintln!("typforge: sauvegarde annulée — fermeture annulée");
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(w) = ww.upgrade() {
                w.hide().ok();
            }
        });
    }

    // on_nav_tab_changed
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_nav_tab_changed(move |tab| {
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                let current_kind = w.get_selected_kind();
                if current_kind >= 0 && current_kind != tab {
                    w.set_selected_kind(-1);
                    w.set_selected_idx(-1);
                    w.set_selected_txt_code("".into());
                    w.set_txt_edit_mode(false);
                    w.set_txt_edit_buffer("".into());
                    w.set_txt_edit_error("".into());
                }
                rebuild_gallery(doc, &w, tab);
            }
        });
    }

    // on_element_selected (liste gauche)
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_element_selected(move |kind, idx| {
            if idx < 0 { return; }
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                w.set_selected_kind(kind);
                w.set_selected_idx(idx);
                let mode = w.get_preview_mode();
                let (day, night) = render_preview_with_mode(doc, kind, idx as usize, mode);
                w.set_preview_day(day);
                w.set_preview_night(night);
                let txt = crate::typ::text_writer::element_to_display_txt(doc, kind, idx as usize);
                w.set_selected_txt_code(txt.into());
                w.set_txt_edit_mode(false);
                w.set_txt_edit_buffer("".into());
                w.set_txt_edit_error("".into());
            }
        });
    }

    // on_gallery_item_selected
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_gallery_item_selected(move |kind, idx| {
            if idx < 0 { return; }
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                w.set_selected_kind(kind);
                w.set_selected_idx(idx);
                let mode = w.get_preview_mode();
                let (day, night) = render_preview_with_mode(doc, kind, idx as usize, mode);
                w.set_preview_day(day);
                w.set_preview_night(night);
                let txt = crate::typ::text_writer::element_to_display_txt(doc, kind, idx as usize);
                w.set_selected_txt_code(txt.into());
                w.set_txt_edit_mode(false);
                w.set_txt_edit_buffer("".into());
                w.set_txt_edit_error("".into());
            }
        });
    }

    // on_preview_mode_changed — re-rend la preview avec le nouveau mode
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_preview_mode_changed(move |mode| {
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                let kind = w.get_selected_kind();
                let idx = w.get_selected_idx();
                if kind >= 0 && idx >= 0 {
                    let (day, night) = render_preview_with_mode(doc, kind, idx as usize, mode);
                    w.set_preview_day(day);
                    w.set_preview_night(night);
                }
            }
        });
    }

    // on_txt_edit_apply — applique un bloc TXT édité à la main
    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_txt_edit_apply(move |text| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let kind = w.get_selected_kind();
            let idx  = w.get_selected_idx();
            if kind < 0 || idx < 0 { return; }
            let result = {
                let mut a = app_c.borrow_mut();
                if let Some(doc) = &mut a.doc {
                    let r = apply_txt_edit(doc, kind, idx as usize, text.as_str());
                    if r.is_ok() { a.dirty = true; }
                    r
                } else {
                    Err("Aucun document ouvert".to_string())
                }
            };
            match result {
                Ok(()) => {
                    w.set_txt_edit_error("".into());
                    w.set_txt_edit_mode(false);
                    let a = app_c.borrow();
                    if let Some(doc) = &a.doc {
                        let new_txt = crate::typ::text_writer::element_to_display_txt(doc, kind, idx as usize);
                        w.set_selected_txt_code(new_txt.into());
                        let mode = w.get_preview_mode();
                        let (day, night) = render_preview_with_mode(doc, kind, idx as usize, mode);
                        w.set_preview_day(day);
                        w.set_preview_night(night);
                        let nav = w.get_active_nav_tab();
                        rebuild_gallery(doc, &w, nav);
                    }
                }
                Err(e) => {
                    w.set_txt_edit_error(e.into());
                }
            }
        });
    }

    // on_txt_edit_cancel
    {
        let ww = window.as_weak();
        window.on_txt_edit_cancel(move || {
            if let Some(w) = ww.upgrade() {
                w.set_txt_edit_mode(false);
                w.set_txt_edit_error("".into());
            }
        });
    }

    // on_editor_apply — applique les modifications au TypDocument
    {
        let app_c = Rc::clone(&app);
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_editor_apply(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };

            // F9 — guard idx négatif
            if w.get_editor_idx() < 0 {
                w.set_editor_visible(false);
                return;
            }

            w.set_editor_error("".into());
            let kind = w.get_editor_kind();
            let idx = w.get_editor_idx() as usize;
            let mut had_error = false;

            {
                let mut a = app_c.borrow_mut();
                if let Some(doc) = &mut a.doc {
                    match kind {
                        0 => {
                            // Valider TypeCode/SubType
                            let tc_str = w.get_ep_type_code_text();
                            let st_str = w.get_ep_sub_type_text();
                            let new_tc = match parse_type_code(tc_str.as_str()) {
                                Some(v) => v,
                                None => {
                                    w.set_editor_error("TypeCode invalide (ex: 0x01 ou 1)".into());
                                    had_error = true;
                                    0
                                }
                            };
                            let new_st = if !had_error {
                                match parse_sub_type(st_str.as_str()) {
                                    Some(v) => v,
                                    None => {
                                        w.set_editor_error("SubType invalide (ex: 0x00 ou 0)".into());
                                        had_error = true;
                                        0
                                    }
                                }
                            } else { 0 };
                            // Vérifier doublon
                            if !had_error {
                                let dup = doc.polygons.iter().enumerate()
                                    .any(|(i, p)| i != idx && p.type_code == new_tc && p.sub_type == new_st);
                                if dup {
                                    w.set_editor_error(format!("Un polygone 0x{:02X}/0x{:02X} existe déjà", new_tc, new_st).into());
                                    had_error = true;
                                }
                            }
                            // Collecter XPM jour/nuit dans des locaux — écriture atomique ensuite
                            if let Some(p) = doc.polygons.get_mut(idx) {
                                let mut new_day_xpm = p.day_xpm.clone();
                                if w.get_ep_is_pattern() {
                                    let dm_borrow = dm_c.borrow();
                                    if let Some(dm) = dm_borrow.as_ref() {
                                        let fg = slint_to_rgb(w.get_ep_dm_fg_color());
                                        let bg = if w.get_ep_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_bg_color())) };
                                        new_day_xpm = Some(dm_pixels_to_xpm(&dm.day_pixels, fg, bg));
                                    }
                                } else {
                                    let xpm_text = w.get_ep_xpm_text();
                                    if !xpm_text.trim().is_empty() {
                                        match typ::text_reader::parse_xpm_lines(&xpm_text) {
                                            Ok(Some(xpm)) => new_day_xpm = Some(xpm),
                                            Ok(None) => {}
                                            Err(e) => {
                                                w.set_editor_error(format!("XPM jour : {}", e).into());
                                                had_error = true;
                                            }
                                        }
                                    } else if let Some(c) = hex_to_rgb(w.get_ep_day_fill_text().as_str()) {
                                        set_xpm_fill_color(&mut new_day_xpm, c);
                                    }
                                }
                                let mut new_night_xpm = p.night_xpm.clone();
                                if !had_error {
                                    if w.get_ep_is_pattern() {
                                        let dm_borrow = dm_c.borrow();
                                        if let Some(dm) = dm_borrow.as_ref() {
                                            if dm.has_night {
                                                let fg = slint_to_rgb(w.get_ep_dm_night_fg_color());
                                                let bg = if w.get_ep_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_night_bg_color())) };
                                                new_night_xpm = Some(dm_pixels_to_xpm(&dm.night_pixels, fg, bg));
                                            }
                                        }
                                    } else {
                                        let night_text = w.get_ep_night_xpm_text();
                                        if !night_text.trim().is_empty() {
                                            match typ::text_reader::parse_xpm_lines(&night_text) {
                                                Ok(Some(xpm)) => new_night_xpm = Some(xpm),
                                                Ok(None) => {}
                                                Err(e) => {
                                                    w.set_editor_error(format!("XPM nuit : {}", e).into());
                                                    had_error = true;
                                                }
                                            }
                                        } else if let Some(c) = hex_to_rgb(w.get_ep_night_fill_text().as_str()) {
                                            set_xpm_fill_color(&mut new_night_xpm, c);
                                        }
                                    }
                                }
                                // Écriture atomique : seulement si tout est valide
                                if !had_error {
                                    p.type_code = new_tc;
                                    p.sub_type = new_st;
                                    p.grmn_type = w.get_ep_grmn_type_text().to_string();
                                    p.day_xpm = new_day_xpm;
                                    p.night_xpm = new_night_xpm;
                                    p.extended_labels = w.get_ep_extended_labels();
                                    p.font_style = int_to_font_style(w.get_ep_font_style());
                                    p.contour_color = if w.get_ep_contour_enabled() {
                                        hex_to_rgb(w.get_ep_contour_text().as_str())
                                            .map(ContourColor::Solid)
                                            .unwrap_or(ContourColor::No)
                                    } else {
                                        ContourColor::No
                                    };
                                }
                            }
                        }
                        1 => {
                            // Valider TypeCode/SubType
                            let tc_str = w.get_el_type_code_text();
                            let st_str = w.get_el_sub_type_text();
                            let new_tc = match parse_type_code(tc_str.as_str()) {
                                Some(v) => v,
                                None => {
                                    w.set_editor_error("TypeCode invalide (ex: 0x01 ou 1)".into());
                                    had_error = true;
                                    0
                                }
                            };
                            let new_st = if !had_error {
                                match parse_sub_type(st_str.as_str()) {
                                    Some(v) => v,
                                    None => {
                                        w.set_editor_error("SubType invalide (ex: 0x00 ou 0)".into());
                                        had_error = true;
                                        0
                                    }
                                }
                            } else { 0 };
                            // Vérifier doublon
                            if !had_error {
                                let dup = doc.lines.iter().enumerate()
                                    .any(|(i, l)| i != idx && l.type_code == new_tc && l.sub_type == new_st);
                                if dup {
                                    w.set_editor_error(format!("Une ligne 0x{:02X}/0x{:02X} existe déjà", new_tc, new_st).into());
                                    had_error = true;
                                }
                            }
                            // Collecter XPM jour/nuit dans des locaux — écriture atomique ensuite
                            if let Some(l) = doc.lines.get_mut(idx) {
                                let mut new_day_xpm = l.day_xpm.clone();
                                if w.get_el_is_pattern() {
                                    let dm_borrow = dm_c.borrow();
                                    if let Some(dm) = dm_borrow.as_ref() {
                                        let fg = slint_to_rgb(w.get_el_dm_fg_color());
                                        let bg = if w.get_el_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_bg_color())) };
                                        new_day_xpm = Some(dm_pixels_to_xpm(&dm.day_pixels, fg, bg));
                                    }
                                } else {
                                    let xpm_text = w.get_el_xpm_text();
                                    if !xpm_text.trim().is_empty() {
                                        match typ::text_reader::parse_xpm_lines(&xpm_text) {
                                            Ok(Some(xpm)) => new_day_xpm = Some(xpm),
                                            Ok(None) => {}
                                            Err(e) => {
                                                w.set_editor_error(format!("XPM jour : {}", e).into());
                                                had_error = true;
                                            }
                                        }
                                    } else if let Some(c) = hex_to_rgb(w.get_el_day_text().as_str()) {
                                        set_xpm_fill_color(&mut new_day_xpm, c);
                                    }
                                }
                                let mut new_night_xpm = l.night_xpm.clone();
                                if !had_error {
                                    if w.get_el_is_pattern() {
                                        let dm_borrow = dm_c.borrow();
                                        if let Some(dm) = dm_borrow.as_ref() {
                                            if dm.has_night {
                                                let fg = slint_to_rgb(w.get_el_dm_night_fg_color());
                                                let bg = if w.get_el_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_night_bg_color())) };
                                                new_night_xpm = Some(dm_pixels_to_xpm(&dm.night_pixels, fg, bg));
                                            }
                                        }
                                    } else {
                                        let night_text = w.get_el_night_xpm_text();
                                        if !night_text.trim().is_empty() {
                                            match typ::text_reader::parse_xpm_lines(&night_text) {
                                                Ok(Some(xpm)) => new_night_xpm = Some(xpm),
                                                Ok(None) => {}
                                                Err(e) => {
                                                    w.set_editor_error(format!("XPM nuit : {}", e).into());
                                                    had_error = true;
                                                }
                                            }
                                        } else if let Some(c) = hex_to_rgb(w.get_el_night_text().as_str()) {
                                            set_xpm_fill_color(&mut new_night_xpm, c);
                                        }
                                    }
                                }
                                // Écriture atomique : seulement si tout est valide
                                if !had_error {
                                    l.type_code = new_tc;
                                    l.sub_type = new_st;
                                    l.grmn_type = w.get_el_grmn_type_text().to_string();
                                    l.day_xpm = new_day_xpm;
                                    l.night_xpm = new_night_xpm;
                                    l.line_width = w.get_el_line_width().clamp(0, 255) as u8;
                                    l.border_width = w.get_el_border_width().clamp(0, 255) as u8;
                                    l.use_orientation = w.get_el_use_orientation();
                                    l.extended_labels = w.get_el_extended_labels();
                                    l.font_style = int_to_font_style(w.get_el_font_style());
                                }
                            }
                        }
                        _ => {}
                    }
                    if !had_error {
                        // Mettre à jour uniquement la liste concernée (F9)
                        match kind {
                            0 => {
                                w.set_polygons(build_list_model(doc.polygons.iter().map(|p| (p.type_code, p.sub_type, p.grmn_type.clone()))));
                                if let Some(p) = doc.polygons.get(idx) {
                                    w.set_editor_title(format!("Polygone 0x{:02X}/0x{:02X}", p.type_code, p.sub_type).into());
                                }
                            }
                            1 => {
                                w.set_lines(build_list_model(doc.lines.iter().map(|l| (l.type_code, l.sub_type, l.grmn_type.clone()))));
                                if let Some(l) = doc.lines.get(idx) {
                                    w.set_editor_title(format!("Ligne 0x{:02X}/0x{:02X}", l.type_code, l.sub_type).into());
                                }
                            }
                            _ => {}
                        }
                        let nav = w.get_active_nav_tab();
                        rebuild_gallery(doc, &w, nav);
                        let (day, night) = render_preview(doc, kind, idx);
                        w.set_preview_day(day);
                        w.set_preview_night(night);
                        let new_txt = crate::typ::text_writer::element_to_display_txt(doc, kind, idx);
                        w.set_selected_txt_code(new_txt.into());
                    }
                }
            }

            if !had_error {
                app_c.borrow_mut().dirty = true;
                w.set_editor_visible(false);
            }
        });
    }

    // on_editor_cancel
    {
        let ww = window.as_weak();
        window.on_editor_cancel(move || {
            if let Some(w) = ww.upgrade() {
                w.set_editor_error("".into());
                w.set_editor_visible(false);
            }
        });
    }

    // on_edit_element — handler unique pour polygone, ligne, POI
    // NOTE : Slint ne permet qu'un seul handler par callback ; on écrase et réimplémente.
    {
        let app_c = Rc::clone(&app);
        let poi_c = Rc::clone(&poi_state);
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_edit_element(move |kind, idx| {
            if idx < 0 { return; }
            let a = app_c.borrow();
            if let (Some(doc), Some(w)) = (&a.doc, ww.upgrade()) {
                if kind == 2 {
                    // Ouvre le POI editor
                    if let Some(state) = open_poi_editor(doc, kind, idx as usize, &w) {
                        *poi_c.borrow_mut() = Some(state);
                    }
                    return;
                }
                // Polygone / Ligne — logique existante
                match kind {
                    0 => {
                        if let Some(p) = doc.polygons.get(idx as usize) {
                            let (day_text, day_color) = xpm_fill_color(p.day_xpm.as_ref());
                            let (night_text, night_color) = xpm_fill_color(p.night_xpm.as_ref());
                            w.set_editor_title(format!("Polygone 0x{:02X}/0x{:02X}", p.type_code, p.sub_type).into());
                            w.set_ep_type_code_text(format!("0x{:02X}", p.type_code).into());
                            w.set_ep_sub_type_text(format!("0x{:02X}", p.sub_type).into());
                            w.set_ep_grmn_type_text(p.grmn_type.as_str().into());
                            w.set_ep_day_fill_text(day_text);
                            w.set_ep_day_fill_color(day_color);
                            w.set_ep_night_fill_text(night_text);
                            w.set_ep_night_fill_color(night_color);
                            // XPM positionné avant contour pour que le changed-watcher
                            // ep-contour-enabled lise un état déjà cohérent
                            w.set_ep_xpm_text(xpm_to_text_opt(p.day_xpm.as_ref()));
                            w.set_ep_night_xpm_text(xpm_to_text_opt(p.night_xpm.as_ref()));
                            match p.contour_color {
                                ContourColor::No => {
                                    w.set_ep_contour_enabled(false);
                                    w.set_ep_contour_text("#000000".into());
                                    w.set_ep_contour_color(slint::Color::from_rgb_u8(0, 0, 0));
                                }
                                ContourColor::Solid(c) => {
                                    w.set_ep_contour_enabled(true);
                                    let hex = format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b);
                                    w.set_ep_contour_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                                    w.set_ep_contour_text(hex.into());
                                }
                            }
                            w.set_ep_extended_labels(p.extended_labels);
                            w.set_ep_font_style(font_style_to_int(p.font_style));
                            push_ep_previews(&w, p.day_xpm.as_ref(), p.night_xpm.as_ref());
                            let is_pattern = p.day_xpm.as_ref().map(|x| x.width == 32 && x.height == 32).unwrap_or(false);
                            w.set_ep_is_pattern(is_pattern);
                            let mut dm = init_dm_ep(p);
                            dm.doc_idx = idx as usize;
                            push_dm_to_window_ep(&dm, &w);
                            *dm_c.borrow_mut() = Some(dm);
                            w.set_editor_error("".into());
                            w.set_editor_kind(0);
                            w.set_editor_idx(idx);
                            w.set_editor_visible(true);
                        }
                    }
                    1 => {
                        if let Some(l) = doc.lines.get(idx as usize) {
                            let (day_text, day_color) = xpm_fill_color(l.day_xpm.as_ref());
                            let (night_text, night_color) = xpm_fill_color(l.night_xpm.as_ref());
                            w.set_editor_title(format!("Ligne 0x{:02X}/0x{:02X}", l.type_code, l.sub_type).into());
                            w.set_el_type_code_text(format!("0x{:02X}", l.type_code).into());
                            w.set_el_sub_type_text(format!("0x{:02X}", l.sub_type).into());
                            w.set_el_grmn_type_text(l.grmn_type.as_str().into());
                            w.set_el_day_text(day_text);
                            w.set_el_day_color(day_color);
                            w.set_el_night_text(night_text);
                            w.set_el_night_color(night_color);
                            w.set_el_line_width(l.line_width as i32);
                            w.set_el_border_width(l.border_width as i32);
                            w.set_el_use_orientation(l.use_orientation);
                            w.set_el_extended_labels(l.extended_labels);
                            w.set_el_font_style(font_style_to_int(l.font_style));
                            w.set_el_xpm_text(xpm_to_text_opt(l.day_xpm.as_ref()));
                            w.set_el_night_xpm_text(xpm_to_text_opt(l.night_xpm.as_ref()));
                            push_el_previews(&w, l.day_xpm.as_ref(), l.night_xpm.as_ref(), l.line_width, l.border_width);
                            let is_pattern = l.day_xpm.as_ref().map(|x| x.width == 32).unwrap_or(false);
                            w.set_el_is_pattern(is_pattern);
                            let mut dm = init_dm_el(l);
                            dm.doc_idx = idx as usize;
                            push_dm_to_window_el(&dm, &w);
                            *dm_c.borrow_mut() = Some(dm);
                            w.set_editor_error("".into());
                            w.set_editor_kind(1);
                            w.set_editor_idx(idx);
                            w.set_editor_visible(true);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    // ── Callbacks POI editor ──────────────────────────────────────

    // poi_grid_clicked
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_grid_clicked(move |fx, fy, btn| {
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };

            let zoom = state.zoom as f32 + 1.0;
            let px = (fx / zoom) as u32;
            let py = (fy / zoom) as u32;

            // Copier les champs nécessaires avant les emprunts mutables
            let tool = state.tool;
            let brush = state.brush_size;
            let color_idx = state.active_color_idx;
            let editing_night = state.editing_night;

            // Créer l'XPM si absent
            {
                let xpm_opt = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
                if xpm_opt.is_none() { *xpm_opt = Some(Xpm::new(16, 16, ColorMode::Indexed)); }
            }

            let xpm = (if editing_night { &mut state.night_xpm } else { &mut state.day_xpm }).as_mut().unwrap();

            if px >= xpm.width as u32 || py >= xpm.height as u32 { return; }

            match (tool, btn) {
                (0, 0) => {
                    paint_brush(xpm, px, py, brush, color_idx);
                }
                (1, _) | (0, 1) => {
                    let t_idx = ensure_transparent(xpm);
                    paint_brush(xpm, px, py, brush, t_idx);
                }
                (2, 0) => {
                    flood_fill(xpm, px, py, color_idx);
                }
                (3, 0) => {
                    if let Some(&picked) = xpm.pixels.get(py as usize).and_then(|r| r.get(px as usize)) {
                        state.active_color_idx = picked;
                        w.set_poi_active_palette_idx(picked as i32);
                    }
                    return;
                }
                (4, 0) => {
                    if let Some((x0, y0)) = state.line_start.take() {
                        draw_line(xpm, x0, y0, px, py, color_idx);
                    } else {
                        state.line_start = Some((px, py));
                        return;
                    }
                }
                _ => return,
            }

            let state_ref: &PoiEditorState = state_opt.as_ref().unwrap();
            refresh_poi_grid(state_ref, &w);
        });
    }

    // poi_palette_selected
    {
        let poi_c = Rc::clone(&poi_state);
        window.on_poi_palette_selected(move |idx| {
            if let Some(s) = poi_c.borrow_mut().as_mut() {
                s.active_color_idx = idx as usize;
            }
        });
    }

    // poi_palette_add
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_palette_add(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let hex_str = w.get_poi_new_color_hex().to_string();
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };

            let xpm_opt = state.active_xpm_mut();
            if xpm_opt.is_none() { *xpm_opt = Some(Xpm::new(16, 16, ColorMode::Indexed)); }
            let xpm = xpm_opt.as_mut().unwrap();

            if let Some(rgb) = hex_to_rgb(hex_str.trim()) {
                let tag = format!("{}", (b'!' + xpm.palette.len() as u8) as char);
                xpm.palette.push((tag, Rgba::opaque(rgb.r, rgb.g, rgb.b)));
                w.set_poi_palette(build_palette_entries(xpm));
                w.set_poi_colour_count(xpm.palette.len() as i32);
            }
        });
    }

    // poi_palette_remove
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_palette_remove(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };
            let mut active_idx = state.active_color_idx;
            let editing_night = state.editing_night;
            let xpm_opt = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
            if let Some(xpm) = xpm_opt {
                if active_idx < xpm.palette.len() {
                    xpm.palette.remove(active_idx);
                    for row in &mut xpm.pixels {
                        for px in row { if *px >= active_idx { *px = px.saturating_sub(1); } }
                    }
                    let new_len = xpm.palette.len();
                    if active_idx >= new_len && new_len > 0 { active_idx = new_len - 1; }
                    w.set_poi_palette(build_palette_entries(xpm));
                    w.set_poi_colour_count(new_len as i32);
                    w.set_poi_active_palette_idx(active_idx as i32);
                }
            }
            state.active_color_idx = active_idx;
        });
    }

    // poi_resize_bmp
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_resize_bmp(move |new_w, new_h| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let nw = (new_w as u16).max(1).min(64);
            let nh = (new_h as u16).max(1).min(64);
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };
            let zoom = state.zoom;
            let editing_night = state.editing_night;
            let xpm_field = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
            if xpm_field.is_none() { *xpm_field = Some(Xpm::new(nw, nh, ColorMode::Indexed)); }
            let xpm = xpm_field.as_mut().unwrap();
            let old_rows = std::mem::take(&mut xpm.pixels);
            let mut new_pixels = vec![vec![0usize; nw as usize]; nh as usize];
            for y in 0..(nh as usize).min(old_rows.len()) {
                let old_row = &old_rows[y];
                for x in 0..(nw as usize).min(old_row.len()) {
                    new_pixels[y][x] = old_row[x];
                }
            }
            xpm.pixels = new_pixels;
            xpm.width = nw;
            xpm.height = nh;
            w.set_poi_bmp_width(nw as i32);
            w.set_poi_bmp_height(nh as i32);
            w.set_poi_grid_image(render_grid(xpm, zoom));
        });
    }

    // poi_import_image
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_import_image(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg"])
                .set_title("Importer une image")
                .pick_file();
            if let Some(path) = picked {
                let w = match ww.upgrade() { Some(w) => w, None => return };
                match std::fs::read(&path).map_err(anyhow::Error::from)
                    .and_then(|b| typ::xpm::import_image(&b).map_err(anyhow::Error::from))
                {
                    Ok(new_xpm) => {
                        let mut state_opt = poi_c.borrow_mut();
                        if let Some(state) = state_opt.as_mut() {
                            let zoom = state.zoom;
                            let editing_night = state.editing_night;
                            let bw = new_xpm.width;
                            let bh = new_xpm.height;
                            let xpm_field = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
                            *xpm_field = Some(new_xpm);
                            let xpm = xpm_field.as_ref().unwrap();
                            w.set_poi_bmp_width(bw as i32);
                            w.set_poi_bmp_height(bh as i32);
                            w.set_poi_resize_width_text(bw.to_string().into());
                            w.set_poi_resize_height_text(bh.to_string().into());
                            w.set_poi_colour_count(xpm.palette.len() as i32);
                            w.set_poi_palette(build_palette_entries(xpm));
                            w.set_poi_grid_image(render_grid(xpm, zoom));
                            // preview
                            let day_prev = render_poi_xpm_preview(state.day_xpm.as_ref(), 80, false);
                            let night_prev = render_poi_xpm_preview(state.night_xpm.as_ref(), 80, true);
                            w.set_poi_preview_day(day_prev);
                            w.set_poi_preview_night(night_prev);
                        }
                    }
                    Err(e) => eprintln!("poi import: {}", e),
                }
            }
        });
    }

    // poi_trim_colours
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_trim_colours(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };
            let zoom = state.zoom;
            let editing_night = state.editing_night;
            let xpm_field = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
            if let Some(xpm) = xpm_field {
                typ::xpm::trim_colours(xpm);
                w.set_poi_palette(build_palette_entries(xpm));
                w.set_poi_colour_count(xpm.palette.len() as i32);
                w.set_poi_grid_image(render_grid(xpm, zoom));
            }
        });
    }

    // poi_garmin_colours
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_garmin_colours(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            let state = match state_opt.as_mut() { Some(s) => s, None => return };
            let zoom = state.zoom;
            let editing_night = state.editing_night;
            let xpm_field = if editing_night { &mut state.night_xpm } else { &mut state.day_xpm };
            if let Some(xpm) = xpm_field {
                typ::xpm::snap_garmin_palette(xpm);
                w.set_poi_palette(build_palette_entries(xpm));
                w.set_poi_grid_image(render_grid(xpm, zoom));
            }
        });
    }

    // poi_copy_day_to_night
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_copy_day_to_night(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            if let Some(state) = state_opt.as_mut() {
                state.night_xpm = state.day_xpm.clone();
                state.has_night_bmp = true;
                w.set_poi_has_night_bmp(true);
                w.set_poi_preview_night(render_poi_xpm_preview(state.night_xpm.as_ref(), 80, true));
            }
        });
    }

    // poi_toggle_night_edit
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_toggle_night_edit(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            if let Some(state) = state_opt.as_mut() {
                let new_night = !state.editing_night;
                state.editing_night = new_night;
                w.set_poi_editing_night(new_night);
                let zoom = state.zoom;
                let xpm = if new_night { state.night_xpm.as_ref() } else { state.day_xpm.as_ref() };
                if let Some(xpm) = xpm {
                    w.set_poi_bmp_width(xpm.width as i32);
                    w.set_poi_bmp_height(xpm.height as i32);
                    w.set_poi_colour_count(xpm.palette.len() as i32);
                    w.set_poi_palette(build_palette_entries(xpm));
                    w.set_poi_grid_image(render_grid(xpm, zoom));
                }
                let day_prev = render_poi_xpm_preview(state.day_xpm.as_ref(), 80, false);
                let night_prev = render_poi_xpm_preview(state.night_xpm.as_ref(), 80, true);
                w.set_poi_preview_day(day_prev);
                w.set_poi_preview_night(night_prev);
            }
        });
    }

    // poi_lang_selected
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_lang_selected(move |code| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let state_opt = poi_c.borrow();
            if let Some(state) = state_opt.as_ref() {
                let text = state.labels.iter()
                    .find(|l| l.lang == code as u8)
                    .map(|l| l.text.as_str())
                    .unwrap_or("");
                w.set_poi_selected_lang_text(text.into());
            }
        });
    }

    // poi_lang_text_changed
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_lang_text_changed(move |text| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            if let Some(state) = state_opt.as_mut() {
                let code = w.get_poi_selected_lang() as u8;
                if let Some(label) = state.labels.iter_mut().find(|l| l.lang == code) {
                    label.text = text.to_string();
                } else {
                    state.labels.push(TypLabel { lang: code, text: text.to_string() });
                }
                w.set_poi_lang_labels(build_lang_entries(&state.labels));
            }
        });
    }

    // poi_set_as_default
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_set_as_default(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            if let Some(state) = state_opt.as_mut() {
                let code = w.get_poi_selected_lang() as u8;
                let text = w.get_poi_selected_lang_text().to_string();
                // copier le texte du code sélectionné vers lang=0x00
                if code != 0x00 {
                    if let Some(label) = state.labels.iter_mut().find(|l| l.lang == 0x00) {
                        label.text = text;
                    } else {
                        state.labels.push(TypLabel { lang: 0x00, text });
                    }
                    w.set_poi_lang_labels(build_lang_entries(&state.labels));
                }
            }
        });
    }

    // poi_zoom_changed
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_zoom_changed(move |v| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut state_opt = poi_c.borrow_mut();
            if let Some(state) = state_opt.as_mut() {
                let zoom = (v as u32).clamp(4, 32);
                state.zoom = zoom;
                let editing_night = state.editing_night;
                let xpm = if editing_night { state.night_xpm.as_ref() } else { state.day_xpm.as_ref() };
                if let Some(xpm) = xpm {
                    w.set_poi_grid_image(render_grid(xpm, zoom));
                }
            }
        });
    }

    // poi_tool_changed
    {
        let poi_c = Rc::clone(&poi_state);
        window.on_poi_tool_changed(move |t| {
            if let Some(s) = poi_c.borrow_mut().as_mut() {
                s.tool = t;
                s.line_start = None; // reset ligne
            }
        });
    }

    // poi_editor_apply
    {
        let app_c = Rc::clone(&app);
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_editor_apply(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };

            // Extraire les données nécessaires depuis le state et la fenêtre avant tout borrow mut
            let (doc_idx, day_xpm, night_xpm, labels) = {
                let state_opt = poi_c.borrow();
                let state = match state_opt.as_ref() { Some(s) => s, None => return };
                (state.doc_idx, state.day_xpm.clone(), state.night_xpm.clone(), state.labels.clone())
            };

            // Valider TypeCode/SubType depuis l'UI
            w.set_poi_editor_error("".into());
            let tc_str = w.get_poi_type_code_text();
            let st_str = w.get_poi_sub_type_text();
            let new_tc = match parse_type_code(tc_str.as_str()) {
                Some(v) => v,
                None => {
                    w.set_poi_editor_error("TypeCode invalide (ex: 0x01 ou 1)".into());
                    return;
                }
            };
            let new_st = match parse_sub_type(st_str.as_str()) {
                Some(v) => v,
                None => {
                    w.set_poi_editor_error("SubType invalide (ex: 0x00 ou 0)".into());
                    return;
                }
            };

            // Lire champs typography depuis la fenêtre
            let ext = w.get_poi_extended_labels();
            let fs = int_to_font_style(w.get_poi_font_style_idx());
            let day_fc = hex_to_rgb(w.get_poi_font_color_day_text().as_str());
            let night_fc = hex_to_rgb(w.get_poi_font_color_night_text().as_str());
            let has_night = w.get_poi_has_night_bmp();

            let mut a = app_c.borrow_mut();
            if let Some(doc) = &mut a.doc {
                // Vérifier doublon
                let dup = doc.points.iter().enumerate()
                    .any(|(i, p)| i != doc_idx && p.type_code == new_tc && p.sub_type == new_st);
                if dup {
                    w.set_poi_editor_error(format!("Un POI 0x{:02X}/0x{:02X} existe déjà", new_tc, new_st).into());
                    return;
                }

                if let Some(point) = doc.points.get_mut(doc_idx) {
                    point.type_code = new_tc;
                    point.sub_type = new_st;
                    point.grmn_type = w.get_poi_grmn_type_text().to_string();
                    point.day_xpm = day_xpm;
                    point.night_xpm = if has_night { night_xpm } else { None };
                    point.labels = labels;
                    point.extended_labels = ext;
                    point.font_style = fs;
                    point.day_font_colour = day_fc;
                    point.night_font_colour = night_fc;
                }
                // Mettre à jour la liste latérale (type_code peut avoir changé)
                w.set_points(build_list_model(doc.points.iter().map(|p| (p.type_code, p.sub_type, p.grmn_type.clone()))));
                let nav = w.get_active_nav_tab();
                rebuild_gallery(doc, &w, nav);
                let (day_img, night_img) = render_preview(doc, 2, doc_idx);
                w.set_preview_day(day_img);
                w.set_preview_night(night_img);
                let new_txt = crate::typ::text_writer::element_to_display_txt(doc, 2, doc_idx);
                w.set_selected_txt_code(new_txt.into());
                a.dirty = true;
            }

            *poi_c.borrow_mut() = None;
            w.set_poi_editor_visible(false);
        });
    }

    // poi_editor_cancel
    {
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_poi_editor_cancel(move || {
            *poi_c.borrow_mut() = None;
            if let Some(w) = ww.upgrade() {
                w.set_poi_editor_visible(false);
            }
        });
    }

    // ── Color picker callbacks ────────────────────────────────────
    // Les subprocessus zenity/kdialog sont lancés dans un thread dédié pour
    // ne pas bloquer la boucle événementielle Slint pendant que la dialog est ouverte.

    {
        let ww = window.as_weak();
        window.on_pick_ep_day_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_ep_day_fill_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_ep_day_fill_color(hex_to_slint_color(&hex));
                            w.set_ep_day_fill_text(hex.clone().into());
                            w.set_ep_xpm_text("".into());
                            w.set_editor_error("".into());
                            ep_rebuild_preview(&w);
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_ep_night_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_ep_night_fill_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_ep_night_fill_color(hex_to_slint_color(&hex));
                            w.set_ep_night_fill_text(hex.clone().into());
                            w.set_ep_night_xpm_text("".into());
                            w.set_editor_error("".into());
                            ep_rebuild_preview(&w);
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_ep_contour_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_ep_contour_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_ep_contour_color(hex_to_slint_color(&hex));
                            w.set_ep_contour_text(hex.into());
                            w.set_editor_error("".into());
                            ep_rebuild_preview(&w);
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_el_day_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_el_day_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_el_day_color(hex_to_slint_color(&hex));
                            w.set_el_day_text(hex.clone().into());
                            w.set_el_xpm_text("".into());
                            w.set_editor_error("".into());
                            if let Some(c) = hex_to_rgb(&hex) {
                                let lw = w.get_el_line_width().clamp(1, 20) as u8;
                                let xpm = solid_xpm(c);
                                w.set_el_day_xpm_preview(render_line_xpm_preview(Some(&xpm), lw, false, EDITOR_PREVIEW_SIZE));
                            }
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_el_night_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_el_night_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_el_night_color(hex_to_slint_color(&hex));
                            w.set_el_night_text(hex.clone().into());
                            w.set_el_night_xpm_text("".into());
                            if let Some(c) = hex_to_rgb(&hex) {
                                let lw = w.get_el_line_width().clamp(1, 20) as u8;
                                let xpm = solid_xpm(c);
                                w.set_el_night_xpm_preview(render_line_xpm_preview(Some(&xpm), lw, true, EDITOR_PREVIEW_SIZE));
                            }
                            w.set_editor_error("".into());
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_poi_new_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_poi_new_color_hex().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_poi_new_color_preview(hex_to_slint_color(&hex));
                            w.set_poi_new_color_hex(hex.into());
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_poi_font_day_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_poi_font_color_day_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_poi_font_color_day(hex_to_slint_color(&hex));
                            w.set_poi_font_color_day_text(hex.into());
                            w.set_editor_error("".into());
                        }
                    });
                }
            });
        });
    }
    {
        let ww = window.as_weak();
        window.on_pick_poi_font_night_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let current = w.get_poi_font_color_night_text().to_string();
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = ww2.upgrade() {
                            w.set_poi_font_color_night(hex_to_slint_color(&hex));
                            w.set_poi_font_color_night_text(hex.into());
                            w.set_editor_error("".into());
                        }
                    });
                }
            });
        });
    }

    // ── Callbacks preview live hex → couleur ─────────────────────
    // Mis à jour quand l'utilisateur tape un code hex valide dans les champs éditeur.
    {
        let ww = window.as_weak();
        window.on_ep_day_text_changed(move |s| {
            if let (Some(w), Some(c)) = (ww.upgrade(), hex_to_rgb(s.as_str())) {
                w.set_ep_day_fill_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                ep_rebuild_preview(&w);
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_ep_night_text_changed(move |s| {
            if let (Some(w), Some(c)) = (ww.upgrade(), hex_to_rgb(s.as_str())) {
                w.set_ep_night_fill_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                ep_rebuild_preview(&w);
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_ep_contour_text_changed(move |s| {
            if let (Some(w), Some(c)) = (ww.upgrade(), hex_to_rgb(s.as_str())) {
                w.set_ep_contour_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                ep_rebuild_preview(&w);
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_ep_contour_enabled_changed(move |_| {
            if let Some(w) = ww.upgrade() {
                ep_rebuild_preview(&w);
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_el_day_text_changed(move |s| {
            if let (Some(w), Some(c)) = (ww.upgrade(), hex_to_rgb(s.as_str())) {
                w.set_el_day_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                let lw = w.get_el_line_width().clamp(0, 255) as u8;
                let bw = w.get_el_border_width().clamp(0, 255) as u8;
                let total_lw = lw.saturating_add(2u8.saturating_mul(bw));
                let xpm = solid_xpm(c);
                w.set_el_day_xpm_preview(render_line_xpm_preview(Some(&xpm), total_lw, false, EDITOR_PREVIEW_SIZE));
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_el_night_text_changed(move |s| {
            if let (Some(w), Some(c)) = (ww.upgrade(), hex_to_rgb(s.as_str())) {
                w.set_el_night_color(slint::Color::from_rgb_u8(c.r, c.g, c.b));
                let lw = w.get_el_line_width().clamp(0, 255) as u8;
                let bw = w.get_el_border_width().clamp(0, 255) as u8;
                let total_lw = lw.saturating_add(2u8.saturating_mul(bw));
                let xpm = solid_xpm(c);
                w.set_el_night_xpm_preview(render_line_xpm_preview(Some(&xpm), total_lw, true, EDITOR_PREVIEW_SIZE));
            }
        });
    }
    // ── Handlers épaisseur ligne / bordure → preview live ──────────
    {
        let ww = window.as_weak();
        window.on_el_line_width_changed(move |lw| {
            if let Some(w) = ww.upgrade() {
                let bw = w.get_el_border_width().clamp(0, 255) as u8;
                el_rebuild_preview(&w, lw.clamp(0, 255) as u8, bw);
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_el_bdr_width_changed(move |bw| {
            if let Some(w) = ww.upgrade() {
                let lw = w.get_el_line_width().clamp(0, 255) as u8;
                el_rebuild_preview(&w, lw, bw.clamp(0, 255) as u8);
            }
        });
    }
    // ── Handlers XPM text edited (preview live dans les éditeurs) ──

    {
        let ww = window.as_weak();
        window.on_ep_xpm_text_edited(move |text| {
            if let Some(w) = ww.upgrade() {
                if typ::text_reader::parse_xpm_lines(text.as_str()).ok().flatten().is_some() {
                    ep_rebuild_preview(&w);
                }
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_ep_night_xpm_text_edited(move |text| {
            if let Some(w) = ww.upgrade() {
                if typ::text_reader::parse_xpm_lines(text.as_str()).ok().flatten().is_some() {
                    ep_rebuild_preview(&w);
                }
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_el_xpm_text_edited(move |text| {
            if let Some(w) = ww.upgrade() {
                if typ::text_reader::parse_xpm_lines(text.as_str()).ok().flatten().is_some() {
                    let lw = w.get_el_line_width().clamp(0, 255) as u8;
                    let bw = w.get_el_border_width().clamp(0, 255) as u8;
                    el_rebuild_preview(&w, lw, bw);
                }
            }
        });
    }
    {
        let ww = window.as_weak();
        window.on_el_night_xpm_text_edited(move |text| {
            if let Some(w) = ww.upgrade() {
                if typ::text_reader::parse_xpm_lines(text.as_str()).ok().flatten().is_some() {
                    let lw = w.get_el_line_width().clamp(0, 255) as u8;
                    let bw = w.get_el_border_width().clamp(0, 255) as u8;
                    el_rebuild_preview(&w, lw, bw);
                }
            }
        });
    }

    {
        let ww = window.as_weak();
        window.on_el_dash_preset(move |preset| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let line_w = w.get_el_line_width().max(1) as u32;
            let color_hex = w.get_el_day_text().to_string();
            let color = if hex_to_rgb(&color_hex).is_some() {
                color_hex.to_uppercase()
            } else {
                "#000000".to_string()
            };
            let (on, off): (u32, u32) = match preset {
                0 => (8, 4),
                1 => (4, 4),
                2 => (2, 4),
                3 => (6, 2),
                _ => (4, 4),
            };
            let w_total = on + off;
            let row: String = "a".repeat(on as usize) + &".".repeat(off as usize);
            let mut xpm = format!("{} {} 2 1\na  c {}\n.  c none\n", w_total, line_w, color);
            for _ in 0..line_w {
                xpm.push_str(&row);
                xpm.push('\n');
            }
            w.set_el_xpm_text(xpm.into());
            // Générer le XPM nuit avec la même trame et la couleur nuit (alignement pointillés)
            let night_color_hex = w.get_el_night_text().to_string();
            let night_color = if hex_to_rgb(&night_color_hex).is_some() {
                night_color_hex.to_uppercase()
            } else {
                color.clone()
            };
            let mut night_xpm = format!("{} {} 2 1\na  c {}\n.  c none\n", w_total, line_w, night_color);
            for _ in 0..line_w {
                night_xpm.push_str(&row);
                night_xpm.push('\n');
            }
            w.set_el_night_xpm_text(night_xpm.into());
            // Synchronise le SpinBox épaisseur avec la hauteur effective de l'XPM
            // (line_w peut diverger de el_line_width si la ligne était un bitmap width=0)
            w.set_el_line_width(line_w as i32);
            // Mettre à jour la preview jour + nuit avec le nouveau motif XPM
            let bw = w.get_el_border_width().clamp(0, 255) as u8;
            el_rebuild_preview(&w, line_w as u8, bw);
        });
    }

    // ── Task 21 : Ajouter des éléments ───────────────────────────
    {
        let app_c = Rc::clone(&app);
        let poi_c = Rc::clone(&poi_state);
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_add_element(move |kind| {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut open_poly: Option<usize> = None;
            let mut open_line: Option<usize> = None;
            let mut open_poi: Option<usize> = None;
            {
                let mut a = app_c.borrow_mut();
                if a.doc.is_none() { a.doc = Some(TypDocument::default()); }
                let doc = a.doc.as_mut().unwrap();
                match kind {
                    0 => {
                        let tc = doc.polygons.iter().map(|p| p.type_code).max()
                            .and_then(|m| m.checked_add(1)).unwrap_or(1);
                        doc.polygons.push(TypPolygon { type_code: tc, ..Default::default() });
                        let idx = doc.polygons.len() - 1;
                        update_ui_from_doc(doc, &w);
                        open_poly = Some(idx);
                    }
                    1 => {
                        let tc = doc.lines.iter().map(|l| l.type_code).max()
                            .and_then(|m| m.checked_add(1)).unwrap_or(1);
                        doc.lines.push(TypLine { type_code: tc, line_width: 1, ..Default::default() });
                        let idx = doc.lines.len() - 1;
                        update_ui_from_doc(doc, &w);
                        open_line = Some(idx);
                    }
                    2 => {
                        let tc = doc.points.iter().map(|p| p.type_code).max()
                            .and_then(|m| m.checked_add(1)).unwrap_or(1);
                        doc.points.push(TypPoint { type_code: tc, ..Default::default() });
                        let idx = doc.points.len() - 1;
                        update_ui_from_doc(doc, &w);
                        open_poi = Some(idx);
                    }
                    3 => {
                        let tc = doc.icons.iter().map(|ic| ic.type_code).max()
                            .and_then(|m| m.checked_add(1)).unwrap_or(1);
                        doc.icons.push(TypIconSet { type_code: tc, sub_type: 0, icons: Vec::new() });
                        update_ui_from_doc(doc, &w);
                    }
                    _ => return,
                }
                a.dirty = true;
            }
            // Ouvrir les éditeurs après avoir relâché le borrow mut
            if let Some(idx) = open_poly {
                let a = app_c.borrow();
                if let Some(doc) = &a.doc {
                    if let Some(p) = doc.polygons.get(idx) {
                        let (day_text, day_color) = xpm_fill_color(p.day_xpm.as_ref());
                        let (night_text, night_color) = xpm_fill_color(p.night_xpm.as_ref());
                        w.set_editor_title(format!("Polygone 0x{:02X}/0x{:02X}", p.type_code, p.sub_type).into());
                        w.set_ep_type_code_text(format!("0x{:02X}", p.type_code).into());
                        w.set_ep_sub_type_text(format!("0x{:02X}", p.sub_type).into());
                        w.set_ep_day_fill_text(day_text); w.set_ep_day_fill_color(day_color);
                        w.set_ep_night_fill_text(night_text); w.set_ep_night_fill_color(night_color);
                        w.set_ep_xpm_text("".into()); w.set_ep_night_xpm_text("".into());
                        w.set_ep_contour_enabled(false);
                        w.set_ep_contour_text("#000000".into());
                        w.set_ep_contour_color(slint::Color::from_rgb_u8(0, 0, 0));
                        w.set_ep_extended_labels(false); w.set_ep_font_style(0);
                        push_ep_previews(&w, p.day_xpm.as_ref(), p.night_xpm.as_ref());
                        w.set_ep_is_pattern(false);
                        let mut dm = init_dm_ep(p);
                        dm.doc_idx = idx;
                        push_dm_to_window_ep(&dm, &w);
                        *dm_c.borrow_mut() = Some(dm);
                        w.set_editor_kind(0); w.set_editor_idx(idx as i32);
                        w.set_editor_visible(true);
                    }
                }
            }
            if let Some(idx) = open_line {
                let a = app_c.borrow();
                if let Some(doc) = &a.doc {
                    if let Some(l) = doc.lines.get(idx) {
                        let (day_text, day_color) = xpm_fill_color(l.day_xpm.as_ref());
                        let (night_text, night_color) = xpm_fill_color(l.night_xpm.as_ref());
                        w.set_editor_title(format!("Ligne 0x{:02X}/0x{:02X}", l.type_code, l.sub_type).into());
                        w.set_el_type_code_text(format!("0x{:02X}", l.type_code).into());
                        w.set_el_sub_type_text(format!("0x{:02X}", l.sub_type).into());
                        w.set_el_day_text(day_text); w.set_el_day_color(day_color);
                        w.set_el_night_text(night_text); w.set_el_night_color(night_color);
                        w.set_el_line_width(1); w.set_el_border_width(0);
                        w.set_el_use_orientation(true); w.set_el_extended_labels(false);
                        w.set_el_font_style(0);
                        w.set_el_xpm_text("".into()); w.set_el_night_xpm_text("".into());
                        push_el_previews(&w, l.day_xpm.as_ref(), l.night_xpm.as_ref(), l.line_width, l.border_width);
                        w.set_el_is_pattern(false);
                        let mut dm = init_dm_el(l);
                        dm.doc_idx = idx;
                        push_dm_to_window_el(&dm, &w);
                        *dm_c.borrow_mut() = Some(dm);
                        w.set_editor_kind(1); w.set_editor_idx(idx as i32);
                        w.set_editor_visible(true);
                    }
                }
            }
            if let Some(idx) = open_poi {
                let a = app_c.borrow();
                if let Some(doc) = &a.doc {
                    if let Some(state) = open_poi_editor(doc, 2, idx, &w) {
                        *poi_c.borrow_mut() = Some(state);
                    }
                }
            }
        });
    }

    // ── Task 21 : Supprimer des éléments ─────────────────────────
    {
        let app_c = Rc::clone(&app);
        let poi_c = Rc::clone(&poi_state);
        let ww = window.as_weak();
        window.on_delete_element(move |kind, idx| {
            if idx < 0 { return; }
            let w = match ww.upgrade() { Some(w) => w, None => return };
            // Fermer l'éditeur POI si l'élément supprimé est celui en cours d'édition
            if kind == 2 {
                let editing_idx = poi_c.borrow().as_ref().map(|s| s.doc_idx);
                if editing_idx == Some(idx as usize) {
                    *poi_c.borrow_mut() = None;
                    w.set_poi_editor_visible(false);
                }
            }
            let mut a = app_c.borrow_mut();
            {
                let doc = match &mut a.doc { Some(d) => d, None => return };
                let i = idx as usize;
                match kind {
                    0 if i < doc.polygons.len() => { doc.polygons.remove(i); }
                    1 if i < doc.lines.len()    => { doc.lines.remove(i); }
                    2 if i < doc.points.len()   => { doc.points.remove(i); }
                    3 if i < doc.icons.len()    => { doc.icons.remove(i); }
                    _ => return,
                }
            }
            a.dirty = true;
            if let Some(doc) = &a.doc {
                update_ui_from_doc(doc, &w);
            }
        });
    }

    // ── Task 22 : Éditeur DrawOrder ───────────────────────────────
    {
        let ww = window.as_weak();
        window.on_edit_draworder(move || {
            if let Some(w) = ww.upgrade() {
                w.set_do_selected_idx(-1);
                w.set_do_editor_visible(true);
            }
        });
    }

    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_do_add(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let level = w.get_do_new_level_text().trim().parse::<u8>().unwrap_or(0);
            let type_str = w.get_do_new_type_text();
            let sub_str  = w.get_do_new_subtype_text();
            let type_code = u16::from_str_radix(type_str.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).unwrap_or(0);
            let sub_type  = u8::from_str_radix(sub_str.trim().trim_start_matches("0x").trim_start_matches("0X"), 16).unwrap_or(0);
            let entry = DrawOrderEntry { level, type_code, sub_type };
            let mut a = app_c.borrow_mut();
            if let Some(doc) = &mut a.doc {
                doc.draw_order.push(entry);
                w.set_draworder(ModelRc::new(VecModel::from(
                    doc.draw_order.iter()
                        .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
                        .collect::<Vec<_>>()
                )));
                a.dirty = true;
            }
        });
    }

    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_do_delete(move |idx| {
            if idx < 0 { return; }
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut a = app_c.borrow_mut();
            if let Some(doc) = &mut a.doc {
                if (idx as usize) < doc.draw_order.len() {
                    doc.draw_order.remove(idx as usize);
                    w.set_draworder(ModelRc::new(VecModel::from(
                        doc.draw_order.iter()
                            .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
                            .collect::<Vec<_>>()
                    )));
                    a.dirty = true;
                }
            }
        });
    }

    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_do_move_up(move |idx| {
            if idx <= 0 { return; }
            let i = idx as usize;
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut a = app_c.borrow_mut();
            if let Some(doc) = &mut a.doc {
                if i < doc.draw_order.len() {
                    doc.draw_order.swap(i - 1, i);
                    w.set_draworder(ModelRc::new(VecModel::from(
                        doc.draw_order.iter()
                            .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
                            .collect::<Vec<_>>()
                    )));
                    w.set_do_selected_idx(idx - 1);
                    a.dirty = true;
                }
            }
        });
    }

    {
        let app_c = Rc::clone(&app);
        let ww = window.as_weak();
        window.on_do_move_down(move |idx| {
            if idx < 0 { return; }
            let i = idx as usize;
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let mut a = app_c.borrow_mut();
            if let Some(doc) = &mut a.doc {
                if i + 1 < doc.draw_order.len() {
                    doc.draw_order.swap(i, i + 1);
                    w.set_draworder(ModelRc::new(VecModel::from(
                        doc.draw_order.iter()
                            .map(|e| make_item(format!("L{} 0x{:02X}/0x{:02X}", e.level, e.type_code, e.sub_type)))
                            .collect::<Vec<_>>()
                    )));
                    w.set_do_selected_idx(idx + 1);
                    a.dirty = true;
                }
            }
        });
    }

    {
        let ww = window.as_weak();
        window.on_do_cancel(move || {
            if let Some(w) = ww.upgrade() { w.set_do_editor_visible(false); }
        });
    }

    // ── DrawMask callbacks — polygone (ep) ───────────────────────

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_ep_dm_grid_clicked(move |fx, fy, btn| {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let col = fx as usize;
            let row = fy as usize;
            let value = if btn == 0 { 1u8 } else { 0u8 };
            let night = w.get_ep_dm_editing_night();
            let pixels = if night { &mut state.night_pixels } else { &mut state.day_pixels };
            if row < pixels.len() && col < 32 {
                match state.tool {
                    1 => dm_flood_fill(pixels, col, row, value),
                    _ => pixels[row][col] = value,
                }
            }
            let img = ep_dm_render_from_window(state, &w);
            w.set_ep_dm_grid_image(img);
            let fg = if night { slint_to_rgb(w.get_ep_dm_night_fg_color()) } else { slint_to_rgb(w.get_ep_dm_fg_color()) };
            let bg = if night {
                if w.get_ep_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_night_bg_color())) }
            } else {
                if w.get_ep_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_bg_color())) }
            };
            let pix = if night { &state.night_pixels } else { &state.day_pixels };
            let xpm = dm_pixels_to_xpm(pix, fg, bg);
            let contour = if w.get_ep_contour_enabled() { Some(slint_to_rgb(w.get_ep_contour_color())) } else { None };
            if night {
                w.set_ep_night_xpm_preview(render_polygon_thumb_xpm(Some(&xpm), EDITOR_PREVIEW_SIZE, true, contour));
            } else {
                w.set_ep_day_xpm_preview(render_polygon_thumb_xpm(Some(&xpm), EDITOR_PREVIEW_SIZE, false, contour));
            }
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_ep_dm_zoom_changed(move |v| {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            state.zoom = v as u32;
            let img = ep_dm_render_from_window(state, &w);
            w.set_ep_dm_grid_image(img);
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        window.on_ep_dm_tool_changed(move |v| {
            if let Some(state) = dm_c.borrow_mut().as_mut() { state.tool = v; }
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_ep_dm_toggle_night(move || {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            state.editing_night = w.get_ep_dm_editing_night();
            let img = ep_dm_render_from_window(state, &w);
            w.set_ep_dm_grid_image(img);
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_ep_dm_fg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let night = w.get_ep_dm_editing_night();
            let c = if night { w.get_ep_dm_night_fg_color() } else { w.get_ep_dm_fg_color() };
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, bg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let pix = if night { s.night_pixels.clone() } else { s.day_pixels.clone() };
                        let bg = if night {
                            if w.get_ep_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_night_bg_color())) }
                        } else {
                            if w.get_ep_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_bg_color())) }
                        };
                        (pix, bg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                let sc = slint::Color::from_rgb_u8(nc.r, nc.g, nc.b);
                                if night { w.set_ep_dm_night_fg_color(sc); } else { w.set_ep_dm_fg_color(sc); }
                                let img = render_draw_mask_grid(&pixels, nc, bg, zoom);
                                w.set_ep_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_ep_dm_bg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let night = w.get_ep_dm_editing_night();
            let c = if night { w.get_ep_dm_night_bg_color() } else { w.get_ep_dm_bg_color() };
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, fg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let pix = if night { s.night_pixels.clone() } else { s.day_pixels.clone() };
                        let fg = if night { slint_to_rgb(w.get_ep_dm_night_fg_color()) } else { slint_to_rgb(w.get_ep_dm_fg_color()) };
                        (pix, fg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                let sc = slint::Color::from_rgb_u8(nc.r, nc.g, nc.b);
                                if night {
                                    w.set_ep_dm_night_bg_color(sc);
                                    w.set_ep_dm_night_bg_is_clear(false);
                                } else {
                                    w.set_ep_dm_bg_color(sc);
                                    w.set_ep_dm_bg_is_clear(false);
                                }
                                let img = render_draw_mask_grid(&pixels, fg, Some(nc), zoom);
                                w.set_ep_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_ep_dm_night_fg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let c = w.get_ep_dm_night_fg_color();
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, bg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let bg = if w.get_ep_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_ep_dm_night_bg_color())) };
                        (s.night_pixels.clone(), bg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                w.set_ep_dm_night_fg_color(slint::Color::from_rgb_u8(nc.r, nc.g, nc.b));
                                let img = render_draw_mask_grid(&pixels, nc, bg, zoom);
                                w.set_ep_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_ep_dm_night_bg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let c = w.get_ep_dm_night_bg_color();
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, fg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let fg = slint_to_rgb(w.get_ep_dm_night_fg_color());
                        (s.night_pixels.clone(), fg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                w.set_ep_dm_night_bg_color(slint::Color::from_rgb_u8(nc.r, nc.g, nc.b));
                                w.set_ep_dm_night_bg_is_clear(false);
                                let img = render_draw_mask_grid(&pixels, fg, Some(nc), zoom);
                                w.set_ep_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    // ── DrawMask callbacks — ligne (el) ──────────────────────────

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_el_dm_grid_clicked(move |fx, fy, btn| {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let col = fx as usize;
            let row = fy as usize;
            let value = if btn == 0 { 1u8 } else { 0u8 };
            let night = w.get_el_dm_editing_night();
            let wide = w.get_el_dm_wide_tool();
            let pixels = if night { &mut state.night_pixels } else { &mut state.day_pixels };
            if row < pixels.len() && col < 32 {
                match state.tool {
                    1 => dm_flood_fill(pixels, col, row, value),
                    _ => {
                        pixels[row][col] = value;
                        if wide {
                            if col > 0 { pixels[row][col - 1] = value; }
                            if col + 1 < 32 { pixels[row][col + 1] = value; }
                        }
                    }
                }
            }
            let img = el_dm_render_from_window(state, &w);
            w.set_el_dm_grid_image(img);
            let lw = w.get_el_line_width().clamp(1, 20) as u8;
            let fg = if night { slint_to_rgb(w.get_el_dm_night_fg_color()) } else { slint_to_rgb(w.get_el_dm_fg_color()) };
            let bg = if night {
                if w.get_el_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_night_bg_color())) }
            } else {
                if w.get_el_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_bg_color())) }
            };
            let pix = if night { &state.night_pixels } else { &state.day_pixels };
            let xpm = dm_pixels_to_xpm(pix, fg, bg);
            if night {
                w.set_el_night_xpm_preview(render_line_xpm_preview(Some(&xpm), lw, true, EDITOR_PREVIEW_SIZE));
            } else {
                w.set_el_day_xpm_preview(render_line_xpm_preview(Some(&xpm), lw, false, EDITOR_PREVIEW_SIZE));
            }
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_el_dm_zoom_changed(move |v| {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            state.zoom = v as u32;
            let img = el_dm_render_from_window(state, &w);
            w.set_el_dm_grid_image(img);
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        window.on_el_dm_tool_changed(move |v| {
            if let Some(state) = dm_c.borrow_mut().as_mut() { state.tool = v; }
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_el_dm_resize_rows(move |new_rows| {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let nr = (new_rows as usize).clamp(1, 31);
            let cur = state.day_pixels.len();
            match nr.cmp(&cur) {
                std::cmp::Ordering::Greater => {
                    for _ in cur..nr { state.day_pixels.push(vec![0u8; 32]); }
                    let ncur = state.night_pixels.len();
                    for _ in ncur..nr { state.night_pixels.push(vec![0u8; 32]); }
                }
                std::cmp::Ordering::Less => {
                    state.day_pixels.truncate(nr);
                    state.night_pixels.truncate(nr);
                }
                std::cmp::Ordering::Equal => {}
            }
            w.set_el_dm_num_rows(nr as i32);
            let img = el_dm_render_from_window(state, &w);
            w.set_el_dm_grid_image(img);
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_el_dm_toggle_night(move || {
            let mut opt = dm_c.borrow_mut();
            let state = match opt.as_mut() { Some(s) => s, None => return };
            let w = match ww.upgrade() { Some(w) => w, None => return };
            state.editing_night = w.get_el_dm_editing_night();
            let img = el_dm_render_from_window(state, &w);
            w.set_el_dm_grid_image(img);
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_el_dm_fg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let night = w.get_el_dm_editing_night();
            let c = if night { w.get_el_dm_night_fg_color() } else { w.get_el_dm_fg_color() };
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, bg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let pix = if night { s.night_pixels.clone() } else { s.day_pixels.clone() };
                        let bg = if night {
                            if w.get_el_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_night_bg_color())) }
                        } else {
                            if w.get_el_dm_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_bg_color())) }
                        };
                        (pix, bg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                let sc = slint::Color::from_rgb_u8(nc.r, nc.g, nc.b);
                                if night { w.set_el_dm_night_fg_color(sc); } else { w.set_el_dm_fg_color(sc); }
                                let img = render_draw_mask_grid(&pixels, nc, bg, zoom);
                                w.set_el_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_el_dm_bg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let night = w.get_el_dm_editing_night();
            let c = if night { w.get_el_dm_night_bg_color() } else { w.get_el_dm_bg_color() };
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, fg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let pix = if night { s.night_pixels.clone() } else { s.day_pixels.clone() };
                        let fg = if night { slint_to_rgb(w.get_el_dm_night_fg_color()) } else { slint_to_rgb(w.get_el_dm_fg_color()) };
                        (pix, fg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                let sc = slint::Color::from_rgb_u8(nc.r, nc.g, nc.b);
                                if night {
                                    w.set_el_dm_night_bg_color(sc);
                                    w.set_el_dm_night_bg_is_clear(false);
                                } else {
                                    w.set_el_dm_bg_color(sc);
                                    w.set_el_dm_bg_is_clear(false);
                                }
                                let img = render_draw_mask_grid(&pixels, fg, Some(nc), zoom);
                                w.set_el_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_el_dm_night_fg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let c = w.get_el_dm_night_fg_color();
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, bg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let bg = if w.get_el_dm_night_bg_is_clear() { None } else { Some(slint_to_rgb(w.get_el_dm_night_bg_color())) };
                        (s.night_pixels.clone(), bg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                w.set_el_dm_night_fg_color(slint::Color::from_rgb_u8(nc.r, nc.g, nc.b));
                                let img = render_draw_mask_grid(&pixels, nc, bg, zoom);
                                w.set_el_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    {
        let dm_c = Rc::clone(&dm_state);
        let ww = window.as_weak();
        window.on_pick_el_dm_night_bg_color(move || {
            let w = match ww.upgrade() { Some(w) => w, None => return };
            let c = w.get_el_dm_night_bg_color();
            let current = format!("#{:02X}{:02X}{:02X}", c.red(), c.green(), c.blue());
            let (pixels, fg, zoom) = {
                let opt = dm_c.borrow();
                match opt.as_ref() {
                    Some(s) => {
                        let fg = slint_to_rgb(w.get_el_dm_night_fg_color());
                        (s.night_pixels.clone(), fg, s.zoom)
                    }
                    None => return,
                }
            };
            let ww2 = ww.clone();
            std::thread::spawn(move || {
                if let Some(hex) = pick_color(&current) {
                    if let Some(nc) = hex_to_rgb(&hex) {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = ww2.upgrade() {
                                w.set_el_dm_night_bg_color(slint::Color::from_rgb_u8(nc.r, nc.g, nc.b));
                                w.set_el_dm_night_bg_is_clear(false);
                                let img = render_draw_mask_grid(&pixels, fg, Some(nc), zoom);
                                w.set_el_dm_grid_image(img);
                            }
                        });
                    }
                }
            });
        });
    }

    window.run()?;
    Ok(())
}
