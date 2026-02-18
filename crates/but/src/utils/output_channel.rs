use std::io::{IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use minus::ExitStrategy;

use crate::{args::OutputFormat, utils::json_pretty_to_stdout};

/// Default value for a confirmation prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmDefault {
    Yes,
    No,
}

/// Response from a confirmation prompt with a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    Yes,
    No,
}

/// Response from a confirmation prompt without a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOrEmpty {
    Yes,
    No,
    NoInput,
}

/// A utility `std::io::Write` implementation that can always be used to generate output for humans or for scripts.
pub struct OutputChannel {
    /// How to print the output, one should match on it. Match on this if you prefer this style.
    format: OutputFormat,
    /// The output to use if there is no pager.
    stdout: std::io::Stdout,
    /// Possibly a pager we are using. If `Some`, the pager itself is used for output instead of `stdout`.
    pager: Option<minus::Pager>,
    /// When `Some`, JSON values written via `write_value` are captured here instead of going to stdout.
    /// Used by `--status-after` to buffer mutation JSON before combining with status JSON.
    json_buffer: Option<serde_json::Value>,
}

/// A channel that implements [`std::io::Write`], to make unbuffered writes to [`std::io::stderr`]
/// if the error channel is connected to a terminal and the output format is for humans,
/// for providing progress or error information.
/// Broken pipes will also be ignored, thus the output written to this channel should be considered optional.
pub struct ProgressChannel {
    /// The channel writes will go to, if we are connected to a terminal and output is for humans.
    inner: Option<std::io::Stderr>,
}

impl ProgressChannel {
    /// Create a new progress channel that writes to stderr if it's a terminal and `for_humans` is true.
    /// If `for_humans` is false, the channel becomes a no-op.
    pub fn new(for_humans: bool) -> Self {
        ProgressChannel {
            inner: if for_humans {
                let stderr = std::io::stderr();
                stderr.is_terminal().then_some(stderr)
            } else {
                None
            },
        }
    }
}

impl std::io::Write for ProgressChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(stderr) = self.inner.as_mut() {
            stderr
                .write(buf)
                .or_else(|err| ignore_broken_pipe(err).map(|()| buf.len()))
        } else {
            // Pretend we wrote everything
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(stderr) = self.inner.as_mut() {
            stderr.flush().or_else(ignore_broken_pipe)
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Write for ProgressChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        self.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

fn ignore_broken_pipe(err: std::io::Error) -> std::io::Result<()> {
    if err.kind() == std::io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(err)
    }
}

/// Access
impl OutputChannel {
    /// Get the output format setting.
    pub fn format(&self) -> OutputFormat {
        self.format
    }
}

/// Output
impl OutputChannel {
    /// Provide a write implementation for humans, if the format setting permits.
    pub fn for_human(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Human).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a write implementation for Shell output, if the format setting permits.
    pub fn for_shell(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Shell).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a write implementation for text output (human or shell), if the format setting permits.
    pub fn for_human_or_shell(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Human | OutputFormat::Shell).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a handle to receive a serde-serializable value to write to stdout.
    pub fn for_json(&mut self) -> Option<&mut Self> {
        matches!(self.format, OutputFormat::Json).then_some(self)
    }

    /// A convenience function to create a progress channel that only writes when the output is for humans.
    /// The progress channel writes to stderr if it's a terminal and the output format is [`OutputFormat::Human`].
    pub fn progress_channel(&self) -> ProgressChannel {
        ProgressChannel::new(matches!(self.format, OutputFormat::Human))
    }
}

/// User input
impl OutputChannel {
    /// Return `true` if external prompt support like [`Selection`](cli_prompts::prompts::Selection) can be used,
    /// *and* the output is meant *for humans*.
    ///
    /// Note that this is implied to be true if [Self::prepare_for_terminal_input()] returns `Some()`.
    pub fn can_prompt(&self) -> bool {
        matches!(self.format, OutputFormat::Human) && std::io::stdin().is_terminal() && self.stdout.is_terminal()
    }

    /// Before performing further output, obtain an input channel which always bypasses the pager when writing,
    /// while allowing prompting the user for input.
    /// If `None` is returned, terminal input isn't available or the output isn't meant for humans,
    /// and the caller should suggest to use command-line arguments to unambiguously specify an operation.
    pub fn prepare_for_terminal_input(&mut self) -> Option<InputOutputChannel<'_>> {
        use std::io::IsTerminal;
        let stdin = std::io::stdin();
        if !stdin.is_terminal() || !self.stdout.is_terminal() {
            return None;
        }
        if self.for_human().is_none() {
            tracing::warn!("Stdin is a terminal, and output wasn't configured for human consumption");
            return None;
        }
        Some(InputOutputChannel { out: self })
    }
}

