use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReaderError {
    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),
    #[error("Invalid encoding")]
    InvalidEncoding,
    #[error("Unexpected end of file")]
    UnexpectedEof,
}

pub type Result<T> = std::result::Result<T, ReaderError>;