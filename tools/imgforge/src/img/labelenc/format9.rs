// Format9Encoder — single-byte codepage encoding, faithful to mkgmap AnyCharsetEncoder.java
// Default: CP1252 (Windows Latin-1)

/// Encode text as single-byte codepage. Null-terminated.
/// Supports CP1252 (Western), CP1250 (Central European), CP1251 (Cyrillic).
pub fn encode(text: &str, codepage: u16) -> Vec<u8> {
    if text.is_empty() {
        return vec![0x00];
    }

    let mut buf = Vec::with_capacity(text.len() + 1);
    for c in text.chars() {
        let b = match codepage {
            1250 => unicode_to_cp1250(c),
            1251 => unicode_to_cp1251(c),
            _ => unicode_to_cp1252(c), // default/1252
        };
        buf.push(b.unwrap_or(b'?'));
    }
    buf.push(0x00); // null terminator
    buf
}

/// Decode single-byte codepage bytes to string. Stops at null terminator.
pub fn decode(data: &[u8], codepage: u16) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    data[..end].iter().map(|&b| match codepage {
        1250 => cp1250_to_unicode(b),
        1251 => cp1251_to_unicode(b),
        _ => cp1252_to_unicode(b),
    }).collect()
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
pub fn cp1252_to_unicode(b: u8) -> char {
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

    #[test]
    fn test_cp1250_encode_decode() {
        // Polish: ą = 0xB9, ć = 0xE6, ę = 0xEA, ł = 0xB3, ń = 0xF1, ś = 0x9C, ź = 0x9F, ż = 0xBF
        let encoded = encode("ą", 1250);
        assert_eq!(encoded[0], 0xB9);
        let decoded = decode(&encoded, 1250);
        assert_eq!(decoded, "ą");

        let encoded = encode("ś", 1250);
        assert_eq!(encoded[0], 0x9C);
    }

    #[test]
    fn test_cp1251_encode_decode() {
        // Russian: А = 0xC0, Б = 0xC1, В = 0xC2
        let encoded = encode("АБВ", 1251);
        assert_eq!(encoded[0], 0xC0);
        assert_eq!(encoded[1], 0xC1);
        assert_eq!(encoded[2], 0xC2);
        let decoded = decode(&encoded, 1251);
        assert_eq!(decoded, "АБВ");
    }
}

// ── CP1250 (Central European) ───────────────────────────────────────────

