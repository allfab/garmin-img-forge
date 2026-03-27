//! Domain-specific error types for imgforge-cli.

use thiserror::Error;

/// Errors produced by the IMG filesystem writer.
#[derive(Error, Debug)]
pub enum ImgError {
    /// Map ID is invalid (non-numeric or empty or too long).
    #[error("Invalid map ID: '{id}'")]
    InvalidMapId { id: String },

    /// I/O error while writing the IMG file.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Block alignment or size calculation error.
    #[error("Block alignment error: {message}")]
    BlockAlignmentError { message: String },

    /// Input directory is empty (no .mp files found) or does not exist.
    #[error("répertoire d'entrée vide ou introuvable : {path}")]
    EmptyInputDir { path: String },

    /// All .mp tiles found in the directory failed to compile.
    #[error("{count} tuile(s) trouvée(s) dans '{path}' mais toutes ont échoué à compiler")]
    AllTilesFailedToCompile { path: String, count: usize },
}

#[derive(Error, Debug)]
pub enum ParseError {
    /// Invalid .mp format at a specific line.
    #[error("Invalid Polish Map format at line {line}: {message}")]
    InvalidFormat { line: usize, message: String },

    /// The mandatory [IMG ID] section was not found.
    #[error("Missing [IMG ID] section in Polish Map file")]
    MissingImgId,

    /// I/O error while reading the file.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- ImgError tests -----

    #[test]
    fn test_img_invalid_map_id_display() {
        let err = ImgError::InvalidMapId {
            id: "NOTDIGIT".to_string(),
        };
        assert_eq!(err.to_string(), "Invalid map ID: 'NOTDIGIT'");
    }

    #[test]
    fn test_img_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = ImgError::IoError(io_err);
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_img_empty_input_dir_display() {
        let err = ImgError::EmptyInputDir { path: "/no/mp/here".to_string() };
        assert!(err.to_string().contains("/no/mp/here"));
    }

    #[test]
    fn test_img_all_tiles_failed_display() {
        let err = ImgError::AllTilesFailedToCompile {
            path: "/tiles".to_string(),
            count: 3,
        };
        assert!(err.to_string().contains("/tiles"));
        assert!(err.to_string().contains("3"));
    }

    #[test]
    fn test_img_block_alignment_error_display() {
        let err = ImgError::BlockAlignmentError {
            message: "block size must be a power of two".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Block alignment error: block size must be a power of two"
        );
    }

    #[test]
    fn test_img_io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let img_err: ImgError = io_err.into();
        assert!(matches!(img_err, ImgError::IoError(_)));
    }

    // ----- ParseError tests -----

    #[test]
    fn test_invalid_format_display() {
        let err = ParseError::InvalidFormat {
            line: 42,
            message: "malformed Data0 coordinate".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid Polish Map format at line 42: malformed Data0 coordinate"
        );
    }

    #[test]
    fn test_missing_img_id_display() {
        let err = ParseError::MissingImgId;
        assert_eq!(
            err.to_string(),
            "Missing [IMG ID] section in Polish Map file"
        );
    }

    #[test]
    fn test_io_error_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ParseError::IoError(io_err);
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_io_error_from_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let parse_err: ParseError = io_err.into();
        assert!(matches!(parse_err, ParseError::IoError(_)));
    }

    #[test]
    fn test_invalid_format_source_chain() {
        let err = ParseError::InvalidFormat {
            line: 1,
            message: "test".to_string(),
        };
        // ParseError::InvalidFormat has no source
        use std::error::Error;
        assert!(err.source().is_none());
    }
}
