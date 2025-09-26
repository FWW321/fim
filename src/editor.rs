pub mod key;

use std::io::Write;
use std::ops::Drop;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;
use std::u16;

use crossterm::{ExecutableCommand, QueueableCommand, cursor, terminal};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;

use super::error::{EditorError, Result};
use super::utils;
use crate::reader::ByteStream;
use crate::reader::Decoder;
use crate::reader::KeyStream;
use crate::utils::color;
use utils::find_subsequence;

pub use key::{ControlKey, Direction, Key};

struct Row {
    // 是否需要存储为string
    // 如果存储key每次保存都需要转换
    // 但是可以保留原始输入
    raw: Vec<Key>,
    rendered: String,
}

impl Row {
    fn new(raw: Vec<Key>) -> Self {
        let rendered = String::new();
        let mut row = Self { raw, rendered };
        row.render();
        row
    }

    fn display_len(&self) -> usize {
        self.rendered.len()
    }

    fn append(&mut self, other: &Row) {
        self.raw.extend_from_slice(&other.raw);
        self.rendered.push_str(&other.rendered);
    }

    fn chars(&self) -> std::str::Chars<'_> {
        self.rendered.chars()
    }

    fn raw(&self) -> String {
        Self::raw_str(&self.raw)
    }

    fn raw_str(keys: &[Key]) -> String {
        let mut raw = String::new();
        for key in keys {
            match key {
                Key::ControlKey(ControlKey::Tab) => {
                    raw.push('\t');
                }
                Key::Char(c) => {
                    raw.push(*c);
                }
                _ => {}
            }
        }
        raw
    }

    fn render(&mut self) {
        for key in &self.raw {
            match key {
                Key::ControlKey(ControlKey::Backspace) => {
                    // render函数只不可变借用了raw字段
                    // backspace函数只可变借用了rendered字段
                    // 但是借用检察器只查看函数签名，认为backspace函数可变借用了self
                    // self.backspace();
                    if self.rendered.is_empty() {
                        return;
                    }
                    let key = self.raw.last().unwrap();
                    for _ in 0..key.get_display_width() {
                        self.rendered.pop();
                    }
                }
                _ => {
                    let s = key.render();
                    if s.is_empty() {
                        continue;
                    }
                    self.rendered.push_str(&s);
                }
            }
        }
    }

    fn backspace(&mut self, at: usize) -> usize{
        if at >= self.rendered.len() {
            let last_key = self.raw.pop().unwrap();
            let width = last_key.get_display_width();
            for _ in 0..width {
                self.rendered.pop();
            }
            width
        } else {
            let raw_index = self.get_raw_index(at - 1);
            let (start, end) = self.get_render_index(raw_index);
            self.rendered.drain(start..end);
            self.raw.remove(raw_index);
            end - start
        }
    }

    fn get_render_index(&self, raw_index: usize) -> (usize, usize) {
        let mut render_index = 0;
        for key in &self.raw[..raw_index] {
            render_index += key.get_display_width();
        }
        (
            render_index,
            render_index + &self.raw[raw_index].get_display_width(),
        )
    }

    fn push(&mut self, key: Key) {
        let rendered = key.render();
        if !rendered.is_empty() {
            self.raw.push(key);
            self.rendered.push_str(&rendered);
        }
    }

    fn get_raw_index(&self, render_index: usize) -> usize {
        let mut current_render_index = 0;
        for (i, key) in self.raw.iter().enumerate() {
            let key_width = key.get_display_width();
            if current_render_index + key_width > render_index {
                return i;
            }
            current_render_index += key_width;
        }
        self.raw.len()
    }

    fn split(&mut self, at: usize) -> Row {
        if at >= self.rendered.len() {
            return Row::new(Vec::new());
        }
        let raw_index = self.get_raw_index(at);
        let new_raw = self.raw.split_off(raw_index);
        let new_row = Row::new(new_raw);
        self.rendered.truncate(at);
        new_row
    }

    fn insert(&mut self, at: usize, key: Key) -> bool {
        if at >= self.rendered.len() {
            let appended = key.render();
            if appended.is_empty() {
                return false;
            }
            self.raw.push(key);
            self.rendered.push_str(&appended);
        } else {
            let inserted = key.render();
            if inserted.is_empty() {
                return false;
            }
            let raw_index = self.get_raw_index(at);
            self.raw.insert(raw_index, key);
            self.rendered.insert_str(at, &inserted);
        }
        true
    }
}

