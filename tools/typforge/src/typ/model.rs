/// Couleur RGB opaque.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Couleur RGBA.
///
/// Convention alpha-inverse TYP : `a == 0xff` = transparent, `a == 0x00` = opaque.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn opaque(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 0x00 }
    }

    pub fn transparent() -> Self {
        Rgba { r: 0, g: 0, b: 0, a: 0xff }
    }

    pub fn is_transparent(&self) -> bool {
        self.a == 0xff
    }
}

/// Mode couleur d'une bitmap XPM TYP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    #[default]
    Indexed,
    #[allow(dead_code)]
    True16,
    True32,
}

/// Bitmap XPM avec palette et pixels comme indices dans la palette.
///
/// `pixels[y][x]` = index dans `palette` ; index 0 est souvent "none"/transparent.
#[derive(Debug, Clone, Default)]
pub struct Xpm {
    pub width: u16,
    pub height: u16,
    pub colour_mode: ColorMode,
    /// Palette : `(tag, couleur)` — le tag est la chaîne XPM (1 ou 2 chars).
    pub palette: Vec<(String, Rgba)>,
    /// Pixels : indices dans `palette`, ligne par ligne.
    pub pixels: Vec<Vec<usize>>,
}

impl Xpm {
    pub fn new(width: u16, height: u16, colour_mode: ColorMode) -> Self {
        let pixels = vec![vec![0usize; width as usize]; height as usize];
        Xpm { width, height, colour_mode, palette: Vec::new(), pixels }
    }
}

/// Label multilingue (`StringN=langcode,texte`).
#[derive(Debug, Clone, Default)]
pub struct TypLabel {
    /// Code langue (0x00 = défaut, 0x01 = français, 0x04 = anglais…).
    pub lang: u8,
    pub text: String,
}

/// Style de police pour les labels affichés sur la carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontStyle {
    #[default]
    Default,
    NoLabel,
    Small,
    Normal,
    Large,
    Custom(u8),
}

/// Paramètres de la section `[_id]`.
#[derive(Debug, Clone, Default)]
pub struct TypParam {
    pub family_id: u16,
    pub product_id: u16,
    /// Codepage : 1252 (CP1252) ou 65001 (UTF-8).
    pub codepage: u16,
    /// Chaîne d'en-tête optionnelle (commentaire de création).
    pub header_str: String,
}

/// Couleur de contour d'un polygone TYP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContourColor {
    /// Pas de contour visible (`ContourColor=No`).
    #[default]
    No,
    /// Contour solide d'une couleur donnée.
    Solid(Rgb),
}

/// Polygone TYP (surface).
#[derive(Debug, Clone, Default)]
pub struct TypPolygon {
    pub type_code: u16,
    pub sub_type: u8,
    pub grmn_type: String,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    pub font_style: FontStyle,
    pub day_font_colour: Option<Rgb>,
    pub night_font_colour: Option<Rgb>,
    pub extended_labels: bool,
    pub contour_color: ContourColor,
}

/// Polyligne TYP.
#[derive(Debug, Clone, Default)]
pub struct TypLine {
    pub type_code: u16,
    pub sub_type: u8,
    pub grmn_type: String,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    pub line_width: u8,
    pub border_width: u8,
    pub font_style: FontStyle,
    pub day_font_colour: Option<Rgb>,
    pub night_font_colour: Option<Rgb>,
    pub extended_labels: bool,
    pub use_orientation: bool,
}

/// Point TYP (POI).
#[derive(Debug, Clone, Default)]
pub struct TypPoint {
    pub type_code: u16,
    pub sub_type: u8,
    pub grmn_type: String,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    pub font_style: FontStyle,
    pub day_font_colour: Option<Rgb>,
    pub night_font_colour: Option<Rgb>,
    pub extended_labels: bool,
}

/// Entrée de draworder (`[_drawOrder]`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DrawOrderEntry {
    pub level: u8,
    pub type_code: u16,
    pub sub_type: u8,
}

/// Document TYP complet.
#[derive(Debug, Clone, Default)]
pub struct TypDocument {
    pub param: TypParam,
    pub polygons: Vec<TypPolygon>,
    pub lines: Vec<TypLine>,
    pub points: Vec<TypPoint>,
    pub icons: Vec<TypIconSet>,
    pub draw_order: Vec<DrawOrderEntry>,
    pub comments: String,
}

/// Jeu d'icônes POI extra (`[_icons]`).
#[derive(Debug, Clone, Default)]
pub struct TypIconSet {
    pub type_code: u16,
    pub sub_type: u8,
    pub icons: Vec<Xpm>,
}
