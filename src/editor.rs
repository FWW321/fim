pub mod key;
pub mod state;

use std::char;
use std::io::Write;
use std::ops::Drop;
use std::process::exit;

use crossterm::{ExecutableCommand, QueueableCommand, cursor, terminal};
use tokio::fs::File;

use super::error::Result;
use super::utils;
use crate::reader::ByteStream;
use crate::reader::Decoder;
use crate::reader::KeyStream;
pub use key::{ControlKey, Direction, Key};
pub use state::State;

const TAB_STOP: u8 = 8;

struct Row {
    raw: Vec<Key>,
    rendered: String,
    is_dirty: bool,
}

impl Row {
    fn new(raw: Vec<Key>) -> Self {
        let rendered = Self::render(&raw);
        Self {
            raw,
            rendered,
            is_dirty: false,
        }
    }

    fn len(&self) -> usize {
        self.rendered.len()
    }

    fn chars(&self) -> std::str::Chars<'_> {
        self.rendered.chars()
    }

    fn render(keys: &[Key]) -> String {
        let mut rendered = String::new();
        for key in keys {
            let Some(c) = Self::render_key(key) else {
                continue;
            };
            for ch in c {
                rendered.push(ch);
            }
        }
        rendered
    }

    fn render_key(key: &Key) -> Option<Vec<char>> {
        // 将制表符转换为多个空格
        // 制表符不会擦除其所在位置的屏幕上的任何字符
        // 制表符只是将光标向前移动到下一个制表位
        // 空格会擦除之前存在的字符
        match key {
            Key::ControlKey(ControlKey::Tab) => {
                let mut chars = Vec::with_capacity(TAB_STOP as usize);
                for _ in 0..TAB_STOP {
                    chars.push(' ');
                }
                Some(chars)
            }
            Key::Char(c) => {
                let mut chars = Vec::with_capacity(1);
                chars.push(*c);
                Some(chars)
            }
            _ => None,
        }
    }
}

pub struct Editor<W: Write> {
    writer: W,
    // cursor横坐标
    cx: u16,
    // cursor纵坐标
    cy: u16,
    // 行偏移量
    row_offset: usize,
    // 列偏移量
    col_offset: usize,
    max_col: u16,
    max_row: u16,
    // rows应该存储key而不是string
    // 显示的时候再进行渲染
    // 这样可以控制比如tab等的渲染方式
    // 同时也可以按照原始输入存储
    // 每个key占据的宽度可能不一样，列数是渲染后的宽度
    // 真的需要存储key吗？
    // 如果希望tab转换为空格，那么存储时是不是也希望将tab存储为空格呢？
    // 使用key的灵活性更高，可以设置一个开关，决定是否将tab转换为空格
    // 如果不转换，则按空格显示，但是按tab存储
    // 如果转换，则按空格存储和显示
    // rows: Vec<String>,
    rows: Vec<Row>,
}

impl<W: Write> Editor<W> {
    pub async fn start(writer: W, file: Option<&str>) -> Self {
        // 进入原始模式
        terminal::enable_raw_mode().unwrap();
        let (max_col, max_row) = terminal::size().unwrap();
        let mut editor = Self {
            cx: 0,
            cy: 0,
            row_offset: 0,
            col_offset: 0,
            writer,
            max_col,
            max_row,
            rows: Vec::new(),
        };
        editor
            .writer
            // 进入备用屏幕
            .queue(terminal::EnterAlternateScreen)
            .unwrap()
            // 设置标题
            .queue(terminal::SetTitle("editor"))
            .unwrap();

        if let Some(file) = file {
            editor.open_file(file).await.unwrap();
        }
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

        // 清除屏幕内容
        self.writer
            .queue(terminal::Clear(terminal::ClearType::All))?;

        self.draw_rows()?;

        self.writer
            // 将光标移动回来
            // cx和cy是rows中的坐标，所以需要减去偏移量
            .queue(cursor::MoveTo(
                self.cx - self.col_offset as u16,
                self.cy - self.row_offset as u16,
            ))?
            .execute(cursor::Show)?;

        Ok(())
    }