/// Map Unicode code point to CP1250 byte value
fn unicode_to_cp1250(c: char) -> Option<u8> {
    let cp = c as u32;
    if cp <= 0x7F { return Some(cp as u8); }
    // Direct mappings for 0xA0-0xFF range where CP1250 matches Unicode
    match c {
        '\u{00A0}' => Some(0xA0), // NBSP
        '\u{00A4}' => Some(0xA4), // ¤
        '\u{00A6}' => Some(0xA6), // ¦
        '\u{00A7}' => Some(0xA7), // §
        '\u{00A8}' => Some(0xA8), // ¨
        '\u{00A9}' => Some(0xA9), // ©
        '\u{00AB}' => Some(0xAB), // «
        '\u{00AC}' => Some(0xAC), // ¬
        '\u{00AD}' => Some(0xAD), // SHY
        '\u{00AE}' => Some(0xAE), // ®
        '\u{00B0}' => Some(0xB0), // °
        '\u{00B1}' => Some(0xB1), // ±
        '\u{00B4}' => Some(0xB4), // ´
        '\u{00B5}' => Some(0xB5), // µ
        '\u{00B6}' => Some(0xB6), // ¶
        '\u{00B7}' => Some(0xB7), // ·
        '\u{00B8}' => Some(0xB8), // ¸
        '\u{00BB}' => Some(0xBB), // »
        '\u{00C1}' => Some(0xC1), // Á
        '\u{00C2}' => Some(0xC2), // Â
        '\u{00C4}' => Some(0xC4), // Ä
        '\u{00C7}' => Some(0xC7), // Ç
        '\u{00C9}' => Some(0xC9), // É
        '\u{00CB}' => Some(0xCB), // Ë
        '\u{00CD}' => Some(0xCD), // Í
        '\u{00CE}' => Some(0xCE), // Î
        '\u{00D3}' => Some(0xD3), // Ó
        '\u{00D4}' => Some(0xD4), // Ô
        '\u{00D6}' => Some(0xD6), // Ö
        '\u{00D7}' => Some(0xD7), // ×
        '\u{00DA}' => Some(0xDA), // Ú
        '\u{00DC}' => Some(0xDC), // Ü
        '\u{00DD}' => Some(0xDD), // Ý
        '\u{00DF}' => Some(0xDF), // ß
        '\u{00E1}' => Some(0xE1), // á
        '\u{00E2}' => Some(0xE2), // â
        '\u{00E4}' => Some(0xE4), // ä
        '\u{00E7}' => Some(0xE7), // ç
        '\u{00E9}' => Some(0xE9), // é
        '\u{00EB}' => Some(0xEB), // ë
        '\u{00ED}' => Some(0xED), // í
        '\u{00EE}' => Some(0xEE), // î
        '\u{00F3}' => Some(0xF3), // ó
        '\u{00F4}' => Some(0xF4), // ô
        '\u{00F6}' => Some(0xF6), // ö
        '\u{00F7}' => Some(0xF7), // ÷
        '\u{00FA}' => Some(0xFA), // ú
        '\u{00FC}' => Some(0xFC), // ü
        '\u{00FD}' => Some(0xFD), // ý
        // CP1250-specific mappings (0x80-0x9F and other)
        '\u{20AC}' => Some(0x80), // €
        '\u{201A}' => Some(0x82), // ‚
        '\u{201E}' => Some(0x84), // „
        '\u{2026}' => Some(0x85), // …
        '\u{2020}' => Some(0x86), // †
        '\u{2021}' => Some(0x87), // ‡
        '\u{2030}' => Some(0x89), // ‰
        '\u{0160}' => Some(0x8A), // Š
        '\u{2039}' => Some(0x8B), // ‹
        '\u{015A}' => Some(0x8C), // Ś
        '\u{0164}' => Some(0x8D), // Ť
        '\u{017D}' => Some(0x8E), // Ž
        '\u{0179}' => Some(0x8F), // Ź
        '\u{2018}' => Some(0x91), // '
        '\u{2019}' => Some(0x92), // '
        '\u{201C}' => Some(0x93), // "
        '\u{201D}' => Some(0x94), // "
        '\u{2022}' => Some(0x95), // •
        '\u{2013}' => Some(0x96), // –
        '\u{2014}' => Some(0x97), // —
        '\u{2122}' => Some(0x99), // ™
        '\u{0161}' => Some(0x9A), // š
        '\u{203A}' => Some(0x9B), // ›
        '\u{015B}' => Some(0x9C), // ś
        '\u{0165}' => Some(0x9D), // ť
        '\u{017E}' => Some(0x9E), // ž
        '\u{017A}' => Some(0x9F), // ź
        '\u{02C7}' => Some(0xA1), // ˇ
        '\u{02D8}' => Some(0xA2), // ˘
        '\u{0141}' => Some(0xA3), // Ł
        '\u{0104}' => Some(0xA5), // Ą
        '\u{015E}' => Some(0xAA), // Ş
        '\u{017B}' => Some(0xAF), // Ż
        '\u{02DB}' => Some(0xB2), // ˛
        '\u{0142}' => Some(0xB3), // ł
        '\u{0105}' => Some(0xB9), // ą
        '\u{015F}' => Some(0xBA), // ş
        '\u{013D}' => Some(0xBC), // Ľ
        '\u{02DD}' => Some(0xBD), // ˝
        '\u{013E}' => Some(0xBE), // ľ
        '\u{017C}' => Some(0xBF), // ż
        '\u{0154}' => Some(0xC0), // Ŕ
        '\u{0102}' => Some(0xC3), // Ă
        '\u{0139}' => Some(0xC5), // Ĺ
        '\u{0106}' => Some(0xC6), // Ć
        '\u{010C}' => Some(0xC8), // Č
        '\u{0118}' => Some(0xCA), // Ę
        '\u{011A}' => Some(0xCC), // Ě
        '\u{010E}' => Some(0xCF), // Ď
        '\u{0110}' => Some(0xD0), // Đ
        '\u{0143}' => Some(0xD1), // Ń
        '\u{0147}' => Some(0xD2), // Ň
        '\u{0150}' => Some(0xD5), // Ő
        '\u{0158}' => Some(0xD8), // Ř
        '\u{016E}' => Some(0xD9), // Ů
        '\u{0170}' => Some(0xDB), // Ű
        '\u{0162}' => Some(0xDE), // Ţ
        '\u{0155}' => Some(0xE0), // ŕ
        '\u{0103}' => Some(0xE3), // ă
        '\u{013A}' => Some(0xE5), // ĺ
        '\u{0107}' => Some(0xE6), // ć
        '\u{010D}' => Some(0xE8), // č
        '\u{0119}' => Some(0xEA), // ę
        '\u{011B}' => Some(0xEC), // ě
        '\u{010F}' => Some(0xEF), // ď
        '\u{0111}' => Some(0xF0), // đ
        '\u{0144}' => Some(0xF1), // ń
        '\u{0148}' => Some(0xF2), // ň
        '\u{0151}' => Some(0xF5), // ő
        '\u{0159}' => Some(0xF8), // ř
        '\u{016F}' => Some(0xF9), // ů
        '\u{0171}' => Some(0xFB), // ű
        '\u{0163}' => Some(0xFE), // ţ
        '\u{02D9}' => Some(0xFF), // ˙
        _ => None,
    }
}

