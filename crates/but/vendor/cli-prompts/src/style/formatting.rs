use crate::engine::CommandBuffer;

use super::color::Color;

/// Set of text formatting options
#[derive(Clone, Copy)]
pub enum FormattingOption {
    /// Reset the formatting
    Reset,

    /// Make the text bold
    Bold,

    /// Make the text italic
    Italic,

    /// Underline the text
    Underline,

    /// Cross the text out
    CrossedOut,
}

/// Represent the text formatting which includes
/// - Color of the text
/// - Color of the background
/// - Text formatting options
#[derive(Clone)]
pub struct Formatting {
    /// Text color
    pub foreground_color: Option<Color>,

    /// Background color
    pub background_color: Option<Color>,

    /// List of formatting options
    pub text_formatting: Vec<FormattingOption>,
}

impl Default for Formatting {
    fn default() -> Self {
        Formatting {
            foreground_color: None,
            background_color: None,
            text_formatting: vec![],
        }
    }
}

impl Formatting {

    /// Set the text color
    pub fn foreground_color(mut self, color: Color) -> Self {
        self.foreground_color = Some(color);
        self
    }

    /// Set the background color
    pub fn background_color(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    /// Make the text bold
    pub fn bold(mut self) -> Self {
        self.text_formatting.push(FormattingOption::Bold);
        self
    }

    /// Make the text italic
    pub fn italic(mut self) -> Self {
        self.text_formatting.push(FormattingOption::Italic);
        self
    }

    /// Underline the text
    pub fn underline(mut self) -> Self {
        self.text_formatting.push(FormattingOption::Underline);
        self
    }

    /// Cross the text out
    pub fn crossed_out(mut self) -> Self {
        self.text_formatting.push(FormattingOption::CrossedOut);
        self
    }

    /// Reset text formatting (colors and options)
    pub fn reset() -> Self {
        let mut f = Self::default();
        f.text_formatting.push(FormattingOption::Reset);
        f
    }

    /// Print the given text using the current formatting to the provided command buffer
    pub fn print(&self, text: impl Into<String>, cmd_buffer: &mut impl CommandBuffer) {
        cmd_buffer.set_formatting(self);
        cmd_buffer.print(&text.into());
        cmd_buffer.reset_formatting();
    }
}
