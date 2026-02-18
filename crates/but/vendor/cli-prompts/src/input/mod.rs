//! Module for handling input

/// Represents different keyboard keys
pub enum Key {
    /// Backspace key
    Backspace,

    /// Enter (Return) key
    Enter,

    /// Left arrow key
    Left,

    /// Right arrow key
    Right,

    /// Up arrow key
    Up,

    /// Down arrow key
    Down,

    /// Home key
    Home,
    
    /// End key
    End,

    /// Page Up key
    PageUp,

    /// Page Down key
    PageDown,

    /// Tab key
    Tab,

    /// Shift+Tab key
    BackTab,

    /// Delete key
    Delete,

    /// Insert key
    Insert,

    /// Function keys from F1 to F12
    F(u8),

    /// All the character keys
    Char(char),

    /// Esc key
    Esc,
}
