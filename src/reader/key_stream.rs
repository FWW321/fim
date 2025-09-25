use std::collections::VecDeque;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::time;
use tracing::{debug, instrument, warn};

use super::decoder::Decoder;
use crate::editor::key::{ControlKey, Direction, Key};
use crate::error::{EditorError, Result};

/// 按键解析状态
// #[derive(Debug, Clone, PartialEq)]
// pub enum SequenceState {
//     /// 正常状态，处理普通字符
//     Normal,
//     /// 转义序列状态，处理所有以ESC开头的序列
//     EscapeSequence,
// }

/// 转义序列的最大长度，用于预分配缓冲区
const MAX_ESCAPE_SEQUENCE_LENGTH: usize = 16;
/// 字符缓冲区的初始容量
// const CHAR_BUFFER_CAPACITY: usize = 32;
/// 转义序列超时时间（毫秒）
const BUFFER_SIZE: usize = 10;
const ESCAPE_SEQUENCE_TIMEOUT: Duration = Duration::from_millis(10);

pub struct KeyStream<R: AsyncReadExt + Unpin> {
    decoder: Decoder<R>,
    // state: SequenceState,
    buffer: VecDeque<Key>,
}

impl<R: AsyncReadExt + Unpin> KeyStream<R> {
    #[instrument(skip(decoder))]
    pub fn new(decoder: Decoder<R>) -> Self {
        debug!(
            "Creating new KeyStream with buffer capacities: {}",
            BUFFER_SIZE
        );
        Self {
            decoder,
            buffer: VecDeque::with_capacity(BUFFER_SIZE),
        }
    }

    /// 解析字符为按键事件
    ///
    /// # Returns
    /// - `Ok(Some(Key))` - 成功解析出一个按键
    /// - `Ok(None)` - 需要更多字符才能完成解析
    /// - `Err(error)` - 解析过程中发生错误
    #[instrument(skip(self))]
    pub async fn next_key(&mut self) -> Result<Option<Key>> {
        if !self.buffer.is_empty() {
            return Ok(self.buffer.pop_front());
        }

        if let Some(c) = self.decoder.decode_char().await? {
            if c != '\u{001B}' {
                self.buffer.push_back(Self::convert_char_to_key(c));
            } else {
                if let Some(key) = self.process_escape().await {
                    self.buffer.push_back(key);
                }
            }
        }
        if self.buffer.is_empty() {
            Ok(None)
        } else {
            Ok(self.buffer.pop_front())
        }
    }

    #[instrument(skip(self))]
    async fn process_escape(&mut self) -> Option<Key> {
        let mut sequence = vec!['\u{001B}'];
        // 如果next是esc，那么说明当前转义序列是失败的
        // 且可能产生新的转义序列
        // 如果不及时终止当前转义序列的处理
        // 且不去处理新的可能的转义序列
        // 则新的转义序列会和当前的一起识别为失败的转义序列，转换为普通字符
        loop {
            let Ok(is_next_esc) =
                time::timeout(ESCAPE_SEQUENCE_TIMEOUT, self.decoder.is_next_esc()).await
            else {
                warn!(
                    "escape sequence timeout after {}ms, flushing buffer",
                    ESCAPE_SEQUENCE_TIMEOUT.as_millis()
                );
                break;
            };

            if is_next_esc {
                break;
            }

            let Ok(Some(next)) = self.decoder.decode_char().await else {
                break;
            };

            sequence.push(next);

            if sequence.len() == MAX_ESCAPE_SEQUENCE_LENGTH {
                warn!("KeyParser: escape sequence too long, flushing");
                break;
            }

            match Self::parse_escape_sequence(&sequence) {
                Err(_) => break,
                Ok(None) => {}
                Ok(Some(key)) => {
                    return Some(key);
                }
            }
        }

        for c in sequence {
            self.buffer.push_back(Self::convert_char_to_key(c));
        }

        None
    }

    fn parse_escape_sequence(sequence: &[char]) -> Result<Option<Key>> {
        if sequence.len() < 2 {
            return Ok(None);
        }
        match sequence[1] {
            // CSI序列
            '[' => Self::parse_csi_sequence(sequence),
            // SS3序列
            'O' => Self::parse_ss3_key(sequence),
            _ => Err(EditorError::invalid_sequence(
                sequence.iter().collect::<String>(),
                sequence.len(),
            )),
        }
    }

