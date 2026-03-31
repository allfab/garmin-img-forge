use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImgError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid IMG format: {0}")]
    InvalidFormat(String),

    #[error("Block overflow: {0}")]
    BlockOverflow(String),

    #[error("Label encoding error: {0}")]
    LabelEncoding(String),

    #[error("Subdivision error: {0}")]
    Subdivision(String),

    #[error("Routing error: {0}")]
    Routing(String),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Parse error at line {line}: {message}")]
    InvalidLine { line: usize, message: String },

    #[error("Missing required section: {0}")]
    MissingSection(String),

    #[error("Invalid coordinate: {0}")]
    InvalidCoord(String),

    #[error("Invalid type code: {0}")]
    InvalidType(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
