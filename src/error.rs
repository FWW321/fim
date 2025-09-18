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
    #[error("Invalid Sequence")]
    InvalidSequence,
    //  #[error("Channel receive error: {0}")]
    // ChannelRecvError(#[from] RecvError),
}

pub type Result<T> = std::result::Result<T, ReaderError>;