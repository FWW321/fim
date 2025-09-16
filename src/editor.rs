pub mod state;

use std::io;
use std::io::Write;
use std::ops::Drop;

use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};

pub use state::State;
use super::utils;


pub struct Editor<W: Write> {
    writer: W,
}

impl<W: Write> Editor<W> {
    pub fn new(writer: W) -> Self {
        // 进入原始模式
        terminal::enable_raw_mode().unwrap();
        let mut editor = Self {
            writer,
        };
        editor.refresh_screen().unwrap();
        editor
    }

    pub fn refresh_screen(&mut self) -> io::Result<()> {
        // execute会隐式调用flush，queue不会
        // 刷新屏幕之前隐藏光标，刷新完成之后显示，这样可以防止光标闪烁
        self.writer.execute(cursor::Hide)?;

        self.writer
            .queue(terminal::EnterAlternateScreen)?
            .queue(terminal::SetTitle("editor"))?
            // 终端的光标起始位置以1开始
            // crossterm的光标起始位置以0开始
            .queue(cursor::MoveTo(0, 0))?;

        let (columns, rows) = terminal::size()?;

        self.draw_rows(columns, rows)?;

        self.writer
            .queue(cursor::MoveTo(0, 0))?
            .execute(cursor::Show)?;

        Ok(())
    }

    fn draw_rows(&mut self, columns: u16, rows: u16) -> io::Result<()> {
        for i in 1..=rows {
            write!(&mut self.writer, "~")?;

            if i == rows / 3 {
                let mut welcome =
                    format!("fww editor -- version: {}", utils::get_version_from_env());
                // 如果欢迎字符串的宽度超过终端宽带，则截断
                if welcome.len() > columns as usize {
                    let bytes = welcome.as_bytes();
                    let len = std::cmp::min(bytes.len(), columns as usize);
                    // 安全：因为我们知道welcome中是ASCII，所以可以直接从字节重建字符串
                    welcome = unsafe { String::from_utf8_unchecked(bytes[..len].to_vec()) };
                }
                // welcome足够短，u16不会丢失信息
                // 计算边距
                let margin = (columns - welcome.len() as u16) / 2;
                self.writer.queue(cursor::MoveToColumn(margin))?;
                self.writer.write(welcome.as_bytes())?;
            }

            // 最后一行不打印\r\n
            // 如果最后一行打印\r\n会导致屏幕滚动到下一行
            // 这样最后一行没有~
            if i < rows {
                write!(&mut self.writer, "\r\n")?;
            }
        }
        Ok(())
    }

    pub fn process_char(&self, c: char) -> State {
        match c {
            c if Some(c) == utils::ctrl_key('q') => State::Exit,
            c => {
                println!("{c}");
                State::Continue
            }
        }
    }
}

impl<W: Write> Drop for Editor<W> {
    fn drop(&mut self) {
        // 禁用终端的原始模式，恢复到规范模式（canonical mode）
        terminal::disable_raw_mode().unwrap();
        // 离开备用屏幕
        self.writer.execute(terminal::LeaveAlternateScreen).unwrap();
    }
}
