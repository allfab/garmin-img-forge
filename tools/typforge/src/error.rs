use thiserror::Error;

#[derive(Debug, Error)]
pub enum TypforgeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error at line {line}: {context}")]
    Parse { line: usize, context: String },

    #[error("Encoding error: {0}")]
    Encode(String),

    #[error("Binary format error: {0}")]
    Binary(String),
}

pub type Result<T> = std::result::Result<T, TypforgeError>;
