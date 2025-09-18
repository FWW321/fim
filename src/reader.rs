pub mod encoding;
pub mod strategy;

use std::collections::VecDeque;
use std::time::Duration;
use std::vec;
use std::io::{self, Read};
use std::thread::{spawn, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};

use super::editor::key::{ControlKey, Direction, Key};
use super::error::{ReaderError, Result};
use encoding::Encoding;
use strategy::{utf8::Utf8Strategy, DecodingStrategy};

const BUFFER_SIZE: usize = 10;
const ESCAPE_TIMEOUT: Duration = Duration::from_millis(10);

pub struct CharReader {
    encoding: Encoding,
    buffer: VecDeque<Key>,
    reciver: Receiver<char>,
    // handle: JoinHandle<()>,
    tx: Sender<Encoding>,
}

impl CharReader {
    pub fn new(encoding: Encoding) -> Self {
        let (_, tx, reciver) = Self::spawn_reader(encoding);
        Self {
            buffer: VecDeque::with_capacity(BUFFER_SIZE),
            // handle,
            reciver,
            encoding,
            tx,
        }
    }

    fn spawn_reader(encoding: Encoding) -> (JoinHandle<()>, Sender<Encoding>, Receiver<char>) {
        let (tx, rx) = bounded(1);
        let (s, r) = bounded(BUFFER_SIZE);
         // std::io::stdin() 会返回返回当前进程的标准输入流 stdin 的句柄
        let handle = spawn(move || {
            let stdin = io::stdin();
            // lock() 方法返回一个 StdinLock，对 Stdin 句柄的锁定引用
            // 每次对stdin进行读取都会临时加锁
            // stdinLock只需要锁定一次，就可以进行多次读取操作，不需要每次加锁
            let mut reader = stdin.lock();
            let mut strategy = Self::get_strategy(encoding);
            loop {
                if let Ok(encoding) = rx.try_recv() {
                    strategy = Self::get_strategy(encoding);
                }
                match strategy.read_char(&mut reader) {
                Ok(Some(c)) => {
                    if let Err(e) = s.send(c) {
                        eprintln!("send error: {}", e)
                    };
                },
                Ok(None) => {
                    // EOF reached
                    println!("End of input reached.");
                    break;
                },
                Err(e) => {
                    eprintln!("Error reading char: {}", e);
                }
            }
        }
        });
        (handle, tx, r)
    }

    pub fn get_key(&mut self) -> Result<Option<Key>> {
        if !self.buffer.is_empty() {
            return Ok(self.buffer.pop_front());
        }

        if let Ok(c) = self.reciver.recv() {
            if c != '\u{001B}' {
                self.buffer.push_back(Self::convert_char_to_key(c));
            } else {
                if let Some(key) = self.process_escape()? {
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

    fn process_escape(&mut self) -> Result<Option<Key>> {
        let mut sequence = vec!['\u{001B}'];
        // 如果next是esc，那么说明当前转义序列是失败的
        // 且可能产生新的转义序列
        // 如果不及时终止当前转义序列的处理
        // 且不去处理新的可能的转义序列
        // 则新的转义序列会和当前的一起识别为失败的转义序列，转换为普通字符
        let mut is_next_esc = false;
        loop {
            let Ok(next) = self.reciver.recv_timeout(ESCAPE_TIMEOUT) else {
                break;
            };

            if next == '\u{001B}' {
                is_next_esc = true;
                break;
            }

            sequence.push(next);

            if sequence.len() == BUFFER_SIZE {
                break;
            }

            match Self::parse_escape_sequence(&sequence) {
                Err(_) => break,
                Ok(None) => {}
                Ok(Some(key)) => {
                    return Ok(Some(key));
                }
            }
        }

        for c in sequence {
            self.buffer.push_back(Self::convert_char_to_key(c));
        }

        if is_next_esc {
            self.process_escape()
        } else {
            Ok(None)
        }
    }

    fn parse_escape_sequence(sequence: &[char]) -> Result<Option<Key>> {
        if sequence.len() < 2 {
            return Ok(None);
        }
        match sequence[1] {
            '[' => Self::parse_csi_sequence(sequence),
            'O' => Self::parse_single_function_key(sequence),
            _ => Err(ReaderError::InvalidSequence),
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
            _ => Err(ReaderError::InvalidSequence),
        }
    }

    fn parse_single_function_key(sequence: &[char]) -> Result<Option<Key>> {
        if sequence.len() != 3 {
            return Ok(None);
        }

        match sequence[2] {
            'P' => Ok(Some(Key::FunctionKey(1))),
            'Q' => Ok(Some(Key::FunctionKey(2))),
            'R' => Ok(Some(Key::FunctionKey(3))),
            'S' => Ok(Some(Key::FunctionKey(4))),
            _ => Err(ReaderError::InvalidSequence),
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
            _ => Err(ReaderError::InvalidSequence),
        }
    }

    fn convert_char_to_key(c: char) -> Key {
        match c {
            '\u{001B}' => Key::ControlKey(ControlKey::Escape),
            c @ '\u{0000}'..='\u{001F}' => {
                Key::ControlKey(ControlKey::Ctrl(Self::ctrl_key_reverse(c).unwrap()))
            }
            '\u{007F}' => Key::ControlKey(ControlKey::Delete),
            _ => Key::Char(c),
        }
    }

    // fn read_char(&mut self) -> Result<Option<char>> {
    //     self.strategy.read_char(&mut self.reader)
    // }

    // pub fn get_encoding_name(&self) -> &'static str {
    //     self.strategy.get_encoding_name()
    // }

    pub fn get_encoding(&self) -> Encoding {
        self.encoding
    }

    fn get_strategy<R: Read>(encoding: Encoding) -> Box<dyn DecodingStrategy<R>> {
        match encoding {
            Encoding::Utf8 => Box::new(Utf8Strategy::new()),
            Encoding::Ascii => unimplemented!("ASCII strategy not implemented yet"),
        }
    }

    // pub fn set_encoding(&mut self, encoding: Encoding) {
    //     self.strategy = Self::get_strategy(encoding);
    // }

    pub fn set_encoding(&mut self, encoding: Encoding) {
        if self.encoding == encoding {
            return;
        }

        match self.tx.send(encoding) {
            Ok(_) => {
                println!("编码设置成功");
                self.encoding = encoding;
            },
            Err(e) => println!("编码设置失败: {e}"),
        };
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
