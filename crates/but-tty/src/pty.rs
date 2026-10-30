//! The user's shell on a pseudo-terminal, with what it writes read on its own thread.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::Pty;
#[cfg(unix)]
pub use unix::{Kept, Pty};

/// A grid size, with the pixel size of one cell for programs that ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub cell_width: u16,
    pub cell_height: u16,
}