/// JSON utilities
impl OutputChannel {
    /// Write `value` as pretty JSON to the output.
    ///
    /// When JSON buffering is active (via [`Self::start_json_buffering`]), the value is captured
    /// in the buffer instead of being written to stdout. Only one value should be written per
    /// buffering session; if called again while the buffer already holds data, a warning is
    /// emitted to stderr and the previous value is replaced.
    ///
    /// Note that it's owned to avoid double-printing with [ResultJsonExt::output_json]
    pub fn write_value(&mut self, value: impl serde::Serialize) -> std::io::Result<()> {
        if self.json_buffer.is_some() {
            let new_value = serde_json::to_value(&value).map_err(std::io::Error::other)?;
            if !matches!(self.json_buffer, Some(serde_json::Value::Null)) {
                eprintln!(
                    "warning: write_value called while buffer already contains data; previous value will be lost"
                );
            }
            self.json_buffer = Some(new_value);
            Ok(())
        } else {
            json_pretty_to_stdout(&value)
        }
    }

    /// Start buffering JSON output instead of writing to stdout.
    pub fn start_json_buffering(&mut self) {
        self.json_buffer = Some(serde_json::Value::Null);
    }

    /// Conditionally start JSON buffering for `--status-after` mode.
    ///
    /// If `status_after` is `true` and the output is in JSON mode,
    /// begins buffering so mutation output can be captured and later
    /// combined with workspace status.
    pub fn begin_status_after(&mut self, status_after: bool) {
        if status_after && matches!(self.format, OutputFormat::Json) {
            self.start_json_buffering();
        }
    }

    /// Take the buffered JSON value, stopping buffering.
    pub fn take_json_buffer(&mut self) -> Option<serde_json::Value> {
        self.json_buffer.take()
    }

    /// Returns `true` if output is in JSON mode.
    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

impl std::fmt::Write for OutputChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        match self.format {
            OutputFormat::Human | OutputFormat::Shell => {
                if let Some(out) = self.pager.as_mut() {
                    out.write_str(s)
                } else {
                    self.stdout.write_all(s.as_bytes()).or_else(|err| {
                        if err.kind() == std::io::ErrorKind::BrokenPipe {
                            // Ignore broken pipes and keep writing.
                            // This allows the caller to use `?` without having to think
                            // about ignoring errors selectively.
                            Ok(())
                        } else {
                            Err(std::fmt::Error)
                        }
                    })
                }
            }
            OutputFormat::Json | OutputFormat::None => {
                // It's not an error to try to write in JSON mode, it's a feature.
                // However, the only way to write JSON is to use [Self::write_value()].
                Ok(())
            }
        }
    }
}

/// A channel to obtain various kinds of user input from a terminal, bypassing the pager.
pub struct InputOutputChannel<'out> {
    out: &'out mut OutputChannel,
}

impl std::fmt::Write for InputOutputChannel<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        // bypass the pager, fail on broken pipes (we are prompting)
        self.out.stdout.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

