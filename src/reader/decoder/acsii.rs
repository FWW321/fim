use tokio::io::AsyncReadExt;
use tracing::{error, instrument, trace};

use crate::{
    error::{EditorError, Result},
    reader::byte_stream::ByteStream,
};

pub struct AsciiDecoder<R: AsyncReadExt + Unpin> {
    byte_stream: ByteStream<R>,
}

impl<R: AsyncReadExt + Unpin> AsciiDecoder<R> {
    pub fn new(byte_stream: ByteStream<R>) -> Self {
        Self { byte_stream }
    }

    #[instrument(skip(self))]
    pub async fn decode_char(&mut self) -> Result<Option<char>> {
        let Some(byte) = self.byte_stream.read_next_byte().await? else {
            trace!("ASCII decoder: reached EOF");
            return Ok(None);
        };

        if byte > 127 {
            error!("ASCII decoder: invalid byte 0x{:02X} (> 127)", byte);
            Err(EditorError::invalid_encoding(
                0,
                format!("Byte 0x{:02X} is not valid ASCII (must be <= 127)", byte),
                vec![byte],
            ))
        } else {
            let ch = byte as char;
            trace!("ASCII decoder: decoded character '{}' (0x{:02X})", ch, byte);
            Ok(Some(ch))
        }
    }

    pub async fn is_next_esc(&mut self) -> bool {
        if let Ok(byte) = self.byte_stream.peek_ahead(1).await {
            byte[0] == 0x1B
        } else {
            false
        }
    }

    // pub fn get_name(&self) -> &'static str {
    //     "ASCII"
    // }

    pub fn take_stream(self) -> ByteStream<R> {
        self.byte_stream
    }

    pub async fn read_line(&mut self) -> Result<Option<String>> {
        let mut line = String::new();
        loop {
            match self.decode_char().await? {
                Some(c) => {
                    if c == '\n' {
                        break;
                    } else if c == '\r' {
                        // 忽略回车符
                        continue;
                    } else {
                        line.push(c);
                    }
                }
                None => {
                    // EOF reached
                    if line.is_empty() {
                        return Ok(None);
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(Some(line))
    }
}
