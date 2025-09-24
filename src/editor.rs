pub mod key;
pub mod state;

use std::io::Write;
use std::ops::Drop;

use crossterm::{ExecutableCommand, QueueableCommand, cursor, terminal};

use super::error::Result;
use super::utils;
pub use key::{ControlKey, Direction, Key};
pub use state::State;

pub struct Editor<W: Write> {
    writer: W,
    // cursor横坐标
    cx: u16,
    // cursor纵坐标
    cy: u16,
    columns: u16,
    rows: u16,
}

impl<W: Write> Editor<W> {
    pub fn new(writer: W) -> Self {
        // 进入原始模式
        terminal::enable_raw_mode().unwrap();
        let (columns, rows) = terminal::size().unwrap();
        let mut editor = Self {
            cx: 0,
            cy: 0,
            writer,
            columns,
            rows,
        };
        editor
            .writer
            // 进入备用屏幕
            .queue(terminal::EnterAlternateScreen)
            .unwrap()
            // 设置标题
            .queue(terminal::SetTitle("editor"))
            .unwrap();

        editor.refresh_screen().unwrap();
        editor
    }

    pub fn refresh_screen(&mut self) -> Result<()> {
        // execute会隐式调用flush，queue不会
        // 刷新屏幕之前隐藏光标，刷新完成之后显示，这样可以防止光标闪烁
        self.writer.execute(cursor::Hide)?;

        self.writer
            // 终端的光标起始位置以1开始
            // crossterm的光标起始位置以0开始
            // 将光标移到左上角开始绘制
            .queue(cursor::MoveTo(0, 0))?;

        self.draw_rows()?;

        self.writer
            // 将光标移动回来
            .queue(cursor::MoveTo(self.cx, self.cy))?
            .execute(cursor::Show)?;

        Ok(())
    }

    fn draw_rows(&mut self) -> Result<()> {
        for i in 1..=self.rows {
            write!(&mut self.writer, "~")?;

            if i == self.rows / 3 {
                let mut welcome = format!("fim -- version: {}", utils::get_version_from_env());
                // 如果欢迎字符串的宽度超过终端宽带，则截断
                if welcome.len() > self.columns as usize {
                    let bytes = welcome.as_bytes();
                    let len = std::cmp::min(bytes.len(), self.columns as usize);
                    // 安全：因为我们知道welcome中是ASCII，所以可以直接从字节重建字符串
                    welcome = unsafe { String::from_utf8_unchecked(bytes[..len].to_vec()) };
                }
                // welcome足够短，u16不会丢失信息
                // 计算边距
                let margin = (self.columns - welcome.len() as u16) / 2;
                self.writer.queue(cursor::MoveToColumn(margin))?;
                self.writer.write(welcome.as_bytes())?;
            }

            // 最后一行不打印\r\n
            // 如果最后一行打印\r\n会导致屏幕滚动到下一行
            // 这样最后一行没有~
            if i < self.rows {
                write!(&mut self.writer, "\r\n")?;
            }
        }
        Ok(())
    }

    pub fn process_key(&mut self, key: Key) -> State {
        match key {
            Key::ControlKey(ControlKey::Ctrl('q')) => State::Exit,
            // 必须使用括号分组，不然只绑定了'a'，是不完整的绑定
            key @ (Key::ArrowKey(Direction::Left)
            | Key::ArrowKey(Direction::Right)
            | Key::ArrowKey(Direction::Up)
            | Key::ArrowKey(Direction::Down)) => {
                self.move_cursor(key);
                State::Continue
            }
            Key::Char(c) => {
                print!("{c}");
                self.add_cx();
                State::Continue
            }
            Key::FunctionKey(n) => {
                println!("F{n}");
                State::Continue
            }
            Key::ControlKey(ControlKey::Escape) => {
                print!("esc");
                State::Continue
            }
            Key::ControlKey(ControlKey::PageUp) => {
                self.scroll_srceen(self.rows, Direction::Up);
                State::Continue
            }
            Key::ControlKey(ControlKey::PageDown) => {
                self.scroll_srceen(self.rows, Direction::Down);
                State::Continue
            }
            _ => State::Continue,
        }
    }

    fn move_cursor(&mut self, key: Key) {
        match key {
            Key::ArrowKey(Direction::Left) => self.sub_cx(),
            Key::ArrowKey(Direction::Right) => self.add_cx(),
            Key::ArrowKey(Direction::Up) => self.sub_cy(),
            Key::ArrowKey(Direction::Down) => self.add_cy(),
            _ => println!("unknow key"),
        }
    }

    fn scroll_srceen(&mut self, nums: u16, direction: Direction) {
        match direction {
            Direction::Up => {
                for _ in 0..nums {
                    self.move_cursor(Key::ArrowKey(Direction::Up));
                }
            }
            Direction::Down => {
                for _ in 0..nums {
                    self.move_cursor(Key::ArrowKey(Direction::Down));
                }
            }
            Direction::Left => {}
            Direction::Right => {}
        }
    }

    fn add_cx(&mut self) {
        if self.cx != self.columns - 1 {
            self.cx += 1
        }
    }

    fn sub_cx(&mut self) {
        // 注意：坐标不能小于0
        if self.cx != 0 {
            self.cx -= 1
        }
    }

    fn add_cy(&mut self) {
        // crossterm的cursor左上角单元格是(0,0)
        // crossterm的size左上角单元格是(1,1)
        // 注意转换
        if self.cy != self.rows - 1 {
            self.cy += 1
        }
    }

    fn sub_cy(&mut self) {
        if self.cy != 0 {
            self.cy -= 1
        }
    }
}

impl<W: Write> Drop for Editor<W> {
    // 当值不再需要时，Rust会自动运行析构函数
    // 析构函数分两部分：
    // 1.如果该类型实现了ops::Drop trait，调用其Drop::drop方法
    // 2.自动生成的"drop glue"会递归调用该值所有字段的析构函数
    // 不能主动调用该方法
    // 原因是drop(&mut self)不会移动其值，在析构后依旧可以使用该值，有危险
    // 显式析构使用mem::drop代替
    fn drop(&mut self) {
        // 禁用终端的原始模式，恢复到规范模式（canonical mode）
        terminal::disable_raw_mode().unwrap();
        // 离开备用屏幕
        self.writer.execute(terminal::LeaveAlternateScreen).unwrap();
    }
}