/// Map CP1250 byte to Unicode character
fn cp1250_to_unicode(b: u8) -> char {
    // 0x00-0x7F: ASCII (direct)
    if b <= 0x7F { return b as char; }
    match b {
        0x80 => '\u{20AC}', 0x82 => '\u{201A}', 0x84 => '\u{201E}',
        0x85 => '\u{2026}', 0x86 => '\u{2020}', 0x87 => '\u{2021}',
        0x89 => '\u{2030}', 0x8A => '\u{0160}', 0x8B => '\u{2039}',
        0x8C => '\u{015A}', 0x8D => '\u{0164}', 0x8E => '\u{017D}',
        0x8F => '\u{0179}', 0x91 => '\u{2018}', 0x92 => '\u{2019}',
        0x93 => '\u{201C}', 0x94 => '\u{201D}', 0x95 => '\u{2022}',
        0x96 => '\u{2013}', 0x97 => '\u{2014}', 0x99 => '\u{2122}',
        0x9A => '\u{0161}', 0x9B => '\u{203A}', 0x9C => '\u{015B}',
        0x9D => '\u{0165}', 0x9E => '\u{017E}', 0x9F => '\u{017A}',
        0xA0 => '\u{00A0}', 0xA1 => '\u{02C7}', 0xA2 => '\u{02D8}',
        0xA3 => '\u{0141}', 0xA4 => '\u{00A4}', 0xA5 => '\u{0104}',
        0xA6 => '\u{00A6}', 0xA7 => '\u{00A7}', 0xA8 => '\u{00A8}',
        0xA9 => '\u{00A9}', 0xAA => '\u{015E}', 0xAB => '\u{00AB}',
        0xAC => '\u{00AC}', 0xAD => '\u{00AD}', 0xAE => '\u{00AE}',
        0xAF => '\u{017B}', 0xB0 => '\u{00B0}', 0xB1 => '\u{00B1}',
        0xB2 => '\u{02DB}', 0xB3 => '\u{0142}', 0xB4 => '\u{00B4}',
        0xB5 => '\u{00B5}', 0xB6 => '\u{00B6}', 0xB7 => '\u{00B7}',
        0xB8 => '\u{00B8}', 0xB9 => '\u{0105}', 0xBA => '\u{015F}',
        0xBB => '\u{00BB}', 0xBC => '\u{013D}', 0xBD => '\u{02DD}',
        0xBE => '\u{013E}', 0xBF => '\u{017C}',
        0xC0 => '\u{0154}', 0xC1 => '\u{00C1}', 0xC2 => '\u{00C2}',
        0xC3 => '\u{0102}', 0xC4 => '\u{00C4}', 0xC5 => '\u{0139}',
        0xC6 => '\u{0106}', 0xC7 => '\u{00C7}', 0xC8 => '\u{010C}',
        0xC9 => '\u{00C9}', 0xCA => '\u{0118}', 0xCB => '\u{00CB}',
        0xCC => '\u{011A}', 0xCD => '\u{00CD}', 0xCE => '\u{00CE}',
        0xCF => '\u{010E}', 0xD0 => '\u{0110}', 0xD1 => '\u{0143}',
        0xD2 => '\u{0147}', 0xD3 => '\u{00D3}', 0xD4 => '\u{00D4}',
        0xD5 => '\u{0150}', 0xD6 => '\u{00D6}', 0xD7 => '\u{00D7}',
        0xD8 => '\u{0158}', 0xD9 => '\u{016E}', 0xDA => '\u{00DA}',
        0xDB => '\u{0170}', 0xDC => '\u{00DC}', 0xDD => '\u{00DD}',
        0xDE => '\u{0162}', 0xDF => '\u{00DF}',
        0xE0 => '\u{0155}', 0xE1 => '\u{00E1}', 0xE2 => '\u{00E2}',
        0xE3 => '\u{0103}', 0xE4 => '\u{00E4}', 0xE5 => '\u{013A}',
        0xE6 => '\u{0107}', 0xE7 => '\u{00E7}', 0xE8 => '\u{010D}',
        0xE9 => '\u{00E9}', 0xEA => '\u{0119}', 0xEB => '\u{00EB}',
        0xEC => '\u{011B}', 0xED => '\u{00ED}', 0xEE => '\u{00EE}',
        0xEF => '\u{010F}', 0xF0 => '\u{0111}', 0xF1 => '\u{0144}',
        0xF2 => '\u{0148}', 0xF3 => '\u{00F3}', 0xF4 => '\u{00F4}',
        0xF5 => '\u{0151}', 0xF6 => '\u{00F6}', 0xF7 => '\u{00F7}',
        0xF8 => '\u{0159}', 0xF9 => '\u{016F}', 0xFA => '\u{00FA}',
        0xFB => '\u{0171}', 0xFC => '\u{00FC}', 0xFD => '\u{00FD}',
        0xFE => '\u{0163}', 0xFF => '\u{02D9}',
        0x81 | 0x83 | 0x88 | 0x90 | 0x98 => '?', // undefined
        _ => b as char,
    }
}