    fn draw_rows(&mut self) -> Result<()> {
        for i in self.row_offset..self.max_row as usize + self.row_offset {
            if i < self.rows.len() {
                let row = &self.rows[i];
                for (i, c) in row.chars().enumerate() {
                    if i < self.col_offset {
                        continue;
                    }

                    write!(&mut self.writer, "{c}")?;

                    if i + 1 == self.col_offset + self.max_col as usize {
                        break;
                    }
                }
            } else {
                write!(&mut self.writer, "~")?;
            }

            if i + 1 == self.max_row as usize / 3 && self.rows.is_empty() {
                let mut welcome = format!("fim -- version: {}", utils::get_version_from_env());
                // 如果欢迎字符串的宽度超过终端宽带，则截断
                if welcome.len() > self.max_col as usize {
                    let bytes = welcome.as_bytes();
                    let len = std::cmp::min(bytes.len(), self.max_col as usize);
                    // 安全：因为我们知道welcome中是ASCII，所以可以直接从字节重建字符串
                    welcome = unsafe { String::from_utf8_unchecked(bytes[..len].to_vec()) };
                }
                // welcome足够短，u16不会丢失信息
                // 计算边距
                let margin = (self.max_col - welcome.len() as u16) / 2;
                self.writer.queue(cursor::MoveToColumn(margin))?;
                self.writer.write(welcome.as_bytes())?;
            }

            // 最后一行不打印\r\n
            // 如果最后一行打印\r\n会导致屏幕滚动到下一行
            // 这样最后一行没有~
            if i + 1 < self.row_offset + self.max_row as usize {
                write!(&mut self.writer, "\r\n")?;
            }
        }
        Ok(())
    }

    pub async fn open_file(&mut self, filename: &str) -> Result<()> {
        // file和stdin一样实现了read trait，可以用byte_stream包装
        // decoder实现一个read_line和lines方法
        // 这样可以支持不同编码的文件读取
        let file = File::open(filename).await?;
        // lines获取的行不会包含换行符
        // 因为我们知道一个line代表一行，因此存储换行符是没有意义的
        let byte_stream = ByteStream::new(file);
        let decoder = Decoder::builder()
            .encoding("utf-8".to_string())
            .byte_stream(byte_stream)
            .build()?;

        let mut key_stream = KeyStream::new(decoder);

        let mut key_line = Vec::new();

        while let Some(key) = key_stream.next_key().await? {
            if key == Key::ControlKey(ControlKey::CR) {
                continue;
            } else if key == Key::ControlKey(ControlKey::LF) {
                let row = Row::new(key_line);
                self.rows.push(row);
                key_line = Vec::new();
            } else {
                key_line.push(key);
            }
        }
        Ok(())
    }

    pub fn handle_command(&mut self, key: &Key) {
        match key {
            Key::ControlKey(ControlKey::Ctrl('q')) => {
                self.end();
                exit(0);
            }
            // 必须使用括号分组，不然只绑定了'a'，是不完整的绑定
            key @ (Key::ArrowKey(Direction::Left)
            | Key::ArrowKey(Direction::Right)
            | Key::ArrowKey(Direction::Up)
            | Key::ArrowKey(Direction::Down)
            | Key::ControlKey(ControlKey::Home) 
            | Key::ControlKey(ControlKey::End)) => {
                self.move_cursor(key);
            }
            Key::FunctionKey(n) => {
                println!("F{n}");
            }
            Key::ControlKey(ControlKey::Escape) => {
                print!("esc");
            }
            Key::ControlKey(ControlKey::PageUp) => {
                self.scroll_srceen(self.cy as usize + self.row_offset, Direction::Up);
            }
            Key::ControlKey(ControlKey::PageDown) => {
                self.scroll_srceen(self.rows.len() - self.cy as usize, Direction::Down);
            }
            _ => {}
        }
    }

