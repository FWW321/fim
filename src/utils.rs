pub fn get_version_from_env() -> String {
    // Cargo 会在编译时自动设置一些环境变量
    // option_env! 是 Rust 中的一个编译时宏，用于在编译时获取环境变量的值，
    // 如果环境变量不存在也不会导致编译错误。
    option_env!("CARGO_PKG_VERSION")
        .unwrap_or("unknown")
        .to_string()
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