// ── CP1251 (Cyrillic) ───────────────────────────────────────────────────

/// Map Unicode code point to CP1251 byte value
fn unicode_to_cp1251(c: char) -> Option<u8> {
    let cp = c as u32;
    if cp <= 0x7F { return Some(cp as u8); }
    // Cyrillic block: А-я (U+0410-U+044F) maps to 0xC0-0xFF
    if cp >= 0x0410 && cp <= 0x044F {
        return Some((cp - 0x0410 + 0xC0) as u8);
    }
    match c {
        '\u{0401}' => Some(0xA8), // Ё
        '\u{0402}' => Some(0x80), // Ђ
        '\u{0403}' => Some(0x81), // Ѓ
        '\u{0404}' => Some(0xAA), // Є
        '\u{0405}' => Some(0xBD), // Ѕ
        '\u{0406}' => Some(0xB2), // І
        '\u{0407}' => Some(0xAF), // Ї
        '\u{0408}' => Some(0xA3), // Ј
        '\u{0409}' => Some(0x8A), // Љ
        '\u{040A}' => Some(0x8C), // Њ
        '\u{040B}' => Some(0x8E), // Ћ
        '\u{040C}' => Some(0x8D), // Ќ
        '\u{040E}' => Some(0xA1), // Ў
        '\u{040F}' => Some(0x8F), // Џ
        '\u{0451}' => Some(0xB8), // ё
        '\u{0452}' => Some(0x90), // ђ
        '\u{0453}' => Some(0x83), // ѓ
        '\u{0454}' => Some(0xBA), // є
        '\u{0455}' => Some(0xBE), // ѕ
        '\u{0456}' => Some(0xB3), // і
        '\u{0457}' => Some(0xBF), // ї
        '\u{0458}' => Some(0xBC), // ј
        '\u{0459}' => Some(0x9A), // љ
        '\u{045A}' => Some(0x9C), // њ
        '\u{045B}' => Some(0x9E), // ћ
        '\u{045C}' => Some(0x9D), // ќ
        '\u{045E}' => Some(0xA2), // ў
        '\u{045F}' => Some(0x9F), // џ
        '\u{0490}' => Some(0xA5), // Ґ
        '\u{0491}' => Some(0xB4), // ґ
        '\u{20AC}' => Some(0x88), // €
        '\u{2026}' => Some(0x85), // …
        '\u{2013}' => Some(0x96), // –
        '\u{2014}' => Some(0x97), // —
        '\u{2018}' => Some(0x91), // '
        '\u{2019}' => Some(0x92), // '
        '\u{201C}' => Some(0x93), // "
        '\u{201D}' => Some(0x94), // "
        '\u{201E}' => Some(0x84), // „
        '\u{2116}' => Some(0xB9), // №
        '\u{2122}' => Some(0x99), // ™
        '\u{00A0}' => Some(0xA0), // NBSP
        '\u{00A4}' => Some(0xA4), // ¤
        '\u{00A6}' => Some(0xA6), // ¦
        '\u{00A7}' => Some(0xA7), // §
        '\u{00A9}' => Some(0xA9), // ©
        '\u{00AB}' => Some(0xAB), // «
        '\u{00AC}' => Some(0xAC), // ¬
        '\u{00AD}' => Some(0xAD), // SHY
        '\u{00AE}' => Some(0xAE), // ®
        '\u{00B0}' => Some(0xB0), // °
        '\u{00B1}' => Some(0xB1), // ±
        '\u{00B5}' => Some(0xB5), // µ
        '\u{00B6}' => Some(0xB6), // ¶
        '\u{00B7}' => Some(0xB7), // ·
        '\u{00BB}' => Some(0xBB), // »
        _ => None,
    }
}

