//! Helpers d'encodage TYP : codepage CP1252 ↔ UTF-8, packing type/subtype,
//! alpha inverse.

use super::TypEncoding;
use crate::error::TypError;

/// BOM UTF-8.
const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Longueur de l'en-tête TYP.
///
/// Trois valeurs historiques : `0x5B` (TYPViewer natif, sans icons/labels),
/// `0x6E` (+ icons), `0x9C` (+ labels/stringIndex/typeIndex, mkgmap natif).
///
/// Nous émettons `0x5B` pour **compatibilité maximale** avec QMapShack,
/// TYPViewer et les firmwares Garmin anciens. Le reader accepte les trois.
pub const HEADER_LEN: usize = 0x5B;

/// Décode des bytes texte en `String`.
///
/// - `Auto` : BOM UTF-8 détecté → UTF-8, sinon CP1252 (défaut historique
///   TYPViewer, cf. `I2023100.txt`).
/// - `Utf8` / `Cp1252` : force l'encodage.
///
/// Propage [`TypError::Encoding`] si l'UTF-8 contient des octets invalides
/// (CP1252 est tolérant : tout octet est mappé, pas d'erreur possible).
pub fn detect_and_decode(bytes: &[u8], forced: TypEncoding) -> Result<String, TypError> {
    let (enc, data) = match forced {
        TypEncoding::Utf8 => (encoding_rs::UTF_8, strip_bom(bytes)),
        TypEncoding::Cp1252 => (encoding_rs::WINDOWS_1252, bytes),
        TypEncoding::Auto => {
            if bytes.starts_with(&BOM_UTF8) {
                (encoding_rs::UTF_8, &bytes[3..])
            } else {
                (encoding_rs::WINDOWS_1252, bytes)
            }
        }
    };
    let (text, _, had_errors) = enc.decode(data);
    if had_errors {
        return Err(TypError::Encoding(format!(
            "decode errors using {}",
            enc.name()
        )));
    }
    Ok(text.into_owned())
}

/// Encode une `str` vers des bytes selon le codepage cible (1252 ou 65001).
///
/// Écrit le BOM UTF-8 en tête si `codepage == 65001`.
pub fn encode(text: &str, codepage: u16) -> Result<Vec<u8>, TypError> {
    match codepage {
        65001 => {
            let mut out = Vec::with_capacity(text.len() + 3);
            out.extend_from_slice(&BOM_UTF8);
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        1252 => {
            let (bytes, _, had_errors) = encoding_rs::WINDOWS_1252.encode(text);
            if had_errors {
                return Err(TypError::Encoding(
                    "character not representable in CP1252".into(),
                ));
            }
            Ok(bytes.into_owned())
        }
        other => Err(TypError::UnknownCodepage(other)),
    }
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(&BOM_UTF8) {
        &bytes[3..]
    } else {
        bytes
    }
}

/// Empaquette `(type, subtype)` en un `u16` : `(type << 5) | (subtype & 0x1f)`.
///
/// Cf. `TypElement.java`. `type` est tronqué sur 11 bits (masque `0x7ff`)
/// pour rester compatible du champ emballé.
#[inline]
pub fn pack_type_subtype(type_: u16, sub: u8) -> u16 {
    ((type_ & 0x7ff) << 5) | u16::from(sub & 0x1f)
}

/// Inverse de [`pack_type_subtype`].
#[inline]
pub fn unpack_type_subtype(v: u16) -> (u16, u8) {
    (v >> 5, (v & 0x1f) as u8)
}

/// Encode une valeur alpha TYP : `255 - ((n << 4) | (n & 0x0f))`.
///
/// Cf. `CommonSection.java:~210`. `n` ∈ [0, 15] (les 4 bits hauts sont
/// ignorés).
#[inline]
pub fn encode_alpha_inverse(n: u8) -> u8 {
    let n = n & 0x0f;
    255 - ((n << 4) | n)
}

/// Inverse de [`encode_alpha_inverse`] : récupère `n` ∈ [0, 15] depuis un
/// octet alpha TYP.
#[inline]
pub fn decode_alpha_inverse(b: u8) -> u8 {
    // `encode(n)` produit des octets de la forme `0xXX` où les deux nibbles
    // sont égaux. Décodage robuste : arrondir au `n` le plus proche.
    let inv = 255u16 - u16::from(b);
    // inv = (n << 4) | n = n * 17 ; on retrouve n.
    let n = ((inv + 8) / 17).min(15) as u8;
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_detection() {
        let utf8 = b"\xEF\xBB\xBFhello";
        let s = detect_and_decode(utf8, TypEncoding::Auto).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn cp1252_fallback() {
        // 0xE9 = 'é' en CP1252, illegal en UTF-8 seul.
        let bytes = b"caf\xE9";
        let s = detect_and_decode(bytes, TypEncoding::Auto).unwrap();
        assert_eq!(s, "café");
    }

    #[test]
    fn force_cp1252() {
        let bytes = b"caf\xE9";
        let s = detect_and_decode(bytes, TypEncoding::Cp1252).unwrap();
        assert_eq!(s, "café");
    }

    #[test]
    fn force_utf8_invalid_fails() {
        let bytes = b"caf\xE9";
        assert!(detect_and_decode(bytes, TypEncoding::Utf8).is_err());
    }

    #[test]
    fn encode_utf8_has_bom() {
        let out = encode("café", 65001).unwrap();
        assert!(out.starts_with(&BOM_UTF8));
        assert_eq!(&out[3..], "café".as_bytes());
    }

    #[test]
    fn encode_cp1252_roundtrip() {
        let out = encode("café", 1252).unwrap();
        assert_eq!(out, b"caf\xE9");
    }

    #[test]
    fn encode_unknown_codepage_errors() {
        assert!(matches!(
            encode("x", 437),
            Err(TypError::UnknownCodepage(437))
        ));
    }

    #[test]
    fn type_subtype_roundtrip() {
        for t in [0u16, 1, 0x2a, 0x10f, 0x7ff] {
            for s in 0u8..32 {
                let v = pack_type_subtype(t, s);
                assert_eq!(unpack_type_subtype(v), (t, s));
            }
        }
    }

    #[test]
    fn type_subtype_packing() {
        // 0x2a = type 42, subtype 5 → (42<<5)|5 = 0x545
        assert_eq!(pack_type_subtype(42, 5), (42 << 5) | 5);
    }

    #[test]
    fn alpha_inverse_roundtrip() {
        for n in 0u8..16 {
            let enc = encode_alpha_inverse(n);
            assert_eq!(decode_alpha_inverse(enc), n, "failed for n={}", n);
        }
    }

    #[test]
    fn alpha_inverse_known_values() {
        // n=0 → 0xFF (opaque en alpha-inverse)
        assert_eq!(encode_alpha_inverse(0), 0xFF);
        // n=15 → 0 (transparent)
        assert_eq!(encode_alpha_inverse(15), 0);
    }
}
