use std::io;
use thiserror::Error;

/// 详细的错误类型，提供丰富的上下文信息用于调试和诊断
#[derive(Debug, Error)]
pub enum EditorError {
    /// I/O操作错误，包含底层系统错误信息
    #[error("I/O operation failed: {source}")]
    Io {
        #[from]
        source: io::Error,
    },
    
    /// 编码错误，包含详细的上下文信息
    #[error("Invalid encoding detected at position {position}: {details}")]
    InvalidEncoding {
        /// 错误发生的字节位置
        position: usize,
        /// 错误的详细描述
        details: String,
        /// 导致错误的字节序列
        invalid_bytes: Vec<u8>,
    },
    
    /// 转义序列错误，包含序列内容和状态信息（CIS等状态）
    // #[error("Invalid escape sequence: {sequence} (state: {state})")]
    #[error("Invalid escape sequence: {sequence}")]
    InvalidSequence {
        /// 无效的转义序列
        sequence: String,
        /// 解析器当前状态
        // state: String,
        /// 序列长度
        length: usize,
    },
    
    /// 意外的输入结束，包含期望的内容信息
    #[error("Unexpected end of input while expecting {expected} (got {actual} bytes)")]
    UnexpectedEof {
        /// 期望的内容描述
        expected: String,
        /// 实际读取的字节数
        actual: usize,
    },
    
    /// 不支持的编码类型
    #[error("Encoding '{encoding}' is not supported. Available encodings: {}", available.join(", "))]
    UnsupportedEncoding {
        /// 请求的编码名称
        encoding: String,
        /// 可用的编码列表
        available: Vec<&'static str>,
    },
    
    /// 缓冲区溢出错误
    // #[error("Buffer overflow: attempted to store {attempted} bytes, but maximum capacity is {capacity}")]
    // BufferOverflow {
    //     /// 尝试存储的字节数
    //     attempted: usize,
    //     /// 缓冲区最大容量
    //     capacity: usize,
    // },
    
    /// 解析超时错误
    #[error("Parsing timeout: sequence '{sequence}' incomplete after {duration_ms}ms")]
    ParseTimeout {
        /// 未完成的序列
        sequence: String,
        /// 超时时长（毫秒）
        duration_ms: u64,
    },
    
    /// 资源耗尽错误
    #[error("Resource exhausted: {resource} limit of {limit} exceeded")]
    ResourceExhausted {
        /// 资源类型
        resource: String,
        /// 资源限制
        limit: usize,
    },

    // #[error("Encoding type not specified")]
    // EncodingNotSet,

    // #[error("Byte stream not provided")]
    // ByteStreamNotSet,
}

impl EditorError {
    /// 创建编码错误，包含详细的上下文信息
    pub fn invalid_encoding(position: usize, details: impl Into<String>, invalid_bytes: Vec<u8>) -> Self {
        Self::InvalidEncoding {
            position,
            details: details.into(),
            invalid_bytes,
        }
    }
    
    /// 创建转义序列错误
    pub fn invalid_sequence(sequence: impl Into<String>, length: usize) -> Self {
        Self::InvalidSequence {
            sequence: sequence.into(),
            // state: state.into(),
            length,
        }
    }

    // pub fn invalid_sequence(sequence: impl Into<String>, state: impl Into<String>, length: usize) -> Self {
    //     Self::InvalidSequence {
    //         sequence: sequence.into(),
    //         // state: state.into(),
    //         length,
    //     }
    // }
    
    /// 创建EOF错误
    pub fn unexpected_eof(expected: impl Into<String>, actual: usize) -> Self {
        Self::UnexpectedEof {
            expected: expected.into(),
            actual,
        }
    }
    
    /// 创建不支持编码错误
    pub fn unsupported_encoding(encoding: impl Into<String>, available: Vec<&'static str>) -> Self {
        Self::UnsupportedEncoding {
            encoding: encoding.into(),
            available,
        }
    }
    
    /// 创建缓冲区溢出错误
    // pub fn buffer_overflow(attempted: usize, capacity: usize) -> Self {
    //     Self::BufferOverflow { attempted, capacity }
    // }
    
    /// 创建解析超时错误
    pub fn parse_timeout(sequence: impl Into<String>, duration_ms: u64) -> Self {
        Self::ParseTimeout {
            sequence: sequence.into(),
            duration_ms,
        }
    }
    
    /// 创建资源耗尽错误
    pub fn resource_exhausted(resource: impl Into<String>, limit: usize) -> Self {
        Self::ResourceExhausted {
            resource: resource.into(),
            limit,
        }
    }
    
    /// 检查错误是否可恢复
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Io { .. } => false,
            Self::InvalidEncoding { .. } => true,
            Self::InvalidSequence { .. } => true,
            Self::UnexpectedEof { .. } => false,
            Self::UnsupportedEncoding { .. } => false,
            // Self::BufferOverflow { .. } => false,
            Self::ParseTimeout { .. } => true,
            Self::ResourceExhausted { .. } => false,
            // Self::ByteStreamNotSet => true,
            // Self::EncodingNotSet => true,
        }
    }
    
    /// 获取错误的严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Io { .. } => ErrorSeverity::Critical,
            Self::InvalidEncoding { .. } => ErrorSeverity::Warning,
            Self::InvalidSequence { .. } => ErrorSeverity::Warning,
            Self::UnexpectedEof { .. } => ErrorSeverity::Error,
            Self::UnsupportedEncoding { .. } => ErrorSeverity::Error,
            // Self::BufferOverflow { .. } => ErrorSeverity::Critical,
            Self::ParseTimeout { .. } => ErrorSeverity::Warning,
            Self::ResourceExhausted { .. } => ErrorSeverity::Critical,
            // Self::ByteStreamNotSet => ErrorSeverity::Error,
            // Self::EncodingNotSet => ErrorSeverity::Error
        }
    }
}

/// 错误严重程度枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// 警告：可恢复的错误，不影响主要功能
    Warning,
    /// 错误：影响功能但不致命
    Error,
    /// 严重：可能导致程序崩溃或数据丢失
    Critical,
}

impl ErrorSeverity {
    /// 获取严重程度的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
            Self::Critical => "CRITICAL",
        }
    }
}

/// 结果类型别名，简化错误处理
pub type Result<T> = std::result::Result<T, EditorError>;