    fn move_cursor(&mut self, key: &Key) {
        match key {
            Key::ArrowKey(Direction::Left) => self.sub_cx(),
            Key::ArrowKey(Direction::Right) => self.add_cx(),
            Key::ArrowKey(Direction::Up) => self.sub_cy(),
            Key::ArrowKey(Direction::Down) => self.add_cy(),
            Key::ControlKey(ControlKey::Home) => {
                self.startx();
            }
            Key::ControlKey(ControlKey::End) => {
                self.endx();
            }
            _ => println!("unknow key"),
        }
    }

    fn scroll_srceen(&mut self, nums: usize, direction: Direction) {
        match direction {
            Direction::Up => {
                for _ in 0..nums {
                    self.move_cursor(&Key::ArrowKey(Direction::Up));
                }
            }
            Direction::Down => {
                for _ in 0..nums {
                    self.move_cursor(&Key::ArrowKey(Direction::Down));
                }
            }
            Direction::Left => {}
            Direction::Right => {}
        }
    }

    fn endx(&mut self) {
        let row_len = if self.rows.is_empty() {
                    0
                } else {
                    self.rows[self.cy as usize].len()
                };

                if row_len == 0 {
                    self.cx = 0;
                    return;
                }

                if row_len > self.max_col as usize {
                    self.col_offset = row_len - self.max_col as usize;
                    self.cx = self.max_col + self.col_offset as u16 - 1;
                } else {
                    self.cx = row_len as u16 - 1;
                    // self.col_offset = 0;
                }
    }

    fn startx(&mut self) {
        self.cx = 0;
        self.col_offset = 0;
    }

    fn add_cx(&mut self) {
        let row_len = if self.rows.is_empty() {
            0
        } else {
            self.rows[self.cy as usize].len()
        };

        if row_len == 0 {
            self.cx = 0;
            return;
        }

        if (self.cx as usize) < row_len - 1 {
            self.cx += 1;

            if self.cx as usize - self.col_offset == self.max_col as usize {
                self.col_offset += 1;
            }
        }else {
            let pre_cy = self.cy;
            self.add_cy();
            if pre_cy != self.cy {
                self.cx = 0;
                self.col_offset = 0;
            }
        }
    }

    fn sub_cx(&mut self) {
        // 注意：坐标不能小于0
        if self.cx != 0 {
            self.cx -= 1;

            if self.cx as usize + 1 - self.col_offset == 0 {
                if self.col_offset != 0 {
                    self.col_offset -= 1;
                }
            }
        } else {
            let pre_cy = self.cy;
            self.sub_cy();
            if pre_cy != self.cy {
                self.endx();
            }
        }
    }

    fn add_cy(&mut self) {
        // crossterm的cursor左上角单元格是(0,0)
        // crossterm的size左上角单元格是(1,1)
        // 注意转换
        if self.rows.is_empty() {
            self.cy = 0;
            return;
        }

        if (self.cy as usize) < self.rows.len() - 1 {
            self.cy += 1;

            // 如果光标移动到屏幕底部，则滚动屏幕
            if self.cy as usize - self.row_offset == self.max_row as usize {
                self.row_offset += 1;
            }
        }
        self.clamp_cursor_x();
    }

    fn sub_cy(&mut self) {
        if self.cy != 0 {
            self.cy -= 1;

            // 如果光标移动到屏幕顶部，则滚动屏幕
            if self.cy as usize + 1 - self.row_offset == 0 {
                if self.row_offset != 0 {
                    self.row_offset -= 1;
                }
            }
        }
        self.clamp_cursor_x();
    }

    fn clamp_cursor_x(&mut self) {
        let row_len = if self.rows.is_empty() {
            0
        } else {
            self.rows[self.cy as usize].len()
        };

        if row_len <= self.max_col as usize {
            self.col_offset = 0;
        }

        if row_len == 0 {
            self.cx = 0;
            return;
        }

        if self.cx as usize > row_len - 1 {
            self.cx = (row_len - 1) as u16;
        }
    }

    pub fn end(&mut self) {
        // 禁用终端的原始模式，恢复到规范模式（canonical mode）
        terminal::disable_raw_mode().unwrap();
        // 离开备用屏幕
        self.writer.execute(terminal::LeaveAlternateScreen).unwrap();
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
        self.end();
    }
}
