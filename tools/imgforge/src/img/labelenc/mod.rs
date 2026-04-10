pub mod format6;
pub mod format9;
pub mod format10;

/// Label encoding format, matching mkgmap CodeFunctions.java
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelEncoding {
    /// 6-bit packed ASCII (uppercase only)
    Format6,
    /// Single-byte codepage (e.g. cp1252)
    Format9(u16),
    /// UTF-8
    Format10,
}

impl LabelEncoding {
    /// Create encoder from charset string (mkgmap CodeFunctions.createEncoderForLBL)
    #[cfg(test)]
    fn from_charset(charset: &str) -> Self {
        match charset.to_lowercase().as_str() {
            "ascii" => LabelEncoding::Format6,
            "cp1252" | "latin1" | "iso-8859-1" => LabelEncoding::Format9(1252),
            "cp65001" | "unicode" | "utf-8" | "utf8" => LabelEncoding::Format10,
            s if s.starts_with("cp") => {
                let codepage = s[2..].parse::<u16>().unwrap_or(1252);
                LabelEncoding::Format9(codepage)
            }
            _ => LabelEncoding::Format9(1252),
        }
    }

    pub fn format_id(&self) -> u8 {
        match self {
            LabelEncoding::Format6 => 6,
            LabelEncoding::Format9(_) => 9,
            LabelEncoding::Format10 => 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_charset_routing() {
        assert_eq!(LabelEncoding::from_charset("ascii"), LabelEncoding::Format6);
        assert_eq!(LabelEncoding::from_charset("cp1252"), LabelEncoding::Format9(1252));
        assert_eq!(LabelEncoding::from_charset("latin1"), LabelEncoding::Format9(1252));
        assert_eq!(LabelEncoding::from_charset("cp65001"), LabelEncoding::Format10);
        assert_eq!(LabelEncoding::from_charset("unicode"), LabelEncoding::Format10);
        assert_eq!(LabelEncoding::from_charset("utf-8"), LabelEncoding::Format10);
        assert_eq!(LabelEncoding::from_charset("cp932"), LabelEncoding::Format9(932));
    }

    #[test]
    fn test_format_id() {
        assert_eq!(LabelEncoding::Format6.format_id(), 6);
        assert_eq!(LabelEncoding::Format9(1252).format_id(), 9);
        assert_eq!(LabelEncoding::Format10.format_id(), 10);
    }
}
