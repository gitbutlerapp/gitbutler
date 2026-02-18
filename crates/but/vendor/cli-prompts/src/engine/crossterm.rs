use std::io::{Result, Write};

use crossterm::{
    cursor::{position, MoveTo, MoveToPreviousLine},
    event::{read, Event},
    execute, queue,
    style::{Attribute, Attributes, Color as Cc, Colors, Print, SetAttributes, SetColors},
    terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled, Clear, ClearType},
};

use crate::{
    input::Key,
    style::{Color, Formatting, FormattingOption}
};

use super::{CommandBuffer, Engine};

struct RawMode(bool);

/// Terminal handing backend implemented with the [crossterm](https://docs.rs/crossterm/latest/crossterm/) crate
pub struct CrosstermEngine<W: Write> {
    buffer: W,
    raw_mode: RawMode,
    previous_line_count: u16,
}

/// Command buffer for the `CrosstermEngine`
pub struct CrosstermCommandBuffer<W: Write> {
    commands: Vec<Box<dyn Command<W>>>,
    lines_count: u16,
}

impl<W: Write> CrosstermEngine<W> {
    pub fn new(buffer: W) -> Self {
        CrosstermEngine {
            buffer,
            raw_mode: RawMode::ensure(),
            previous_line_count: 1,
        }
    }
}

impl<W: Write> Engine for CrosstermEngine<W> {
    type Buffer = CrosstermCommandBuffer<W>;

    fn get_command_buffer(&self) -> Self::Buffer {
        CrosstermCommandBuffer::new()
    }

    fn render(&mut self, render_commands: &Self::Buffer) -> Result<()> {
        for _ in 0..self.previous_line_count - 1 {
            queue!(self.buffer, MoveToPreviousLine(1))?;
        }

        queue!(self.buffer, MoveTo(0, position()?.1))?;

        for cmd in &render_commands.commands {
            cmd.execute(&mut self.buffer)?;
        }

        queue!(self.buffer, Clear(ClearType::FromCursorDown))?;

        self.previous_line_count = render_commands.lines_count;
        self.buffer.flush()
    }

    fn finish_rendering(&mut self) -> Result<()> {
        execute!(self.buffer, Print("\r\n"))
    }

