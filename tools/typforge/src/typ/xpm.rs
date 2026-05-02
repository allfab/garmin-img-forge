use super::model::{ColorMode, Rgba, Xpm};
use crate::error::{Result, TypforgeError};

/// Palette Garmin standard 16 couleurs.
pub const GARMIN_PALETTE_16: [Rgba; 16] = [
    Rgba { r: 0xFF, g: 0xFF, b: 0xFF, a: 0 }, // blanc
    Rgba { r: 0x00, g: 0x00, b: 0x00, a: 0 }, // noir
    Rgba { r: 0xFF, g: 0x00, b: 0x00, a: 0 }, // rouge
    Rgba { r: 0x00, g: 0xFF, b: 0x00, a: 0 }, // vert vif
    Rgba { r: 0x00, g: 0x00, b: 0xFF, a: 0 }, // bleu
    Rgba { r: 0xFF, g: 0xFF, b: 0x00, a: 0 }, // jaune
    Rgba { r: 0xFF, g: 0x00, b: 0xFF, a: 0 }, // magenta
    Rgba { r: 0x00, g: 0xFF, b: 0xFF, a: 0 }, // cyan
    Rgba { r: 0x80, g: 0x80, b: 0x80, a: 0 }, // gris moyen
    Rgba { r: 0xC0, g: 0xC0, b: 0xC0, a: 0 }, // gris clair
    Rgba { r: 0x80, g: 0x00, b: 0x00, a: 0 }, // bordeaux
    Rgba { r: 0x00, g: 0x80, b: 0x00, a: 0 }, // vert foncé
    Rgba { r: 0x00, g: 0x00, b: 0x80, a: 0 }, // bleu marine
    Rgba { r: 0x80, g: 0x80, b: 0x00, a: 0 }, // olive
    Rgba { r: 0x80, g: 0x00, b: 0x80, a: 0 }, // violet
    Rgba { r: 0x00, g: 0x80, b: 0x80, a: 0 }, // bleu-vert
];

/// Convertit un XPM en grille RGBA (ligne par ligne).
pub fn xpm_to_image(xpm: &Xpm) -> Vec<Vec<Rgba>> {
    let mut result = Vec::with_capacity(xpm.height as usize);
    for row in &xpm.pixels {
        let mut rgba_row = Vec::with_capacity(xpm.width as usize);
        for &idx in row {
            let color = xpm
                .palette
                .get(idx)
                .map(|(_, c)| *c)
                .unwrap_or(Rgba::transparent());
            rgba_row.push(color);
        }
        result.push(rgba_row);
    }
    result
}

/// Convertit une grille RGBA en XPM en dédupliquant la palette.
pub fn image_to_xpm(pixels: &[Vec<Rgba>], colour_mode: ColorMode) -> Xpm {
    let height = pixels.len() as u16;
    let width = pixels.first().map(|r| r.len() as u16).unwrap_or(0);

    // Construire la palette en préservant l'ordre de première apparition.
    let mut palette: Vec<Rgba> = Vec::new();
    let mut pixel_indices: Vec<Vec<usize>> = Vec::with_capacity(height as usize);

    for row in pixels {
        let mut idx_row = Vec::with_capacity(width as usize);
        for &color in row {
            let idx = if let Some(pos) = palette.iter().position(|c| *c == color) {
                pos
            } else {
                palette.push(color);
                palette.len() - 1
            };
            idx_row.push(idx);
        }
        pixel_indices.push(idx_row);
    }

    let tagged_palette: Vec<(String, Rgba)> = palette
        .iter()
        .enumerate()
        .map(|(i, &c)| (make_tag(i), c))
        .collect();

    Xpm {
        width,
        height,
        colour_mode,
        palette: tagged_palette,
        pixels: pixel_indices,
    }
}

/// Importe une image PNG ou JPEG et la convertit en XPM Colormode 16 (max 16 couleurs).
pub fn import_image(bytes: &[u8]) -> Result<Xpm> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| TypforgeError::Binary(format!("Import image: {}", e)))?;

    let rgba_img = img.to_rgba8();
    let (img_w, img_h) = rgba_img.dimensions();

    // Quantisation simple : snap chaque pixel vers la palette Garmin 16 couleurs.
    let raw_pixels: Vec<Vec<Rgba>> = (0..img_h)
        .map(|y| {
            (0..img_w)
                .map(|x| {
                    let p = rgba_img.get_pixel(x, y);
                    // Pixel transparent → transparent XPM
                    if p[3] < 128 {
                        return Rgba::transparent();
                    }
                    snap_to_garmin(Rgba::opaque(p[0], p[1], p[2]))
                })
                .collect()
        })
        .collect();

    let mut xpm = image_to_xpm(&raw_pixels, ColorMode::Indexed);

    // Limiter à 16 couleurs : si la quantisation a produit plus, merger par distance
    if xpm.palette.len() > 16 {
        xpm = quantize_to_n_colors(xpm, 16);
    }

    Ok(xpm)
}

