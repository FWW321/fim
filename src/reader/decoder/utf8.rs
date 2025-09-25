use std::marker::Unpin;

use tokio::io::AsyncReadExt;
use tracing::{error, instrument, trace};

use crate::{
    error::{EditorError, Result},
    reader::byte_stream::ByteStream,
};

pub struct Utf8Decoder<R: AsyncReadExt + Unpin> {
    byte_stream: ByteStream<R>,
}

impl<R: AsyncReadExt + Unpin> Utf8Decoder<R> {
    pub fn new(byte_stream: ByteStream<R>) -> Self {
        Self { byte_stream }
    }

    /// 根据第一个字节确定UTF-8字符需要的字节数
    // 如果第一个字节以0开头，则是单字节字符
    // 0b是Rust中的二进制字面量前缀（binary literal prefix），表示后面跟着的是二进制数字
    // UTF-8编码规则：
    // 1字节：0xxxxxxx
    // 2字节：110xxxxx 10xxxxxx
    // 3字节：1110xxxx 10xxxxxx 10xxxxxx
    // 4字节：11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
    // 如果要判断该位是否为1，可以让该位与1进行与运算，如果结果为1则该位为1，否则为0
    // 如果要判断该位是否为0，可以让该位与1进行与运算，如果结果为0则该位为0，否则为1
    // 其余位可以为任意值，与0想与运结果固定为0
    fn calculate_byte_count(&self, first_byte: u8) -> u8 {
        match first_byte {
            b if b & 0b1000_0000 == 0 => 1,
            b if b & 0b1110_0000 == 0b1100_0000 => 2,
            b if b & 0b1111_0000 == 0b1110_0000 => 3,
            b if b & 0b1111_1000 == 0b1111_0000 => 4,
            _ => 0, // 无效的UTF-8起始字节
        }
    }

    /// 检查是否为UTF-8续字节 (10xxxxxx)
    fn is_continuation_byte(&self, byte: u8) -> bool {
        byte & 0b1100_0000 == 0b1000_0000
    }

    #[instrument(skip(self))]
    pub async fn decode_char(&mut self) -> Result<Option<char>> {
        let Some(leading_byte) = self.byte_stream.read_next_byte().await? else {
            trace!("UTF-8 decoder: reached EOF");
            return Ok(None);
        };

        let byte_count = self.calculate_byte_count(leading_byte);
        trace!(
            "UTF-8 decoder: leading byte 0x{:02X} requires {} bytes",
            leading_byte, byte_count
        );

        if byte_count == 1 {
            // 单字节ASCII字符
            let ch = leading_byte as char;
            trace!("UTF-8 decoder: decoded ASCII character '{}'", ch);
            Ok(Some(ch))
        } else if byte_count > 1 {
            // U+0000是空字符（NUL），即'\0'，其UTF-8编码为0x00
            // 空字符表示无或者空
            // 移除控制信息，保留数据位，解码为Unicode码点
            let mut unicode_point = (leading_byte & (0xFF >> (byte_count + 1))) as u32;
            let mut bytes_collected = vec![leading_byte];

            for i in 1..byte_count {
                let Some(continuation_byte) = self.byte_stream.read_next_byte().await? else {
                    error!(
                        "UTF-8 decoder: unexpected EOF while reading continuation byte {} of {}",
                        i, byte_count
                    );
                    return Err(EditorError::unexpected_eof(
                        format!("UTF-8 continuation byte {} of {}", i, byte_count),
                        i as usize,
                    ));
                };

                bytes_collected.push(continuation_byte);

                if !self.is_continuation_byte(continuation_byte) {
                    error!(
                        "UTF-8 decoder: invalid continuation byte 0x{:02X} at position {}",
                        continuation_byte, i
                    );
                    return Err(EditorError::invalid_encoding(
                        i as usize,
                        format!(
                            "Expected UTF-8 continuation byte (10xxxxxx), got 0x{:02X}",
                            continuation_byte
                        ),
                        bytes_collected,
                    ));
                }

                // 移除控制信息提取6位数据并合并到Unicode码点
                unicode_point = unicode_point << 6 | (continuation_byte & 0b0011_1111) as u32;
            }

            // 将Unicode码点转换为字符
            match std::char::from_u32(unicode_point) {
                Some(ch) => {
                    trace!(
                        "UTF-8 decoder: successfully decoded character '{}' (U+{:04X}) from {} bytes",
                        ch, unicode_point, byte_count
                    );
                    Ok(Some(ch))
                }
                None => {
                    error!(
                        "UTF-8 decoder: invalid Unicode code point U+{:08X}",
                        unicode_point
                    );
                    Err(EditorError::invalid_encoding(
                        0,
                        format!("Invalid Unicode code point U+{:08X}", unicode_point),
                        bytes_collected,
                    ))
                }
            }
        } else {
            error!("UTF-8 decoder: invalid leading byte 0x{:02X}", leading_byte);
            Err(EditorError::invalid_encoding(
                0,
                format!("Invalid UTF-8 leading byte 0x{:02X}", leading_byte),
                vec![leading_byte],
            ))
        }
    }

    pub fn take_stream(self) -> ByteStream<R> {
        self.byte_stream
    }

    pub async fn is_next_esc(&mut self) -> bool {
        if let Ok(byte) = self.byte_stream.peek_ahead(1).await {
            byte[0] == 0x1B
        } else {
            false
        }
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

    // pub fn get_name(&self) -> &'static str {
    //     "UTF-8"
    // }
}
