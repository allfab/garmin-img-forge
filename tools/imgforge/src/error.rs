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
