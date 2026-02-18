#[derive(Copy, Clone)]
pub enum Color {
    /// Reset the color to default value
    Reset,

    Black,
    DarkGrey,

    Red,
    DarkRed,

    Green,
    DarkGreen,

    Yellow,
    DarkYellow,
    Blue,
    DarkBlue,

    Magenta,
    DarkMagenta,

    Cyan,
    DarkCyan,

    White,
    Grey,

    /// True-color with given RGB values
    Rgb { r: u8, g: u8, b: u8 },

    AnsiValue(u8),
}