/// Snap chaque couleur de la palette vers la couleur Garmin standard la plus proche.
pub fn snap_garmin_palette(xpm: &mut Xpm) {
    for (_, color) in &mut xpm.palette {
        if !color.is_transparent() {
            *color = snap_to_garmin(*color);
        }
    }
}

/// Supprime les couleurs de la palette non référencées par les pixels.
pub fn trim_colours(xpm: &mut Xpm) {
    let used: std::collections::HashSet<usize> =
        xpm.pixels.iter().flat_map(|row| row.iter().copied()).collect();

    // Construire la nouvelle palette avec réindexation
    let mut new_palette: Vec<(String, Rgba)> = Vec::new();
    let mut remap: Vec<usize> = vec![0; xpm.palette.len()];

    for (old_idx, entry) in xpm.palette.iter().enumerate() {
        if used.contains(&old_idx) {
            remap[old_idx] = new_palette.len();
            new_palette.push((make_tag(new_palette.len()), entry.1));
        }
    }

    // Réindexer les pixels
    for row in &mut xpm.pixels {
        for idx in row {
            *idx = remap[*idx];
        }
    }

    xpm.palette = new_palette;
}

// ─── helpers privés ──────────────────────────────────────────────────────────

fn snap_to_garmin(c: Rgba) -> Rgba {
    GARMIN_PALETTE_16
        .iter()
        .copied()
        .min_by_key(|&gc| color_distance_sq(c, gc))
        .unwrap_or(c)
}

fn color_distance_sq(a: Rgba, b: Rgba) -> u32 {
    let dr = (a.r as i32 - b.r as i32).pow(2) as u32;
    let dg = (a.g as i32 - b.g as i32).pow(2) as u32;
    let db = (a.b as i32 - b.b as i32).pow(2) as u32;
    dr + dg + db
}

fn quantize_to_n_colors(xpm: Xpm, n: usize) -> Xpm {
    // Quantisation par médiane : remplace chaque couleur en dehors des n
    // premières par la plus proche parmi les n premières.
    let kept: Vec<Rgba> = xpm.palette.iter().take(n).map(|(_, c)| *c).collect();

    let remap: Vec<usize> = xpm
        .palette
        .iter()
        .map(|(_, c)| {
            if c.is_transparent() {
                kept.iter()
                    .position(|k| k.is_transparent())
                    .unwrap_or(0)
            } else {
                kept.iter()
                    .enumerate()
                    .min_by_key(|(_, k)| {
                        if k.is_transparent() {
                            u32::MAX
                        } else {
                            color_distance_sq(*c, **k)
                        }
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        })
        .collect();

    let new_palette: Vec<(String, Rgba)> = kept
        .iter()
        .enumerate()
        .map(|(i, &c)| (make_tag(i), c))
        .collect();

    let new_pixels: Vec<Vec<usize>> = xpm
        .pixels
        .iter()
        .map(|row| row.iter().map(|&idx| remap.get(idx).copied().unwrap_or(0)).collect())
        .collect();

    Xpm {
        width: xpm.width,
        height: xpm.height,
        colour_mode: xpm.colour_mode,
        palette: new_palette,
        pixels: new_pixels,
    }
}

/// Génère un tag XPM 1-char pour l'index `i`.
///
/// Utilise les caractères printables `!`..`~` sauf `"` et `\`.
fn make_tag(i: usize) -> String {
    let chars: Vec<char> = (0x21u8..=0x7E)
        .filter(|&b| b != b'"' && b != b'\\')
        .map(|b| b as char)
        .collect();
    let c = chars[i % chars.len()];
    c.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_to_xpm_round_trip() {
        let pixels = vec![
            vec![Rgba::opaque(255, 0, 0), Rgba::opaque(0, 255, 0)],
            vec![Rgba::opaque(0, 0, 255), Rgba::transparent()],
        ];
        let xpm = image_to_xpm(&pixels, ColorMode::Indexed);
        assert_eq!(xpm.width, 2);
        assert_eq!(xpm.height, 2);
        assert_eq!(xpm.palette.len(), 4);
        let restored = xpm_to_image(&xpm);
        assert_eq!(restored[0][0], Rgba::opaque(255, 0, 0));
        assert_eq!(restored[1][1], Rgba::transparent());
    }

    #[test]
    fn trim_unused_colors() {
        let mut xpm = image_to_xpm(
            &[vec![Rgba::opaque(255, 0, 0), Rgba::opaque(255, 0, 0)]],
            ColorMode::Indexed,
        );
        // Ajouter une couleur fantôme non utilisée dans les pixels
        xpm.palette.push(("z".to_string(), Rgba::opaque(0, 255, 0)));
        assert_eq!(xpm.palette.len(), 2);
        trim_colours(&mut xpm);
        assert_eq!(xpm.palette.len(), 1);
    }

    #[test]
    fn snap_garmin_white() {
        let white = Rgba::opaque(254, 254, 254);
        let snapped = snap_to_garmin(white);
        assert_eq!(snapped, Rgba::opaque(255, 255, 255));
    }
}
