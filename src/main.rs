use std::io::{self, BufWriter};

use fim::editor::{Editor, State};
use fim::error::Result;
use fim::reader::encoding::Encoding;
use fim::reader::CharReader;

fn main() -> Result<()> {
    // std::io::stout() 会返回返回当前进程的标准输出流 stdout 的句柄
    // 将内容刷新到终端是很昂贵的操作
    // 封装一个writer并缓冲其输出，避免频繁系统调用
    // BufWriter::new方法创建的缓冲区默认容量为8KiB
    // 务必在BufWrite drop前调用flush
    // 虽然BufWrite drop时会尝试刷新缓冲区，但会忽略错误
    // 调用flush能确保缓冲区被清空
    let stdout = BufWriter::new(io::stdout());

    let mut editor = Editor::new(stdout);

    let mut char_reader = CharReader::new(Encoding::Utf8);

    loop {
        match char_reader.get_key() {
            Ok(Some(key)) => match editor.process_key(key) {
                State::Continue => {
                    // 每次更新完editor的状态后刷新屏幕
                    editor.refresh_screen()?;
                }
                State::Exit => break,
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

    // 不需要join来使主线程阻塞等待handler关联的线程结束
    // 线程不结束sender不会被销毁，receiver的循环也不会结束
    // if let Err(e) = handler.join() {
    //     eprintln!("线程发生恐慌：{:?}", e);
    // };

    Ok(())
}
