use std::io::Read;
use super::DecodingStrategy;
use crate::error::{Result, ReaderError};

pub struct Utf8Strategy;

impl Utf8Strategy {
    pub fn new() -> Self {
        Self {}
    }

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
    fn required_bytes(&self, first_byte: u8) -> u8 {
        match first_byte {
        b if b & 0b1000_0000 == 0 => 1,
        b if b & 0b1110_0000 == 0b1100_0000 => 2,
        b if b & 0b1111_0000 == 0b1110_0000 => 3,
        b if b & 0b1111_1000 == 0b1111_0000 => 4,
        _ => 0, // 无效的UTF-8起始字节
        }
    }

    fn is_continuation_byte(&self, byte: u8) -> bool {
        byte & 0b1100_0000 == 0b1000_0000
    }
}

impl<R: Read> DecodingStrategy<R> for Utf8Strategy {
    fn read_char(&self, reader: &mut R) -> Result<Option<char>> {
        let Some(first_byte) = self.read_byte(reader)? else {
          return Ok(None)
        };

        #[cfg(debug_assertions)]
        println!("first_byte: {:#010b}", first_byte);

        let required_bytes = self.required_bytes(first_byte);

        if required_bytes == 1 {
            Ok(Some(first_byte as char)) // 单字节字符
        } else if required_bytes > 1 {
            // U+0000是空字符（NUL），即'\0'，其UTF-8编码为0x00
            // 空字符表示无或者空
            // 移除控制信息，保留数据位，解码为Unicode码点
            let mut code_point = (first_byte & (0xFF >> (required_bytes + 1))) as u32;

            for _ in 1..required_bytes {
                let Some(next_byte) = self.read_byte(reader)? else {
                  return Err(ReaderError::UnexpectedEof);
                };

                if !self.is_continuation_byte(next_byte) {
                    return Err(ReaderError::InvalidEncoding);
                }

                code_point = code_point << 6 | (next_byte & 0b0011_1111) as u32;
            }
            
            if let Some(c) = std::char::from_u32(code_point) {
                Ok(Some(c))
            } else {
                Err(ReaderError::InvalidEncoding)
            }
        } else {
            Err(ReaderError::InvalidEncoding)
        }
    }

    fn get_encoding_name(&self) -> &'static str {
        "UTF-8"
    }
}