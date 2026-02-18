use crate::engine::CommandBuffer;

use super::{Color, Formatting};

/// Style for the common part of all prompts: the prompt itself.
#[derive(Clone)]
pub struct LabelStyle {
    prefix: String,
    prefix_formatting: Formatting,
    prompt_formatting: Formatting,
}

impl LabelStyle {

    /// Sets the string that is displayed before the user's input
    pub fn prefix<S: Into<String>>(mut self, p: S) -> Self {
        self.prefix = p.into();
        self
    }

    /// Sets formatting for the prefix string
    pub fn prefix_formatting(mut self, f: Formatting) -> Self {
        self.prefix_formatting = f;
        self
    }

    /// Sets formatting for the user input string
    pub fn prompt_formatting(mut self, f: Formatting) -> Self {
        self.prompt_formatting = f;
        self
    }

    /// Prints the formatted prefix and the input text to the provided command buffer
    pub fn print(&self, text: impl Into<String>, cmd_buffer: &mut impl CommandBuffer) {
        cmd_buffer.set_formatting(&self.prefix_formatting);
        cmd_buffer.print(&self.prefix);
        cmd_buffer.reset_formatting();
        cmd_buffer.print(" ");
        cmd_buffer.set_formatting(&self.prompt_formatting);
        cmd_buffer.print(&text.into());
        cmd_buffer.print(":");
        cmd_buffer.reset_formatting();
        cmd_buffer.print(" ");
    }
}

impl Default for LabelStyle {
    fn default() -> Self {
        LabelStyle {
            prefix: "?".into(),
            prefix_formatting: Formatting::default().bold().foreground_color(Color::Green),
            prompt_formatting: Formatting::default().bold(),
        }
    }
}
