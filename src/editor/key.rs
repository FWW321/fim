#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    // 普通字符
    Char(char),
    // 方向键
    ArrowKey(Direction),
    // 功能键
    FunctionKey(u8),
    // 控制键
    ControlKey(ControlKey),
    /// 其他特殊键
    SpecialKey(SpecialKey),
    // 鼠标事件
    // MouseEvent(MouseEvent),
    // 未知或无法解析的输入
    // Unknown(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlKey {
    Ctrl(char), // Ctrl+字母/数字
    Alt(char),  // Alt+字符
    Tab,
    LF,
    CR,
    // Enter,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialKey {
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    PauseBreak,
    Menu,
}

// pub enum MouseEvent {
//     Click(u8, u16, u16),    // 按钮, x, y
//     Scroll(i8, u16, u16),   // 滚动方向, x, y
//     Move(u16, u16),         // x, y
// }
