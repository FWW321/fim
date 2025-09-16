pub mod utf8;

use std::io::Read;
use crate::error::Result;

pub trait DecodingStrategy<R: Read> {
    fn read_char(&self, reader: &mut R) -> Result<Option<char>>;

    fn get_encoding_name(&self) -> &'static str;

    fn read_byte(&self, reader: &mut R) -> Result<Option<u8>> {
        let mut buffer = [0; 1];
        // read方法返回一个Result类型，Ok包含读取的字节数，Err包含错误信息
        // read方法返回Ok(0)表示输入流已关闭
        let num = reader.read(&mut buffer)?;
        if num == 0 {
            // None表示没有更多的字节可读
            return Ok(None);
        }
        Ok(Some(buffer[0]))
    }
}