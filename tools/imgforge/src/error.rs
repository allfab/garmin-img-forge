use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImgError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid IMG format: {0}")]
    InvalidFormat(String),

    #[error("Block overflow: {0}")]
    BlockOverflow(String),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid coordinate: {0}")]
    InvalidCoord(String),
}

#[derive(Error, Debug)]
pub enum TypError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid section: {0}")]
    InvalidSection(String),

    #[error("Invalid key at line {line}: {key}")]
    InvalidKey { line: usize, key: String },

    #[error("Invalid value at line {line}: {context}")]
    InvalidValue { line: usize, context: String },

    #[error("Unknown codepage: {0}")]
    UnknownCodepage(u16),

    #[error("Bad TYP header: {0}")]
    BadHeader(String),

    #[error("Encoding error: {0}")]
    Encoding(String),
}
