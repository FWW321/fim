use std::error::Error;
use std::io::{self, BufWriter};
use std::thread;
use std::sync::mpsc;

use fim::reader::CharReader;
use fim::reader::encoding::Encoding;
use fim::editor::{Editor, State};

fn main() -> Result<(), Box<dyn Error>>{

    // std::io::stout() 会返回返回当前进程的标准输出流 stdout 的句柄
    // 将内容刷新到终端是很昂贵的操作
    // 封装一个writer并缓冲其输出，避免频繁系统调用
    // BufWriter::new方法创建的缓冲区默认容量为8KiB
    // 务必在BufWrite drop前调用flush
    // 虽然BufWrite drop时会尝试刷新缓冲区，但会忽略错误
    // 调用flush能确保缓冲区被清空
    let stdout = BufWriter::new(io::stdout());

    let mut editor = Editor::new(stdout);

    let (sender, receiver) = mpsc::channel::<char>();

    thread::spawn(move || {
        // std::io::stdin() 会返回返回当前进程的标准输入流 stdin 的句柄
        let stdin = io::stdin();
        // lock() 方法返回一个 StdinLock，对 Stdin 句柄的锁定引用
        // 每次对stdin进行读取都会临时加锁
        // stdinLock只需要锁定一次，就可以进行多次读取操作，不需要每次加锁
        let handle = stdin.lock();
        let mut char_reader = CharReader::new(handle, Encoding::Utf8);
        loop {
            match char_reader.read_char() {
                Ok(Some(c)) => {
                    if let Err(e) = sender.send(c) {
                        eprintln!("send error: {}", e)
                    };
                },
                Ok(None) => {
                    // EOF reached
                    println!("End of input reached.");
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading char: {}", e);
                }
            }
        }
    });

    for c in receiver {
        match editor.process_char(c) {
            State::Continue => {},
            State::Exit => break,
        }
        // 每次更新完editor的状态后刷新屏幕
        editor.refresh_screen()?;
    }

    // 不需要join来使主线程阻塞等待handler关联的线程结束
    // 线程不结束sender不会被销毁，receiver的循环也不会结束
    // if let Err(e) = handler.join() {
    //     eprintln!("线程发生恐慌：{:?}", e);
    // };

    Ok(())
}




