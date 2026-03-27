//! SRT (Sort Routines) subfile writer for Garmin IMG format.
//!
//! The SRT subfile defines character collation rules for GPS address search.
//! For French maps using CP1252 encoding, it maps accented characters (é, è,
//! ê, ë, à, â, ç, …) to the same primary sort key as their base letter,
//! enabling the GPS search to find "Église" by typing "EGL".
//!
//! # Binary layout
//! ```text
//! [Header — 27 bytes]
//!   0x00  LE16  header_length  = 27
//!   0x02  LE16  unknown        = 0
//!   0x04  LE32  data_offset    = 27
//!   0x08  LE32  data_length    = 4352 (256 × 17)
//!   0x0C  LE16  codepage       = 1252
//!   0x0E  LE16  record_size    = 17
//!   0x10  LE16  record_count   = 256
//!   0x12  LE32  unknown        = 0
//!   0x16  LE32  unknown        = 0
//!   0x1A  u8    unknown        = 0
//!
//! [Data — 256 × 17 bytes]
//!   Per CP1252 byte (0x00..=0xFF):
//!     byte  0 : primary   (0=ignore, 1=space/digit, 2..=27=letter A..Z)
//!     byte  1 : secondary (accent variant: 0=none, 1..N=accented)
//!     byte  2 : tertiary  (case: 0=uppercase, 1=lowercase)
//!     bytes 3..16 : 0x00 padding
//! ```
//!
//! Total subfile size: 27 + 4352 = **4379 bytes**.

/// Writer for SRT (Sort Routines) subfiles in Garmin IMG format.
pub struct SrtWriter;

impl SrtWriter {
    /// Builds the complete binary SRT subfile for French CP1252 collation.
    ///
    /// Returns a [`Vec<u8>`] of exactly **4379 bytes** ready to embed in a
    /// `gmapsupp.img` as a `{family_id:08}.SRT` subfile.
    pub fn build_french_cp1252() -> Vec<u8> {
        const HEADER_LEN: u16 = 27;
        const DATA_LEN: u32 = 256 * 17; // 4352
        const CODEPAGE: u16 = 1252;
        const RECORD_SIZE: u16 = 17;
        const RECORD_COUNT: u16 = 256;

        let mut out = Vec::with_capacity((HEADER_LEN as usize) + (DATA_LEN as usize));

        // ── Header (27 bytes) ─────────────────────────────────────────────────
        out.extend_from_slice(&HEADER_LEN.to_le_bytes()); // 0x00 header_length
        out.extend_from_slice(&0u16.to_le_bytes()); // 0x02 unknown
        out.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes()); // 0x04 data_offset
        out.extend_from_slice(&DATA_LEN.to_le_bytes()); // 0x08 data_length
        out.extend_from_slice(&CODEPAGE.to_le_bytes()); // 0x0C codepage = 1252
        out.extend_from_slice(&RECORD_SIZE.to_le_bytes()); // 0x0E record_size = 17
        out.extend_from_slice(&RECORD_COUNT.to_le_bytes()); // 0x10 record_count = 256
        out.extend_from_slice(&0u32.to_le_bytes()); // 0x12 unknown
        out.extend_from_slice(&0u32.to_le_bytes()); // 0x16 unknown
        out.push(0u8); // 0x1A unknown

        debug_assert_eq!(out.len(), 27, "SRT header must be exactly 27 bytes");

        // ── Data section (256 × 17 bytes) ────────────────────────────────────
        let table = Self::french_cp1252_table();
        for (primary, secondary, tertiary) in &table {
            out.push(*primary);
            out.push(*secondary);
            out.push(*tertiary);
            out.extend_from_slice(&[0u8; 14]); // padding
        }

