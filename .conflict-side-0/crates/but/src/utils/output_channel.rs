use std::io::{IsTerminal, Write};

use but_secret::Sensitive;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use nonempty::NonEmpty;

use crate::{
    args::OutputFormat,
    tui::{self, PickerOptions},
    utils::{
        json_pretty_to_stdout,
        pager::{self, Pager},
    },
};

pub mod experimental;

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
    pager: Option<Pager>,
    /// When `Some`, JSON values written via `write_value` are captured here instead of going to stdout.
    /// Used to buffer mutation JSON before combining with status JSON.
    json_buffer: Option<serde_json::Value>,
}

/// A channel that implements [`std::io::Write`], to make unbuffered writes to [`std::io::stderr`]
/// if the error channel is connected to a terminal and the output format permits progress,
/// for providing progress or error information.
/// Broken pipes will also be ignored, thus the output written to this channel should be considered optional.
pub struct ProgressChannel {
    /// The channel writes will go to, if we are connected to a terminal and progress is enabled.
    inner: Option<std::io::Stderr>,
}

impl ProgressChannel {
    /// Create a new progress channel that writes to stderr if it's a terminal and progress is enabled.
    /// If progress is disabled, the channel becomes a no-op.
    pub fn new(progress_enabled: bool) -> Self {
        ProgressChannel {
            inner: if progress_enabled {
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

fn ignore_broken_pipe_for_fmt(err: std::io::Error) -> std::fmt::Result {
    ignore_broken_pipe(err).map_err(|_| std::fmt::Error)
}

/// Access
impl OutputChannel {
    /// Get the output format setting.
    pub fn format(&self) -> OutputFormat {
        self.format
    }
}

/// An [`std::fmt::Write`] implementation that supports additional utility methods for output formatting.
pub trait WriteWithUtils: std::fmt::Write {
    /// Truncate the given text to the specified maximum width, unless the output is passed through a pager
    /// or the output format opts out of truncation.
    ///
    /// Note that while copy-on-write is used internally, the returned value is always owned as it's typically
    /// used with coloring, and that always copies the string.
    fn truncate_if_unpaged(&self, text: &str, max_width: usize) -> String;

    /// Return `true` if the output is being passed through a pager.
    ///
    /// This is typically used to avoid truncating text when output is being piped to a pager.
    fn is_paged(&self) -> bool;
}

impl WriteWithUtils for OutputChannel {
    fn truncate_if_unpaged(&self, text: &str, max_width: usize) -> String {
        if self.pager.is_some() || !self.format.allows_truncation() {
            text.to_owned()
        } else {
            tui::text::truncate_text(text, max_width).into_owned()
        }
    }

    fn is_paged(&self) -> bool {
        self.pager.is_some()
    }
}

/// Output
impl OutputChannel {
    /// Provide a write implementation for human-readable text, if the format setting permits.
    ///
    /// This is `Some` for both human and agent output. Route terminal-only ambient messages
    /// through [`Self::for_human_ui`] instead, so agents don't receive them.
    pub fn for_human(&mut self) -> Option<&mut dyn WriteWithUtils> {
        self.format
            .is_human_text()
            .then_some(self as &mut dyn WriteWithUtils)
    }
    /// Provide a write implementation for ambient human UI messages, if the format setting permits them.
    ///
    /// Unlike [`Self::for_human`], this excludes agent output, which receives results as
    /// human-readable text but without ambient UI messages.
    pub fn for_human_ui(&mut self) -> Option<&mut dyn WriteWithUtils> {
        self.format
            .allows_human_ui()
            .then_some(self as &mut dyn WriteWithUtils)
    }
    /// Provide a write implementation for Shell output, if the format setting permits.
    pub fn for_shell(&mut self) -> Option<&mut dyn WriteWithUtils> {
        matches!(self.format, OutputFormat::Shell).then_some(self as &mut dyn WriteWithUtils)
    }
    /// Provide a write implementation for text output (human or shell), if the format setting permits.
    pub fn for_human_or_shell(&mut self) -> Option<&mut dyn WriteWithUtils> {
        self.format
            .is_text()
            .then_some(self as &mut dyn WriteWithUtils)
    }
    /// Provide a handle to receive a serde-serializable value to write to stdout.
    pub fn for_json(&mut self) -> Option<&mut Self> {
        self.format.is_json().then_some(self)
    }

    /// A convenience function to create a progress channel that only writes when the output format permits progress.
    /// The progress channel writes to stderr if it's a terminal and the output format permits progress.
    pub fn progress_channel(&self) -> ProgressChannel {
        ProgressChannel::new(self.format.allows_human_ui())
    }
}

/// User input
impl OutputChannel {
    /// Return `true` if external prompt support like [`InputOutputChannel::prompt_select`] can be used,
    /// and the output format permits prompts.
    ///
    /// Note that this is implied to be true if [Self::prepare_for_terminal_input()] returns `Some()`.
    pub fn can_prompt(&self) -> bool {
        self.format.allows_human_ui() && std::io::stdin().is_terminal() && self.stdout.is_terminal()
    }

    /// Before performing further output, obtain an input channel which always bypasses the pager when writing,
    /// while allowing prompting the user for input.
    /// If `None` is returned, terminal input isn't available or the output format does not permit prompts,
    /// and the caller should suggest to use command-line arguments to unambiguously specify an operation.
    pub fn prepare_for_terminal_input(&mut self) -> Option<InputOutputChannel<'_>> {
        use std::io::IsTerminal;
        let stdin = std::io::stdin();
        if !stdin.is_terminal() || !self.stdout.is_terminal() {
            return None;
        }
        if !self.format.allows_human_ui() {
            tracing::warn!(
                "Stdin is a terminal, and output wasn't configured for interactive prompts"
            );
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

    /// Conditionally start JSON buffering for mutation status output.
    ///
    /// If `status_after` is `true` and the output is in JSON mode,
    /// begins buffering so mutation output can be captured and later
    /// combined with workspace status.
    pub fn begin_status_after(&mut self, status_after: bool) {
        if status_after && self.format.is_json() {
            self.start_json_buffering();
        }
    }

    /// Take the buffered JSON value, stopping buffering.
    pub fn take_json_buffer(&mut self) -> Option<serde_json::Value> {
        self.json_buffer.take()
    }

    /// Returns `true` if output is in JSON mode.
    pub fn is_json(&self) -> bool {
        self.format.is_json()
    }
}

impl std::fmt::Write for OutputChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        if !self.format.is_text() {
            return Ok(());
        }
        match self.pager.as_mut() {
            Some(Pager::Builtin(pager)) => pager.write_str(s),
            Some(Pager::External(_, stdin)) => stdin
                .write_all(s.as_bytes())
                .or_else(ignore_broken_pipe_for_fmt),
            None => self
                .stdout
                .write_all(s.as_bytes())
                .or_else(ignore_broken_pipe_for_fmt),
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
        self.out
            .stdout
            .write_all(s.as_bytes())
            .map_err(|_| std::fmt::Error)
    }
}

impl WriteWithUtils for InputOutputChannel<'_> {
    fn truncate_if_unpaged(&self, text: &str, max_width: usize) -> String {
        self.out.truncate_if_unpaged(text, max_width)
    }

    fn is_paged(&self) -> bool {
        self.out.is_paged()
    }
}

impl InputOutputChannel<'_> {
    fn readline(&mut self, prompt: &str, echo: InputEcho) -> anyhow::Result<ReadlineInput> {
        const PLACEHOLDER_FOR_SECRET: &str = "•";
        self.out.stdout.write_all(prompt.as_bytes())?;
        self.out.stdout.flush()?;

        let _raw_mode = RawModeGuard::new()?;
        let mut line = String::new();

        loop {
            match event::read()? {
                Event::Key(key) => match key_to_edit_action(key, line.is_empty()) {
                    KeyEditAction::Insert(ch) => {
                        line.push(ch);
                        match echo {
                            InputEcho::Visible => {
                                write!(self.out.stdout, "{ch}")?;
                                self.out.stdout.flush()?;
                            }
                            InputEcho::Hidden => {
                                self.out
                                    .stdout
                                    .write_all(PLACEHOLDER_FOR_SECRET.as_bytes())?;
                                self.out.stdout.flush()?;
                            }
                        }
                    }
                    KeyEditAction::Backspace => {
                        if line.pop().is_some() {
                            // Move back, erase one char, then move back again.
                            self.out.stdout.write_all(b"\x08 \x08")?;
                            self.out.stdout.flush()?;
                        }
                    }
                    KeyEditAction::Submit => {
                        // In raw mode, '\n' may not return to column 0, so always emit CRLF.
                        self.out.stdout.write_all(b"\r\n")?;
                        self.out.stdout.flush()?;
                        let trimmed = line.trim().to_owned();
                        return if trimmed.is_empty() {
                            Ok(ReadlineInput::Empty)
                        } else {
                            Ok(ReadlineInput::Text(trimmed))
                        };
                    }
                    KeyEditAction::EndOfInput => {
                        // Keep follow-up output aligned even after prompt cancellation.
                        self.out.stdout.write_all(b"\r\n")?;
                        self.out.stdout.flush()?;
                        return Ok(ReadlineInput::EndOfInput);
                    }
                    KeyEditAction::Ignore => {}
                },
                Event::Paste(text) if !text.is_empty() => {
                    line.push_str(&text);
                    match echo {
                        InputEcho::Visible => {
                            self.out.stdout.write_all(text.as_bytes())?;
                            self.out.stdout.flush()?;
                        }
                        InputEcho::Hidden => {
                            let placeholders = PLACEHOLDER_FOR_SECRET.repeat(text.chars().count());
                            self.out.stdout.write_all(placeholders.as_bytes())?;
                            self.out.stdout.flush()?;
                        }
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
        Ok(
            match self.readline(&format!("{}\n> ", prompt.as_ref()), InputEcho::Visible)? {
                ReadlineInput::Text(line) => Some(line),
                ReadlineInput::Empty | ReadlineInput::EndOfInput => None,
            },
        )
    }

    /// Like [`Self::prompt`] but without a newline between the prompt and the input.
    pub fn prompt_single_line(
        &mut self,
        prompt: impl AsRef<str>,
    ) -> anyhow::Result<Option<String>> {
        Ok(match self.prompt_single_line_input(prompt)? {
            PromptLine::Text(line) => Some(line),
            PromptLine::Empty | PromptLine::Cancelled => None,
        })
    }

    pub(crate) fn prompt_single_line_input(
        &mut self,
        prompt: impl AsRef<str>,
    ) -> anyhow::Result<PromptLine> {
        Ok(
            match self.readline(&format!("{} ", prompt.as_ref()), InputEcho::Visible)? {
                ReadlineInput::Text(line) => PromptLine::Text(line),
                ReadlineInput::Empty => PromptLine::Empty,
                ReadlineInput::EndOfInput => PromptLine::Cancelled,
            },
        )
    }

    /// Prompt for a non-empty secret string from the user, or `None` if the
    /// input was empty.
    ///
    /// The entered text is masked in the terminal with placeholders.
    pub fn prompt_secret(
        &mut self,
        prompt: impl AsRef<str>,
    ) -> anyhow::Result<Option<Sensitive<String>>> {
        Ok(
            match self.readline(&format!("{}\n> ", prompt.as_ref()), InputEcho::Hidden)? {
                ReadlineInput::Text(line) => Some(Sensitive(line)),
                ReadlineInput::Empty | ReadlineInput::EndOfInput => None,
            },
        )
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
    pub fn confirm(
        &mut self,
        prompt: impl AsRef<str>,
        default: ConfirmDefault,
    ) -> anyhow::Result<Confirm> {
        let suffix = match default {
            ConfirmDefault::Yes => "[Y/n]",
            ConfirmDefault::No => "[y/N]",
        };
        match self.readline(
            &format!("{} {}: ", prompt.as_ref(), suffix),
            InputEcho::Visible,
        )? {
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
    /// Re-prompts on empty input and returns `NoInput` only if input is ended
    /// (for example Ctrl-C, Ctrl-D, or Esc).
    ///
    /// ```ignore
    /// let result = inout.confirm_no_default("Are you sure you want to do this?")?;
    /// // Outputs:
    /// // Are you sure you want to do this? [y/n]:
    /// ```
    pub fn confirm_no_default(
        &mut self,
        prompt: impl AsRef<str>,
    ) -> anyhow::Result<ConfirmOrEmpty> {
        let prompt = format!("{} [y/n]: ", prompt.as_ref());
        loop {
            match self.readline(&prompt, InputEcho::Visible)? {
                ReadlineInput::Text(input) => {
                    if input.to_lowercase().starts_with('y') {
                        return Ok(ConfirmOrEmpty::Yes);
                    }
                    return Ok(ConfirmOrEmpty::No);
                }
                ReadlineInput::Empty => continue,
                ReadlineInput::EndOfInput => return Ok(ConfirmOrEmpty::NoInput),
            }
        }
    }

    pub fn prompt_select<'a, Key, Value>(
        &mut self,
        prompt: impl AsRef<str>,
        items: &'a NonEmpty<(Key, Value)>,
    ) -> anyhow::Result<Option<&'a Value>>
    where
        Key: std::fmt::Display,
    {
        let Some(picks) = tui::run_picker(
            self,
            prompt.as_ref(),
            items,
            PickerOptions {
                allow_multiple: false,
                default_selected: Vec::new(),
                disabled: Vec::new(),
            },
        )?
        else {
            return Ok(None);
        };

        match &picks[..] {
            [] => Ok(None),
            [pick] => Ok(Some(pick)),
            _ => {
                anyhow::bail!(
                    "the picker was configured to not allow multiple picks, yet multiple picks were returned"
                )
            }
        }
    }

    pub fn prompt_multi_select<'a, Key, Value>(
        &mut self,
        prompt: impl AsRef<str>,
        items: &'a NonEmpty<(Key, Value)>,
    ) -> anyhow::Result<Option<Vec<&'a Value>>>
    where
        Key: std::fmt::Display,
    {
        tui::run_picker(
            self,
            prompt.as_ref(),
            items,
            PickerOptions {
                allow_multiple: true,
                default_selected: Vec::new(),
                disabled: Vec::new(),
            },
        )
    }

    pub fn prompt_select_with_help<'a, Key, Value>(
        &mut self,
        prompt: impl AsRef<str>,
        items: &'a NonEmpty<(Key, Value)>,
        default_selected: Option<usize>,
        help: impl Fn(&Key) -> Option<&str>,
    ) -> anyhow::Result<Option<&'a Value>>
    where
        Key: std::fmt::Display,
    {
        let Some(picks) = tui::run_picker_with_help(
            self,
            prompt.as_ref(),
            items,
            PickerOptions {
                allow_multiple: false,
                default_selected: default_selected.into_iter().collect(),
                disabled: Vec::new(),
            },
            help,
        )?
        else {
            return Ok(None);
        };

        match &picks[..] {
            [] => Ok(None),
            [pick] => Ok(Some(pick)),
            _ => {
                anyhow::bail!(
                    "the picker was configured to not allow multiple picks, yet multiple picks were returned"
                )
            }
        }
    }

    /// `disabled` lists the indices of rows the user cannot toggle; they render
    /// dimmed and never appear in the returned selection, but the cursor can
    /// still rest on them so their help explains why they are unavailable.
    pub fn prompt_multi_select_with_help<'a, Key, Value>(
        &mut self,
        prompt: impl AsRef<str>,
        items: &'a NonEmpty<(Key, Value)>,
        default_selected: Vec<usize>,
        disabled: Vec<usize>,
        help: impl Fn(&Key) -> Option<&str>,
    ) -> anyhow::Result<Option<Vec<&'a Value>>>
    where
        Key: std::fmt::Display,
    {
        tui::run_picker_with_help(
            self,
            prompt.as_ref(),
            items,
            PickerOptions {
                allow_multiple: true,
                default_selected,
                disabled,
            },
            help,
        )
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

pub(crate) enum PromptLine {
    Text(String),
    Empty,
    Cancelled,
}

/// How to play input back to the user during prompts.
#[derive(Debug)]
enum InputEcho {
    /// Show everything that's typed and displayable.
    Visible,
    /// Do not show what's printed.
    Hidden,
}

/// Editing operation derived from a terminal key event.
#[derive(Debug, PartialEq, Eq)]
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

/// Map a terminal key event to a normalized line-editing action.
///
/// Only `Press` and `Repeat` events are handled; key releases are ignored.
/// `line_is_empty` is used to model terminal EOF behavior: `Ctrl-D` ends input
/// only when nothing has been typed on the current line.
fn key_to_edit_action(key: KeyEvent, line_is_empty: bool) -> KeyEditAction {
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return KeyEditAction::Ignore;
    }
    match key.code {
        KeyCode::Enter => KeyEditAction::Submit,
        KeyCode::Backspace => KeyEditAction::Backspace,
        KeyCode::Tab => KeyEditAction::Insert('\t'),
        KeyCode::Esc => KeyEditAction::EndOfInput,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            KeyEditAction::EndOfInput
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) && line_is_empty => {
            KeyEditAction::EndOfInput
        }
        KeyCode::Char(ch)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
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
    /// Create a new instance to output with `format`.
    pub fn new(format: OutputFormat) -> Self {
        OutputChannel {
            format,
            stdout: std::io::stdout(),
            pager: None,
            json_buffer: None,
        }
    }

    /// Request paging for large output. The pager is only started when human UI is allowed,
    /// stdout is a terminal, and paging is not disabled by the environment.
    pub fn request_pager(&mut self) {
        if self.pager.is_some()
            || !self.format.allows_human_ui()
            || std::env::var_os("NOPAGER").is_some()
            || !self.stdout.is_terminal()
        {
            return;
        }
        self.pager = pager::try_init_pager();
    }
}

impl Drop for OutputChannel {
    fn drop(&mut self) {
        // Flush any buffered JSON that was never consumed — this means
        // the combined status path did not complete, but we should still
        // emit the mutation result rather than silently discarding it.
        if let Some(buffer) = self.json_buffer.take()
            && buffer != serde_json::Value::Null
            && let Err(err) = json_pretty_to_stdout(&buffer)
        {
            eprintln!("warning: failed to flush buffered JSON on drop: {err}");
        }
        match self.pager.take() {
            Some(Pager::Builtin(pager)) => {
                minus::page_all(pager).ok();
            }
            Some(Pager::External(mut child, child_stdin)) => {
                // Drop the child process stdin to signal EOF to the pager, which should cause it
                // to exit.
                drop(child_stdin);
                child.wait().ok();
            }
            None => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::args::OutputFormat;

    use super::{KeyEditAction, OutputChannel, WriteWithUtils, key_to_edit_action};

    #[test]
    fn ctrl_c_ends_input() {
        assert_eq!(
            key_to_edit_action(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                true
            ),
            KeyEditAction::EndOfInput
        );
    }

    #[test]
    fn ctrl_d_only_ends_input_when_line_is_empty() {
        assert_eq!(
            key_to_edit_action(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                true
            ),
            KeyEditAction::EndOfInput
        );
        assert_eq!(
            key_to_edit_action(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                false
            ),
            KeyEditAction::Ignore
        );
    }

    #[test]
    fn agent_output_is_human_text_not_json() {
        let mut out = OutputChannel::new(OutputFormat::Agent);

        assert!(
            out.for_human().is_some(),
            "agent output should allow human text rendering"
        );
        assert!(
            out.for_json().is_none(),
            "agent output should not be treated as JSON"
        );
        assert!(!out.is_json(), "agent output should not be JSON mode");
    }

    #[test]
    fn agent_output_does_not_truncate_unpaged_text() {
        let out = OutputChannel::new(OutputFormat::Agent);
        let text = "0123456789abcdef";

        assert_eq!(out.truncate_if_unpaged(text, 4), text);
    }

    #[test]
    fn agent_output_does_not_allow_progress() {
        let out = OutputChannel::new(OutputFormat::Agent);

        assert!(!out.format.allows_human_ui());
        assert!(out.progress_channel().inner.is_none());
    }

    #[test]
    fn agent_output_never_uses_pager() {
        let mut out = OutputChannel::new(OutputFormat::Agent);
        out.request_pager();

        assert!(!out.is_paged(), "agent output should not use a pager");
    }
}
