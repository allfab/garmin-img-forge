// Format10Encoder — UTF-8 encoding, faithful to mkgmap Utf8Encoder.java

/// Encode text as UTF-8. Null-terminated.
pub fn encode(text: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(text.len() + 1);
    buf.extend_from_slice(text.as_bytes());
    buf.push(0x00); // null terminator
    buf
}

/// Decode UTF-8 bytes to string. Stops at null terminator.
#[cfg(test)]
fn decode(data: &[u8]) -> String {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_ascii() {
        let encoded = encode("Hello");
        assert_eq!(&encoded[..5], b"Hello");
        assert_eq!(encoded[5], 0x00);
    }

    #[test]
    fn test_encode_utf8() {
        let encoded = encode("日本語");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "日本語");
    }

    #[test]
    fn test_encode_french() {
        let encoded = encode("Château de Versailles");
        let decoded = decode(&encoded);
        assert_eq!(decoded, "Château de Versailles");
    }

    #[test]
    fn test_empty() {
        let encoded = encode("");
        assert_eq!(encoded, vec![0x00]);
        let decoded = decode(&encoded);
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_null_terminated() {
        let encoded = encode("Test");
        assert_eq!(*encoded.last().unwrap(), 0x00);
    }
}