impl InputOutputChannel<'_> {
    fn readline(&mut self, prompt: &str) -> anyhow::Result<ReadlineInput> {
        self.out.stdout.write_all(prompt.as_bytes())?;
        self.out.stdout.flush()?;

        let _raw_mode = RawModeGuard::new()?;
        let mut line = String::new();

        loop {
            match event::read()? {
                Event::Key(key) => match key_to_edit_action(key, line.is_empty()) {
                    KeyEditAction::Insert(ch) => {
                        line.push(ch);
                        write!(self.out.stdout, "{ch}")?;
                        self.out.stdout.flush()?;
                    }
                    KeyEditAction::Backspace => {
                        if line.pop().is_some() {
                            // Move back, erase one char, then move back again.
                            self.out.stdout.write_all(b"\x08 \x08")?;
                            self.out.stdout.flush()?;
                        }
                    }
                    KeyEditAction::Submit => {
                        self.out.stdout.write_all(b"\n")?;
                        self.out.stdout.flush()?;
                        let trimmed = line.trim().to_owned();
                        return if trimmed.is_empty() {
                            Ok(ReadlineInput::Empty)
                        } else {
                            Ok(ReadlineInput::Text(trimmed))
                        };
                    }
                    KeyEditAction::EndOfInput => {
                        self.out.stdout.write_all(b"\n")?;
                        self.out.stdout.flush()?;
                        return Ok(ReadlineInput::EndOfInput);
                    }
                    KeyEditAction::Ignore => {}
                },
                Event::Paste(text) => {
                    if !text.is_empty() {
                        line.push_str(&text);
                        self.out.stdout.write_all(text.as_bytes())?;
                        self.out.stdout.flush()?;
                    }
                }
                _ => {}
            }
        }
    }

    /// Prompt a non-empty string from the user, or `None` if the input was
    /// empty.
    ///
    /// If you are looking to make a yes or no style confirmation prompt,
    /// consider using [`Self::confirm`] or [`Self::confirm_no_default`].
    ///
    /// ```ignore
    /// let result = inout.prompt("Do you like cheese?")?;
    /// // Outputs:
    /// // Do you like cheese?
    /// // >
    /// ```
    pub fn prompt(&mut self, prompt: impl AsRef<str>) -> anyhow::Result<Option<String>> {
        Ok(match self.readline(&format!("{}\n> ", prompt.as_ref()))? {
            ReadlineInput::Text(line) => Some(line),
            ReadlineInput::Empty | ReadlineInput::EndOfInput => None,
        })
    }

    /// Prompt for y/n confirmation with a default value. Automatically appends
    /// `[Y/n]` or `[y/N]` based on the default. Empty input returns the
    /// default. Input starting with 'y'/'Y' returns Yes, anything else returns
    /// No.
    ///
    /// ```ignore
    /// let result = inout.confirm("Are you sure you want to do this?", ConfirmDefault::Yes)?;
    /// // Outputs:
    /// // Are you sure you want to do this? [Y/n]:
    /// ```
    pub fn confirm(&mut self, prompt: impl AsRef<str>, default: ConfirmDefault) -> anyhow::Result<Confirm> {
        let suffix = match default {
            ConfirmDefault::Yes => "[Y/n]",
            ConfirmDefault::No => "[y/N]",
        };
        match self.readline(&format!("{} {}: ", prompt.as_ref(), suffix))? {
            ReadlineInput::Text(input) => {
                if input.to_lowercase().starts_with('y') {
                    Ok(Confirm::Yes)
                } else {
                    Ok(Confirm::No)
                }
            }
            ReadlineInput::Empty => Ok(match default {
                ConfirmDefault::Yes => Confirm::Yes,
                ConfirmDefault::No => Confirm::No,
            }),
            // Ctrl-D/Ctrl-C should not auto-accept a default action.
            ReadlineInput::EndOfInput => Ok(Confirm::No),
        }
    }

    /// Prompt for y/n confirmation without a default.
    /// Automatically appends `[y/n]` to the prompt.
    /// Returns `NoInput` if the user provides empty input.
    ///
    /// ```ignore
    /// let result = inout.confirm_no_default("Are you sure you want to do this?")?;
    /// // Outputs:
    /// // Are you sure you want to do this? [y/n]:
    /// ```
    pub fn confirm_no_default(&mut self, prompt: impl AsRef<str>) -> anyhow::Result<ConfirmOrEmpty> {
        match self.readline(&format!("{} [y/n]: ", prompt.as_ref()))? {
            ReadlineInput::Text(input) => {
                if input.to_lowercase().starts_with('y') {
                    Ok(ConfirmOrEmpty::Yes)
                } else {
                    Ok(ConfirmOrEmpty::No)
                }
            }
            ReadlineInput::Empty | ReadlineInput::EndOfInput => Ok(ConfirmOrEmpty::NoInput),
        }
    }
}