    fn parse_csi_sequence(sequence: &[char]) -> Result<Option<Key>> {
        if sequence.len() < 3 {
            return Ok(None);
        }

        match sequence[2] {
            'A' => Ok(Some(Key::ArrowKey(Direction::Up))),
            'B' => Ok(Some(Key::ArrowKey(Direction::Down))),
            'C' => Ok(Some(Key::ArrowKey(Direction::Right))),
            'D' => Ok(Some(Key::ArrowKey(Direction::Left))),
            'H' => Ok(Some(Key::ControlKey(ControlKey::Home))),
            'F' => Ok(Some(Key::ControlKey(ControlKey::End))),
            // 'M' => parse_mouse_event(sequence),
            '0'..='9' => Self::parse_csi_with_number(sequence),
            _ => Err(EditorError::invalid_sequence(
                sequence.iter().collect::<String>(),
                sequence.len(),
            )),
        }
    }

    fn parse_ss3_key(sequence: &[char]) -> Result<Option<Key>> {
        if sequence.len() != 3 {
            return Ok(None);
        }

        match sequence[2] {
            'P' => Ok(Some(Key::FunctionKey(1))),
            'Q' => Ok(Some(Key::FunctionKey(2))),
            'R' => Ok(Some(Key::FunctionKey(3))),
            'S' => Ok(Some(Key::FunctionKey(4))),
            _ => Err(EditorError::invalid_sequence(
                sequence.iter().collect::<String>(),
                sequence.len(),
            )),
        }
    }

    // fn parse_mouse_event(sequence: &[char]) -> Result<Option<Key>> {
    //     if sequence < 6 {
    //         return Ok(None);
    //     }
    // }

    fn parse_csi_with_number(sequence: &[char]) -> Result<Option<Key>> {
        let len = sequence.len();
        if len < 4 || sequence[len - 1] != '~' {
            return Ok(None);
        }

        let number_chars = &sequence[2..len - 1];

        match number_chars {
            &['1'] => Ok(Some(Key::ControlKey(ControlKey::Home))),
            &['2'] => Ok(Some(Key::ControlKey(ControlKey::Insert))),
            &['3'] => Ok(Some(Key::ControlKey(ControlKey::Delete))),
            &['4'] => Ok(Some(Key::ControlKey(ControlKey::End))),
            &['5'] => Ok(Some(Key::ControlKey(ControlKey::PageUp))),
            &['6'] => Ok(Some(Key::ControlKey(ControlKey::PageDown))),
            &['1', '1'] => Ok(Some(Key::FunctionKey(1))),
            &['1', '2'] => Ok(Some(Key::FunctionKey(2))),
            &['1', '3'] => Ok(Some(Key::FunctionKey(3))),
            &['1', '4'] => Ok(Some(Key::FunctionKey(4))),
            &['1', '5'] => Ok(Some(Key::FunctionKey(5))),
            &['1', '7'] => Ok(Some(Key::FunctionKey(6))),
            &['1', '8'] => Ok(Some(Key::FunctionKey(7))),
            &['1', '9'] => Ok(Some(Key::FunctionKey(8))),
            &['2', '0'] => Ok(Some(Key::FunctionKey(9))),
            &['2', '1'] => Ok(Some(Key::FunctionKey(10))),
            &['2', '3'] => Ok(Some(Key::FunctionKey(11))),
            &['2', '4'] => Ok(Some(Key::FunctionKey(12))),
            _ => Err(EditorError::invalid_sequence(
                sequence.iter().collect::<String>(),
                sequence.len(),
            )),
        }
    }

    fn convert_char_to_key(c: char) -> Key {
        match c {
            '\u{001B}' => Key::ControlKey(ControlKey::Escape),
            '\r' => Key::ControlKey(ControlKey::CR),
            // 换行 Line Feed
            '\n' => Key::ControlKey(ControlKey::LF),
            '\t' => Key::ControlKey(ControlKey::Tab),
            '\u{007F}' => Key::ControlKey(ControlKey::Delete),
            c @ '\u{0000}'..='\u{001F}' => {
                Key::ControlKey(ControlKey::Ctrl(Self::ctrl_key_reverse(c).unwrap()))
            },
            // 回车 Carriage Return
            _ => Key::Char(c),
        }
    }

    /// 将 Ctrl+字符 转换回基础字符
    fn ctrl_key_reverse(ctrl_char: char) -> Option<char> {
        match ctrl_char as u8 {
            0 => Some('@'),
            // Ctrl+A 到 Ctrl+Z → a 到 z
            1..=26 => Some((ctrl_char as u8 - 1 + b'a') as char),

            // 特殊符号
            27 => Some('['),  // Ctrl+[
            28 => Some('\\'), // Ctrl+\
            29 => Some(']'),  // Ctrl+]
            30 => Some('^'),  // Ctrl+^
            31 => Some('_'),  // Ctrl+_

            // 无效控制字符
            _ => None,
        }
    }
}