    fn read_key(&self) -> Result<Key> {
        loop {
            match read() {
                Ok(evt) => {
                    if let Event::Key(key) = evt {
                        return Ok(key.code.into());
                    } else {
                        continue;
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
}

impl<W: Write> CrosstermCommandBuffer<W> {
    fn new() -> Self {
        CrosstermCommandBuffer {
            commands: vec![],
            lines_count: 1,
        }
    }
}

impl<W: Write> CommandBuffer for CrosstermCommandBuffer<W> {
    fn new_line(&mut self) {
        self.commands.push(Box::new(NewLineCommand));
        self.lines_count += 1;
    }

    fn print(&mut self, text: &str) {
        self.commands.push(Box::new(PrintCommand(text.to_owned())));
    }

    fn set_formatting(&mut self, formatting: &Formatting) {
        self.commands
            .push(Box::new(SetFormattingCommand(formatting.to_owned())));
    }

    fn reset_formatting(&mut self) {
        self.commands
            .push(Box::new(SetFormattingCommand(Formatting::reset())));
    }
}

impl<W: Write> super::Clear for CrosstermCommandBuffer<W> {
    fn clear(&mut self) {
        self.commands.clear();
        self.lines_count = 1;
    }
}

impl RawMode {
    pub fn ensure() -> Self {
        let is_raw = is_raw_mode_enabled().unwrap_or(false);
        if !is_raw {
            enable_raw_mode().unwrap_or_default();
        }

        Self(is_raw)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        if !self.0 {
            disable_raw_mode().unwrap_or_default();
        }
    }
}

struct NewLineCommand;
struct PrintCommand(String);
struct SetFormattingCommand(Formatting);

trait Command<W: Write> {
    fn execute(&self, buffer: &mut W) -> Result<()>;
}

impl<W: Write> Command<W> for PrintCommand {
    fn execute(&self, buffer: &mut W) -> Result<()> {
        queue!(buffer, Print(&self.0), Clear(ClearType::UntilNewLine))
    }
}

impl<W: Write> Command<W> for NewLineCommand {
    fn execute(&self, buffer: &mut W) -> Result<()> {
        queue!(buffer, Print("\r\n"))
    }
}

impl<W: Write> Command<W> for SetFormattingCommand {
    fn execute(&self, buffer: &mut W) -> Result<()> {
        let colors = Colors {
            foreground: self.0.foreground_color.map(|c| c.into()),
            background: self.0.background_color.map(|c| c.into()),
        };

        let attributes_vec: Vec<Attribute> =
            self.0.text_formatting.iter().map(|&f| f.into()).collect();
        let attributes_ref: &[Attribute] = &attributes_vec;
        let attributes: Attributes = attributes_ref.into();

        queue!(buffer, SetColors(colors), SetAttributes(attributes))
    }
}

impl From<FormattingOption> for crossterm::style::Attribute {
    fn from(value: FormattingOption) -> Self {
        match value {
            FormattingOption::Reset => Attribute::Reset,
            FormattingOption::Bold => Attribute::Bold,
            FormattingOption::Italic => Attribute::Italic,
            FormattingOption::Underline => Attribute::Underlined,
            FormattingOption::CrossedOut => Attribute::CrossedOut,
        }
    }
}

impl From<crossterm::event::KeyCode> for Key {
    fn from(key_code: crossterm::event::KeyCode) -> Self {
        match key_code {
            crossterm::event::KeyCode::Backspace => Key::Backspace,
            crossterm::event::KeyCode::Enter => Key::Enter,
            crossterm::event::KeyCode::Left => Key::Left,
            crossterm::event::KeyCode::Right => Key::Right,
            crossterm::event::KeyCode::Up => Key::Up,
            crossterm::event::KeyCode::Down => Key::Down,
            crossterm::event::KeyCode::Home => Key::Home,
            crossterm::event::KeyCode::End => Key::End,
            crossterm::event::KeyCode::PageUp => Key::PageUp,
            crossterm::event::KeyCode::PageDown => Key::PageDown,
            crossterm::event::KeyCode::Tab => Key::Tab,
            crossterm::event::KeyCode::BackTab => Key::BackTab,
            crossterm::event::KeyCode::Delete => Key::Delete,
            crossterm::event::KeyCode::Insert => Key::Insert,
            crossterm::event::KeyCode::F(func) => Key::F(func),
            crossterm::event::KeyCode::Char(c) => Key::Char(c),
            crossterm::event::KeyCode::Null => Key::Esc,
            crossterm::event::KeyCode::Esc => Key::Esc,
        }
    }
}

impl From<Color> for Cc {
    fn from(value: Color) -> Self {
        match value {
            Color::Reset => Cc::Reset,
            Color::Black => Cc::Black,
            Color::DarkGrey => Cc::DarkGrey,
            Color::Red => Cc::Red,
            Color::DarkRed => Cc::DarkRed,
            Color::Green => Cc::Green,
            Color::DarkGreen => Cc::DarkGreen,
            Color::Yellow => Cc::Yellow,
            Color::DarkYellow => Cc::DarkYellow,
            Color::Blue => Cc::Blue,
            Color::DarkBlue => Cc::DarkBlue,
            Color::Magenta => Cc::Magenta,
            Color::DarkMagenta => Cc::DarkMagenta,
            Color::Cyan => Cc::Cyan,
            Color::DarkCyan => Cc::DarkCyan,
            Color::White => Cc::White,
            Color::Grey => Cc::Grey,
            Color::Rgb { r, g, b } => Cc::Rgb { r, g, b },
            Color::AnsiValue(c) => Cc::AnsiValue(c),
        }
    }
}