/// Normalized result of collecting one line of terminal input.
enum ReadlineInput {
    /// User entered non-empty text and submitted it.
    Text(String),
    /// User submitted an empty line (pressed Enter without text).
    Empty,
    /// Input ended without a submission (for example Ctrl-C, Ctrl-D, or Esc).
    EndOfInput,
}

/// Editing operation derived from a terminal key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyEditAction {
    /// Insert the provided character into the current input buffer.
    Insert(char),
    /// Delete one character from the input buffer.
    Backspace,
    /// Submit the current input buffer as a completed line.
    Submit,
    /// End input without submitting a line.
    EndOfInput,
    /// Ignore this key event because it does not affect editing.
    Ignore,
}

fn key_to_edit_action(key: KeyEvent, line_is_empty: bool) -> KeyEditAction {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return KeyEditAction::Ignore;
    }
    match key.code {
        KeyCode::Enter => KeyEditAction::Submit,
        KeyCode::Backspace => KeyEditAction::Backspace,
        KeyCode::Tab => KeyEditAction::Insert('\t'),
        KeyCode::Esc => KeyEditAction::EndOfInput,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyEditAction::EndOfInput,
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) && line_is_empty => {
            KeyEditAction::EndOfInput
        }
        KeyCode::Char(ch) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            KeyEditAction::Insert(ch)
        }
        _ => KeyEditAction::Ignore,
    }
}

/// RAII guard that enables terminal raw mode on creation and restores normal mode on drop.
struct RawModeGuard;

impl RawModeGuard {
    fn new() -> std::io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().ok();
    }
}

/// Be sure to flush everything written after the prompt as well - the output channel may be buffered.
impl Drop for InputOutputChannel<'_> {
    fn drop(&mut self) {
        self.out.stdout.flush().ok();
    }
}

/// Lifecycle
impl OutputChannel {
    /// Create a new instance to output with `format` (advisory), which affects where it prints to.
    /// The `use_pager` parameter controls whether a pager should be created.
    ///
    /// It's configured to print to stdout unless [`OutputFormat::Json`] is used, then it prints everything
    /// to a `/dev/null` equivalent, so callers never have to worry if they interleave JSON with other output.
    pub fn new_with_optional_pager(format: OutputFormat, use_pager: bool) -> Self {
        OutputChannel {
            format,
            stdout: std::io::stdout(),
            pager: if !matches!(format, OutputFormat::Human) || std::env::var_os("NOPAGER").is_some() || !use_pager {
                None
            } else {
                let pager = minus::Pager::new();
                let msg = "can talk to newly created pager";
                pager.set_exit_strategy(ExitStrategy::PagerQuit).expect(msg);
                pager.set_prompt("GitButler").expect(msg);
                Some(pager)
            },
            json_buffer: None,
        }
    }

    /// Like [`Self::new_with_optional_pager`], but will never create a pager or write JSON.
    /// Use this if a second instance of a channel is needed, and the first one could have a pager.
    pub fn new_without_pager_non_json(format: OutputFormat) -> Self {
        OutputChannel {
            format: match format {
                OutputFormat::Human | OutputFormat::Shell | OutputFormat::None => format,
                OutputFormat::Json => OutputFormat::None,
            },
            stdout: std::io::stdout(),
            pager: None,
            json_buffer: None,
        }
    }
}

impl Drop for OutputChannel {
    fn drop(&mut self) {
        // Flush any buffered JSON that was never consumed — this means
        // the status-after path did not complete, but we should still
        // emit the mutation result rather than silently discarding it.
        if let Some(buffer) = self.json_buffer.take()
            && buffer != serde_json::Value::Null
            && let Err(err) = json_pretty_to_stdout(&buffer)
        {
            eprintln!("warning: failed to flush buffered JSON on drop: {err}");
        }
        if let Some(pager) = self.pager.take() {
            minus::page_all(pager).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{KeyEditAction, key_to_edit_action};

    #[test]
    fn ctrl_c_ends_input() {
        assert_eq!(
            key_to_edit_action(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), true),
            KeyEditAction::EndOfInput
        );
    }

    #[test]
    fn ctrl_d_only_ends_input_when_line_is_empty() {
        assert_eq!(
            key_to_edit_action(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), true),
            KeyEditAction::EndOfInput
        );
        assert_eq!(
            key_to_edit_action(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL), false),
            KeyEditAction::Ignore
        );
    }
}