/// Map CP1251 byte to Unicode character
fn cp1251_to_unicode(b: u8) -> char {
    if b <= 0x7F { return b as char; }
    // Cyrillic block: 0xC0-0xFF → U+0410-U+044F
    if b >= 0xC0 { return char::from_u32((b as u32) - 0xC0 + 0x0410).unwrap_or('?'); }
    match b {
        0x80 => '\u{0402}', 0x81 => '\u{0403}', 0x82 => '\u{201A}',
        0x83 => '\u{0453}', 0x84 => '\u{201E}', 0x85 => '\u{2026}',
        0x86 => '\u{2020}', 0x87 => '\u{2021}', 0x88 => '\u{20AC}',
        0x89 => '\u{2030}', 0x8A => '\u{0409}', 0x8B => '\u{2039}',
        0x8C => '\u{040A}', 0x8D => '\u{040C}', 0x8E => '\u{040B}',
        0x8F => '\u{040F}', 0x90 => '\u{0452}', 0x91 => '\u{2018}',
        0x92 => '\u{2019}', 0x93 => '\u{201C}', 0x94 => '\u{201D}',
        0x95 => '\u{2022}', 0x96 => '\u{2013}', 0x97 => '\u{2014}',
        0x99 => '\u{2122}', 0x9A => '\u{0459}', 0x9B => '\u{203A}',
        0x9C => '\u{045A}', 0x9D => '\u{045C}', 0x9E => '\u{045B}',
        0x9F => '\u{045F}', 0xA0 => '\u{00A0}', 0xA1 => '\u{040E}',
        0xA2 => '\u{045E}', 0xA3 => '\u{0408}', 0xA4 => '\u{00A4}',
        0xA5 => '\u{0490}', 0xA6 => '\u{00A6}', 0xA7 => '\u{00A7}',
        0xA8 => '\u{0401}', 0xA9 => '\u{00A9}', 0xAA => '\u{0404}',
        0xAB => '\u{00AB}', 0xAC => '\u{00AC}', 0xAD => '\u{00AD}',
        0xAE => '\u{00AE}', 0xAF => '\u{0407}', 0xB0 => '\u{00B0}',
        0xB1 => '\u{00B1}', 0xB2 => '\u{0406}', 0xB3 => '\u{0456}',
        0xB4 => '\u{0491}', 0xB5 => '\u{00B5}', 0xB6 => '\u{00B6}',
        0xB7 => '\u{00B7}', 0xB8 => '\u{0451}', 0xB9 => '\u{2116}',
        0xBA => '\u{0454}', 0xBB => '\u{00BB}', 0xBC => '\u{0458}',
        0xBD => '\u{0405}', 0xBE => '\u{0455}', 0xBF => '\u{0457}',
        0x98 => '?', // undefined
        _ => '?',
    }
}
