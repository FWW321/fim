use std::collections::VecDeque;
use std::marker::Unpin;

use tracing::{debug, error, instrument, trace};
use tokio::io::AsyncReadExt;

use crate::error::{EditorError, Result};

const BUFFER_SIZE: usize = 1024;

/// 读取原始字节数据
/// 负责缓冲和管理IO
pub struct ByteStream<R: AsyncReadExt + Unpin> {
    /// 读取器
    reader: R,
    /// 字节缓冲区，用于预读和缓存
    byte_buffer: VecDeque<u8>,
    /// 读取缓冲区，减少系统调用次数
    // 可以直接去掉，这样由传入的reader控制
    // 如果reader可以自带buffer机制或者不带
    // 用上也没有什么开销，如果自带buffer多的开销只是拷贝了一次
    read_buffer: Vec<u8>,
}

impl<R: AsyncReadExt + Unpin> ByteStream<R> {
    #[instrument(skip(reader))]
    pub fn new(reader: R) -> Self {
        debug!("Creating new ByteStream with buffer sizes: {}", BUFFER_SIZE);

        Self {
            reader,
            byte_buffer: VecDeque::with_capacity(BUFFER_SIZE),
            read_buffer: Vec::with_capacity(BUFFER_SIZE),
        }
    }

    /// 读取单个字节
    ///
    /// # Returns
    /// - `Ok(Some(byte))` - 成功读取到字节
    /// - `Ok(None)` - 输入流关闭
    /// - `Err(error)` - 读取过程中发生错误
    #[instrument(skip(self))]
    pub async fn read_next_byte(&mut self) -> Result<Option<u8>> {
        // 优先从缓冲区读取，读到直接返回
        if let Some(byte) = self.byte_buffer.pop_front() {
            trace!(
                "Read byte from buffer: 0x{:02X} ( '{} )",
                byte,
                if byte.is_ascii_graphic() {
                    byte as char
                } else {
                    '.'
                }
            );
            return Ok(Some(byte));
        }

        // 没有数据时填充缓冲区
        self.fill_buffer().await.map_err(|e| {
            error!("Failed to fill buffer: {}", e);
            e
        })?;

        let result = self.byte_buffer.pop_front();
        if let Some(byte) = result {
            trace!(
                "Read byte after buffer fill: 0x{:02X} ('{}')",
                byte,
                if byte.is_ascii_graphic() {
                    byte as char
                } else {
                    '.'
                }
            );
        } else {
            debug!("​​Input stream closed");
        }

        Ok(result)
    }

    /// 缓冲区为空时填充缓冲区
    #[instrument(skip(self))]
    async fn fill_buffer(&mut self) -> Result<()> {
        if !self.byte_buffer.is_empty() {
            trace!(
                "Buffer not empty, skipping fill (current size: {})",
                self.byte_buffer.len()
            );
            return Ok(());
        }

        // 切片的长度是read_buffer的len长度，所以只需要调整长度就行，避免内存分配
        self.read_buffer.resize(BUFFER_SIZE, 0);

        match self.reader.read(&mut self.read_buffer).await {
            Ok(0) => {
                debug!("​​Input stream closed​​");
            }
            Ok(size) => {
                self.byte_buffer.extend(&self.read_buffer[..size]);
                trace!("Buffer filled with {} bytes", size);
            }
            Err(e) => {
                error!("I/O error during buffer fill: {}", e);
                return Err(EditorError::Io { source: e });
            }
        }
        Ok(())
    }

    /// 预读指定数量的字节，不从缓冲区移除
    /// 用于转义序列解析等需要前瞻的场景
    ///
    /// # Arguments
    /// * `count` - 需要预读的字节数量
    ///
    /// # Returns
    /// 返回可用的字节切片，长度可能小于请求的数量（如遇到EOF）
    #[instrument(skip(self))]
    pub async fn peek_ahead(&mut self, count: usize) -> Result<&[u8]> {
        // 限制预读数量以避免过度缓冲
        let safe_count = count.min(BUFFER_SIZE);

        while self.byte_buffer.len() < safe_count {
            // read一般会从索引0覆盖写入，可以不用clear
            // self.read_buffer.clear();
            self.read_buffer.resize(safe_count - self.byte_buffer.len(), 0);

            // 切片的长度是read_buffer的len长度，所以只需要调整长度就行，避免内存分配
            match self.reader.read(&mut self.read_buffer).await? {
                0 => break,
                size => {
                    self.byte_buffer.extend(&self.read_buffer[..size]);
                }
            }
        }

        let available_count = self.byte_buffer.len().min(safe_count);

        // slice和self.byte_buffer.make_contiguous()即便没有同时存在
        // 但是它们的生命周期都是与返回值的生命周期相同
        // 所以rust编译器会认为借用冲突
        // {
        //     let slice = self.byte_buffer.as_slices().0;

        //     if slice.len() >= available_count {
        //         return Ok(&slice[..available_count]);
        //     }
        // }
        // Ok(self.byte_buffer.make_contiguous())

        // 不可变借用会在作用域结束时drop
        let need_contiguous = {
            // VecDeque使用环形缓冲区存储数据，其内部维护一个 ​​逻辑上的连续序列
            // 如果数据被环形缓冲区分割，两个切片分别对应前半段和后半段
            let slice = self.byte_buffer.as_slices().0;
            slice.len() < available_count
        };

        // if分支，两者不可能同时存在
        if need_contiguous {
            // 重整数据
            Ok(self.byte_buffer.make_contiguous())
        } else {
            let (first_slice, _) = self.byte_buffer.as_slices();
            Ok(&first_slice[..available_count])
        }
    }

    /// 获取缓冲区中的字节数量
    pub fn buffered_count(&self) -> usize {
        self.byte_buffer.len()
    }
}
