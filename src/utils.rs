pub mod color {
    // 基础 8 色
pub const BLACK: &str = "\x1b[30m";      // 黑色
pub const RED: &str = "\x1b[31m";        // 红色
pub const GREEN: &str = "\x1b[32m";      // 绿色
pub const YELLOW: &str = "\x1b[33m";     // 黄色
pub const BLUE: &str = "\x1b[34m";       // 蓝色
pub const MAGENTA: &str = "\x1b[35m";    // 品红
pub const CYAN: &str = "\x1b[36m";       // 青色
pub const WHITE: &str = "\x1b[37m";      // 白色

// 256 色扩展 (常用)
pub const ORANGE: &str = "\x1b[38;5;208m";     // 橙色
pub const PINK: &str = "\x1b[38;5;205m";      // 粉色
pub const PURPLE: &str = "\x1b[38;5;141m";    // 紫色
pub const GRAY: &str = "\x1b[38;5;245m";       // 灰色
pub const DARK_GREEN: &str = "\x1b[38;5;22m";  // 深绿

// 亮色变体 (Bright)​
pub const BRIGHT_BLACK: &str = "\x1b[90m";   // 亮黑（灰）
pub const BRIGHT_RED: &str = "\x1b[91m";     // 亮红
pub const BRIGHT_GREEN: &str = "\x1b[92m";   // 亮绿
pub const BRIGHT_YELLOW: &str = "\x1b[93m";  // 亮黄
pub const BRIGHT_BLUE: &str = "\x1b[94m";    // 亮蓝
pub const BRIGHT_MAGENTA: &str = "\x1b[95m"; // 亮品红
pub const BRIGHT_CYAN: &str = "\x1b[96m";    // 亮青
pub const BRIGHT_WHITE: &str = "\x1b[97m";   // 亮白

// 背景色
pub const BG_BLACK: &str = "\x1b[40m";      // 黑色背景
pub const BG_RED: &str = "\x1b[41m";        // 红色背景
pub const BG_GREEN: &str = "\x1b[42m";      // 绿色背景
pub const BG_YELLOW: &str = "\x1b[43m";     // 黄色背景
pub const BG_BLUE: &str = "\x1b[44m";       // 蓝色背景
pub const BG_MAGENTA: &str = "\x1b[45m";    // 品红背景
pub const BG_CYAN: &str = "\x1b[46m";       // 青色背景
pub const BG_WHITE: &str = "\x1b[47m";      // 白色背景

// 样式控制
pub const RESET: &str = "\x1b[0m";       // 重置所有文本样式（颜色/加粗/下划线等）
pub const BOLD: &str = "\x1b[1m";       // 加粗
pub const DIM: &str = "\x1b[2m";        // 暗淡
pub const ITALIC: &str = "\x1b[3m";     // 斜体 (部分终端不支持)
pub const UNDERLINE: &str = "\x1b[4m";  // 下划线
pub const BLINK: &str = "\x1b[5m";      // 闪烁 (慎用)
pub const REVERSE: &str = "\x1b[7m";    // 反色 (前景/背景互换)
pub const HIDDEN: &str = "\x1b[8m";     // 隐藏文字 (如密码输入)

// RGB 真彩色 (现代终端支持)​
pub fn rgb_fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

pub fn rgb_bg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{};{};{}m", r, g, b)
}
}

pub fn get_version_from_env() -> String {
    // Cargo 会在编译时自动设置一些环境变量
    // option_env! 是 Rust 中的一个编译时宏，用于在编译时获取环境变量的值，
    // 如果环境变量不存在也不会导致编译错误。
    option_env!("CARGO_PKG_VERSION")
        .unwrap_or("unknown")
        .to_string()
}

pub fn find_subsequence<T: PartialEq>(haystack: &[T], needle: &[T]) -> Option<usize> {
    let needle_len = needle.len();
    if needle_len == 0 {
        return Some(0); // 空子序列默认匹配索引 0
    }
    haystack
        .windows(needle_len)
        .enumerate()
        .find(|(_, window)| *window == needle)
        .map(|(i, _)| i)
}

pub fn find_all_subsequences<T: PartialEq>(haystack: &[T], needle: &[T]) -> Vec<usize> {
    if needle.is_empty() {
        return vec![0]; // 空子序列默认匹配起始位置 0
    }
    if haystack.len() < needle.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .map(|(i, _)| i)
        .collect()
}

// Ctrl 键会剥离第5位和第6位
// 按照惯例，位编号从0开始，从低位到高位为0-7
// 所有标准 ASCII 字符的第7位都是0，范围为 0-127 (0x00-0x7F)
// Ctrl+A 和 Ctrl+a 产生相同的控制字符
// 小写转大写：清除第五位 & 0b11011111
// 大写转小写： 设置第五位 | 0b00100000
// pub fn ctrl_key(c: char) -> Option<char> {
//     let result = c as u32 & 0b00011111;

//     char::from_u32(result)
// }