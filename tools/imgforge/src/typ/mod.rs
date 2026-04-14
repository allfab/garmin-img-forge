//! TYP file format : conversion texte <-> binaire.
//!
//! Port de `mkgmap/src/uk/me/parabola/mkgmap/typ/` (writer) et implémentation
//! originale pour le décompileur binaire → texte (mkgmap ne décompile pas).

pub mod data;
pub mod encoding;
pub mod text_reader;
pub mod binary_writer;
pub mod binary_reader;
pub mod text_writer;

pub use data::*;

use crate::error::TypError;

/// Cible d'encodage d'I/O texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypEncoding {
    /// UTF-8 avec BOM pour écriture, auto-détection à la lecture.
    Utf8,
    /// Windows-1252 (CP1252) — legacy TYPViewer.
    Cp1252,
    /// Auto-détection à la lecture (BOM UTF-8 sinon CP1252).
    Auto,
}

/// Compile un fichier TYP texte en binaire.
///
/// `encoding_override` force l'interprétation des bytes d'entrée ; par défaut
/// (`Auto`) : BOM UTF-8 détecté → UTF-8, sinon CP1252.
pub fn compile_text_to_binary(
    text_bytes: &[u8],
    encoding_override: TypEncoding,
) -> Result<Vec<u8>, TypError> {
    let text = encoding::detect_and_decode(text_bytes, encoding_override)?;
    let data = text_reader::read_typ_text(&text)?;
    binary_writer::write_typ_binary(&data)
}

/// Décompile un fichier TYP binaire en texte.
///
/// `target_encoding` détermine l'encodage de sortie : `Utf8` / `Auto` →
/// UTF-8 avec BOM, `Cp1252` → Windows-1252.
pub fn decompile_binary_to_text(
    typ_bytes: &[u8],
    target_encoding: TypEncoding,
) -> Result<Vec<u8>, TypError> {
    let data = binary_reader::read_typ_binary(typ_bytes)?;
    text_writer::write_typ_text(&data, target_encoding)
}
