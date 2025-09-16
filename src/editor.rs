pub mod state;

use std::io;
use std::io::Write;
use std::ops::Drop;

use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};

use super::utils;
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

    pub fn refresh_screen(&mut self) -> io::Result<()> {
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

    fn draw_rows(&mut self) -> io::Result<()> {
        for i in 1..=self.rows {
            write!(&mut self.writer, "~")?;

            if i == self.rows / 3 {
                let mut welcome =
                    format!("fww editor -- version: {}", utils::get_version_from_env());
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

    pub fn process_char(&mut self, c: char) -> State {
        match c {
            c if Some(c) == utils::ctrl_key('q') => State::Exit,
            // 必须使用括号分组，不然只绑定了'a'，是不完整的绑定
            c @ ('a' | 'd' | 'w' | 's') => {
                self.move_cursor(c);
                State::Continue
            }
            c => {
                println!("{c}");
                State::Continue
            }
        }
    }

    fn move_cursor(&mut self, key: char) {
        // 注意：坐标不能小于0
        match key {
            'a' => {
                if self.cx > 0 {
                    self.cx = self.cx - 1
                }
            }
            'd' => {
                if self.cx < self.columns - 1 {
                    self.cx = self.cx + 1
                }
            },
            'w' => {
                if self.cy > 0 {
                    self.cy = self.cy - 1
                }
            }
            's' => {
                // crossterm的cursor左上角单元格是(0,0)
                // crossterm的size左上角单元格是(1,1)
                // 注意转换
                if self.cy < self.rows - 1 {
                    self.cy = self.cy + 1
                }
            },
            _ => println!("unknow key"),
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
