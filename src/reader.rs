pub mod encoding;
pub mod strategy;

use std::io::Read;
use strategy::{DecodingStrategy, utf8::Utf8Strategy};
use encoding::Encoding;
use crate::error::Result;

pub struct CharReader<R: Read> {
    strategy: Box<dyn DecodingStrategy<R>>,
    reader: R, // 保持对读取器的所有权
}

impl<R: Read> CharReader<R> {
    pub fn new(reader: R, encoding: Encoding) -> Self {
        let strategy: Box<dyn DecodingStrategy<R>> = Self::get_strategy(encoding);
        Self { strategy, reader }
    }

    pub fn read_char(&mut self) -> Result<Option<char>> {
        self.strategy.read_char(&mut self.reader)
    }

    pub fn get_encoding_name(&self) -> &'static str {
        self.strategy.get_encoding_name()
    }

    pub fn get_strategy(encoding: Encoding) -> Box<dyn DecodingStrategy<R>> {
        match encoding {
            Encoding::Utf8 => Box::new(Utf8Strategy::new()),
            Encoding::Ascii => unimplemented!("ASCII strategy not implemented yet"), 
        }
    }

    pub fn set_encoding(&mut self, encoding: Encoding) -> Result<()> {
        self.strategy = Self::get_strategy(encoding);
        Ok(())
    }
}