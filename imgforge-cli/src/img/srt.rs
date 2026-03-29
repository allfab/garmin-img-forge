// SRT — Sort routines for Garmin label collation

/// Generate a basic French sort table for gmapsupp
pub fn build_french_srt() -> Vec<u8> {
    // Minimal SRT: header + collation table
    // This is a simplified version; the full SRT is complex
    let mut buf = Vec::new();

    // SRT header
    buf.extend_from_slice(&1u16.to_le_bytes()); // version
    buf.extend_from_slice(&1252u16.to_le_bytes()); // codepage

    // Simple ASCII collation: each byte maps to its sort order
    for i in 0u16..256 {
        buf.extend_from_slice(&i.to_le_bytes());
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_not_empty() {
        let srt = build_french_srt();
        assert!(!srt.is_empty());
        // Header (4B) + 256 entries × 2B = 516
        assert_eq!(srt.len(), 4 + 256 * 2);
    }
}
