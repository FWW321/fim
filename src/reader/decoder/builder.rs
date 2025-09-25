// use tokio::io::AsyncReadExt;

// use super::Decoder;
// use crate::error::{EditorError, Result};
// use crate::reader::byte_stream::ByteStream;
// use crate::reader::decoder::{AsciiDecoder, Utf8Decoder};

// pub struct DecoderBuilder<R: AsyncReadExt + Unpin> {
//     encoding: Option<String>,
//     byte_stream: Option<ByteStream<R>>,
// }

// // 使用bon crate替代
// impl<R: AsyncReadExt + Unpin> DecoderBuilder<R> {
//     pub fn new() -> Self {
//         Self {
//             encoding: None,
//             byte_stream: None,
//         }
//     }

//     /// 设置编码类型
//     pub fn encoding(mut self, encoding: String) -> Self {
//         self.encoding = Some(encoding);
//         self
//     }

//     /// 设置字节流
//     pub fn byte_stream(mut self, byte_stream: ByteStream<R>) -> Self {
//         self.byte_stream = Some(byte_stream);
//         self
//     }

//     /// 构建 Decoder
//     pub fn build(self) -> Result<Decoder<R>> {
//         let encoding = self.encoding.ok_or(EditorError::EncodingNotSet)?;

//         let byte_stream = self.byte_stream.ok_or(EditorError::ByteStreamNotSet)?;

//         // 编码名称仅需要ascii字符，使用to_ascii_lowercase()而不是to_lowercase
//         match encoding.to_ascii_lowercase().as_str() {
//             "utf-8" => Ok(Decoder::Utf8(Utf8Decoder::new(byte_stream))),
//             "ascii" => Ok(Decoder::Ascii(AsciiDecoder::new(byte_stream))),
//             _ => Err(EditorError::UnsupportedEncoding {
//                 encoding: encoding,
//                 available: Decoder::<R>::get_list(),
//             }),
//         }
//     }
// }
