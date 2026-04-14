//! Modèle de données intermédiaire TYP.
//!
//! Port littéral de `mkgmap/.../imgfmt/app/typ/TypData.java` (et classes sœurs
//! `TypElement`, `TypPoint`, `TypLine`, `TypPolygon`, `TypIconSet`, `TypLabel`,
//! `Xpm`, `ColourInfo`, `Rgb`, `DrawOrder`).
//!
//! Représentation indépendante du format (texte ou binaire) : à la fois
//! produit par `text_reader` / `binary_reader` et consommé par `text_writer` /
//! `binary_writer`.

/// Identité globale d'une carte TYP (section `[_id]`).
#[derive(Debug, Clone, Default)]
pub struct TypParams {
    /// Family ID (FID).
    pub family_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Codepage : 1252 (Windows-1252) ou 65001 (UTF-8).
    pub codepage: u16,
}

/// Couleur RGBA.
///
/// ⚠ **Convention inverse de mkgmap** :
/// - `a == 0`    → couleur **opaque** (affichée).
/// - `a == 0xff` → couleur **transparente** (mappée à `none` en XPM).
///
/// Ceci reflète l'« alpha-inverse » utilisé nativement par le format TYP
/// (cf. `encode_alpha_inverse` dans [`super::encoding`]). Toute nouvelle
/// logique couleur doit respecter cette convention sous peine de régressions
/// silencieuses — une assertion est plus sûre qu'un commentaire lointain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// `true` si la couleur représente un pixel transparent TYP.
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.a == 0xff
    }
}

/// Mode couleur d'une bitmap TYP.
///
/// - `Indexed` : palette + indices, bitmap LSB-first.
/// - `True16`  : mode 16-bit RGB565-like, pas de palette.
/// - `True32`  : mode 32-bit RGBA avec alpha inverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Indexed,
    True16,
    True32,
}

/// Bitmap XPM (mode jour ou nuit).
#[derive(Debug, Clone)]
pub struct Xpm {
    pub width: u16,
    pub height: u16,
    /// Palette (vide pour `True16`/`True32`).
    pub colors: Vec<Rgba>,
    /// Pixels : pour `Indexed` = indices palette ; pour true-color = données
    /// brutes (interprétation selon `mode`).
    pub pixels: Vec<u8>,
    pub mode: ColorMode,
}

/// Label multi-langues (`StringN=code,texte`).
#[derive(Debug, Clone)]
pub struct TypLabel {
    /// Code langue (cf. liste mkgmap `TypLabel.java`).
    pub lang: u8,
    pub text: String,
}

/// Style de police utilisé par les points/lignes/polygones.
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

/// Point TYP (élément rendu ponctuel).
#[derive(Debug, Clone)]
pub struct TypPoint {
    pub type_code: u32,
    pub subtype: u8,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    pub font_style: FontStyle,
    pub day_font_color: Option<Rgba>,
    pub night_font_color: Option<Rgba>,
}

/// Ligne TYP (polyline stylée).
#[derive(Debug, Clone)]
pub struct TypLine {
    pub type_code: u32,
    pub subtype: u8,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    /// Largeur du trait (0 = bitmap seul).
    pub line_width: u8,
    pub border_width: u8,
    pub font_style: FontStyle,
    pub day_font_color: Option<Rgba>,
    pub night_font_color: Option<Rgba>,
}

/// Polygone TYP (surface stylée).
#[derive(Debug, Clone)]
pub struct TypPolygon {
    pub type_code: u32,
    pub subtype: u8,
    pub labels: Vec<TypLabel>,
    pub day_xpm: Option<Xpm>,
    pub night_xpm: Option<Xpm>,
    pub font_style: FontStyle,
    pub day_font_color: Option<Rgba>,
    pub night_font_color: Option<Rgba>,
}

/// Ensemble d'icônes pour un POI à plusieurs résolutions.
#[derive(Debug, Clone)]
pub struct TypIconSet {
    pub type_code: u32,
    pub subtype: u8,
    /// Icône par niveau de zoom (généralement 3 à 5 variantes).
    pub icons: Vec<Xpm>,
}

/// Entrée de `[_drawOrder]` (shape stacking).
#[derive(Debug, Clone, Copy)]
pub struct DrawOrderEntry {
    pub type_code: u32,
    pub level: u8,
}

/// Document TYP complet.
#[derive(Debug, Clone, Default)]
pub struct TypData {
    pub params: TypParams,
    pub points: Vec<TypPoint>,
    pub lines: Vec<TypLine>,
    pub polygons: Vec<TypPolygon>,
    pub icons: Vec<TypIconSet>,
    pub draw_order: Vec<DrawOrderEntry>,
}

impl TypData {
    pub fn new() -> Self {
        Self::default()
    }
}
