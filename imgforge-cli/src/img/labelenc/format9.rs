// Format9Encoder — single-byte codepage encoding, faithful to mkgmap AnyCharsetEncoder.java
// Default: CP1252 (Windows Latin-1)

/// Encode text as single-byte codepage (CP1252 by default). Null-terminated.
pub fn encode(text: &str, _codepage: u16) -> Vec<u8> {
    if text.is_empty() {
        return vec![0x00];
    }

    let mut buf = Vec::with_capacity(text.len() + 1);
    for c in text.chars() {
        if let Some(b) = unicode_to_cp1252(c) {
            buf.push(b);
        } else {
            buf.push(b'?');
        }
    }
    buf.push(0x00); // null terminator
    buf
}

/// Decode single-byte codepage bytes to string. Stops at null terminator.
pub fn decode(data: &[u8], _codepage: u16) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    data[..end].iter().map(|&b| cp1252_to_unicode(b)).collect()
}

/// Map Unicode code point to CP1252 byte value
fn unicode_to_cp1252(c: char) -> Option<u8> {
    let cp = c as u32;
    // ASCII + Latin-1 Supplement (0x00-0x7F, 0xA0-0xFF) map directly
    if cp <= 0x7F {
        return Some(cp as u8);
    }
    if cp >= 0xA0 && cp <= 0xFF {
        return Some(cp as u8);
    }
    // CP1252 special range 0x80-0x9F: Unicode code points differ from byte values
    match c {
        '\u{20AC}' => Some(0x80), // €
        '\u{201A}' => Some(0x82), // ‚
        '\u{0192}' => Some(0x83), // ƒ
        '\u{201E}' => Some(0x84), // „
        '\u{2026}' => Some(0x85), // …
        '\u{2020}' => Some(0x86), // †
        '\u{2021}' => Some(0x87), // ‡
        '\u{02C6}' => Some(0x88), // ˆ
        '\u{2030}' => Some(0x89), // ‰
        '\u{0160}' => Some(0x8A), // Š
        '\u{2039}' => Some(0x8B), // ‹
        '\u{0152}' => Some(0x8C), // Œ
        '\u{017D}' => Some(0x8E), // Ž
        '\u{2018}' => Some(0x91), // '
        '\u{2019}' => Some(0x92), // '
        '\u{201C}' => Some(0x93), // "
        '\u{201D}' => Some(0x94), // "
        '\u{2022}' => Some(0x95), // •
        '\u{2013}' => Some(0x96), // –
        '\u{2014}' => Some(0x97), // —
        '\u{02DC}' => Some(0x98), // ˜
        '\u{2122}' => Some(0x99), // ™
        '\u{0161}' => Some(0x9A), // š
        '\u{203A}' => Some(0x9B), // ›
        '\u{0153}' => Some(0x9C), // œ
        '\u{017E}' => Some(0x9E), // ž
        '\u{0178}' => Some(0x9F), // Ÿ
        _ => None,
    }
}

/// Map CP1252 byte to Unicode character
fn cp1252_to_unicode(b: u8) -> char {
    match b {
        0x80 => '\u{20AC}', // €
        0x82 => '\u{201A}', // ‚
        0x83 => '\u{0192}', // ƒ
        0x84 => '\u{201E}', // „
        0x85 => '\u{2026}', // …
        0x86 => '\u{2020}', // †
        0x87 => '\u{2021}', // ‡
        0x88 => '\u{02C6}', // ˆ
        0x89 => '\u{2030}', // ‰
        0x8A => '\u{0160}', // Š
        0x8B => '\u{2039}', // ‹
        0x8C => '\u{0152}', // Œ
        0x8E => '\u{017D}', // Ž
        0x91 => '\u{2018}', // '
        0x92 => '\u{2019}', // '
        0x93 => '\u{201C}', // "
        0x94 => '\u{201D}', // "
        0x95 => '\u{2022}', // •
        0x96 => '\u{2013}', // –
        0x97 => '\u{2014}', // —
        0x98 => '\u{02DC}', // ˜
        0x99 => '\u{2122}', // ™
        0x9A => '\u{0161}', // š
        0x9B => '\u{203A}', // ›
        0x9C => '\u{0153}', // œ
        0x9E => '\u{017E}', // ž
        0x9F => '\u{0178}', // Ÿ
        0x81 | 0x8D | 0x8F | 0x90 | 0x9D => '?', // undefined in CP1252
        _ => b as char, // 0x00-0x7F and 0xA0-0xFF map directly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_simple() {
        let encoded = encode("Hello", 1252);
        assert_eq!(&encoded[..5], b"Hello");
        assert_eq!(encoded[5], 0x00);
    }

    #[test]
    fn test_encode_accented() {
        let encoded = encode("Château", 1252);
        assert_eq!(encoded[2], 0xE2); // â
        assert_eq!(*encoded.last().unwrap(), 0x00);
    }

    #[test]
    fn test_decode_roundtrip() {
        let text = "Château Fort";
        let encoded = encode(text, 1252);
        let decoded = decode(&encoded, 1252);
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_empty() {
        let encoded = encode("", 1252);
        assert_eq!(encoded, vec![0x00]);
    }

    #[test]
    fn test_non_mappable() {
        let encoded = encode("日本語", 1252);
        assert_eq!(encoded[0], b'?');
    }

    #[test]
    fn test_euro_sign() {
        let encoded = encode("€", 1252);
        assert_eq!(encoded[0], 0x80);
        let decoded = decode(&encoded, 1252);
        assert_eq!(decoded, "€");
    }

    #[test]
    fn test_oe_ligature() {
        let encoded = encode("œ", 1252);
        assert_eq!(encoded[0], 0x9C);
        let decoded = decode(&encoded, 1252);
        assert_eq!(decoded, "œ");
    }
}