struct Message {
    text: String,
    time: Instant,
}

impl Message {
    fn new(text: String) -> Self {
        Self {
            text,
            time: Instant::now(),
        }
    }
}

pub struct Editor<R: AsyncReadExt + Unpin, W: Write> {
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
    current_file: Option<PathBuf>,
    message: Option<Message>,
    // 可以将dirty设置为一个整数，可以反映该文件到底有脏
    is_dirty: bool,
    key_stream: KeyStream<R>,
}

impl<R: AsyncReadExt + Unpin, W: Write> Editor<R, W> {
    pub async fn new(key_stream: KeyStream<R>, writer: W) -> Self {
        Self {
            cx: 0,
            cy: 0,
            row_offset: 0,
            col_offset: 0,
            writer,
            max_col: 0,
            // 留给状态栏和消息栏
            max_row: 0,
            rows: Vec::new(),
            current_file: None,
            message: None,
            is_dirty: false,
            key_stream,
        }
    }

    pub async fn start(&mut self, file: Option<&str>) {
        // 进入原始模式
        terminal::enable_raw_mode().unwrap();

        let (max_col, max_row) = terminal::size().unwrap();

        self.max_col = max_col;
        self.max_row = max_row - 2;

        self
            .writer
            // 进入备用屏幕
            .queue(terminal::EnterAlternateScreen)
            .unwrap()
            // 设置标题
            .queue(terminal::SetTitle("editor"))
            .unwrap();

        self.current_file = file.map(|f| PathBuf::from(f));

        if let Some(file) = file {
            self.open_file(file).await.unwrap();
        }
        self.refresh_screen().unwrap();
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
            // 有了状态栏便不是最后一行了
            // 如果动态调整，那么就不需要考虑最后一行的问题
            // 由bar自己添加换行符
            // 状态栏应该常驻
            // if i + 1 < self.row_offset + self.max_row as usize {
            //     write!(&mut self.writer, "\r\n")?;
            // }
            write!(&mut self.writer, "\r\n")?;
        }

