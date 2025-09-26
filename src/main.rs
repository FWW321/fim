use std::io::{self, BufWriter};

use fim::reader::{ByteStream, Decoder, KeyStream};
use tokio::io::stdin;
use tracing::Level;

use fim::editor::Editor;
use fim::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::ERROR)
        .init();

    // std::io::stout() 会返回返回当前进程的标准输出流 stdout 的句柄
    // 将内容刷新到终端是很昂贵的操作
    // 封装一个writer并缓冲其输出，避免频繁系统调用
    // BufWriter::new方法创建的缓冲区默认容量为8KiB
    // 务必在BufWrite drop前调用flush
    // 虽然BufWrite drop时会尝试刷新缓冲区，但会忽略错误
    // 调用flush能确保缓冲区被清空
    let stdout = BufWriter::new(io::stdout());

    // let mut editor = Editor::start(stdout, None).await;

    let reader = stdin();

    let byte_stream = ByteStream::new(reader);

    let decoder = Decoder::builder()
        .encoding("utf-8".to_owned())
        .byte_stream(byte_stream)
        .build()?;

    let key_stream = KeyStream::new(decoder);

    let mut editor = Editor::new(key_stream, stdout).await;

    editor.start(Some("test.txt")).await;

    editor.run().await;

    // 不需要join来使主线程阻塞等待handler关联的线程结束
    // 线程不结束sender不会被销毁，receiver的循环也不会结束
    // if let Err(e) = handler.join() {
    //     eprintln!("线程发生恐慌：{:?}", e);
    // };

    Ok(())
}
