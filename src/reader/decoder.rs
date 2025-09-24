pub mod acsii;
pub mod builder;
pub mod utf8;

use std::cmp::{Eq, PartialEq};
use std::fmt;
use std::marker::Unpin;

use tokio::io::AsyncReadExt;

use crate::error::Result;
use crate::reader::ByteStream;
pub use acsii::AsciiDecoder;
pub use builder::DecoderBuilder;
pub use utf8::Utf8Decoder;

pub enum Decoder<R: AsyncReadExt + Unpin> {
    Utf8(Utf8Decoder<R>),
    Ascii(AsciiDecoder<R>),
}

impl<R: AsyncReadExt + Unpin> fmt::Display for Decoder<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Decoder::Utf8(_) => write!(f, "UTF-8"),
            Decoder::Ascii(_) => write!(f, "ASCII"),
        }
    }
}

impl<R: AsyncReadExt + Unpin> PartialEq for Decoder<R> {
    fn eq(&self, other: &Self) -> bool {
        self.get_name() == other.get_name()
    }
}

// Eq没有方法
impl<R: AsyncReadExt + Unpin> Eq for Decoder<R> {}

impl<R: AsyncReadExt + Unpin> Decoder<R> {
    pub fn get_name(&self) -> &'static str {
        match self {
            Decoder::Utf8(_) => "UTF-8",
            Decoder::Ascii(_) => "ASCII",
        }
    }

    // pub fn get_name(&self) -> &'static str {
    //     match self {
    //         Decoder::Utf8(decoder) => decoder.get_name(),
    //         Decoder::Ascii(decoder) => decoder.get_name(),
    //     }
    // }

    pub fn get_list() -> Vec<&'static str> {
        vec!["UTF-8", "ASCII"]
    }

    pub fn take_stream(self) -> ByteStream<R> {
        match self {
            Decoder::Utf8(decoder) => decoder.take_stream(),
            Decoder::Ascii(decoder) => decoder.take_stream(),
        }
    }

    pub fn switch_to_encoding(self, encoding: String) -> Result<Self> {
        if encoding.to_ascii_lowercase() == self.get_name().to_ascii_lowercase() {
            return Ok(self);
        }

        let byte_stream = self.take_stream();
        DecoderBuilder::new()
            .encoding(encoding)
            .byte_stream(byte_stream)
            .build()
    }

    pub async fn decode_char(&mut self) -> Result<Option<char>> {
        match self {
            Decoder::Utf8(decoder) => decoder.decode_char().await,
            Decoder::Ascii(decoder) => decoder.decode_char().await,
        }
    }

    pub async fn is_next_esc(&mut self) -> bool {
        match self {
            Decoder::Utf8(decoder) => decoder.is_next_esc().await,
            Decoder::Ascii(decoder) => decoder.is_next_esc().await,
        }
    }
}
