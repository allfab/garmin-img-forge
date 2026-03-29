// Format6Encoder — 6-bit packed ASCII, faithful to mkgmap Format6Encoder.java
// Uses the specific c<<2 packing scheme from mkgmap's put6/get6

const SYMBOL_SHIFT: u8 = 0x1C;
const SYMBOLS: &str = "@!\"#$%&'()*+,-./";

/// Encode text in Format6 (6-bit packed ASCII, uppercase only)
pub fn encode(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }

    let upper = transliterate_and_upper(text);
    let mut codes: Vec<u8> = Vec::with_capacity(upper.len() * 2);

    for c in upper.chars() {
        if c == ' ' {
            codes.push(0x00);
        } else if c.is_ascii_uppercase() {
            codes.push((c as u8) - b'A' + 1);
        } else if c.is_ascii_digit() {
            codes.push((c as u8) - b'0' + 0x20);
        } else if let Some(idx) = SYMBOLS.find(c) {
            codes.push(SYMBOL_SHIFT);
            codes.push(idx as u8);
        }
    }

    // Terminator
    codes.push(0xFF);

    // Pack using mkgmap's put6 algorithm
    let buf_len = codes.len() * 2 + 4;
    let mut buf = vec![0u8; buf_len];

    for (off, &c) in codes.iter().enumerate() {
        put6(&mut buf, off, c);
    }

    // Length formula from mkgmap: ((off - 1) * 6) / 8 + 1
    let len = ((codes.len() - 1) * 6) / 8 + 1;
    buf.truncate(len);
    buf
}

/// Decode Format6 bytes back to string
pub fn decode(data: &[u8]) -> String {
    let mut result = String::new();
    let total_bits = data.len() * 8;
    let mut i = 0;

    // Pad buffer so get6 can safely read a second byte at boundaries
    let mut padded = data.to_vec();
    padded.push(0xFF); // pad with terminator bits
    padded.push(0xFF);

    while i * 6 + 6 <= total_bits + 2 {
        let v = get6(&padded, i);
        if v >= 0x3F {
            break; // terminator (0x3F from 0xFF code)
        }
        if v == SYMBOL_SHIFT {
            i += 1;
            let sym = get6(&padded, i);
            if (sym as usize) < SYMBOLS.len() {
                result.push(SYMBOLS.as_bytes()[sym as usize] as char);
            }
        } else if v == 0x00 {
            result.push(' ');
        } else if v >= 1 && v <= 26 {
            result.push((b'A' + v - 1) as char);
        } else if v >= 0x20 && v <= 0x29 {
            result.push((b'0' + v - 0x20) as char);
        }
        i += 1;
    }

    result
}

/// mkgmap Format6Encoder.put6 — packs one 6-bit character
fn put6(buf: &mut [u8], off: usize, c: u8) {
    let bit_off = off * 6;
    let byte_off = bit_off / 8;
    let shift = bit_off % 8;

    let c16 = (c as u16) << 2;
    let mask = 0xFCu16 >> shift;
    buf[byte_off] |= ((c16 >> shift) & mask) as u8;

    if shift > 2 && byte_off + 1 < buf.len() {
        let mask2 = (0xFCu16 << (8 - shift)) & 0xFF;
        buf[byte_off + 1] = ((c16 << (8 - shift)) & mask2) as u8;
    }
}

/// Reverse of put6 — extracts one 6-bit character
fn get6(buf: &[u8], off: usize) -> u8 {
    let bit_off = off * 6;
    let byte_off = bit_off / 8;
    let shift = bit_off % 8;

    if byte_off >= buf.len() {
        return 0x3F; // EOF = terminator
    }

    // Reverse of: buf[byteOff] |= ((c << 2) >> shift) & (0xfc >> shift)
    let mask1 = (0xFCu16 >> shift) as u8;
    let part1 = ((buf[byte_off] & mask1) as u16) << shift;

    let mut val = part1;

    if shift > 2 && byte_off + 1 < buf.len() {
        // Reverse of: buf[byteOff + 1] = ((c << 2) << (8 - shift)) & (0xfc << (8 - shift))
        let mask2 = ((0xFCu16 << (8 - shift)) & 0xFF) as u8;
        let part2 = ((buf[byte_off + 1] & mask2) as u16) >> (8 - shift);
        val |= part2;
    }

    // val is (c << 2)
    ((val >> 2) & 0x3F) as u8
}

/// Basic ASCII transliteration and uppercase
fn transliterate_and_upper(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => result.push('A'),
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => result.push('E'),
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => result.push('I'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => result.push('O'),
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'ů' | 'Ů' => result.push('U'),
            'ñ' | 'Ñ' | 'ň' | 'Ň' => result.push('N'),
            'ç' | 'Ç' | 'č' | 'Č' => result.push('C'),
            'ß' => { result.push('S'); result.push('S'); },
            'æ' | 'Æ' => { result.push('A'); result.push('E'); },
            'œ' | 'Œ' => { result.push('O'); result.push('E'); },
            'ý' | 'ÿ' | 'Ý' => result.push('Y'),
            'ð' | 'Ð' | 'ď' | 'Ď' => result.push('D'),
            'þ' | 'Þ' | 'ť' | 'Ť' => result.push('T'),
            'ø' | 'Ø' => result.push('O'),
            'ř' | 'Ř' => result.push('R'),
            'š' | 'Š' => result.push('S'),
            'ž' | 'Ž' => result.push('Z'),
            'ě' | 'Ě' => result.push('E'),
            _ if c.is_ascii() => result.push(c.to_ascii_uppercase()),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put6_get6_roundtrip() {
        let mut buf = vec![0u8; 20];
        put6(&mut buf, 0, 1);  // A
        put6(&mut buf, 1, 2);  // B
        put6(&mut buf, 2, 3);  // C

        assert_eq!(get6(&buf, 0), 1);
        assert_eq!(get6(&buf, 1), 2);
        assert_eq!(get6(&buf, 2), 3);
    }

    #[test]
    fn test_put6_get6_all_codes() {
        let mut buf = vec![0u8; 200];
        for code in 0..0x30u8 {
            buf.fill(0);
            put6(&mut buf, 0, code);
            assert_eq!(get6(&buf, 0), code, "failed for code {code}");
        }
    }

    #[test]
    fn test_encode_simple() {
        let encoded = encode("ROUTE");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "ROUTE");
    }

    #[test]
    fn test_encode_with_spaces() {
        let encoded = encode("MAIN STREET");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "MAIN STREET");
    }

    #[test]
    fn test_encode_with_numbers() {
        let encoded = encode("ROUTE 66");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "ROUTE 66");
    }

    #[test]
    fn test_encode_lowercase_becomes_upper() {
        let encoded = encode("hello");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "HELLO");
    }

    #[test]
    fn test_encode_with_symbols() {
        let encoded = encode("MAIN-STREET");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "MAIN-STREET");
    }

    #[test]
    fn test_encode_accented() {
        let encoded = encode("Château");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "CHATEAU");
    }

    #[test]
    fn test_encode_german() {
        let encoded = encode("Straße");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "STRASSE");
    }

    #[test]
    fn test_empty() {
        let encoded = encode("");
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_single_char() {
        let encoded = encode("A");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "A");
    }
}
