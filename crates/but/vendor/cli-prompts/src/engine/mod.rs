//! This module helps to abstract away the rendering of the prompts to the terminal.
//!
//! It consists of the multiple traits:
//! - `Engine` trait, that represents the backend which draws content on the
//! screen and handles the input;
//! - `CommandBuffer` trait that represents the set of rendering commands to display
//! the given prompt.
//! - `Clear` trait that is complemetary to the `CommandBuffer` and allows to clear its contents
//!
//! Submodules are meant to implement the above traits using terminal manipulation libraries
mod crossterm;

pub use self::crossterm::CrosstermEngine;

use crate::{input::Key, style::Formatting};
use std::io::Result;

/// Represents the backend to draw prompts on the screen and handle input
pub trait Engine {

    /// Type of the corresponding command buffer
    type Buffer: CommandBuffer + Clear;

    /// Creates a new instanse of the `CommandBuffer` implementation
    fn get_command_buffer(&self) -> Self::Buffer;

    /// Renders content to the terminal using the specified rendering commands
    fn render(&mut self, render_commands: &Self::Buffer) -> Result<()>;

    /// This is called when a prompt is submitted and needs to be rendered in its final state.
    fn finish_rendering(&mut self) -> Result<()>;

    /// Reads a key that was pressed. This is a blocking call
    fn read_key(&self) -> Result<Key>;
}

/// Suplementary trait to the `CommandBuffer`
pub trait Clear {

    /// Clear the contents of the buffer
    fn clear(&mut self);
}

/// Represents the set of rendering commands
pub trait CommandBuffer {

    /// Move the cursor to the new line
    fn new_line(&mut self);

    /// Print the text to the screen at the current cursor position
    fn print(&mut self, text: &str);

    /// Set the given formatting to all text before the next `reset_formatting` call
    fn set_formatting(&mut self, formatting: &Formatting);

    /// Resets the previously set formatting to default
    fn reset_formatting(&mut self);
}