        // let message = Message::new(format!("{}x{}", self.max_col, self.max_row));
        // self.message = Some(message);
        self.draw_status_bar()?;
        self.draw_message_bar()?;
        Ok(())
    }

    fn draw_status_bar(&mut self) -> Result<()> {
        // self.writer
        //     .queue(cursor::MoveTo(0, self.max_row))?;

        // 可以使用magical_rs检测文件类型
        let filename = match &self.current_file {
            // 当Option是Some时，and_then应用闭包返回新的Option
            // 如果是None，则直接返回None
            Some(name) => name
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("[No Name]"),
            None => "[No Name]",
        };
        let modified = if self.is_dirty { "(modified)" } else { "" };
        let mut content = format!(
            "{}{} Ln {}/{}, Col {}",
            filename,
            modified,
            self.cy + 1,
            self.rows.len(),
            self.cx + 1
        );
        // TODO: 后面优化显示效果
        if content.len() > self.max_col as usize {
            content.truncate(self.max_col as usize);
        } else {
            while content.len() < self.max_col as usize {
                content.push(' ');
            }
        }
        let status = format!("{}{}{}", color::BG_RED, content, color::RESET);
        write!(&mut self.writer, "{}", status)?;
        Ok(())
    }

    fn draw_message_bar(&mut self) -> Result<()> {
        if let Some(message) = &self.message {
            // 只在按键后才刷新屏幕，所以5秒后按下按键才会消失
            if message.time.elapsed().as_secs() < 5 {
                write!(&mut self.writer, "\r\n")?;
                // 每次都会减去一行，不行，后续优化动态调整
                // self.max_row -= 1;
                let mut content = message.text.clone();
                if content.len() > self.max_col as usize {
                    content.truncate(self.max_col as usize);
                } else {
                    while content.len() < self.max_col as usize {
                        content.push(' ');
                    }
                }
                let message = format!("{}{}{}", color::BG_BLUE, content, color::RESET);
                write!(&mut self.writer, "{}", message)?;
            }
        }
        Ok(())
    }


    fn search(&mut self, query: &[Key]) -> Result<()> {
        for (i, r) in self.rows.iter().enumerate() {
            if let Some(pos) = find_subsequence(&r.raw, query) {
                self.cy = i as u16;
                self.cx = pos as u16;
                if self.cy < self.row_offset as u16 {
                    self.row_offset = self.cy as usize;
                } else if self.cy >= self.row_offset as u16 + self.max_row {
                    self.row_offset = self.cy as usize - self.max_row as usize + 1;
                }
                if self.cx < self.col_offset as u16 {
                    self.col_offset = self.cx as usize;
                } else if self.cx >= self.col_offset as u16 + self.max_col {
                    self.col_offset = self.cx as usize - self.max_col as usize + 1;
                }
                return Ok(());
            }
        }
        Err(EditorError::NotFound)
    }

    async fn get_key(&mut self) -> Result<Key> {
        if let Some(key) = self.key_stream.next_key().await? {
            Ok(key)
        } else {
            Err(EditorError::UnexpectedEof { expected: "ESC OR CR".to_owned(), actual: 0 })
        }
    }

    async fn find(&mut self) {
        let current_cx = self.cx;
        let current_cy = self.cy;
        let mut row = Row::new(Vec::new());
        let prompt = "Search: ";
        self.message = Some(Message::new(prompt.to_string()));
        self.cy = self.max_row + 2 + self.row_offset as u16;
        self.cx = prompt.len() as u16;

        loop {
            self.refresh_screen().unwrap();
            let key = match self.get_key().await {
                Ok(key) => {
                    key
                },
                Err(e) => {
                    self.message = None;
                    self.cy = current_cy;
                    self.cx = current_cx;
                    self.message = Some(Message::new(format!("Error reading Key: {}", e)));
                    break;
            }
        };
            match key {
                Key::ControlKey(ControlKey::Escape) => {
                        self.message = None;
                        self.cy = current_cy;
                        self.cx = current_cx;
                        break;
                    }
                    Key::ControlKey(ControlKey::CR) => {
                        self.message = None;
                        break;
                    },
                    Key::ControlKey(ControlKey::Backspace) => {
                        if !row.raw.is_empty() {
                            row.backspace(self.cx as usize);
                            if self.cx > prompt.len() as u16 {
                                self.cx -= 1;
                            }
                            // TODO: bar的消息显示随着光标位置变化
                            // 需要单独的偏移量，不能直接使用editor的偏移量
                            self.message = Some(Message::new(format!("{}{}",
                            prompt, &row.rendered)));
                        }
                    }
                    Key::ArrowKey(Direction::Left) => {
                        if self.cx > prompt.len() as u16 {
                            self.cx -= 1;
                        }
                        self.message = Some(Message::new(format!("{}{}",
                            prompt, &row.rendered)));
                    }
                    Key::ArrowKey(Direction::Right) => {
                        if (self.cx as usize) < row.display_len() {
                            self.cx += 1;
                        }
                        self.message = Some(Message::new(format!("{}{}",
                            prompt, &row.rendered)));
                    }
                    _ => {
                        row.push(key);
                        if self.cx < self.max_col - 1 {
                            self.cx += 1;
                        }
                        self.message = Some(Message::new(format!("{}{}",
                            prompt, &row.rendered)));
                    }
            }
            if let Err(_) = self.search(&row.raw) {
                    self.cx = current_cx;
                    self.cy = current_cy;
                    self.message = Some(Message::new(format!("Not Found: {}", &row.rendered)));
                }
        }
    }

    fn insert(&mut self, key: Key) {
        let is_last_row = (self.cy as usize) == self.rows.len();
        let row = if !is_last_row {
            &mut self.rows[self.cy as usize]
        } else {
            // 如果光标在最后一行的后面，则添加新行
            self.rows.push(Row::new(Vec::new()));
            self.rows.last_mut().unwrap()
        };
        // raw mode下，enter键发送的是\r
        if  key == Key::ControlKey(ControlKey::CR) {
            self.message = Some(Message::new("".to_string()));
            let new_row = row.split(self.cx as usize);
            self.rows.insert(self.cy as usize + 1, new_row);
            if is_last_row {
                self.rows.pop();
            }
            self.add_cy();
            self.cx = 0;
            self.col_offset = 0;
            self.is_dirty = true;
            return;
        }
        if let true = row.insert(self.cx as usize, key) {
            self.is_dirty = true;
            self.add_cx();
        } else {
            if is_last_row {
                self.rows.pop();
            }
        }
    }

    pub async fn open_file(&mut self, filename: impl AsRef<Path>) -> Result<()> {
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

    pub async fn save(&mut self) -> Result<()> {
        let Some(path) = &self.current_file else {
            let message = Message::new("No file name".to_string());
            self.message = Some(message);
            return Ok(());
        };
        let path = path.as_path();
        // create会完全截断文件，使其变为空文件
        // 然后写入新数据
        // 如果文件不存在则创建新文件
        // 更好的做法是将文件截断为计划写入的数据相同长度
        // 如果长度不够则在文件末尾添加0使其达到指定长度
        // 最佳做法是写入新的临时文件，然后将该文件重命名为用户想要覆盖的实际文件
        let mut file = File::create(path).await?;
        for row in &self.rows {
            let raw = row.raw();
            file.write_all(raw.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        let message = Message::new("File saved".to_string());
        self.message = Some(message);
        self.is_dirty = false;
        Ok(())
    }

    pub async fn run(&mut self) {
        loop {
        match self.key_stream.next_key().await {
            Ok(Some(key)) =>  {
                match key {
                    Key::ControlKey(ControlKey::Ctrl('q')) => {
                        // self.end();
                        break;
                    },
                    _ => {
                        self.handle_command(&key).await;
                        self.refresh_screen().unwrap();
                    }
                }
            },
            Ok(None) => {
                // EOF reached
                println!("End of input reached.");
                break;
            }
            Err(e) => {
                eprintln!("Error reading Key: {}", e);
            }
        }
    }
    }

    pub async fn handle_command(&mut self, key: &Key) {
        match key {
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
            Key::ControlKey(ControlKey::Backspace) => {
                self.backspace();
            }
            Key::ControlKey(ControlKey::Delete) => {
                self.delete();
            }
            Key::ControlKey(ControlKey::Ctrl('f')) => {
                self.find().await;
            }
            Key::ControlKey(ControlKey::Ctrl('s')) => {
                if let Err(e) = self.save().await {
                    let message = Message::new(format!("Error saving file: {}", e));
                    self.message = Some(message);
                }
            }
            _ => {
                self.insert(key.clone());
            }
        }
    }

    fn delete(&mut self) {
        self.add_cx();
        self.backspace();
    }

    fn backspace(&mut self) {
        // 如果是多线程，则is_dirty需要使用mutex保护
                // 整个代码块都是临界区
                if self.cx != 0 && (self.cy as usize) < self.rows.len() {
                    let row = &mut self.rows[self.cy as usize];
                    let width = row.backspace(self.cx as usize);
                    for _ in 0..width {
                        // sub_cx会使用cx计算raw_index，但是row已经被修改了
                        // cx没有修改，所以计算出来的raw_index是错误的
                        // self.sub_cx();

                        self.cx -= 1;
                        if (self.cx as usize)  < self.col_offset {
                            self.col_offset -= 1;
                        }
                    }
                    self.is_dirty = true;
                } else if (self.cy as usize) >= self.rows.len() {
                    self.sub_cx();
                } else {
                    if self.cy == 0 {
                        return;
                    }
                    let current_cy = self.cy;
                    self.sub_cx();
                    let current_row = self.rows.remove(current_cy as usize);
                    let prev_row = &mut self.rows[current_cy as usize - 1];
                    prev_row.append(&current_row);
                    self.is_dirty = true;
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
            if (self.cy as usize) < self.rows.len() {
                self.rows[self.cy as usize].display_len()
            } else {
                0
            }
        };

        if row_len == 0 {
            self.cx = 0;
            return;
        }

        if row_len > self.max_col as usize {
            // 光标可以在最后一个字符的后面，可以插入
            self.col_offset = row_len - self.max_col as usize + 1;

            self.cx = self.max_col + self.col_offset as u16 - 1;
        } else {
            // self.cx = row_len as u16 - 1;
            // self.col_offset = 0;
            self.cx = row_len as u16;
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
            if (self.cy as usize) < self.rows.len() {
                self.rows[self.cy as usize].display_len()
            } else {
                0
            }
        };

        if row_len == 0 {
            self.cx = 0;
            return;
        }

        let row = &self.rows[self.cy as usize];

        // 光标可以在最后一个字符的后面，可以插入
        if (self.cx as usize) < row_len {
            let raw_index = row.get_raw_index(self.cx as usize);
            let (_, end) = row.get_render_index(raw_index);
            self.cx = end as u16;
            // self.cx += 1;

            if self.cx as usize >= self.max_col as usize {
                self.col_offset = self.cx as usize + 1 - self.max_col as usize;
            }
        } else {
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
            let row = &self.rows[self.cy as usize];

            let raw_index = row.get_raw_index(self.cx as usize - 1);
            let (start, _) = row.get_render_index(raw_index);
            let distance = self.cx as usize - start;
            self.cx = start as u16;
            // self.cx -= 1;

            // col_offset代表屏幕左边第一个字符在行中的位置
            // cx代表光标在行中的位置
            // 如果cx小于col_offset，说明光标在屏幕左边第一个字符的左边
            // 需要将col_offset向左移动，保证光标在屏幕内
            if (self.cx as usize) < self.col_offset {
                if self.col_offset >= distance {
                    self.col_offset -= distance;
                } else {
                    self.col_offset = 0;
                }
            }
            // if self.cx as usize >= self.max_col as usize {
            //     self.col_offset = self.cx as usize + 1 - self.max_col as usize;
            // } else {
            //     self.col_offset = 0;
            // }
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

        // 光标可以在最后一行的后面，可以插入
        if (self.cy as usize) < self.rows.len() {
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
            if (self.cy as usize) < self.rows.len() {
                self.rows[self.cy as usize].display_len()
            } else {
                0
            }
        };

        if row_len <= self.max_col as usize {
            self.col_offset = 0;
        }

        if row_len == 0 {
            self.cx = 0;
            return;
        }

        if self.cx as usize > row_len {
            self.cx = row_len as u16;
        }
    }

    fn end(&mut self) {
        // 禁用终端的原始模式，恢复到规范模式（canonical mode）
        terminal::disable_raw_mode().unwrap();
        // 离开备用屏幕
        self.writer.execute(terminal::LeaveAlternateScreen).unwrap();
    }
}

impl<R: AsyncReadExt + Unpin, W: Write> Drop for Editor<R, W> {
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