        debug_assert_eq!(out.len(), 4379, "SRT total must be 4379 bytes");
        out
    }

    /// Returns the French CP1252 collation table as `(primary, secondary, tertiary)`
    /// triples, indexed by CP1252 byte value (0x00..=0xFF).
    ///
    /// # Sort key scheme
    /// - `primary` 0 : non-sortable (control chars, punctuation)
    /// - `primary` 1 : space / digits (sort before letters)
    /// - `primary` 2..=27 : letters A..Z (A=2, B=3, …, Z=27)
    ///
    /// Accented letters carry the same `primary` as their base letter.
    /// `secondary` distinguishes accent variants; `tertiary` encodes case
    /// (0 = uppercase, 1 = lowercase).
    fn french_cp1252_table() -> [(u8, u8, u8); 256] {
        let mut t = [(0u8, 0u8, 0u8); 256];

        // ── Space ─────────────────────────────────────────────────────────────
        t[0x20] = (1, 0, 0);

        // ── Digits 0–9 (sort before letters) ─────────────────────────────────
        for b in b'0'..=b'9' {
            t[b as usize] = (1, 0, 0);
        }

        // ── A–Z uppercase  (primary = letter index + 2) ──────────────────────
        // A=2, B=3, C=4, D=5, E=6, F=7, G=8, H=9, I=10, J=11, K=12, L=13,
        // M=14, N=15, O=16, P=17, Q=18, R=19, S=20, T=21, U=22, V=23, W=24,
        // X=25, Y=26, Z=27
        for (i, b) in (b'A'..=b'Z').enumerate() {
            t[b as usize] = ((i as u8) + 2, 0, 0);
        }

        // ── a–z lowercase  (same primary, tertiary = 1) ───────────────────────
        for (i, b) in (b'a'..=b'z').enumerate() {
            t[b as usize] = ((i as u8) + 2, 0, 1);
        }

        // ── French accented characters (CP1252 byte values) ───────────────────
        //
        // A (primary = 2): À 0xC0 · Â 0xC2 · Æ 0xC6
        t[0xC0] = (2, 1, 0); // À  A grave uppercase
        t[0xC2] = (2, 2, 0); // Â  A circumflex uppercase
        t[0xC6] = (2, 3, 0); // Æ  AE ligature uppercase
        t[0xE0] = (2, 1, 1); // à  A grave lowercase
        t[0xE2] = (2, 2, 1); // â  A circumflex lowercase
        t[0xE6] = (2, 3, 1); // æ  AE ligature lowercase

        // C (primary = 4): Ç 0xC7
        t[0xC7] = (4, 1, 0); // Ç  C cedilla uppercase
        t[0xE7] = (4, 1, 1); // ç  C cedilla lowercase

        // E (primary = 6): È 0xC8 · É 0xC9 · Ê 0xCA · Ë 0xCB
        t[0xC8] = (6, 1, 0); // È  E grave uppercase
        t[0xC9] = (6, 2, 0); // É  E acute uppercase
        t[0xCA] = (6, 3, 0); // Ê  E circumflex uppercase
        t[0xCB] = (6, 4, 0); // Ë  E diaeresis uppercase
        t[0xE8] = (6, 1, 1); // è  E grave lowercase
        t[0xE9] = (6, 2, 1); // é  E acute lowercase
        t[0xEA] = (6, 3, 1); // ê  E circumflex lowercase
        t[0xEB] = (6, 4, 1); // ë  E diaeresis lowercase

        // I (primary = 10): Î 0xCE · Ï 0xCF
        // Note: secondary starts at 2 (not 1) because I grave and I acute do not
        // exist in standard French, so there is no secondary=1 variant for I.
        t[0xCE] = (10, 2, 0); // Î  I circumflex uppercase
        t[0xCF] = (10, 3, 0); // Ï  I diaeresis uppercase
        t[0xEE] = (10, 2, 1); // î  I circumflex lowercase
        t[0xEF] = (10, 3, 1); // ï  I diaeresis lowercase

        // O (primary = 16): Œ 0x8C · Ô 0xD4  (CP1252 special: Œ=0x8C, œ=0x9C)
        t[0x8C] = (16, 1, 0); // Œ  OE ligature uppercase (CP1252 0x8C)
        t[0x9C] = (16, 1, 1); // œ  OE ligature lowercase (CP1252 0x9C)
        t[0xD4] = (16, 2, 0); // Ô  O circumflex uppercase
        t[0xF4] = (16, 2, 1); // ô  O circumflex lowercase

        // U (primary = 22): Ù 0xD9 · Û 0xDB · Ü 0xDC
        t[0xD9] = (22, 1, 0); // Ù  U grave uppercase
        t[0xDB] = (22, 2, 0); // Û  U circumflex uppercase
        t[0xDC] = (22, 3, 0); // Ü  U diaeresis uppercase
        t[0xF9] = (22, 1, 1); // ù  U grave lowercase
        t[0xFB] = (22, 2, 1); // û  U circumflex lowercase
        t[0xFC] = (22, 3, 1); // ü  U diaeresis lowercase

        // Y (primary = 26): Ÿ 0x9F  (CP1252 special: Ÿ=0x9F)
        t[0x9F] = (26, 1, 0); // Ÿ  Y diaeresis uppercase (CP1252 0x9F)
        t[0xFF] = (26, 1, 1); // ÿ  Y diaeresis lowercase

        t
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: get (primary, secondary, tertiary) for a CP1252 byte from data section.
    fn entry(srt: &[u8], byte: u8) -> (u8, u8, u8) {
        let off = 27 + (byte as usize) * 17;
        (srt[off], srt[off + 1], srt[off + 2])
    }

    #[test]
    fn test_srt_header_bytes() {
        let srt = SrtWriter::build_french_cp1252();
        assert_eq!(srt.len(), 4379, "total SRT size must be 4379 bytes");

        // header_length = 27 (LE16)
        assert_eq!(&srt[0x00..0x02], &[0x1B, 0x00], "header_length LE16 = 27");
        // data_offset = 27 (LE32)
        assert_eq!(
            &srt[0x04..0x08],
            &[0x1B, 0x00, 0x00, 0x00],
            "data_offset LE32 = 27"
        );
        // data_length = 4352 = 0x1100 (LE32)
        assert_eq!(
            &srt[0x08..0x0C],
            &[0x00, 0x11, 0x00, 0x00],
            "data_length LE32 = 4352"
        );
        // codepage = 1252 = 0x04E4 (LE16)
        assert_eq!(
            &srt[0x0C..0x0E],
            &[0xE4, 0x04],
            "codepage LE16 = 1252 (0x04E4)"
        );
        // record_size = 17 = 0x0011 (LE16)
        assert_eq!(
            &srt[0x0E..0x10],
            &[0x11, 0x00],
            "record_size LE16 = 17"
        );
        // record_count = 256 = 0x0100 (LE16)
        assert_eq!(
            &srt[0x10..0x12],
            &[0x00, 0x01],
            "record_count LE16 = 256"
        );
    }

    #[test]
    fn test_srt_french_accents_primary() {
        let srt = SrtWriter::build_french_cp1252();

        // ── Base letter primaries ─────────────────────────────────────────────
        assert_eq!(entry(&srt, b'A').0, 2, "A primary = 2");
        assert_eq!(entry(&srt, b'C').0, 4, "C primary = 4");
        assert_eq!(entry(&srt, b'E').0, 6, "E primary = 6");
        assert_eq!(entry(&srt, b'I').0, 10, "I primary = 10");
        assert_eq!(entry(&srt, b'O').0, 16, "O primary = 16");
        assert_eq!(entry(&srt, b'U').0, 22, "U primary = 22");
        assert_eq!(entry(&srt, b'Z').0, 27, "Z primary = 27");

        // ── E variants: all share primary = 6 ────────────────────────────────
        let e_primary = 6u8;
        for &byte in &[0xC8u8, 0xC9, 0xCA, 0xCB, 0xE8, 0xE9, 0xEA, 0xEB] {
            assert_eq!(
                entry(&srt, byte).0,
                e_primary,
                "E-variant 0x{byte:02X} primary must equal {e_primary}"
            );
        }

        // ── A variants: all share primary = 2 ────────────────────────────────
        for &byte in &[0xC0u8, 0xC2, 0xE0, 0xE2] {
            assert_eq!(
                entry(&srt, byte).0,
                2,
                "A-variant 0x{byte:02X} primary must be 2"
            );
        }

        // ── Ç / ç: primary = 4 (C) ───────────────────────────────────────────
        assert_eq!(entry(&srt, 0xC7).0, 4, "Ç primary must be 4 (C)");
        assert_eq!(entry(&srt, 0xE7).0, 4, "ç primary must be 4 (C)");

        // ── Case: uppercase → tertiary 0, lowercase → tertiary 1 ─────────────
        assert_eq!(entry(&srt, b'E').2, 0, "E tertiary must be 0 (uppercase)");
        assert_eq!(entry(&srt, b'e').2, 1, "e tertiary must be 1 (lowercase)");
        assert_eq!(entry(&srt, 0xC9).2, 0, "É tertiary must be 0 (uppercase)");
        assert_eq!(entry(&srt, 0xE9).2, 1, "é tertiary must be 1 (lowercase)");

        // ── Œ / œ (CP1252 0x8C / 0x9C) → primary = 16 (O) ───────────────────
        assert_eq!(entry(&srt, 0x8C).0, 16, "Œ primary must be 16 (O)");
        assert_eq!(entry(&srt, 0x9C).0, 16, "œ primary must be 16 (O)");

        // ── U variants: all share primary = 22 ───────────────────────────────
        for &byte in &[0xD9u8, 0xDB, 0xDC, 0xF9, 0xFB, 0xFC] {
            assert_eq!(
                entry(&srt, byte).0,
                22,
                "U-variant 0x{byte:02X} primary must be 22"
            );
        }
    }

    #[test]
    fn test_srt_case_insensitive_primary() {
        // AC3 — primary('a') == primary('A') for every pair; tertiary(lower) > tertiary(upper)
        let srt = SrtWriter::build_french_cp1252();
        for i in 0u8..26 {
            let upper = b'A' + i;
            let lower = b'a' + i;
            let (upper_primary, _, upper_tertiary) = entry(&srt, upper);
            let (lower_primary, _, lower_tertiary) = entry(&srt, lower);
            assert_eq!(
                upper_primary, lower_primary,
                "primary('{}') must equal primary('{}')",
                upper as char, lower as char
            );
            assert_eq!(upper_tertiary, 0, "tertiary('{}') must be 0 (uppercase)", upper as char);
            assert_eq!(lower_tertiary, 1, "tertiary('{}') must be 1 (lowercase)", lower as char);
        }
    }
}
